use super::shared::{session_context, status};
use tonic::{Response, Status};
use uuid::Uuid;
use xovyra_identity::authentication::{IdentityService, SessionPageRequest, SessionRevocation};
use xovyra_platform::grpc::{
    AccountSessionSummary, ListAccountSessionsRequest, ListAccountSessionsResponse,
    RevokeAccountSessionRequest, RevokeAccountSessionResponse, RevokeOtherAccountSessionsResponse,
};
pub async fn list(
    identity: &IdentityService,
    request: ListAccountSessionsRequest,
) -> Result<Response<ListAccountSessionsResponse>, Status> {
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
        .list_account_sessions(
            context,
            SessionPageRequest {
                cursor,
                page_size: request.page_size,
            },
        )
        .await
        .map_err(status)?;
    Ok(Response::new(ListAccountSessionsResponse {
        sessions: page
            .items
            .into_iter()
            .map(|session| AccountSessionSummary {
                session_id: session.session_id.to_string(),
                created_at_unix: session.created_at_unix,
                last_seen_at_unix: session.last_seen_at_unix,
                idle_expires_at_unix: session.idle_expires_at_unix,
                absolute_expires_at_unix: session.absolute_expires_at_unix,
                current: session.current,
            })
            .collect(),
        next_cursor: page.next_cursor.unwrap_or_default(),
    }))
}
pub async fn revoke(
    identity: &IdentityService,
    request: RevokeAccountSessionRequest,
) -> Result<Response<RevokeAccountSessionResponse>, Status> {
    let session = request
        .session
        .ok_or_else(|| Status::invalid_argument("Account session context is required"))?;
    let target_session_id = request
        .target_session_id
        .parse::<Uuid>()
        .map_err(|_| Status::invalid_argument("Target session identifier is invalid"))?;
    let context = session_context(&session)?;
    let result = identity
        .revoke_account_session(SessionRevocation {
            context,
            target_session_id,
            correlation_id: &session.correlation_id,
        })
        .await
        .map_err(status)?;
    Ok(Response::new(RevokeAccountSessionResponse {
        revoked: result.revoked,
        current_session_revoked: result.current_session_revoked,
    }))
}
pub async fn revoke_others(
    identity: &IdentityService,
    request: xovyra_platform::grpc::AccountSessionRequest,
) -> Result<Response<RevokeOtherAccountSessionsResponse>, Status> {
    let context = session_context(&request)?;
    let revoked_count = identity
        .revoke_other_account_sessions(context, &request.correlation_id)
        .await
        .map_err(status)?;
    Ok(Response::new(RevokeOtherAccountSessionsResponse {
        revoked_count,
    }))
}
