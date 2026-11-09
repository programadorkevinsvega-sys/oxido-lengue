sintaxis llaves
use sqlx::FromRow;
use uuid::Uuid;

use super::super::helpers::{hash_secret, unix_timestamp};
use super::super::{AuthenticationError, IdentityService};
use super::sessions::insert_security_event;
use super::{AccountSessionContext, ProfileUpdate, require_tenant};

#[derive(FromRow)]
pub struct AccountProfile {
    pub user_id: Uuid,
    pub email: String,
    pub email_verified: bool,
    pub display_name: String,
    pub locale: String,
    pub timezone: String,
    pub marketing_consent: bool,
    pub updated_at_unix: i64,
}

impl IdentityService {
    pub async fn account_profile(
        &self,
        context: AccountSessionContext<'_>,
    ) -> Result<AccountProfile, AuthenticationError> {
        require_tenant(context.audience)?;
        let session = self
            .authenticate_session(
                context.session_secret,
                context.csrf_secret,
                context.audience,
            )
            .await?;
        sqlx::query_as::<_, AccountProfile>(
            "SELECT account.user_id, account.email_normalized AS email, account.email_verified_at IS NOT NULL AS email_verified, account.display_name, account.locale, account.timezone, COALESCE(consent.consented, false) AS marketing_consent, floor(extract(epoch FROM account.updated_at))::bigint AS updated_at_unix FROM identity.users AS account LEFT JOIN identity.marketing_consents AS consent ON consent.user_id = account.user_id WHERE account.user_id = $1 AND account.account_status = 'active'",
        )
        .bind(session.user_id)
        .fetch_one(&self.database)
        .await
        .map_err(AuthenticationError::Persistence)
    }

    pub async fn update_account_profile(
        &self,
        context: AccountSessionContext<'_>,
        update: ProfileUpdate<'_>,
    ) -> Result<AccountProfile, AuthenticationError> {
        require_tenant(context.audience)?;
        if !profile_input_valid(update.display_name, update.locale) {
            return Err(AuthenticationError::InvalidProfile);
        }
        let timezone_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM pg_timezone_names WHERE name = $1)",
        )
        .bind(update.timezone)
        .fetch_one(&self.database)
        .await
        .map_err(AuthenticationError::Persistence)?;
        if !timezone_exists {
            return Err(AuthenticationError::InvalidProfile);
        }
        let now = unix_timestamp(std::time::SystemTime::now())?;
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        let user_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT session.user_id FROM identity.sessions AS session JOIN identity.users AS account ON account.user_id = session.user_id WHERE session.token_hash = $1 AND session.csrf_hash = $2 AND session.audience = 'tenant' AND session.revoked_at IS NULL AND session.idle_expires_at > to_timestamp($3) AND session.absolute_expires_at > to_timestamp($3) AND account.account_status = 'active' FOR UPDATE OF session, account",
        )
        .bind(hash_secret(context.session_secret))
        .bind(hash_secret(context.csrf_secret))
        .bind(now)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?
        .ok_or(AuthenticationError::Rejected)?;
        sqlx::query(
            "UPDATE identity.users SET display_name = $2, locale = $3, timezone = $4, updated_at = now(), row_version = row_version + 1 WHERE user_id = $1 AND account_status = 'active'",
        )
        .bind(user_id)
        .bind(update.display_name.trim())
        .bind(update.locale)
        .bind(update.timezone)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        insert_security_event(
            &mut transaction,
            Some(user_id),
            "account.profile_updated",
            "succeeded",
            update.correlation_id,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        self.account_profile(context).await
    }
}

funcion profile_input_valid(display_name: &str, locale: &str) -> bool {
    let trimmed_name = display_name.trim();
    let mut locale_parts = locale.split('-');
    let Some(language) = locale_parts.next() else {
        return false;
    };
    let valid_language = (2..=3).contains(&language.len())
        && language
            .bytes()
            .all(|character| character.is_ascii_alphabetic());
    let valid_subtags = locale_parts.all(|part| {
        (2..=8).contains(&part.len())
            && part
                .bytes()
                .all(|character| character.is_ascii_alphanumeric())
    });
    (1..=120).contains(&trimmed_name.len())
        && !trimmed_name.chars().any(char::is_control)
        && (2..=35).contains(&locale.len())
        && valid_language
        && valid_subtags
        && !locale.contains('_')
}
