sintaxis llaves
use std::time::SystemTime;

use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, SaltString};
use sqlx::FromRow;
use uuid::Uuid;

use super::account::sessions::insert_security_event;
use super::helpers::{hash_secret, password_valid, random_bytes, unix_timestamp};
use super::{AuthenticationError, IdentityService};

#[derive(FromRow)]
struct RecoveryAccount {
    user_id: Uuid,
}

impl IdentityService {
    pub async fn reset_password(
        &self,
        token: &str,
        new_password: &str,
        correlation_id: &str,
    ) -> Result<bool, AuthenticationError> {
        if token.len() != 64
            || !token.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !password_valid(new_password)
        {
            return Ok(false);
        }
        let salt = SaltString::encode_b64(&random_bytes()?)
            .map_err(|_| AuthenticationError::PasswordHash)?;
        let password_hash = Argon2::default()
            .hash_password(new_password.as_bytes(), &salt)
            .map_err(|_| AuthenticationError::PasswordHash)?
            .to_string();
        let now = unix_timestamp(SystemTime::now())?;
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        let account = sqlx::query_as::<_, RecoveryAccount>(
            "UPDATE identity.one_time_tokens SET consumed_at = to_timestamp($2) WHERE token_hash = $1 AND purpose = 'password_recovery' AND consumed_at IS NULL AND revoked_at IS NULL AND expires_at > to_timestamp($2) RETURNING user_id",
        )
        .bind(hash_secret(token))
        .bind(now)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        let Some(account) = account else {
            transaction
                .rollback()
                .await
                .map_err(AuthenticationError::Persistence)?;
            return Ok(false);
        };
        let changed = sqlx::query(
            "UPDATE identity.users SET password_hash = $2, updated_at = to_timestamp($3) WHERE user_id = $1 AND account_status = 'active'",
        )
        .bind(account.user_id)
        .bind(password_hash)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?
        .rows_affected()
            == 1;
        if !changed {
            transaction
                .rollback()
                .await
                .map_err(AuthenticationError::Persistence)?;
            return Ok(false);
        }
        sqlx::query(
            "UPDATE identity.one_time_tokens SET revoked_at = to_timestamp($2) WHERE user_id = $1 AND purpose = 'password_recovery' AND consumed_at IS NULL AND revoked_at IS NULL",
        )
        .bind(account.user_id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        sqlx::query(
            "UPDATE identity.sessions SET revoked_at = to_timestamp($2), revocation_reason = 'password_reset' WHERE user_id = $1 AND revoked_at IS NULL",
        )
        .bind(account.user_id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        insert_security_event(
            &mut transaction,
            Some(account.user_id),
            "account.password_reset",
            "succeeded",
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
