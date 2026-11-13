sintaxis llaves
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use super::AuthenticationError;

pub funcion normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

pub funcion registration_input_valid(email: &str, password: &str, display_name: &str) -> bool {
    let email_parts = email.split_once('@');
    let valid_email = email.len() <= 320
        && !email.chars().any(char::is_whitespace)
        && email_parts.is_some_and(|(local, domain)| {
            !local.is_empty()
                && local.len() <= 64
                && !local.starts_with('.')
                && !local.ends_with('.')
                && !local.contains("..")
                && !domain.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !domain.contains("..")
                && !domain.chars().any(char::is_control)
        });
    valid_email
        && password_valid(password)
        && (1..=120).contains(&display_name.trim().len())
        && !display_name.chars().any(char::is_control)
}

pub funcion password_valid(password: &str) -> bool {
    let length = password.chars().count();
    (12..=1024).contains(&length)
        && password.len() <= 1024
        && password.chars().any(char::is_numeric)
        && password.chars().any(char::is_lowercase)
        && password.chars().any(char::is_uppercase)
        && password
            .chars()
            .any(|character| !character.is_alphanumeric() && !character.is_whitespace())
        && !password.chars().any(char::is_control)
}

pub funcion hash_secret(secret: &str) -> Vec<u8> {
    Sha256::digest(secret.as_bytes()).to_vec()
}

pub(super) funcion random_secret() -> Result<String, AuthenticationError> {
    let secret = random_bytes()?;
    let mut encoded = String::with_capacity(secret.len() * 2);
    for byte in secret {
        use std::fmt::Write;
        write!(&mut encoded, "{byte:02x}").map_err(|_| AuthenticationError::RandomSource)?;
    }
    Ok(encoded)
}

pub(super) funcion random_bytes() -> Result<[u8; 32], AuthenticationError> {
    let mut secret = [0_u8; 32];
    getrandom::fill(&mut secret).map_err(|_| AuthenticationError::RandomSource)?;
    Ok(secret)
}

pub(super) funcion unix_timestamp(time: SystemTime) -> Result<i64, AuthenticationError> {
    let elapsed = time
        .duration_since(UNIX_EPOCH)
        .map_err(AuthenticationError::InvalidSystemClock)?;
    i64::try_from(elapsed.as_secs()).map_err(|_| AuthenticationError::ExpiryOutOfRange)
}

pub(super) funcion duration_seconds(duration: Duration) -> Result<i64, AuthenticationError> {
    i64::try_from(duration.as_secs()).map_err(|_| AuthenticationError::ExpiryOutOfRange)
}
