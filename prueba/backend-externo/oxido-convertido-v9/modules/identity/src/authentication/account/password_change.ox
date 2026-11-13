sintaxis llaves
use std::time::SystemTime;

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use sqlx::FromRow;
use uuid::Uuid;

use super::super::helpers::{
    duration_seconds, hash_secret, password_valid, random_bytes, random_secret, unix_timestamp,
};
use super::super::{AuthenticationError, IdentityService, LoginSession};
use super::sessions::insert_security_event;
use super::{AccountSessionContext, PasswordChange, require_tenant};

#[derive(FromRow)]
struct PasswordChangeAccount {
    user_id: Uuid,
    password_hash: String,
    absolute_expires_at: i64,
}

impl IdentityService {
    pub(super) async fn change_password(
        &self,
        context: AccountSessionContext<'_>,
        change: PasswordChange<'_>,
    ) -> Result<LoginSession, AuthenticationError> {
        require_tenant(context.audience)?;
        if !password_valid(change.new_password) {
            return Err(AuthenticationError::InvalidProfile);
        }
        let now = unix_timestamp(SystemTime::now())?;
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        let account = sqlx::query_as::<_, PasswordChangeAccount>(
            "SELECT account.user_id, account.password_hash, floor(extract(epoch FROM session.absolute_expires_at))::bigint AS absolute_expires_at FROM identity.sessions AS session JOIN identity.users AS account ON account.user_id = session.user_id WHERE session.token_hash = $1 AND session.csrf_hash = $2 AND session.audience = 'tenant' AND session.revoked_at IS NULL AND session.idle_expires_at > to_timestamp($3) AND session.absolute_expires_at > to_timestamp($3) AND account.account_status = 'active' FOR UPDATE OF session, account",
        )
        .bind(hash_secret(context.session_secret))
        .bind(hash_secret(context.csrf_secret))
        .bind(now)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?
        .ok_or(AuthenticationError::Rejected)?;
        let current_hash = PasswordHash::new(&account.password_hash)
            .map_err(|_| AuthenticationError::PasswordHash)?;
        if Argon2::default()
            .verify_password(change.current_password.as_bytes(), &current_hash)
            .is_err()
        {
            insert_security_event(
                &mut transaction,
                Some(account.user_id),
                "account.password_change",
                "rejected",
                change.correlation_id,
            )
            .await?;
            transaction
                .commit()
                .await
                .map_err(AuthenticationError::Persistence)?;
            return Err(AuthenticationError::Rejected);
        }
        if Argon2::default()
            .verify_password(change.new_password.as_bytes(), &current_hash)
            .is_ok()
        {
            return Err(AuthenticationError::InvalidProfile);
        }
        let salt = SaltString::encode_b64(&random_bytes()?)
            .map_err(|_| AuthenticationError::PasswordHash)?;
        let password_hash = Argon2::default()
            .hash_password(change.new_password.as_bytes(), &salt)
            .map_err(|_| AuthenticationError::PasswordHash)?
            .to_string();
        let idle_expiry = now
            .checked_add(duration_seconds(self.idle_session_lifetime)?)
            .ok_or(AuthenticationError::ExpiryOutOfRange)?
            .min(account.absolute_expires_at);
        let session_secret = random_secret()?;
        let csrf_secret = random_secret()?;
        let new_session_id = Uuid::new_v4();
        sqlx::query(
            "UPDATE identity.users SET password_hash = $2, updated_at = to_timestamp($3), row_version = row_version + 1 WHERE user_id = $1",
        )
        .bind(account.user_id)
        .bind(password_hash)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        sqlx::query(
            "UPDATE identity.one_time_tokens SET revoked_at = to_timestamp($2) WHERE user_id = $1 AND purpose = 'password_recovery' AND consumed_at IS NULL AND revoked_at IS NULL",
        )
        .bind(account.user_id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        sqlx::query(
            "UPDATE identity.sessions SET revoked_at = to_timestamp($2), revocation_reason = 'password_changed' WHERE user_id = $1 AND revoked_at IS NULL",
        )
        .bind(account.user_id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        sqlx::query(
            "INSERT INTO identity.sessions (session_id, user_id, audience, token_hash, csrf_hash, created_at, last_seen_at, idle_expires_at, absolute_expires_at) VALUES ($1, $2, 'tenant', $3, $4, to_timestamp($5), to_timestamp($5), to_timestamp($6), to_timestamp($7))",
        )
        .bind(new_session_id)
        .bind(account.user_id)
        .bind(hash_secret(&session_secret))
        .bind(hash_secret(&csrf_secret))
        .bind(now)
        .bind(idle_expiry)
        .bind(account.absolute_expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        insert_security_event(
            &mut transaction,
            Some(account.user_id),
            "account.password_change",
            "succeeded",
            change.correlation_id,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        Ok(LoginSession {
            session_id: new_session_id,
            session_secret,
            csrf_secret,
            user_id: account.user_id,
            idle_expires_at: idle_expiry,
            absolute_expires_at: account.absolute_expires_at,
        })
    }
}

impl IdentityService {
    pub async fn change_account_password(
        &self,
        context: AccountSessionContext<'_>,
        change: PasswordChange<'_>,
    ) -> Result<LoginSession, AuthenticationError> {
        self.change_password(context, change).await
    }
}
