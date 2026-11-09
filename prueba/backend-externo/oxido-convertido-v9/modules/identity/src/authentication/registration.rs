use std::time::SystemTime;

use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, SaltString};
use uuid::Uuid;

use super::account::sessions::insert_security_event;
use super::helpers::normalize_email;
use super::helpers::registration_input_valid;
use super::helpers::{hash_secret, random_bytes, random_secret, unix_timestamp};
use super::login_rate_limit::{
    REGISTER_EMAIL_RATE_LIMIT, REGISTER_RATE_WINDOW_SECONDS, REGISTER_SOURCE_RATE_LIMIT,
};
use super::outbox::append_event;
use super::{AuthenticationError, IdentityService, PendingVerification, RegistrationResult};

impl IdentityService {
    pub async fn register(
        &self,
        email: &str,
        password: &str,
        display_name: &str,
        marketing_consent: bool,
        source_address: &str,
        correlation_id: &str,
    ) -> Result<RegistrationResult, AuthenticationError> {
        let email = normalize_email(email);
        if !registration_input_valid(&email, password, display_name) {
            return Err(AuthenticationError::Rejected);
        }
        let source_address = source_address
            .parse::<std::net::IpAddr>()
            .map_err(|_| AuthenticationError::InvalidSourceAddress)?
            .to_string();
        let email_rate_key = self.rate_limit_key("register-email", &email)?;
        let source_rate_key = self.rate_limit_key("register-source", &source_address)?;
        let mut cache = self.cache.clone();
        let (email_attempt_count, source_attempt_count): (i64, i64) = redis::Script::new(
            "local email_count = redis.call('INCR', KEYS[1]); if email_count == 1 then redis.call('EXPIRE', KEYS[1], ARGV[1]); end; local source_count = redis.call('INCR', KEYS[2]); if source_count == 1 then redis.call('EXPIRE', KEYS[2], ARGV[1]); end; return {email_count, source_count}",
        )
        .key(&email_rate_key)
        .key(&source_rate_key)
        .arg(REGISTER_RATE_WINDOW_SECONDS)
        .invoke_async(&mut cache)
        .await
        .map_err(AuthenticationError::RateLimitUnavailable)?;
        if email_attempt_count > REGISTER_EMAIL_RATE_LIMIT
            || source_attempt_count > REGISTER_SOURCE_RATE_LIMIT
        {
            return Err(AuthenticationError::RateLimited);
        }
        let password_salt = SaltString::encode_b64(&random_bytes()?)
            .map_err(|_| AuthenticationError::PasswordHash)?;
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &password_salt)
            .map_err(|_| AuthenticationError::PasswordHash)?
            .to_string();
        let user_id = Uuid::new_v4();
        let now = unix_timestamp(SystemTime::now())?;
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        let created_user = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO identity.users (user_id, email_normalized, password_hash, account_status, display_name, locale, timezone, created_at, updated_at) VALUES ($1, $2, $3, 'pending_verification', $4, 'es-CO', 'America/Bogota', to_timestamp($5), to_timestamp($5)) ON CONFLICT (email_normalized) DO NOTHING RETURNING user_id",
        )
        .bind(user_id)
        .bind(&email)
        .bind(password_hash)
        .bind(display_name.trim())
        .bind(now)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        let Some(user_id) = created_user else {
            transaction
                .commit()
                .await
                .map_err(AuthenticationError::Persistence)?;
            return Ok(RegistrationResult::AlreadyExists);
        };
        let token_id = Uuid::new_v4();
        let token = random_secret()?;
        let verification_code = self.generate_verification_code()?;
        let expires_at = now
            .checked_add(24 * 60 * 60)
            .ok_or(AuthenticationError::ExpiryOutOfRange)?;
        sqlx::query(
            "INSERT INTO identity.one_time_tokens (token_id, user_id, purpose, token_hash, verification_code_hash, verification_code_expires_at, verification_code_attempts, created_at, expires_at, delivery_state, delivery_updated_at) VALUES ($1, $2, 'email_verification', $3, $4, to_timestamp($5 + 900), 0, to_timestamp($5), to_timestamp($6), 'pending', to_timestamp($5))",
        )
        .bind(token_id)
        .bind(user_id)
        .bind(hash_secret(&token))
        .bind(self.verification_code_hash(&verification_code)?)
        .bind(now)
        .bind(expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        sqlx::query(
            "INSERT INTO identity.marketing_consents (user_id, consented, policy_version, source, changed_at, correlation_id) VALUES ($1, $2, 'marketing-v1', 'public_registration', to_timestamp($3), $4)",
        )
        .bind(user_id)
        .bind(marketing_consent)
        .bind(now)
        .bind(correlation_id)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        insert_security_event(
            &mut transaction,
            Some(user_id),
            "account.registration",
            "succeeded",
            correlation_id,
        )
        .await?;
        append_event(
            &mut transaction,
            "identity.account.marketing_consent_changed",
            serde_json::json!({"user_id": user_id, "consented": marketing_consent, "correlation_id": correlation_id}),
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        Ok(RegistrationResult::Created(PendingVerification {
            user_id,
            token_id,
            email,
            display_name: display_name.trim().to_owned(),
            token,
            verification_code: Some(verification_code),
        }))
    }
}
