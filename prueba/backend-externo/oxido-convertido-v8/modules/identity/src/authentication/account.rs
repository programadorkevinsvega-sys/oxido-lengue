use uuid::Uuid;

use super::{Audience, AuthenticationError};

mod cursor;
mod password_change;
mod profile;
mod security_events;
pub(super) mod sessions;

pub use profile::AccountProfile;
pub use security_events::{SecurityEvent, SecurityEventPage};
pub use sessions::{AccountSession, AccountSessionPage};

#[derive(Clone, Copy)]
pub struct AccountSessionContext<'session> {
    pub session_secret: &'session str,
    pub csrf_secret: &'session str,
    pub audience: Audience,
    pub correlation_id: &'session str,
}

pub struct ProfileUpdate<'profile> {
    pub display_name: &'profile str,
    pub locale: &'profile str,
    pub timezone: &'profile str,
    pub correlation_id: &'profile str,
}

pub struct SessionPageRequest<'cursor> {
    pub cursor: Option<&'cursor str>,
    pub page_size: i64,
}

pub struct SecurityEventPageRequest<'cursor> {
    pub cursor: Option<&'cursor str>,
    pub page_size: i64,
}

pub struct PasswordChange<'password> {
    pub current_password: &'password str,
    pub new_password: &'password str,
    pub correlation_id: &'password str,
}

pub struct SessionRevocation<'session, 'correlation> {
    pub context: AccountSessionContext<'session>,
    pub target_session_id: Uuid,
    pub correlation_id: &'correlation str,
}

pub(super) fn require_tenant(audience: Audience) -> Result<(), AuthenticationError> {
    if audience == Audience::Tenant {
        Ok(())
    } else {
        Err(AuthenticationError::Rejected)
    }
}
