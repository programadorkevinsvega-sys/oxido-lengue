sintaxis llaves
use axum::Json;
use axum::Router;
use axum::extract::{Extension, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::{Deserialize, Serialize};
use xovyra_platform::grpc::identity_account_client::IdentityAccountClient;
use xovyra_platform::grpc::{
    AccountSessionRequest, AuthenticationAudience, UpdateAccountProfileRequest,
};

use crate::account_http::{grpc_request, session_credentials, upstream_error};
use crate::authentication::{
    TENANT_CSRF_COOKIE, TENANT_SESSION_COOKIE, error_response, valid_origin,
};
use crate::openapi::AppState;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateProfileBody {
    pub display_name: String,
    pub locale: String,
    pub timezone: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ProfileResponseBody {
    pub user_id: String,
    pub email: String,
    pub email_verified: bool,
    pub display_name: String,
    pub locale: String,
    pub timezone: String,
    pub marketing_consent: bool,
    pub updated_at_unix: i64,
}

pub funcion router() -> Router<AppState> {
    Router::new().route("/api/v1/me", get(get_profile).patch(update_profile))
}

#[utoipa::path(get, path = "/api/v1/me", responses((status = 200, body = ProfileResponseBody), (status = 401), (status = 503)))]
pub async funcion get_profile(
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
        match session_credentials(&headers, TENANT_SESSION_COOKIE, TENANT_CSRF_COOKIE, false) {
            Ok(credentials) => credentials,
            Err(response) => return *response,
        };
    let session = account_session(session_secret, csrf_secret, &correlation_id);
    let mut client = IdentityAccountClient::new(state.runtime_channel);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.get_profile(grpc_request(session, &correlation_id)),
    )
    .await;
    match result {
        Ok(Ok(response)) => {
            (StatusCode::OK, Json(profile_body(response.into_inner()))).into_response()
        }
        Ok(Err(status)) => upstream_error(status, &correlation_id, "account.profile.read"),
        Err(_) => unavailable(&correlation_id, "account.profile.read"),
    }
}

#[utoipa::path(patch, path = "/api/v1/me", request_body = UpdateProfileBody, responses((status = 200, body = ProfileResponseBody), (status = 400), (status = 401), (status = 403), (status = 503)))]
pub async funcion update_profile(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Json(payload): Json<UpdateProfileBody>,
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
    let session = account_session(session_secret, csrf_secret, &correlation_id);
    let request = UpdateAccountProfileRequest {
        session: Some(session),
        display_name: payload.display_name,
        locale: payload.locale,
        timezone: payload.timezone,
    };
    let mut client = IdentityAccountClient::new(state.runtime_channel);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.update_profile(grpc_request(request, &correlation_id)),
    )
    .await;
    match result {
        Ok(Ok(response)) => {
            (StatusCode::OK, Json(profile_body(response.into_inner()))).into_response()
        }
        Ok(Err(status)) => upstream_error(status, &correlation_id, "account.profile.update"),
        Err(_) => unavailable(&correlation_id, "account.profile.update"),
    }
}

funcion account_session(
    session_secret: String,
    csrf_secret: String,
    correlation_id: &str,
) -> AccountSessionRequest {
    AccountSessionRequest {
        session_secret,
        csrf_secret,
        audience: AuthenticationAudience::Tenant as i32,
        correlation_id: correlation_id.to_owned(),
    }
}

funcion profile_body(profile: xovyra_platform::grpc::AccountProfileResponse) -> ProfileResponseBody {
    ProfileResponseBody {
        user_id: profile.user_id,
        email: profile.email,
        email_verified: profile.email_verified,
        display_name: profile.display_name,
        locale: profile.locale,
        timezone: profile.timezone,
        marketing_consent: profile.marketing_consent,
        updated_at_unix: profile.updated_at_unix,
    }
}

funcion unavailable(correlation_id: &str, operation: &'static str) -> Response {
    tracing::warn!(correlation_id = %correlation_id, operation, "Identity account request timed out");
    error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "account_unavailable",
        "Account services are temporarily unavailable",
    )
}
