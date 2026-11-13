sintaxis llaves
use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::{AuthenticationError, IdentityService};

pub(super) const LOGIN_RATE_LIMIT: i64 = 10;
pub(super) const SOURCE_RATE_LIMIT: i64 = 60;
pub(super) const LOGIN_RATE_WINDOW_SECONDS: u64 = 900;
pub(super) const REGISTER_EMAIL_RATE_LIMIT: i64 = 3;
pub(super) const REGISTER_SOURCE_RATE_LIMIT: i64 = 20;
pub(super) const REGISTER_RATE_WINDOW_SECONDS: u64 = 3_600;

impl IdentityService {
    pub(super) fn verification_code_hash(
        &self,
        code: &str,
    ) -> Result<Vec<u8>, AuthenticationError> {
        let mut hasher = Hmac::<Sha256>::new_from_slice(&self.rate_limit_hmac_secret)
            .map_err(|_| AuthenticationError::RateLimitKey)?;
        hasher.update(b"xovyra:identity:email-verification-code:v1:");
        hasher.update(code.as_bytes());
        Ok(hasher.finalize().into_bytes().to_vec())
    }

    pub(super) fn verification_code_matches(
        &self,
        code: &str,
        stored_hash: &[u8],
    ) -> Result<bool, AuthenticationError> {
        let mut hasher = Hmac::<Sha256>::new_from_slice(&self.rate_limit_hmac_secret)
            .map_err(|_| AuthenticationError::RateLimitKey)?;
        hasher.update(b"xovyra:identity:email-verification-code:v1:");
        hasher.update(code.as_bytes());
        Ok(hasher.verify_slice(stored_hash).is_ok())
    }

    pub(super) fn generate_verification_code(&self) -> Result<String, AuthenticationError> {
        let upper_bound = (u32::MAX as u64 + 1) / 90_000_000 * 90_000_000;
        loop {
            let random_value = u32::from_be_bytes(
                super::helpers::random_bytes()?[..4]
                    .try_into()
                    .map_err(|_| AuthenticationError::RandomSource)?,
            );
            if u64::from(random_value) < upper_bound {
                return Ok(format!("{:08}", random_value % 90_000_000 + 10_000_000));
            }
        }
    }

    pub(super) fn rate_limit_key(
        &self,
        purpose: &str,
        value: &str,
    ) -> Result<String, AuthenticationError> {
        let mut hasher = Hmac::<Sha256>::new_from_slice(&self.rate_limit_hmac_secret)
            .map_err(|_| AuthenticationError::RateLimitKey)?;
        hasher.update(b"xovyra:identity:rate-limit:v1:");
        hasher.update(purpose.as_bytes());
        hasher.update(b":");
        hasher.update(value.as_bytes());
        let digest = hasher.finalize().into_bytes();
        let mut encoded = String::with_capacity(digest.len() * 2);
        for byte in digest {
            use std::fmt::Write;
            write!(&mut encoded, "{byte:02x}").map_err(|_| AuthenticationError::RateLimitKey)?;
        }
        Ok(format!("xovyra:rate-limit:identity:{purpose}:{encoded}"))
    }
}
