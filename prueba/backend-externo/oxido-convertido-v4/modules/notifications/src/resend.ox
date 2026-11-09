sintaxis llaves
use std::time::Duration;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const RESEND_EMAILS_URL: &str = "https://api.resend.com/emails";

#[derive(Clone)]
pub struct ResendMailer {
    client: reqwest::Client,
    api_key: String,
    sender: String,
}

#[derive(Serialize)]
struct ResendEmailRequest<'message> {
    from: &'message str,
    to: [&'message str; 1],
    subject: &'message str,
    html: &'message str,
    text: &'message str,
}

#[derive(Deserialize)]
struct ResendEmailResponse {
    id: String,
}

#[derive(Debug, Error)]
pub enum ResendError {
    #[error("Resend configuration is invalid")]
    Configuration,
    #[error("Resend request failed before receiving a response")]
    Transport(#[source] reqwest::Error),
    #[error("Resend rejected the email request with status {0}")]
    Rejected(u16),
    #[error("Resend returned an invalid success response")]
    InvalidResponse(#[source] reqwest::Error),
    #[error("Resend response did not contain a message identifier")]
    MissingMessageIdentifier,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResendReceipt {
    pub message_id: String,
}

impl ResendMailer {
    pub fn new(api_key: String, sender: String, timeout: Duration) -> Result<Self, ResendError> {
        if api_key.trim().is_empty()
            || sender.trim().is_empty()
            || !sender.contains('@')
            || timeout.is_zero()
        {
            return Err(ResendError::Configuration);
        }
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(ResendError::Transport)?;
        Ok(Self {
            client,
            api_key,
            sender,
        })
    }

    pub async fn send(
        &self,
        recipient: &str,
        subject: &str,
        html: &str,
        text: &str,
        idempotency_key: &str,
    ) -> Result<ResendReceipt, ResendError> {
        if recipient.trim().is_empty()
            || subject.trim().is_empty()
            || html.trim().is_empty()
            || text.trim().is_empty()
            || idempotency_key.is_empty()
            || idempotency_key.len() > 256
        {
            return Err(ResendError::Configuration);
        }
        let authorization = HeaderValue::from_str(&format!("Bearer {}", self.api_key))
            .map_err(|_| ResendError::Configuration)?;
        let idempotency =
            HeaderValue::from_str(idempotency_key).map_err(|_| ResendError::Configuration)?;
        let payload = ResendEmailRequest {
            from: &self.sender,
            to: [recipient],
            subject,
            html,
            text,
        };
        let response = self
            .client
            .post(RESEND_EMAILS_URL)
            .header(AUTHORIZATION, authorization)
            .header(CONTENT_TYPE, "application/json")
            .header("Idempotency-Key", idempotency)
            .json(&payload)
            .send()
            .await
            .map_err(ResendError::Transport)?;
        if !response.status().is_success() {
            return Err(ResendError::Rejected(response.status().as_u16()));
        }
        let receipt = response
            .json::<ResendEmailResponse>()
            .await
            .map_err(ResendError::InvalidResponse)?;
        if receipt.id.trim().is_empty() {
            return Err(ResendError::MissingMessageIdentifier);
        }
        Ok(ResendReceipt {
            message_id: receipt.id,
        })
    }
}
