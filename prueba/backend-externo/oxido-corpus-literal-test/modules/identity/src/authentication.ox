sintaxis llaves
use std::time::Duration;

use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, SaltString};
use sqlx::{FromRow, PgPool};
use thiserror::Error;
use uuid::Uuid;

pub mod account;
pub mod email_actions;
pub mod helpers;
pub mod login;
pub mod login_rate_limit;
pub mod marketing_consent;
mod outbox;
pub mod password_recovery;
pub mod platform_access;
pub mod provider_events;
pub mod registration;
pub mod session_creation;
pub mod session_validation;
pub mod verification;

pub use account::{
    AccountSessionContext, PasswordChange, ProfileUpdate, SecurityEventPageRequest,
    SessionPageRequest, SessionRevocation,
};

const DUMMY_PASSWORD: &str = "identity-login-timing-equalization";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Audience {
    Tenant,
    Platform,
}

impl Audience {
    fn as_database_value(self) -> &'static str {
        match self {
            Self::Tenant => "tenant",
            Self::Platform => "platform",
        }
    }
}

pub struct IdentityService {
    database: PgPool,
    cache: redis::aio::ConnectionManager,
    rate_limit_hmac_secret: Vec<u8>,
    idle_session_lifetime: Duration,
    absolute_session_lifetime: Duration,
    dummy_password_hash: String,
}

pub struct LoginSession {
    pub session_id: Uuid,
    pub session_secret: String,
    pub csrf_secret: String,
    pub user_id: Uuid,
    pub idle_expires_at: i64,
    pub absolute_expires_at: i64,
}

pub struct AuthenticatedSession {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub email_verified: bool,
    pub idle_expires_at: i64,
    pub absolute_expires_at: i64,
}

#[derive(Debug, Error)]
pub enum AuthenticationError {
    #[error("authentication was rejected")]
    Rejected,
    #[error("email verification is required before login")]
    EmailVerificationRequired,
    #[error("session secret is invalid")]
    InvalidSessionSecret,
    #[error("authentication persistence failed")]
    Persistence(#[source] sqlx::Error),
    #[error("password hashing configuration failed")]
    PasswordHash,
    #[error("operating system random source failed")]
    RandomSource,
    #[error("login attempts exceeded the allowed rate")]
    RateLimited,
    #[error("email action requests exceeded the allowed rate")]
    EmailActionRateLimited {
        retry_after_seconds: u64,
        account_limit_exceeded: bool,
        limit: i64,
    },
    #[error("login abuse protection is unavailable")]
    RateLimitUnavailable(#[source] redis::RedisError),
    #[error("login rate limit key could not be computed")]
    RateLimitKey,
    #[error("login source address is invalid")]
    InvalidSourceAddress,
    #[error("system clock is before the Unix epoch")]
    InvalidSystemClock(#[source] std::time::SystemTimeError),
    #[error("session expiry is outside the supported Unix timestamp range")]
    ExpiryOutOfRange,
    #[error("account profile data is invalid")]
    InvalidProfile,
    #[error("account security cursor is invalid")]
    InvalidCursor,
    #[error("the identity lacks the required platform permission")]
    PermissionDenied,
    #[error("identity event is invalid")]
    InvalidEvent,
    #[error("identity event serialization failed")]
    EventSerialization(#[source] serde_json::Error),
}

#[derive(FromRow)]
struct LoginAccount {
    user_id: Uuid,
    password_hash: String,
    account_status: String,
    platform_operator: bool,
}

#[derive(FromRow)]
struct SessionRow {
    session_id: Uuid,
    user_id: Uuid,
    idle_expires_at: i64,
    absolute_expires_at: i64,
}

#[derive(FromRow)]
struct RefreshableSession {
    user_id: Uuid,
    absolute_expires_at: i64,
}

pub struct PendingVerification {
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub token: String,
    pub verification_code: Option<String>,
}

pub enum RegistrationResult {
    Created(PendingVerification),
    AlreadyExists,
}

impl IdentityService {
    pub fn new(
        database: PgPool,
        cache: redis::aio::ConnectionManager,
        rate_limit_hmac_secret: String,
        idle_session_lifetime: Duration,
        absolute_session_lifetime: Duration,
    ) -> Result<Self, AuthenticationError> {
        let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes())
            .map_err(|_| AuthenticationError::PasswordHash)?;
        let dummy_password_hash = Argon2::default()
            .hash_password(DUMMY_PASSWORD.as_bytes(), &salt)
            .map_err(|_| AuthenticationError::PasswordHash)?
            .to_string();
        Ok(Self {
            database,
            cache,
            rate_limit_hmac_secret: rate_limit_hmac_secret.into_bytes(),
            idle_session_lifetime,
            absolute_session_lifetime,
            dummy_password_hash,
        })
    }
}

pub use helpers::{normalize_email, registration_input_valid};
