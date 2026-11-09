sintaxis llaves
use axum::Json;
use axum::extract::{Extension, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use xovyra_platform::grpc::identity_account_client::IdentityAccountClient;
use xovyra_platform::grpc::{
    AccountSessionRequest, AuthenticationAudience, ListAccountSessionsRequest,
};

use crate::account_http::{grpc_request, session_credentials, upstream_error};
use crate::authentication::{
    TENANT_CSRF_COOKIE, TENANT_SESSION_COOKIE, error_response, valid_origin,
};
use crate::openapi::AppState;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionQuery {
    pub cursor: Option<String>,
    pub page_size: Option<i64>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SessionSummaryBody {
    pub session_id: String,
    pub created_at_unix: i64,
    pub last_seen_at_unix: i64,
    pub idle_expires_at_unix: i64,
    pub absolute_expires_at_unix: i64,
    pub current: bool,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SessionListBody {
    pub items: Vec<SessionSummaryBody>,
    pub next_cursor: Option<String>,
}

#[utoipa::path(get, path = "/api/v1/me/sessions", params(("cursor" = Option<String>, Query), ("page_size" = Option<i64>, Query)), responses((status = 200, body = SessionListBody), (status = 401), (status = 503)))]
pub async fn list_sessions(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Query(query): Query<SessionQuery>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let (session_secret, csrf_secret) =
        match session_credentials(&headers, TENANT_SESSION_COOKIE, TENANT_CSRF_COOKIE, false) {
            Ok(credentials) => credentials,
            Err(response) => return *response,
        };
    let request = ListAccountSessionsRequest {
        session: Some(AccountSessionRequest {
            session_secret,
            csrf_secret,
            audience: AuthenticationAudience::Tenant as i32,
            correlation_id: correlation_id.clone(),
        }),
        cursor: query.cursor.unwrap_or_default(),
        page_size: query.page_size.unwrap_or(50),
    };
    let mut client = IdentityAccountClient::new(state.runtime_channel);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.list_sessions(grpc_request(request, &correlation_id)),
    )
    .await;
    match result {
        Ok(Ok(response)) => {
            let response = response.into_inner();
            let items = response
                .sessions
                .into_iter()
                .map(|session| SessionSummaryBody {
                    session_id: session.session_id,
                    created_at_unix: session.created_at_unix,
                    last_seen_at_unix: session.last_seen_at_unix,
                    idle_expires_at_unix: session.idle_expires_at_unix,
                    absolute_expires_at_unix: session.absolute_expires_at_unix,
                    current: session.current,
                })
                .collect();
            (
                StatusCode::OK,
                Json(SessionListBody {
                    items,
                    next_cursor: (!response.next_cursor.is_empty()).then_some(response.next_cursor),
                }),
            )
                .into_response()
        }
        Ok(Err(status)) => upstream_error(status, &correlation_id, "account.sessions.list"),
        Err(_) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "account_unavailable",
            "Account services are temporarily unavailable",
        ),
    }
}
