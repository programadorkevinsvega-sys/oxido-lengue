sintaxis llaves
use axum::Json;
use axum::extract::{Extension, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use xovyra_platform::grpc::identity_authentication_client::IdentityAuthenticationClient;
use xovyra_platform::grpc::{
    AuthenticateSessionRequest, AuthenticationAudience, LogoutRequest, RefreshSessionRequest,
};

use crate::account_http::{grpc_request, session_credentials, upstream_error};
use crate::authentication::{
    PLATFORM_CSRF_COOKIE, PLATFORM_SESSION_COOKIE, append_platform_session_cookies,
    clear_platform_session_cookies, error_response, valid_origin,
};
use crate::openapi::AppState;

#[derive(Serialize, utoipa::ToSchema)]
pub struct PlatformSessionResponseBody {
    pub session_id: String,
    pub user_id: String,
    pub idle_expires_at_unix: i64,
    pub absolute_expires_at_unix: i64,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct PlatformRefreshResponseBody {
    pub user_id: String,
    pub idle_expires_at_unix: i64,
    pub absolute_expires_at_unix: i64,
}

#[utoipa::path(get, path = "/platform/auth/session", responses((status = 200, body = PlatformSessionResponseBody), (status = 401), (status = 403), (status = 503)))]
pub async fn current(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
) -> Response {
    if !valid_origin(&headers, &state.platform_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let (session_secret, csrf_secret) = match session_credentials(
        &headers,
        PLATFORM_SESSION_COOKIE,
        PLATFORM_CSRF_COOKIE,
        false,
    ) {
        Ok(credentials) => credentials,
        Err(response) => return *response,
    };
    let request = AuthenticateSessionRequest {
        session_secret,
        audience: AuthenticationAudience::Platform as i32,
        correlation_id: correlation_id.clone(),
        csrf_secret,
    };
    let mut client = IdentityAuthenticationClient::new(state.runtime_channel);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.authenticate_session(grpc_request(request, &correlation_id)),
    )
    .await;
    match result {
        Ok(Ok(response)) => {
            let session = response.into_inner();
            (
                StatusCode::OK,
                Json(PlatformSessionResponseBody {
                    session_id: session.session_id,
                    user_id: session.user_id,
                    idle_expires_at_unix: session.idle_expires_at_unix,
                    absolute_expires_at_unix: session.absolute_expires_at_unix,
                }),
            )
                .into_response()
        }
        Ok(Err(status)) => upstream_error(status, &correlation_id, "platform.auth.session"),
        Err(_) => unavailable(&correlation_id),
    }
}

#[utoipa::path(post, path = "/platform/auth/refresh", responses((status = 200, body = PlatformRefreshResponseBody), (status = 401), (status = 403), (status = 503)))]
pub async fn refresh(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
) -> Response {
    if !valid_origin(&headers, &state.platform_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let (session_secret, csrf_secret) = match session_credentials(
        &headers,
        PLATFORM_SESSION_COOKIE,
        PLATFORM_CSRF_COOKIE,
        true,
    ) {
        Ok(credentials) => credentials,
        Err(response) => return *response,
    };
    let request = RefreshSessionRequest {
        session_secret,
        csrf_secret,
        audience: AuthenticationAudience::Platform as i32,
        correlation_id: correlation_id.clone(),
    };
    let mut client = IdentityAuthenticationClient::new(state.runtime_channel);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.refresh_session(grpc_request(request, &correlation_id)),
    )
    .await;
    match result {
        Ok(Ok(response)) => {
            let session = response.into_inner();
            let body = (
                StatusCode::OK,
                Json(PlatformRefreshResponseBody {
                    user_id: session.user_id.clone(),
                    idle_expires_at_unix: session.idle_expires_at_unix,
                    absolute_expires_at_unix: session.absolute_expires_at_unix,
                }),
            )
                .into_response();
            append_platform_session_cookies(
                body,
                &session.session_secret,
                &session.csrf_secret,
                session.absolute_expires_at_unix,
                state.secure_cookies,
            )
        }
        Ok(Err(status)) => upstream_error(status, &correlation_id, "platform.auth.refresh"),
        Err(_) => unavailable(&correlation_id),
    }
}

#[utoipa::path(post, path = "/platform/auth/logout", responses((status = 204), (status = 401), (status = 403), (status = 503)))]
pub async fn logout(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
) -> Response {
    if !valid_origin(&headers, &state.platform_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let session_secret = crate::authentication::cookie_value(&headers, PLATFORM_SESSION_COOKIE);
    let Some(session_secret) = session_secret.filter(|secret| !secret.is_empty()) else {
        return clear_platform_session_cookies(state.secure_cookies);
    };
    let csrf_secret = crate::authentication::cookie_value(&headers, PLATFORM_CSRF_COOKIE);
    let csrf_header = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok());
    if csrf_secret.as_deref() != csrf_header || csrf_secret.as_deref().is_none_or(str::is_empty) {
        return error_response(
            StatusCode::FORBIDDEN,
            "csrf_rejected",
            "CSRF validation failed",
        );
    }
    let Some(csrf_secret) = csrf_secret else {
        return error_response(
            StatusCode::FORBIDDEN,
            "csrf_rejected",
            "CSRF validation failed",
        );
    };
    let request = LogoutRequest {
        session_secret,
        audience: AuthenticationAudience::Platform as i32,
        correlation_id: correlation_id.clone(),
        csrf_secret,
    };
    let mut client = IdentityAuthenticationClient::new(state.runtime_channel);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.logout(grpc_request(request, &correlation_id)),
    )
    .await;
    match result {
        Ok(Ok(response)) => {
            let result = response.into_inner();
            tracing::debug!(correlation_id = %correlation_id, revoked = result.revoked, "Platform session logout completed");
            clear_platform_session_cookies(state.secure_cookies)
        }
        Ok(Err(status)) => upstream_error(status, &correlation_id, "platform.auth.logout"),
        Err(_) => unavailable(&correlation_id),
    }
}

fn unavailable(correlation_id: &str) -> Response {
    tracing::warn!(correlation_id = %correlation_id, "Platform identity request timed out");
    error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "authentication_unavailable",
        "Authentication is temporarily unavailable",
    )
}
