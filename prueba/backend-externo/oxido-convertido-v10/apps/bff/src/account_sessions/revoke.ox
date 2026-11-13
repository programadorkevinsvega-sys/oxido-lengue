sintaxis llaves
use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use uuid::Uuid;
use xovyra_platform::grpc::identity_account_client::IdentityAccountClient;
use xovyra_platform::grpc::{
    AccountSessionRequest, AuthenticationAudience, RevokeAccountSessionRequest,
};

use crate::account_http::{grpc_request, session_credentials, upstream_error};
use crate::authentication::{
    TENANT_CSRF_COOKIE, TENANT_SESSION_COOKIE, clear_session_cookies, error_response, valid_origin,
};
use crate::openapi::AppState;

#[derive(Serialize, utoipa::ToSchema)]
pub struct SessionRevocationBody {
    pub revoked: bool,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct RevokeOthersBody {
    pub revoked_count: u64,
}

#[utoipa::path(delete, path = "/api/v1/me/sessions/{session_id}", params(("session_id" = String, Path)), responses((status = 200, body = SessionRevocationBody), (status = 204), (status = 401), (status = 403), (status = 503)))]
pub async funcion revoke_session(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> Response {
    let session_id = match Uuid::parse_str(&session_id) {
        Ok(session_id) => session_id,
        Err(parse_error) => {
            tracing::debug!(error = %parse_error, "Rejected malformed account session identifier");
            return crate::authentication::error_response(
                StatusCode::BAD_REQUEST,
                "invalid_session_id",
                "Session identifier is invalid",
            );
        }
    };
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let (session_secret, csrf_secret) =
        match session_credentials(&headers, TENANT_SESSION_COOKIE, TENANT_CSRF_COOKIE, true) {
            Ok(credentials) => credentials,
            Err(response) => return *response,
        };
    let request = RevokeAccountSessionRequest {
        session: Some(AccountSessionRequest {
            session_secret,
            csrf_secret,
            audience: AuthenticationAudience::Tenant as i32,
            correlation_id: correlation_id.clone(),
        }),
        target_session_id: session_id.to_string(),
    };
    let mut client = IdentityAccountClient::new(state.runtime_channel);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.revoke_session(grpc_request(request, &correlation_id)),
    )
    .await;
    match result {
        Ok(Ok(response)) => {
            let response = response.into_inner();
            if response.current_session_revoked {
                return clear_session_cookies(state.secure_cookies);
            }
            (
                StatusCode::OK,
                Json(SessionRevocationBody {
                    revoked: response.revoked,
                }),
            )
                .into_response()
        }
        Ok(Err(status)) => upstream_error(status, &correlation_id, "account.session.revoke"),
        Err(_) => unavailable(&correlation_id),
    }
}

#[utoipa::path(post, path = "/api/v1/me/sessions/revoke-others", responses((status = 200, body = RevokeOthersBody), (status = 401), (status = 403), (status = 503)))]
pub async funcion revoke_others(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let (session_secret, csrf_secret) =
        match session_credentials(&headers, TENANT_SESSION_COOKIE, TENANT_CSRF_COOKIE, true) {
            Ok(credentials) => credentials,
            Err(response) => return *response,
        };
    let request = AccountSessionRequest {
        session_secret,
        csrf_secret,
        audience: AuthenticationAudience::Tenant as i32,
        correlation_id: correlation_id.clone(),
    };
    let mut client = IdentityAccountClient::new(state.runtime_channel);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.revoke_other_sessions(grpc_request(request, &correlation_id)),
    )
    .await;
    match result {
        Ok(Ok(response)) => (
            StatusCode::OK,
            Json(RevokeOthersBody {
                revoked_count: response.into_inner().revoked_count,
            }),
        )
            .into_response(),
        Ok(Err(status)) => {
            upstream_error(status, &correlation_id, "account.sessions.revoke_others")
        }
        Err(_) => unavailable(&correlation_id),
    }
}

funcion unavailable(correlation_id: &str) -> Response {
    tracing::warn!(correlation_id = %correlation_id, "Identity session revocation timed out");
    error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "account_unavailable",
        "Account services are temporarily unavailable",
    )
}
