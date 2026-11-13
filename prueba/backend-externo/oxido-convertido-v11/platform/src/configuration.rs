use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

use http::uri::Authority;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigurationError {
    #[error("local environment file could not be read")]
    EnvironmentFile(#[source] dotenvy::Error),
    #[error("required environment variable {0} is missing or empty")]
    Missing(&'static str),
    #[error("environment variable {0} is not valid Unicode")]
    InvalidUnicode(&'static str),
    #[error("environment variable {0} has an invalid value")]
    InvalidValue(&'static str),
}

pub fn load_environment_file() -> Result<(), ConfigurationError> {
    match dotenvy::dotenv() {
        Ok(_) => Ok(()),
        Err(error) if error.not_found() => Ok(()),
        Err(error) => Err(ConfigurationError::EnvironmentFile(error)),
    }
}

pub struct RuntimeConfiguration {
    pub database_url: String,
    pub identity_database_url: String,
    pub platform_control_database_url: String,
    pub tenancy_access_database_url: String,
    pub commercial_billing_database_url: String,
    pub notifications_database_url: String,
    pub realtime_database_url: String,
    pub asset_storage_database_url: String,
    pub jobs_database_url: String,
    pub redis_url: String,
    pub nats_url: String,
    pub nats_username: String,
    pub nats_password: String,
    pub bind_address: SocketAddr,
    pub session_idle_ttl_seconds: u64,
    pub session_absolute_ttl_seconds: u64,
    pub rate_limit_hmac_secret: String,
    pub tenancy_invitation_token_secret: String,
    pub public_app_origin: String,
    pub internal_runtime_endpoint: String,
    pub resend_api_key: Option<String>,
    pub resend_from_email: Option<String>,
    pub resend_from_marketing: Option<String>,
    pub resend_webhook_secret: Option<String>,
    pub stripe_secret_key: Option<String>,
    pub stripe_webhook_secret: Option<String>,
    pub r2_endpoint: Option<String>,
    pub r2_bucket: Option<String>,
    pub r2_access_key_id: Option<String>,
    pub r2_secret_access_key: Option<String>,
    pub scratch_root: PathBuf,
    pub scratch_retention_seconds: u64,
    pub asset_worker_auth_secret: String,
}

pub struct BffConfiguration {
    pub bind_address: SocketAddr,
    pub module_runtime_endpoint: String,
    pub tenant_allowed_origins: Vec<String>,
    pub platform_allowed_origins: Vec<String>,
    pub secure_cookies: bool,
}

pub struct MigrationConfiguration {
    pub database_url: String,
}

fn required_environment(name: &'static str) -> Result<String, ConfigurationError> {
    let value = std::env::var(name).map_err(|error| match error {
        std::env::VarError::NotPresent => ConfigurationError::Missing(name),
        std::env::VarError::NotUnicode(_) => ConfigurationError::InvalidUnicode(name),
    })?;
    if value.trim().is_empty() {
        return Err(ConfigurationError::Missing(name));
    }
    Ok(value)
}

fn optional_environment(name: &'static str) -> Result<Option<String>, ConfigurationError> {
    match std::env::var(name) {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(ConfigurationError::InvalidUnicode(name)),
    }
}

fn socket_address(name: &'static str) -> Result<SocketAddr, ConfigurationError> {
    let value = required_environment(name)?;
    SocketAddr::from_str(&value).map_err(|_| ConfigurationError::InvalidValue(name))
}

fn positive_seconds(name: &'static str) -> Result<u64, ConfigurationError> {
    let value = required_environment(name)?;
    let seconds = value
        .parse::<u64>()
        .map_err(|_| ConfigurationError::InvalidValue(name))?;
    if seconds == 0 {
        return Err(ConfigurationError::InvalidValue(name));
    }
    Ok(seconds)
}

impl RuntimeConfiguration {
    pub fn from_environment() -> Result<Self, ConfigurationError> {
        let resend_api_key = optional_environment("RESEND_API_KEY")?;
        let resend_from_email = optional_environment("RESEND_FROM_EMAIL")?;
        let resend_from_marketing = optional_environment("RESEND_FROM_MARKETING")?;
        let resend_webhook_secret = optional_environment("RESEND_WEBHOOK_SECRET")?;
        if resend_api_key.is_some() != resend_from_email.is_some()
            || (resend_from_marketing.is_some() && resend_api_key.is_none())
        {
            return Err(ConfigurationError::InvalidValue(
                "RESEND_API_KEY/RESEND_FROM_EMAIL/RESEND_FROM_MARKETING",
            ));
        }
        if let Some(secret) = &resend_webhook_secret
            && !secret.starts_with("whsec_")
        {
            return Err(ConfigurationError::InvalidValue("RESEND_WEBHOOK_SECRET"));
        }
        let stripe_secret_key = optional_environment("STRIPE_SECRET_KEY")?;
        let stripe_webhook_secret = optional_environment("STRIPE_WEBHOOK_SECRET")?;
        if stripe_secret_key.is_some() != stripe_webhook_secret.is_some()
            || stripe_secret_key
                .as_deref()
                .is_some_and(|key| !key.starts_with("sk_test_"))
            || stripe_webhook_secret
                .as_deref()
                .is_some_and(|secret| !secret.starts_with("whsec_"))
        {
            return Err(ConfigurationError::InvalidValue(
                "STRIPE_SECRET_KEY/STRIPE_WEBHOOK_SECRET (test credentials only)",
            ));
        }
        let r2_endpoint = optional_environment("CLOUDFLARE_R2_ENDPOINT")?;
        let r2_bucket = optional_environment("CLOUDFLARE_R2_BUCKET")?;
        let r2_access_key_id = optional_environment("CLOUDFLARE_R2_ACCESS_KEY_ID")?;
        let r2_secret_access_key = optional_environment("CLOUDFLARE_R2_SECRET_ACCESS_KEY")?;
        let r2_values = [
            r2_endpoint.is_some(),
            r2_bucket.is_some(),
            r2_access_key_id.is_some(),
            r2_secret_access_key.is_some(),
        ];
        if r2_values.iter().any(|value| *value) && r2_values.iter().any(|value| !*value) {
            return Err(ConfigurationError::InvalidValue(
                "CLOUDFLARE_R2_ENDPOINT/CLOUDFLARE_R2_BUCKET/CLOUDFLARE_R2_ACCESS_KEY_ID/CLOUDFLARE_R2_SECRET_ACCESS_KEY",
            ));
        }
        if let Some(endpoint) = &r2_endpoint
            && (!endpoint.starts_with("https://") || endpoint.contains(['?', '#', '@']))
        {
            return Err(ConfigurationError::InvalidValue("CLOUDFLARE_R2_ENDPOINT"));
        }
        let public_app_origin = required_environment("PUBLIC_APP_ORIGIN")?;
        if !valid_origin(&public_app_origin) {
            return Err(ConfigurationError::InvalidValue("PUBLIC_APP_ORIGIN"));
        }
        Ok(Self {
            database_url: required_environment("DATABASE_URL")?,
            identity_database_url: required_environment("IDENTITY_DATABASE_URL")?,
            platform_control_database_url: required_environment("PLATFORM_CONTROL_DATABASE_URL")?,
            tenancy_access_database_url: required_environment("TENANCY_ACCESS_DATABASE_URL")?,
            commercial_billing_database_url: required_environment(
                "COMMERCIAL_BILLING_DATABASE_URL",
            )?,
            notifications_database_url: required_environment("NOTIFICATIONS_DATABASE_URL")?,
            realtime_database_url: required_environment("REALTIME_DATABASE_URL")?,
            asset_storage_database_url: required_environment("ASSET_STORAGE_DATABASE_URL")?,
            jobs_database_url: required_environment("JOBS_DATABASE_URL")?,
            redis_url: required_environment("REDIS_URL")?,
            nats_url: required_environment("NATS_URL")?,
            nats_username: required_environment("NATS_USERNAME")?,
            nats_password: required_environment("NATS_PASSWORD")?,
            bind_address: socket_address("MODULE_RUNTIME_BIND")?,
            session_idle_ttl_seconds: positive_seconds("SESSION_IDLE_TTL_SECONDS")?,
            session_absolute_ttl_seconds: positive_seconds("SESSION_ABSOLUTE_TTL_SECONDS")?,
            rate_limit_hmac_secret: required_minimum_secret("RATE_LIMIT_HMAC_SECRET", 32)?,
            tenancy_invitation_token_secret: required_minimum_secret(
                "TENANCY_INVITATION_TOKEN_SECRET",
                32,
            )?,
            public_app_origin,
            internal_runtime_endpoint: required_environment("MODULE_RUNTIME_INTERNAL_ENDPOINT")?,
            resend_api_key,
            resend_from_email,
            resend_from_marketing,
            resend_webhook_secret,
            stripe_secret_key,
            stripe_webhook_secret,
            r2_endpoint,
            r2_bucket,
            r2_access_key_id,
            r2_secret_access_key,
            scratch_root: PathBuf::from(required_environment("XOVYRA_SCRATCH_ROOT")?),
            scratch_retention_seconds: positive_seconds("XOVYRA_SCRATCH_RETENTION_SECONDS")?,
            asset_worker_auth_secret: required_minimum_secret("ASSET_WORKER_AUTH_SECRET", 32)?,
        })
    }
}

fn required_minimum_secret(
    name: &'static str,
    minimum_bytes: usize,
) -> Result<String, ConfigurationError> {
    let secret = required_environment(name)?;
    if secret.len() < minimum_bytes {
        return Err(ConfigurationError::InvalidValue(name));
    }
    Ok(secret)
}

impl BffConfiguration {
    pub fn from_environment() -> Result<Self, ConfigurationError> {
        let tenant_allowed_origins = configured_origins("BFF_TENANT_ALLOWED_ORIGINS")?;
        let platform_allowed_origins = configured_origins("BFF_PLATFORM_ALLOWED_ORIGINS")?;
        let secure_cookies = match required_environment("BFF_SECURE_COOKIES")?.as_str() {
            "true" => true,
            "false" => false,
            _ => return Err(ConfigurationError::InvalidValue("BFF_SECURE_COOKIES")),
        };
        if !secure_cookies
            && (tenant_allowed_origins
                .iter()
                .chain(platform_allowed_origins.iter())
                .any(|origin| !is_local_origin(origin)))
        {
            return Err(ConfigurationError::InvalidValue("BFF_SECURE_COOKIES"));
        }
        Ok(Self {
            bind_address: socket_address("BFF_BIND")?,
            module_runtime_endpoint: required_environment("MODULE_RUNTIME_ENDPOINT")?,
            tenant_allowed_origins,
            platform_allowed_origins,
            secure_cookies,
        })
    }
}

fn configured_origins(name: &'static str) -> Result<Vec<String>, ConfigurationError> {
    let origins = required_environment(name)?
        .split(',')
        .map(str::trim)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if origins.is_empty()
        || origins.iter().any(|origin| !valid_origin(origin))
        || origins
            .iter()
            .enumerate()
            .any(|(index, origin)| origins.iter().skip(index + 1).any(|other| other == origin))
    {
        return Err(ConfigurationError::InvalidValue(name));
    }
    Ok(origins)
}

fn valid_origin(origin: &str) -> bool {
    let Some((scheme, authority)) = origin.split_once("://") else {
        return false;
    };
    if authority.is_empty()
        || authority
            .chars()
            .any(|character| "/?#@*".contains(character))
    {
        return false;
    }
    let Ok(authority) = Authority::from_str(authority) else {
        return false;
    };
    if authority.host().is_empty() || authority.port().is_some() && authority.port_u16().is_none() {
        return false;
    }
    match scheme {
        "https" => true,
        "http" => is_local_origin(origin),
        _ => false,
    }
}

fn is_local_origin(origin: &str) -> bool {
    let Some(authority) = origin.strip_prefix("http://") else {
        return false;
    };
    ["localhost", "127.0.0.1", "[::1]"].iter().any(|host| {
        authority == *host
            || authority
                .strip_prefix(host)
                .is_some_and(|port| port.starts_with(':') && port[1..].parse::<u16>().is_ok())
    })
}

impl MigrationConfiguration {
    pub fn from_environment() -> Result<Self, ConfigurationError> {
        Ok(Self {
            database_url: required_environment("MIGRATOR_DATABASE_URL")?,
        })
    }
}
