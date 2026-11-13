sintaxis llaves
use super::helpers::normalize_email;
use super::login_rate_limit::{LOGIN_RATE_LIMIT, LOGIN_RATE_WINDOW_SECONDS, SOURCE_RATE_LIMIT};
use super::{Audience, AuthenticationError, IdentityService, LoginAccount, LoginSession};
use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordVerifier};
use redis::AsyncCommands;

impl IdentityService {
    pub async fn login(
        &self,
        email: &str,
        password: &str,
        audience: Audience,
        source_address: &str,
        correlation_id: &str,
    ) -> Result<LoginSession, AuthenticationError> {
        let normalized_email = normalize_email(email);
        let source_address = source_address
            .parse::<std::net::IpAddr>()
            .map_err(|_| AuthenticationError::InvalidSourceAddress)?
            .to_string();
        let account_rate_key = self.rate_limit_key(
            "login-account",
            &format!("{}:{normalized_email}", audience.as_database_value()),
        )?;
        let source_rate_key = self.rate_limit_key("login-source", &source_address)?;
        let mut cache = self.cache.clone();
        let (account_attempt_count, source_attempt_count): (i64, i64) = redis::Script::new(
            "local account_count = redis.call('INCR', KEYS[1]); if account_count == 1 then redis.call('EXPIRE', KEYS[1], ARGV[1]); end; local source_count = redis.call('INCR', KEYS[2]); if source_count == 1 then redis.call('EXPIRE', KEYS[2], ARGV[1]); end; return {account_count, source_count}",
        )
        .key(&account_rate_key)
        .key(&source_rate_key)
        .arg(LOGIN_RATE_WINDOW_SECONDS)
        .invoke_async(&mut cache)
        .await
        .map_err(AuthenticationError::RateLimitUnavailable)?;
        if account_attempt_count > LOGIN_RATE_LIMIT || source_attempt_count > SOURCE_RATE_LIMIT {
            return Err(AuthenticationError::RateLimited);
        }
        let account = sqlx::query_as::<_, LoginAccount>(
            "SELECT account.user_id, account.password_hash, account.account_status, EXISTS (SELECT 1 FROM identity.platform_operator_roles AS operator_role WHERE operator_role.user_id = account.user_id AND operator_role.revoked_at IS NULL) AS platform_operator FROM identity.users AS account WHERE account.email_normalized = $1",
        )
        .bind(&normalized_email)
        .fetch_optional(&self.database)
        .await
        .map_err(AuthenticationError::Persistence)?;

        let candidate_hash = account
            .as_ref()
            .map(|account| account.password_hash.as_str())
            .unwrap_or(&self.dummy_password_hash);
        let parsed_hash =
            PasswordHash::new(candidate_hash).map_err(|_| AuthenticationError::PasswordHash)?;
        let password_matches = Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok();
        let rejected_user_id = account.as_ref().map(|account| account.user_id);
        if password_matches
            && audience == Audience::Tenant
            && account
                .as_ref()
                .is_some_and(|account| account.account_status == "pending_verification")
        {
            self.record_login_rejection(rejected_user_id, correlation_id)
                .await?;
            return Err(AuthenticationError::EmailVerificationRequired);
        }
        let Some(account) = account.filter(|account| {
            password_matches
                && account.account_status == "active"
                && (audience != Audience::Platform || account.platform_operator)
        }) else {
            self.record_login_rejection(rejected_user_id, correlation_id)
                .await?;
            if audience == Audience::Platform {
                self.record_platform_access(
                    rejected_user_id,
                    None,
                    "operator.login",
                    "rejected",
                    None,
                    correlation_id,
                )
                .await?;
            }
            return Err(AuthenticationError::Rejected);
        };

        let session = self
            .create_session(account.user_id, audience, correlation_id)
            .await?;
        let reset_result: Result<usize, redis::RedisError> = cache.del(account_rate_key).await;
        if let Err(error) = reset_result {
            tracing::warn!(error = %error, "Successful login rate counter could not be reset");
        }
        Ok(session)
    }
}
