sintaxis llaves
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use thiserror::Error;

const SIGNATURE_TOLERANCE_SECONDS: i64 = 300;
const MAX_WEBHOOK_BODY_BYTES: usize = 262_144;

#[derive(Debug, Error)]
pub enum WebhookVerificationError {
    #[error("Resend webhook configuration is invalid")]
    Configuration,
    #[error("Resend webhook headers are invalid")]
    Headers,
    #[error("Resend webhook timestamp is outside the accepted window")]
    Timestamp,
    #[error("Resend webhook signature is invalid")]
    Signature,
    #[error("Resend webhook body is invalid")]
    Payload(#[source] serde_json::Error),
    #[error("Resend webhook event is unsupported")]
    UnsupportedEvent,
}

#[derive(Deserialize)]
struct WebhookEnvelope {
    #[serde(rename = "type")]
    event_type: String,
    data: WebhookData,
}

#[derive(Deserialize)]
struct WebhookData {
    email_id: String,
    to: Vec<String>,
}

pub struct VerifiedResendEvent {
    pub event_type: String,
    pub email_id: String,
    pub recipient: String,
}

pub struct ResendWebhookVerifier {
    secret: Vec<u8>,
}

impl ResendWebhookVerifier {
    pub fn new(secret: String) -> Result<Self, WebhookVerificationError> {
        let encoded_secret = secret
            .strip_prefix("whsec_")
            .ok_or(WebhookVerificationError::Configuration)?;
        let secret = STANDARD
            .decode(encoded_secret)
            .map_err(|_| WebhookVerificationError::Configuration)?;
        if secret.len() < 24 {
            return Err(WebhookVerificationError::Configuration);
        }
        Ok(Self { secret })
    }

    pub fn verify(
        &self,
        body: &[u8],
        event_id: &str,
        timestamp: &str,
        signatures: &str,
    ) -> Result<VerifiedResendEvent, WebhookVerificationError> {
        if body.is_empty()
            || body.len() > MAX_WEBHOOK_BODY_BYTES
            || event_id.is_empty()
            || event_id.len() > 255
        {
            return Err(WebhookVerificationError::Headers);
        }
        let timestamp_seconds = timestamp
            .parse::<i64>()
            .map_err(|_| WebhookVerificationError::Headers)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| WebhookVerificationError::Timestamp)?
            .as_secs() as i64;
        if now.abs_diff(timestamp_seconds) > SIGNATURE_TOLERANCE_SECONDS as u64 {
            return Err(WebhookVerificationError::Timestamp);
        }
        let signed_content = [event_id.as_bytes(), b".", timestamp.as_bytes(), b".", body].concat();
        let valid_signature = signatures.split_ascii_whitespace().any(|entry| {
            let Some(encoded) = entry.strip_prefix("v1,") else {
                return false;
            };
            let Ok(signature) = STANDARD.decode(encoded) else {
                return false;
            };
            let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(&self.secret) else {
                return false;
            };
            mac.update(&signed_content);
            mac.verify_slice(&signature).is_ok()
        });
        if !valid_signature {
            return Err(WebhookVerificationError::Signature);
        }
        let event = serde_json::from_slice::<WebhookEnvelope>(body)
            .map_err(WebhookVerificationError::Payload)?;
        let recipient = event
            .data
            .to
            .first()
            .filter(|recipient| !recipient.trim().is_empty() && recipient.len() <= 320)
            .ok_or(WebhookVerificationError::Headers)?
            .trim()
            .to_lowercase();
        if event.data.email_id.is_empty()
            || event.data.email_id.len() > 255
            || !matches!(
                event.event_type.as_str(),
                "email.delivered"
                    | "email.bounced"
                    | "email.complained"
                    | "email.failed"
                    | "email.suppressed"
            )
        {
            return Err(WebhookVerificationError::UnsupportedEvent);
        }
        Ok(VerifiedResendEvent {
            event_type: event.event_type,
            email_id: event.data.email_id,
            recipient,
        })
    }
}
