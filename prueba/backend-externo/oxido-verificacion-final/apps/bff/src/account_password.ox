sintaxis llaves
use axum::Json;
use axum::Router;
use axum::extract::{Extension, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde::{Deserialize, Serialize};
use xovyra_platform::grpc::identity_account_client::IdentityAccountClient;
use xovyra_platform::grpc::{
    AccountSessionRequest, AuthenticationAudience, ChangeAccountPasswordRequest,
};

use crate::account_http::{grpc_request, session_credentials, upstream_error};
use crate::authentication::{
    TENANT_CSRF_COOKIE, TENANT_SESSION_COOKIE, append_session_cookies, error_response, valid_origin,
};
use crate::openapi::AppState;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangePasswordBody {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct PasswordChangedBody {
    pub changed: bool,
    pub user_id: String,
    pub idle_expires_at_unix: i64,
    pub absolute_expires_at_unix: i64,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/auth/password/change", post(change_password))
}

#[utoipa::path(post, path = "/api/v1/auth/password/change", request_body = ChangePasswordBody, responses((status = 200, body = PasswordChangedBody), (status = 400), (status = 401), (status = 403), (status = 503)))]
pub async fn change_password(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Json(payload): Json<ChangePasswordBody>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    if payload.current_password.is_empty() || !(12..=1024).contains(&payload.new_password.len()) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "password_invalid",
            "Password details are invalid",
        );
    }
    let (session_secret, csrf_secret) =
        match session_credentials(&headers, TENANT_SESSION_COOKIE, TENANT_CSRF_COOKIE, true) {
            Ok(credentials) => credentials,
            Err(response) => return *response,
        };
    let request = ChangeAccountPasswordRequest {
        session: Some(AccountSessionRequest {
            session_secret,
            csrf_secret,
            audience: AuthenticationAudience::Tenant as i32,
            correlation_id: correlation_id.clone(),
        }),
        current_password: payload.current_password,
        new_password: payload.new_password,
    };
    let mut client = IdentityAccountClient::new(state.runtime_channel);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(12),
        client.change_password(grpc_request(request, &correlation_id)),
    )
    .await;
    match result {
        Ok(Ok(response)) => {
            let session = response.into_inner();
            let response = (
                StatusCode::OK,
                Json(PasswordChangedBody {
                    changed: true,
                    user_id: session.user_id,
                    idle_expires_at_unix: session.idle_expires_at_unix,
                    absolute_expires_at_unix: session.absolute_expires_at_unix,
                }),
            )
                .into_response();
            append_session_cookies(
                response,
                &session.session_secret,
                &session.csrf_secret,
                session.absolute_expires_at_unix,
                state.secure_cookies,
            )
        }
        Ok(Err(status)) => upstream_error(status, &correlation_id, "account.password.change"),
        Err(_) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "account_unavailable",
            "Account services are temporarily unavailable",
        ),
    }
}
