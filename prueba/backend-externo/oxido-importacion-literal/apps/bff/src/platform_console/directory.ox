sintaxis llaves
use axum::{
    Json, Router,
    extract::{Extension, Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use std::time::Instant;
use xovyra_platform::grpc::{
    ListPlatformWorkspaceAssetsRequest, ListPlatformWorkspaceMembersRequest,
    ListPlatformWorkspacesRequest, asset_storage_client::AssetStorageClient,
    tenancy_access_client::TenancyAccessClient,
};

use super::{grpc_error, platform_session};
use crate::{account_http::grpc_request, openapi::AppState};

#[derive(Deserialize)]
pub struct DirectoryQuery {
    search: Option<String>,
    cursor: Option<String>,
    page_size: Option<i32>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/platform/workspaces", get(list_workspaces))
        .route(
            "/platform/workspaces/{workspace_id}/members",
            get(list_members),
        )
        .route(
            "/platform/workspaces/{workspace_id}/assets",
            get(list_assets),
        )
}

#[utoipa::path(get, path = "/platform/workspaces/{workspace_id}/assets", params(("workspace_id" = String, Path), ("cursor" = Option<String>, Query), ("page_size" = Option<i32>, Query)), responses((status = 200), (status = 401), (status = 403), (status = 503)))]
pub(crate) async fn list_assets(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
    Query(query): Query<DirectoryQuery>,
) -> Response {
    let session = match platform_session(&state, &headers, &correlation_id, false) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut client = AssetStorageClient::new(state.runtime_channel);
    let request = ListPlatformWorkspaceAssetsRequest {
        session: Some(session),
        workspace_id,
        cursor: query.cursor.unwrap_or_default(),
        page_size: query.page_size.unwrap_or(50),
    };
    let dependency_started = Instant::now();
    let result = client
        .list_platform_workspace_assets(grpc_request(request, &correlation_id))
        .await;
    let dependency_duration_ms = dependency_started.elapsed().as_millis() as u64;
    match result {
        Ok(response) => {
            let response = response.into_inner();
            Json(serde_json::json!({"assets": response.assets.into_iter().map(|asset| serde_json::json!({
                "asset_id": asset.asset_id, "content_type": asset.content_type, "size_bytes": asset.size_bytes,
                "lifecycle_state": asset.lifecycle_state, "created_at_unix": asset.created_at_unix
            })).collect::<Vec<_>>(), "next_cursor": response.next_cursor})).into_response()
        }
        Err(error) => grpc_error(
            error,
            &correlation_id,
            "list_platform_workspace_assets",
            Some(dependency_duration_ms),
        ),
    }
}

#[utoipa::path(get, path = "/platform/workspaces", params(("search" = Option<String>, Query), ("cursor" = Option<String>, Query), ("page_size" = Option<i32>, Query)), responses((status = 200), (status = 401), (status = 403), (status = 503)))]
pub(crate) async fn list_workspaces(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Query(query): Query<DirectoryQuery>,
) -> Response {
    let session = match platform_session(&state, &headers, &correlation_id, false) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut client = TenancyAccessClient::new(state.runtime_channel);
    let request = ListPlatformWorkspacesRequest {
        session: Some(session),
        search: query.search.unwrap_or_default(),
        cursor: query.cursor.unwrap_or_default(),
        page_size: query.page_size.unwrap_or(50),
    };
    let dependency_started = Instant::now();
    let result = client
        .list_platform_workspaces(grpc_request(request, &correlation_id))
        .await;
    let dependency_duration_ms = dependency_started.elapsed().as_millis() as u64;
    match result {
        Ok(response) => {
            let response = response.into_inner();
            Json(serde_json::json!({"workspaces": response.workspaces.into_iter().map(|workspace| serde_json::json!({
                "workspace_id": workspace.workspace_id, "name": workspace.name, "workspace_type": workspace.workspace_type,
                "status": workspace.status, "created_at_unix": workspace.created_at_unix,
                "member_count": workspace.member_count, "row_version": workspace.row_version
            })).collect::<Vec<_>>(), "next_cursor": response.next_cursor})).into_response()
        }
        Err(error) => grpc_error(
            error,
            &correlation_id,
            "list_platform_workspaces",
            Some(dependency_duration_ms),
        ),
    }
}

#[utoipa::path(get, path = "/platform/workspaces/{workspace_id}/members", params(("workspace_id" = String, Path), ("cursor" = Option<String>, Query), ("page_size" = Option<i32>, Query)), responses((status = 200), (status = 401), (status = 403), (status = 503)))]
pub(crate) async fn list_members(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
    Query(query): Query<DirectoryQuery>,
) -> Response {
    let session = match platform_session(&state, &headers, &correlation_id, false) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut client = TenancyAccessClient::new(state.runtime_channel);
    let request = ListPlatformWorkspaceMembersRequest {
        session: Some(session),
        workspace_id,
        cursor: query.cursor.unwrap_or_default(),
        page_size: query.page_size.unwrap_or(50),
    };
    let dependency_started = Instant::now();
    let result = client
        .list_platform_workspace_members(grpc_request(request, &correlation_id))
        .await;
    let dependency_duration_ms = dependency_started.elapsed().as_millis() as u64;
    match result {
        Ok(response) => {
            let response = response.into_inner();
            Json(serde_json::json!({"members": response.members.into_iter().map(|member| serde_json::json!({
                "user_id": member.user_id, "role": member.role, "status": member.status, "joined_at_unix": member.joined_at_unix
            })).collect::<Vec<_>>(), "next_cursor": response.next_cursor})).into_response()
        }
        Err(error) => grpc_error(
            error,
            &correlation_id,
            "list_platform_workspace_members",
            Some(dependency_duration_ms),
        ),
    }
}
