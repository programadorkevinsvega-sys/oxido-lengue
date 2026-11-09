use axum::Json;
use axum::extract::{Extension, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use xovyra_platform::grpc::identity_account_client::IdentityAccountClient;
use xovyra_platform::grpc::{AccountSessionRequest, AuthenticationAudience};

use crate::account_http::{grpc_request, session_credentials, upstream_error};
use crate::authentication::{
    PLATFORM_CSRF_COOKIE, PLATFORM_SESSION_COOKIE, error_response, valid_origin,
};
use crate::openapi::AppState;

#[derive(Serialize, utoipa::ToSchema)]
pub struct PlatformPermissionsBody {
    pub user_id: String,
    pub permissions: Vec<String>,
}

#[utoipa::path(get, path = "/platform/me/permissions", responses((status = 200, body = PlatformPermissionsBody), (status = 401), (status = 403), (status = 503)))]
pub async fn get_permissions(
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
    let request = AccountSessionRequest {
        session_secret,
        csrf_secret,
        audience: AuthenticationAudience::Platform as i32,
        correlation_id: correlation_id.clone(),
    };
    let mut client = IdentityAccountClient::new(state.runtime_channel);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.get_platform_permissions(grpc_request(request, &correlation_id)),
    )
    .await;
    match result {
        Ok(Ok(response)) => {
            let permissions = response.into_inner();
            (
                StatusCode::OK,
                Json(PlatformPermissionsBody {
                    user_id: permissions.user_id,
                    permissions: permissions.permissions,
                }),
            )
                .into_response()
        }
        Ok(Err(status)) => upstream_error(status, &correlation_id, "platform.permissions.read"),
        Err(_) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "platform_unavailable",
            "Platform services are temporarily unavailable",
        ),
    }
}
