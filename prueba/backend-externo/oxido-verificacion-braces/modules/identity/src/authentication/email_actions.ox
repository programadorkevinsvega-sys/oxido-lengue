sintaxis llaves
use std::time::SystemTime;

use sqlx::FromRow;
use uuid::Uuid;

use super::account::sessions::insert_security_event;
use super::helpers::{hash_secret, random_secret, unix_timestamp};
use super::outbox::append_event;
use super::{AuthenticationError, IdentityService, PendingVerification};

const EMAIL_ACTION_WINDOW_SECONDS: u64 = 3_600;
const EMAIL_ACTION_LIMIT: i64 = 3;
const RECOVERY_TOKEN_LIFETIME_SECONDS: i64 = 3_600;
const VERIFICATION_TOKEN_LIFETIME_SECONDS: i64 = 86_400;

#[derive(FromRow)]
struct PendingAccount {
    user_id: Uuid,
    email: String,
    display_name: String,
}

pub struct VerifiedEmail {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
}

impl IdentityService {
    pub async fn set_email_delivery(
        &self,
        token_id: Uuid,
        accepted: bool,
        provider_message_id: Option<&str>,
        correlation_id: &str,
    ) -> Result<(), AuthenticationError> {
        let delivery_state = if accepted { "accepted" } else { "failed" };
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        let updated = sqlx::query(
            "UPDATE identity.one_time_tokens SET delivery_state = $2, provider_message_id = $3, delivery_updated_at = now() WHERE token_id = $1 AND consumed_at IS NULL AND revoked_at IS NULL",
        )
        .bind(token_id)
        .bind(delivery_state)
        .bind(provider_message_id)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?
        .rows_affected();
        if updated == 1 {
            append_event(
                &mut transaction,
                "identity.email.delivery_updated",
                serde_json::json!({"token_id": token_id, "delivery_state": delivery_state, "provider_message_id": provider_message_id, "correlation_id": correlation_id}),
            )
            .await?;
        }
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        Ok(())
    }

    pub async fn resend_verification(
        &self,
        email: &str,
        source_address: &str,
        correlation_id: &str,
    ) -> Result<Option<PendingVerification>, AuthenticationError> {
        self.issue_token(email, source_address, correlation_id, "email_verification")
            .await
    }

    pub async fn request_password_recovery(
        &self,
        email: &str,
        source_address: &str,
        correlation_id: &str,
    ) -> Result<Option<PendingVerification>, AuthenticationError> {
        self.issue_token(email, source_address, correlation_id, "password_recovery")
            .await
    }

    async fn issue_token(
        &self,
        email: &str,
        source_address: &str,
        correlation_id: &str,
        purpose: &str,
    ) -> Result<Option<PendingVerification>, AuthenticationError> {
        let email = super::normalize_email(email);
        let source_address = source_address
            .parse::<std::net::IpAddr>()
            .map_err(|_| AuthenticationError::InvalidSourceAddress)?
            .to_string();
        self.enforce_email_rate_limit(purpose, &email, &source_address)
            .await?;
        let now = unix_timestamp(SystemTime::now())?;
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        let account = sqlx::query_as::<_, PendingAccount>(
            "SELECT user_id, email_normalized AS email, display_name FROM identity.users WHERE email_normalized = $1 AND (($2 = 'email_verification' AND account_status = 'pending_verification') OR ($2 = 'password_recovery' AND account_status = 'active')) FOR UPDATE",
        )
        .bind(&email)
        .bind(purpose)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        let Some(account) = account else {
            transaction
                .commit()
                .await
                .map_err(AuthenticationError::Persistence)?;
            return Ok(None);
        };
        sqlx::query(
            "UPDATE identity.one_time_tokens SET revoked_at = to_timestamp($3) WHERE user_id = $1 AND purpose = $2 AND consumed_at IS NULL AND revoked_at IS NULL",
        )
        .bind(account.user_id)
        .bind(purpose)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        let token_id = Uuid::new_v4();
        let token = random_secret()?;
        let verification_code = if purpose == "email_verification" {
            Some(self.generate_verification_code()?)
        } else {
            None
        };
        let lifetime = if purpose == "email_verification" {
            VERIFICATION_TOKEN_LIFETIME_SECONDS
        } else {
            RECOVERY_TOKEN_LIFETIME_SECONDS
        };
        let expires_at = now
            .checked_add(lifetime)
            .ok_or(AuthenticationError::ExpiryOutOfRange)?;
        sqlx::query(
            "INSERT INTO identity.one_time_tokens (token_id, user_id, purpose, token_hash, verification_code_hash, verification_code_expires_at, verification_code_attempts, created_at, expires_at, delivery_state, delivery_updated_at) VALUES ($1, $2, $3, $4, $5, CASE WHEN $3 = 'email_verification' THEN to_timestamp($6 + 900) ELSE NULL END, 0, to_timestamp($6), to_timestamp($7), 'pending', to_timestamp($6))",
        )
        .bind(token_id)
        .bind(account.user_id)
        .bind(purpose)
        .bind(hash_secret(&token))
        .bind(verification_code.as_deref().map(|code| self.verification_code_hash(code)).transpose()?)
        .bind(now)
        .bind(expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        insert_security_event(
            &mut transaction,
            Some(account.user_id),
            if purpose == "email_verification" {
                "account.verification_resent"
            } else {
                "account.recovery_requested"
            },
            "succeeded",
            correlation_id,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        Ok(Some(PendingVerification {
            user_id: account.user_id,
            token_id,
            email: account.email,
            display_name: account.display_name,
            token,
            verification_code,
        }))
    }

    async fn enforce_email_rate_limit(
        &self,
        purpose: &str,
        email: &str,
        source_address: &str,
    ) -> Result<(), AuthenticationError> {
        let account_key = self.rate_limit_key(&format!("{purpose}-email"), email)?;
        let source_key = self.rate_limit_key(&format!("{purpose}-source"), source_address)?;
        let mut cache = self.cache.clone();
        let (account_count, source_count, account_ttl, source_ttl): (i64, i64, i64, i64) = redis::Script::new(
            "local a=redis.call('INCR',KEYS[1]); if a==1 then redis.call('EXPIRE',KEYS[1],ARGV[1]); end; local s=redis.call('INCR',KEYS[2]); if s==1 then redis.call('EXPIRE',KEYS[2],ARGV[1]); end; return {a,s,redis.call('TTL',KEYS[1]),redis.call('TTL',KEYS[2])}",
        )
        .key(account_key)
        .key(source_key)
        .arg(EMAIL_ACTION_WINDOW_SECONDS)
        .invoke_async(&mut cache)
        .await
        .map_err(AuthenticationError::RateLimitUnavailable)?;
        let account_limit_exceeded = account_count > EMAIL_ACTION_LIMIT;
        let source_limit_exceeded = source_count > EMAIL_ACTION_LIMIT * 20;
        if account_limit_exceeded || source_limit_exceeded {
            let retry_after_seconds = match (account_limit_exceeded, source_limit_exceeded) {
                (true, true) => account_ttl.max(source_ttl),
                (true, false) => account_ttl,
                (false, true) => source_ttl,
                (false, false) => 1,
            }
            .max(1) as u64;
            return Err(AuthenticationError::EmailActionRateLimited {
                retry_after_seconds,
                account_limit_exceeded,
                limit: if account_limit_exceeded {
                    EMAIL_ACTION_LIMIT
                } else {
                    EMAIL_ACTION_LIMIT * 20
                },
            });
        }
        Ok(())
    }
}
