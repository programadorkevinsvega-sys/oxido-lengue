sintaxis llaves
use tonic::{Response, Status};
use xovyra_platform::grpc::{
    AccountSecurityEvent, ListAccountSecurityEventsRequest, ListAccountSecurityEventsResponse,
};

use xovyra_identity::authentication::{IdentityService, SecurityEventPageRequest};

use super::shared::{session_context, status};

pub async fn list(
    identity: &IdentityService,
    request: ListAccountSecurityEventsRequest,
) -> Result<Response<ListAccountSecurityEventsResponse>, Status> {
    let session = request
        .session
        .ok_or_else(|| Status::invalid_argument("Account session context is required"))?;
    let context = session_context(&session)?;
    let cursor = if request.cursor.is_empty() {
        None
    } else {
        Some(request.cursor.as_str())
    };
    let page = identity
        .account_security_events(
            context,
            SecurityEventPageRequest {
                cursor,
                page_size: request.page_size,
            },
        )
        .await
        .map_err(status)?;
    Ok(Response::new(ListAccountSecurityEventsResponse {
        events: page
            .items
            .into_iter()
            .map(|event| AccountSecurityEvent {
                event_id: event.event_id.to_string(),
                event_type: event.event_type,
                result: event.result,
                occurred_at_unix: event.occurred_at_unix,
            })
            .collect(),
        next_cursor: page.next_cursor.unwrap_or_default(),
    }))
}
