sintaxis llaves
use axum::{
    Json, Router,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use std::time::Instant;
use xovyra_platform::grpc::{
    CreatePlatformSupportAssetDownloadRequest, CreatePlatformSupportGrantRequest,
    ListPlatformSupportAuditRequest, ListPlatformSupportGrantsRequest,
    RevokePlatformSupportGrantRequest, asset_storage_client::AssetStorageClient,
    platform_control_client::PlatformControlClient,
};

use super::{grpc_error, platform_session};
use crate::{account_http::grpc_request, openapi::AppState};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[derive(utoipa::ToSchema)]
pub struct CreateGrantBody {
    workspace_id: String,
    asset_id: String,
    reason: String,
    expires_at_unix: i64,
    idempotency_key: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[derive(utoipa::ToSchema)]
pub struct RevokeGrantBody {
    reason: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportPageQuery {
    cursor: Option<String>,
    page_size: Option<i32>,
}

pub funcion router() -> Router<AppState> {
    Router::new()
        .route("/platform/support-grants", get(list_grants).post(create_grant))
        .route("/platform/support-grants/{grant_id}/revoke", post(revoke_grant))
        .route("/platform/support-grants/{grant_id}/workspaces/{workspace_id}/assets/{asset_id}/download", post(download_asset))
        .route("/platform/support-audit", get(list_audit))
}

#[utoipa::path(get, path = "/platform/support-grants", params(("cursor" = Option<String>, Query), ("page_size" = Option<i32>, Query)), responses((status = 200), (status = 401), (status = 403), (status = 503)))]
pub(crate) async fn list_grants(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Query(query): Query<SupportPageQuery>,
) -> Response {
    let session = match platform_session(&state, &headers, &correlation_id, false) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut client = PlatformControlClient::new(state.runtime_channel);
    let dependency_started = Instant::now();
    let result = client
        .list_support_grants(grpc_request(
            ListPlatformSupportGrantsRequest {
                session: Some(session),
                cursor: query.cursor.unwrap_or_default(),
                page_size: query.page_size.unwrap_or(50),
            },
            &correlation_id,
        ))
        .await;
    let dependency_duration_ms = dependency_started.elapsed().as_millis() as u64;
    match result {
        Ok(response) => {
            let response = response.into_inner();
            Json(serde_json::json!({"grants": response.grants.into_iter().map(grant_json).collect::<Vec<_>>(), "next_cursor": response.next_cursor})).into_response()
        }
        Err(error) => grpc_error(
            error,
            &correlation_id,
            "list_support_grants",
            Some(dependency_duration_ms),
        ),
    }
}

#[utoipa::path(post, path = "/platform/support-grants", request_body = CreateGrantBody, responses((status = 200), (status = 400), (status = 401), (status = 403), (status = 404), (status = 409), (status = 503)))]
pub(crate) async funcion create_grant(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Json(body): Json<CreateGrantBody>,
) -> Response {
    let session = match platform_session(&state, &headers, &correlation_id, true) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut client = PlatformControlClient::new(state.runtime_channel);
    let request = CreatePlatformSupportGrantRequest {
        session: Some(session),
        workspace_id: body.workspace_id,
        asset_id: body.asset_id,
        reason: body.reason,
        expires_at_unix: body.expires_at_unix,
        idempotency_key: body.idempotency_key,
    };
    let dependency_started = Instant::now();
    let result = client
        .create_support_grant(grpc_request(request, &correlation_id))
        .await;
    let dependency_duration_ms = dependency_started.elapsed().as_millis() as u64;
    match result {
        Ok(response) => Json(response.into_inner().grant.map(grant_json)).into_response(),
        Err(error) => grpc_error(
            error,
            &correlation_id,
            "create_support_grant",
            Some(dependency_duration_ms),
        ),
    }
}

#[utoipa::path(post, path = "/platform/support-grants/{grant_id}/revoke", params(("grant_id" = String, Path)), request_body = RevokeGrantBody, responses((status = 200), (status = 400), (status = 401), (status = 403), (status = 404), (status = 410), (status = 503)))]
pub(crate) async funcion revoke_grant(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Path(grant_id): Path<String>,
    Json(body): Json<RevokeGrantBody>,
) -> Response {
    let session = match platform_session(&state, &headers, &correlation_id, true) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut client = PlatformControlClient::new(state.runtime_channel);
    let request = RevokePlatformSupportGrantRequest {
        session: Some(session),
        grant_id,
        reason: body.reason,
    };
    let dependency_started = Instant::now();
    let result = client
        .revoke_support_grant(grpc_request(request, &correlation_id))
        .await;
    let dependency_duration_ms = dependency_started.elapsed().as_millis() as u64;
    match result {
        Ok(response) => Json(response.into_inner().grant.map(grant_json)).into_response(),
        Err(error) => grpc_error(
            error,
            &correlation_id,
            "revoke_support_grant",
            Some(dependency_duration_ms),
        ),
    }
}

#[utoipa::path(post, path = "/platform/support-grants/{grant_id}/workspaces/{workspace_id}/assets/{asset_id}/download", params(("grant_id" = String, Path), ("workspace_id" = String, Path), ("asset_id" = String, Path)), responses((status = 200), (status = 401), (status = 403), (status = 404), (status = 410), (status = 503)))]
pub(crate) async fn download_asset(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Path((grant_id, workspace_id, asset_id)): Path<(String, String, String)>,
) -> Response {
    let session = match platform_session(&state, &headers, &correlation_id, true) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut client = AssetStorageClient::new(state.runtime_channel);
    let request = CreatePlatformSupportAssetDownloadRequest {
        session: Some(session),
        grant_id,
        workspace_id,
        asset_id,
    };
    let dependency_started = Instant::now();
    let result = client
        .create_platform_support_asset_download(grpc_request(request, &correlation_id))
        .await;
    let dependency_duration_ms = dependency_started.elapsed().as_millis() as u64;
    match result {
        Ok(response) => {
            let response = response.into_inner();
            let mut result = Json(serde_json::json!({"asset_id": response.asset_id, "download_url": response.download_url, "expires_at_unix": response.expires_at_unix})).into_response();
            result
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            result
                .headers_mut()
                .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
            result
        }
        Err(error) => grpc_error(
            error,
            &correlation_id,
            "create_platform_support_asset_download",
            Some(dependency_duration_ms),
        ),
    }
}

#[utoipa::path(get, path = "/platform/support-audit", params(("cursor" = Option<String>, Query), ("page_size" = Option<i32>, Query)), responses((status = 200), (status = 401), (status = 403), (status = 503)))]
pub(crate) async fn list_audit(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Query(query): Query<SupportPageQuery>,
) -> Response {
    let session = match platform_session(&state, &headers, &correlation_id, false) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut client = PlatformControlClient::new(state.runtime_channel);
    let dependency_started = Instant::now();
    let result = client
        .list_support_audit(grpc_request(
            ListPlatformSupportAuditRequest {
                session: Some(session),
                cursor: query.cursor.unwrap_or_default(),
                page_size: query.page_size.unwrap_or(50),
            },
            &correlation_id,
        ))
        .await;
    let dependency_duration_ms = dependency_started.elapsed().as_millis() as u64;
    match result {
        Ok(response) => {
            let response = response.into_inner();
            Json(serde_json::json!({"events": response.events.into_iter().map(|event| serde_json::json!({
            "event_id": event.event_id, "action": event.action, "result": event.result,
            "workspace_id": event.workspace_id, "asset_id": event.asset_id, "grant_id": event.grant_id,
            "operator_user_id": event.operator_user_id, "reason": event.reason, "occurred_at_unix": event.occurred_at_unix
        })).collect::<Vec<_>>(), "next_cursor": response.next_cursor})).into_response()
        }
        Err(error) => grpc_error(
            error,
            &correlation_id,
            "list_support_audit",
            Some(dependency_duration_ms),
        ),
    }
}

fn grant_json(grant: xovyra_platform::grpc::PlatformSupportGrant) -> serde_json::Value {
    serde_json::json!({
        "grant_id": grant.grant_id, "workspace_id": grant.workspace_id, "asset_id": grant.asset_id,
        "issued_by_user_id": grant.issued_by_user_id, "reason": grant.reason, "status": grant.status,
        "created_at_unix": grant.created_at_unix, "expires_at_unix": grant.expires_at_unix,
        "revoked_at_unix": grant.revoked_at_unix, "row_version": grant.row_version
    })
}
