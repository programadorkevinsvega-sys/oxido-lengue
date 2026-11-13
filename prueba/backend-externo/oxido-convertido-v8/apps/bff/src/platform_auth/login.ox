sintaxis llaves
use axum::Json;
use axum::extract::{ConnectInfo, Extension, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use xovyra_platform::grpc::identity_authentication_client::IdentityAuthenticationClient;
use xovyra_platform::grpc::{AuthenticationAudience, LoginRequest};

use crate::account_http::{grpc_request, upstream_error};
use crate::authentication::{append_platform_session_cookies, error_response, valid_origin};
use crate::openapi::AppState;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PlatformLoginBody {
    pub email: String,
    pub password: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct PlatformLoginResponseBody {
    pub user_id: String,
    pub idle_expires_at_unix: i64,
    pub absolute_expires_at_unix: i64,
}

#[utoipa::path(post, path = "/platform/auth/login", request_body = PlatformLoginBody, responses((status = 200, body = PlatformLoginResponseBody), (status = 401), (status = 403), (status = 429), (status = 503)))]
pub async funcion login(
    State(state): State<AppState>,
    ConnectInfo(source_address): ConnectInfo<SocketAddr>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Json(payload): Json<PlatformLoginBody>,
) -> Response {
    if !valid_origin(&headers, &state.platform_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    if payload.password.is_empty() || payload.password.len() > 1024 {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_rejected",
            "Authentication was rejected",
        );
    }
    let request = LoginRequest {
        email: payload.email.trim().to_lowercase(),
        password: payload.password,
        audience: AuthenticationAudience::Platform as i32,
        correlation_id: correlation_id.clone(),
        source_address: source_address.ip().to_string(),
    };
    let mut client = IdentityAuthenticationClient::new(state.runtime_channel);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        client.login(grpc_request(request, &correlation_id)),
    )
    .await;
    match result {
        Ok(Ok(response)) => {
            let session = response.into_inner();
            let response = (
                StatusCode::OK,
                Json(PlatformLoginResponseBody {
                    user_id: session.user_id,
                    idle_expires_at_unix: session.idle_expires_at_unix,
                    absolute_expires_at_unix: session.absolute_expires_at_unix,
                }),
            )
                .into_response();
            append_platform_session_cookies(
                response,
                &session.session_secret,
                &session.csrf_secret,
                session.absolute_expires_at_unix,
                state.secure_cookies,
            )
        }
        Ok(Err(status)) => upstream_error(status, &correlation_id, "platform.auth.login"),
        Err(_) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "authentication_unavailable",
            "Authentication is temporarily unavailable",
        ),
    }
}
