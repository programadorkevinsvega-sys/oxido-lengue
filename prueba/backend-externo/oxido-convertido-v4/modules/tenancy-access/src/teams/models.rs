use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum TeamError {
    #[error("team operation input is invalid")]
    InvalidInput,
    #[error("workspace or team resource is unavailable")]
    NotFound,
    #[error("team permission is required")]
    Forbidden,
    #[error("team state does not permit this operation")]
    InvalidState,
    #[error("team resource changed since it was read")]
    VersionConflict,
    #[error("idempotency key conflicts with an earlier command")]
    IdempotencyConflict,
    #[error("invitation limit exceeded; retry after {retry_after_seconds} seconds")]
    RateLimited { retry_after_seconds: u64 },
    #[error("Redis invitation protection is unavailable")]
    RateLimitUnavailable(#[source] redis::RedisError),
    #[error("team persistence operation failed")]
    Persistence(#[source] sqlx::Error),
    #[error("invitation token is invalid or expired")]
    InvalidInvitation,
    #[error("invitation notification material is unavailable")]
    DeliveryUnavailable,
}

#[derive(Clone, Debug)]
pub struct Member {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub email: Option<String>,
    pub role_key: String,
    pub status: String,
    pub joined_at_unix: i64,
    pub row_version: i64,
}

#[derive(Clone, Debug)]
pub struct Role {
    pub workspace_id: Uuid,
    pub role_key: String,
    pub display_name: String,
    pub permissions: Vec<String>,
    pub system_role: bool,
    pub row_version: i64,
}

#[derive(Clone, Debug)]
pub struct Invitation {
    pub invitation_id: Uuid,
    pub workspace_id: Uuid,
    pub email: String,
    pub role_key: String,
    pub status: String,
    pub delivery_status: String,
    pub created_at_unix: i64,
    pub expires_at_unix: i64,
}

#[derive(Clone, Debug)]
pub struct InvitationDelivery {
    pub invitation_id: Uuid,
    pub email: String,
    pub workspace_name: String,
    pub token: String,
    pub expires_at_unix: i64,
}

#[derive(Clone, Debug)]
pub struct Task {
    pub task_id: Uuid,
    pub workspace_id: Uuid,
    pub title: String,
    pub description: String,
    pub status: String,
    pub assignee_user_id: Option<Uuid>,
    pub author_user_id: Uuid,
    pub created_at_unix: i64,
    pub updated_at_unix: i64,
    pub row_version: i64,
}

#[derive(Clone, Debug)]
pub struct TaskComment {
    pub comment_id: Uuid,
    pub task_id: Uuid,
    pub workspace_id: Uuid,
    pub author_user_id: Uuid,
    pub body: String,
    pub deleted: bool,
    pub mentioned_user_ids: Vec<Uuid>,
    pub created_at_unix: i64,
}
