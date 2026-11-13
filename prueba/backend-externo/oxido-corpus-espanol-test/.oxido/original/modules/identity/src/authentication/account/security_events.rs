use sqlx::FromRow;
use uuid::Uuid;

use super::super::{AuthenticationError, IdentityService};
use super::cursor::{decode, encode};
use super::{AccountSessionContext, SecurityEventPageRequest, require_tenant};

#[derive(FromRow)]
pub struct SecurityEvent {
    pub event_id: Uuid,
    pub event_type: String,
    pub result: String,
    pub occurred_at_unix: i64,
}

pub struct SecurityEventPage {
    pub items: Vec<SecurityEvent>,
    pub next_cursor: Option<String>,
}

impl IdentityService {
    pub async fn account_security_events(
        &self,
        context: AccountSessionContext<'_>,
        page: SecurityEventPageRequest<'_>,
    ) -> Result<SecurityEventPage, AuthenticationError> {
        require_tenant(context.audience)?;
        if !(1..=100).contains(&page.page_size) {
            return Err(AuthenticationError::InvalidCursor);
        }
        let session = self
            .authenticate_session(
                context.session_secret,
                context.csrf_secret,
                context.audience,
            )
            .await?;
        let cursor = decode(page.cursor)?;
        let cursor_timestamp = cursor.map(|cursor| cursor.0);
        let cursor_identifier = cursor.map(|cursor| cursor.1);
        let mut items = sqlx::query_as::<_, SecurityEvent>(
            "SELECT event_id, event_type, result, floor(extract(epoch FROM occurred_at))::bigint AS occurred_at_unix FROM identity.security_events WHERE user_id = $1 AND ($2::bigint IS NULL OR (occurred_at, event_id) < (to_timestamp($2), $3::uuid)) ORDER BY occurred_at DESC, event_id DESC LIMIT $4",
        )
        .bind(session.user_id)
        .bind(cursor_timestamp)
        .bind(cursor_identifier)
        .bind(page.page_size + 1)
        .fetch_all(&self.database)
        .await
        .map_err(AuthenticationError::Persistence)?;
        let has_more = items.len() as i64 > page.page_size;
        if has_more {
            items.pop();
        }
        let next_cursor = if has_more {
            items
                .last()
                .map(|item| encode(item.occurred_at_unix, item.event_id))
        } else {
            None
        };
        Ok(SecurityEventPage { items, next_cursor })
    }
}
