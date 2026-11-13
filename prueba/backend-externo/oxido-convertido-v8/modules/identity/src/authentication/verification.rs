use std::time::SystemTime;

use sqlx::FromRow;
use uuid::Uuid;

use super::account::sessions::insert_security_event;
use super::email_actions::VerifiedEmail;
use super::helpers::{hash_secret, unix_timestamp};
use super::outbox::append_event;
use super::{AuthenticationError, IdentityService};

#[derive(FromRow)]
struct ActivatedAccount {
    email: String,
    display_name: String,
}

impl IdentityService {
    pub async fn verify_email(
        &self,
        token: &str,
        email: &str,
        code: &str,
        source_address: &str,
        correlation_id: &str,
    ) -> Result<Option<VerifiedEmail>, AuthenticationError> {
        let has_token = !token.is_empty();
        let has_code = !email.is_empty() || !code.is_empty();
        if has_token == has_code {
            return Ok(None);
        }
        let now = unix_timestamp(SystemTime::now())?;
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        let user_id = if has_token {
            if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                transaction
                    .rollback()
                    .await
                    .map_err(AuthenticationError::Persistence)?;
                return Ok(None);
            }
            sqlx::query_scalar::<_, Uuid>(
                "UPDATE identity.one_time_tokens SET consumed_at = to_timestamp($2) WHERE token_hash = $1 AND purpose = 'email_verification' AND consumed_at IS NULL AND revoked_at IS NULL AND expires_at > to_timestamp($2) RETURNING user_id",
            )
            .bind(hash_secret(token))
            .bind(now)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(AuthenticationError::Persistence)?
        } else {
            if email.len() > 320
                || code.len() != 8
                || !code.bytes().all(|byte| byte.is_ascii_digit())
            {
                transaction
                    .rollback()
                    .await
                    .map_err(AuthenticationError::Persistence)?;
                return Ok(None);
            }
            let source_address = source_address
                .parse::<std::net::IpAddr>()
                .map_err(|_| AuthenticationError::InvalidSourceAddress)?
                .to_string();
            self.enforce_verification_code_rate_limit(email, &source_address)
                .await?;
            let normalized_email = super::normalize_email(email);
            let verification = sqlx::query_as::<_, (Uuid, Vec<u8>, i32)>(
                "SELECT token.token_id, token.verification_code_hash, token.verification_code_attempts FROM identity.one_time_tokens AS token JOIN identity.users AS account ON account.user_id = token.user_id WHERE account.email_normalized = $1 AND token.purpose = 'email_verification' AND token.consumed_at IS NULL AND token.revoked_at IS NULL AND token.expires_at > to_timestamp($2) AND token.verification_code_expires_at > to_timestamp($2) AND token.verification_code_hash IS NOT NULL AND token.verification_code_attempts < 5 ORDER BY token.created_at DESC LIMIT 1 FOR UPDATE OF token",
            )
            .bind(normalized_email)
            .bind(now)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(AuthenticationError::Persistence)?;
            match verification {
                Some((token_id, stored_hash, attempts)) if attempts < 5 && self.verification_code_matches(code, &stored_hash)? => {
                    sqlx::query_scalar::<_, Uuid>(
                        "UPDATE identity.one_time_tokens SET consumed_at = to_timestamp($2) WHERE token_id = $1 RETURNING user_id",
                    )
                    .bind(token_id)
                    .bind(now)
                    .fetch_optional(&mut *transaction)
                    .await
                    .map_err(AuthenticationError::Persistence)?
                }
                Some((token_id, stored_hash, attempts)) => {
                    let hash_length = stored_hash.len();
                    if hash_length != 32 {
                        transaction.rollback().await.map_err(AuthenticationError::Persistence)?;
                        return Ok(None);
                    }
                    let next_attempts = attempts + 1;
                    sqlx::query(
                        "UPDATE identity.one_time_tokens SET verification_code_attempts = $2, revoked_at = CASE WHEN $2 >= 5 THEN to_timestamp($3) ELSE revoked_at END WHERE token_id = $1",
                    )
                    .bind(token_id)
                    .bind(next_attempts)
                    .bind(now)
                    .execute(&mut *transaction)
                    .await
                    .map_err(AuthenticationError::Persistence)?;
                    transaction.commit().await.map_err(AuthenticationError::Persistence)?;
                    return Ok(None);
                }
                None => None,
            }
        };
        let Some(user_id) = user_id else {
            transaction
                .rollback()
                .await
                .map_err(AuthenticationError::Persistence)?;
            return Ok(None);
        };
        let account = sqlx::query_as::<_, ActivatedAccount>(
            "UPDATE identity.users SET account_status = 'active', email_verified_at = to_timestamp($2), updated_at = to_timestamp($2), row_version = row_version + 1 WHERE user_id = $1 AND account_status = 'pending_verification' RETURNING email_normalized AS email, display_name",
        )
        .bind(user_id)
        .bind(now)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        let Some(account) = account else {
            transaction
                .rollback()
                .await
                .map_err(AuthenticationError::Persistence)?;
            return Ok(None);
        };
        insert_security_event(
            &mut transaction,
            Some(user_id),
            "account.email_verified",
            "succeeded",
            correlation_id,
        )
        .await?;
        append_event(
            &mut transaction,
            "identity.account.email_verified",
            serde_json::json!({"user_id": user_id, "correlation_id": correlation_id}),
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        Ok(Some(VerifiedEmail {
            user_id,
            email: account.email,
            display_name: account.display_name,
        }))
    }

    async fn enforce_verification_code_rate_limit(
        &self,
        email: &str,
        source_address: &str,
    ) -> Result<(), AuthenticationError> {
        let account_key = self.rate_limit_key("verification-code-email", email)?;
        let source_key = self.rate_limit_key("verification-code-source", source_address)?;
        let mut cache = self.cache.clone();
        let (account_count, source_count): (i64, i64) = redis::Script::new(
            "local a=redis.call('INCR',KEYS[1]); if a==1 then redis.call('EXPIRE',KEYS[1],ARGV[1]); end; local s=redis.call('INCR',KEYS[2]); if s==1 then redis.call('EXPIRE',KEYS[2],ARGV[1]); end; return {a,s}",
        )
        .key(account_key)
        .key(source_key)
        .arg(900)
        .invoke_async(&mut cache)
        .await
        .map_err(AuthenticationError::RateLimitUnavailable)?;
        if account_count > 10 || source_count > 30 {
            return Err(AuthenticationError::RateLimited);
        }
        Ok(())
    }
}
