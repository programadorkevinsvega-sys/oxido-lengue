use std::time::SystemTime;

use uuid::Uuid;

use super::account::sessions::insert_security_event;
use super::helpers::unix_timestamp;
use super::{AuthenticationError, IdentityService};

impl IdentityService {
    pub async fn set_marketing_consent(
        &self,
        user_id: &str,
        consented: bool,
        correlation_id: &str,
    ) -> Result<bool, AuthenticationError> {
        let user_id = Uuid::parse_str(user_id).map_err(|_| AuthenticationError::Rejected)?;
        let now = unix_timestamp(SystemTime::now())?;
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        let account_exists = sqlx::query_scalar::<_, Uuid>(
            "SELECT user_id FROM identity.users WHERE user_id = $1 AND account_status = 'active' FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?
        .is_some();
        if !account_exists {
            transaction
                .rollback()
                .await
                .map_err(AuthenticationError::Persistence)?;
            return Ok(false);
        }
        sqlx::query(
            "INSERT INTO identity.marketing_consents (user_id, consented, policy_version, source, changed_at, correlation_id) VALUES ($1, $2, 'marketing-v1', 'account_settings', to_timestamp($3), $4) ON CONFLICT (user_id) DO UPDATE SET consented = EXCLUDED.consented, policy_version = EXCLUDED.policy_version, source = EXCLUDED.source, changed_at = EXCLUDED.changed_at, correlation_id = EXCLUDED.correlation_id",
        )
        .bind(user_id)
        .bind(consented)
        .bind(now)
        .bind(correlation_id)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        insert_security_event(
            &mut transaction,
            Some(user_id),
            "account.marketing_consent",
            if consented { "granted" } else { "withdrawn" },
            correlation_id,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        Ok(true)
    }
}
