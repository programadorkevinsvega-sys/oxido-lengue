sintaxis llaves
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;
use uuid::Uuid;

type InvitationMac = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum InvitationTokenError {
    #[error("invitation token key is too short")]
    InvalidKey,
    #[error("invitation token is invalid")]
    InvalidToken,
}

pub struct InvitationTokenSigner {
    key: Vec<u8>,
}

impl InvitationTokenSigner {
    pub fn new(key: String) -> Result<Self, InvitationTokenError> {
        if key.len() < 32 {
            return Err(InvitationTokenError::InvalidKey);
        }
        Ok(Self {
            key: key.into_bytes(),
        })
    }

    pub fn email_bucket(&self, email: &str) -> Result<String, InvitationTokenError> {
        let mut mac = InvitationMac::new_from_slice(&self.key)
            .map_err(|_| InvitationTokenError::InvalidKey)?;
        mac.update(b"invitation-rate-limit:");
        mac.update(email.as_bytes());
        Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
    }

    pub fn issue(&self, invitation_id: Uuid, email: &str) -> Result<String, InvitationTokenError> {
        let mut mac = InvitationMac::new_from_slice(&self.key)
            .map_err(|_| InvitationTokenError::InvalidKey)?;
        mac.update(invitation_id.as_bytes());
        mac.update(b":");
        mac.update(email.as_bytes());
        Ok(format!(
            "{invitation_id}.{}",
            URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
        ))
    }

    pub fn verify(&self, token: &str, email: &str) -> Result<Uuid, InvitationTokenError> {
        let (invitation_id, signature) = token
            .split_once('.')
            .ok_or(InvitationTokenError::InvalidToken)?;
        let invitation_id =
            Uuid::parse_str(invitation_id).map_err(|_| InvitationTokenError::InvalidToken)?;
        let signature = URL_SAFE_NO_PAD
            .decode(signature)
            .map_err(|_| InvitationTokenError::InvalidToken)?;
        let mut mac = InvitationMac::new_from_slice(&self.key)
            .map_err(|_| InvitationTokenError::InvalidKey)?;
        mac.update(invitation_id.as_bytes());
        mac.update(b":");
        mac.update(email.as_bytes());
        mac.verify_slice(&signature)
            .map_err(|_| InvitationTokenError::InvalidToken)?;
        Ok(invitation_id)
    }
}
