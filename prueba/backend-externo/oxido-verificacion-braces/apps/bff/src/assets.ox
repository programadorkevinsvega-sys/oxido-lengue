sintaxis llaves
use axum::{
    Json, Router,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header::CACHE_CONTROL},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use tonic::Request;
use xovyra_platform::grpc::{
    ActiveWorkspaceRequest, CompleteAssetUploadRequest, CreateAssetDownloadRequest,
    CreateAssetUploadRequest, DeleteAssetRequest, SearchAssetsRequest,
    asset_storage_client::AssetStorageClient, tenancy_access_client::TenancyAccessClient,
};

use crate::{
    authentication::{error_response, valid_origin},
    openapi::AppState,
    workspaces::{authenticate, inject_metadata},
};

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct UploadBody {
    display_name: String,
    content_type: String,
    size_bytes: i64,
}

#[derive(Deserialize)]
pub(crate) struct SearchQuery {
    query: Option<String>,
    page_size: Option<i32>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/assets", get(search))
        .route("/api/v1/assets/uploads", axum::routing::post(create_upload))
        .route(
            "/api/v1/assets/{asset_id}/complete",
            axum::routing::post(complete_upload),
        )
        .route("/api/v1/assets/{asset_id}/download", get(create_download))
        .route(
            "/api/v1/assets/{asset_id}",
            axum::routing::delete(delete_asset),
        )
}

#[utoipa::path(get, path = "/api/v1/assets", params(("query" = Option<String>, Query), ("page_size" = Option<i32>, Query)), responses((status = 200), (status = 401), (status = 403), (status = 503)))]
pub(crate) async fn search(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Query(query): Query<SearchQuery>,
) -> Response {
    let session = match authenticate(&state, &headers, &correlation_id, false).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let workspace_id = match active_workspace(
        &state,
        &session.user_id,
        &session.session_id,
        &correlation_id,
    )
    .await
    {
        Ok(workspace_id) => workspace_id,
        Err(response) => return *response,
    };
    let mut client = AssetStorageClient::new(state.runtime_channel);
    let mut request = Request::new(SearchAssetsRequest {
        workspace_id,
        actor_user_id: session.user_id,
        query: query.query.unwrap_or_default(),
        page_size: query.page_size.unwrap_or(50),
        correlation_id: correlation_id.clone(),
    });
    inject_metadata(&mut request, &correlation_id);
    match client.search_assets(request).await {
        Ok(response) => {
            let response = response.into_inner();
            Json(serde_json::json!({"assets": response.assets.into_iter().map(asset_json).collect::<Vec<_>>()})).into_response()
        }
        Err(error) => grpc_error(error.code(), &correlation_id),
    }
}

#[utoipa::path(post, path = "/api/v1/assets/uploads", request_body = UploadBody, responses((status = 200), (status = 400), (status = 401), (status = 403), (status = 409), (status = 503)))]
pub(crate) async fn create_upload(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Json(body): Json<UploadBody>,
) -> Response {
    let Some(idempotency_key) = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| (16..=128).contains(&value.len()))
    else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "idempotency_key_required",
            "La carga necesita una clave de solicitud válida. Inténtalo de nuevo.",
        );
    };
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "La solicitud no procede de XOVYRA.",
        );
    }
    let session = match authenticate(&state, &headers, &correlation_id, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let workspace_id = match active_workspace(
        &state,
        &session.user_id,
        &session.session_id,
        &correlation_id,
    )
    .await
    {
        Ok(workspace_id) => workspace_id,
        Err(response) => return *response,
    };
    let mut client = AssetStorageClient::new(state.runtime_channel);
    let mut request = Request::new(CreateAssetUploadRequest {
        workspace_id,
        actor_user_id: session.user_id,
        display_name: body.display_name,
        content_type: body.content_type,
        size_bytes: body.size_bytes,
        idempotency_key: idempotency_key.to_owned(),
        correlation_id: correlation_id.clone(),
    });
    inject_metadata(&mut request, &correlation_id);
    match client.create_upload(request).await {
        Ok(response) => {
            let response = response.into_inner();
            let mut response = Json(serde_json::json!({
                "asset": response.asset.map(asset_json),
                "upload_url": response.upload_url,
                "expires_at_unix": response.expires_at_unix,
                "upload_content_type": response.upload_content_type,
            }))
            .into_response();
            response
                .headers_mut()
                .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
            response
        }
        Err(error) => grpc_error(error.code(), &correlation_id),
    }
}

#[utoipa::path(post, path = "/api/v1/assets/{asset_id}/complete", params(("asset_id" = String, Path)), responses((status = 200), (status = 401), (status = 403), (status = 404), (status = 409), (status = 503)))]
pub(crate) async fn complete_upload(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Path(asset_id): Path<String>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "La solicitud no procede de XOVYRA.",
        );
    }
    let session = match authenticate(&state, &headers, &correlation_id, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let workspace_id = match active_workspace(
        &state,
        &session.user_id,
        &session.session_id,
        &correlation_id,
    )
    .await
    {
        Ok(workspace_id) => workspace_id,
        Err(response) => return *response,
    };
    let mut client = AssetStorageClient::new(state.runtime_channel);
    let mut request = Request::new(CompleteAssetUploadRequest {
        workspace_id,
        actor_user_id: session.user_id,
        asset_id,
        correlation_id: correlation_id.clone(),
    });
    inject_metadata(&mut request, &correlation_id);
    match client.complete_upload(request).await {
        Ok(response) => {
            Json(serde_json::json!({"asset": response.into_inner().asset.map(asset_json)}))
                .into_response()
        }
        Err(error) => grpc_error(error.code(), &correlation_id),
    }
}

#[utoipa::path(get, path = "/api/v1/assets/{asset_id}/download", params(("asset_id" = String, Path)), responses((status = 200), (status = 401), (status = 403), (status = 404), (status = 503)))]
pub(crate) async fn create_download(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Path(asset_id): Path<String>,
) -> Response {
    let session = match authenticate(&state, &headers, &correlation_id, false).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let workspace_id = match active_workspace(
        &state,
        &session.user_id,
        &session.session_id,
        &correlation_id,
    )
    .await
    {
        Ok(workspace_id) => workspace_id,
        Err(response) => return *response,
    };
    let mut client = AssetStorageClient::new(state.runtime_channel);
    let mut request = Request::new(CreateAssetDownloadRequest {
        workspace_id,
        actor_user_id: session.user_id,
        asset_id,
        correlation_id: correlation_id.clone(),
    });
    inject_metadata(&mut request, &correlation_id);
    match client.create_download(request).await {
        Ok(response) => {
            let response = response.into_inner();
            let mut response = Json(serde_json::json!({
                "asset": response.asset.map(asset_json),
                "download_url": response.download_url,
                "expires_at_unix": response.expires_at_unix,
            }))
            .into_response();
            response
                .headers_mut()
                .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
            response
        }
        Err(error) => grpc_error(error.code(), &correlation_id),
    }
}

#[utoipa::path(delete, path = "/api/v1/assets/{asset_id}", params(("asset_id" = String, Path)), responses((status = 200), (status = 401), (status = 403), (status = 404), (status = 409), (status = 503)))]
pub(crate) async fn delete_asset(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Path(asset_id): Path<String>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "La solicitud no procede de XOVYRA.",
        );
    }
    let session = match authenticate(&state, &headers, &correlation_id, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let workspace_id = match active_workspace(
        &state,
        &session.user_id,
        &session.session_id,
        &correlation_id,
    )
    .await
    {
        Ok(workspace_id) => workspace_id,
        Err(response) => return *response,
    };
    let mut client = AssetStorageClient::new(state.runtime_channel);
    let mut request = Request::new(DeleteAssetRequest {
        workspace_id,
        actor_user_id: session.user_id,
        asset_id,
        correlation_id: correlation_id.clone(),
    });
    inject_metadata(&mut request, &correlation_id);
    match client.delete_asset(request).await {
        Ok(response) => {
            Json(serde_json::json!({"asset": response.into_inner().asset.map(asset_json)}))
                .into_response()
        }
        Err(error) => grpc_error(error.code(), &correlation_id),
    }
}

async fn active_workspace(
    state: &AppState,
    user_id: &str,
    session_id: &str,
    correlation_id: &str,
) -> Result<String, Box<Response>> {
    let mut client = TenancyAccessClient::new(state.runtime_channel.clone());
    let mut request = Request::new(ActiveWorkspaceRequest {
        user_id: user_id.to_owned(),
        session_id: session_id.to_owned(),
        correlation_id: correlation_id.to_owned(),
    });
    inject_metadata(&mut request, correlation_id);
    match client.get_active_workspace(request).await {
        Ok(response) => response
            .into_inner()
            .workspace
            .map(|workspace| workspace.workspace_id)
            .ok_or_else(|| {
                Box::new(error_response(
                    StatusCode::CONFLICT,
                    "workspace_not_selected",
                    "Selecciona un espacio de trabajo antes de administrar archivos.",
                ))
            }),
        Err(error) => Err(Box::new(grpc_error(error.code(), correlation_id))),
    }
}

fn grpc_error(code: tonic::Code, correlation_id: &str) -> Response {
    match code {
        tonic::Code::InvalidArgument => error_response(
            StatusCode::BAD_REQUEST,
            "asset_request_invalid",
            "Revisa los datos del archivo e inténtalo de nuevo.",
        ),
        tonic::Code::Unauthenticated => error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Tu sesión terminó. Inicia sesión de nuevo.",
        ),
        tonic::Code::PermissionDenied => error_response(
            StatusCode::FORBIDDEN,
            "asset_access_denied",
            "Tu rol no tiene permiso para realizar esta acción.",
        ),
        tonic::Code::NotFound => error_response(
            StatusCode::NOT_FOUND,
            "asset_not_found",
            "El archivo no está disponible en este espacio.",
        ),
        tonic::Code::AlreadyExists => error_response(
            StatusCode::CONFLICT,
            "asset_request_conflict",
            "La solicitud coincide con otra operación anterior.",
        ),
        tonic::Code::FailedPrecondition => error_response(
            StatusCode::CONFLICT,
            "asset_state_conflict",
            "El estado actual del archivo no permite esta acción.",
        ),
        tonic::Code::ResourceExhausted => error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "asset_limit_reached",
            "Se alcanzó un límite de archivos. Inténtalo más tarde.",
        ),
        tonic::Code::Unavailable | tonic::Code::DeadlineExceeded => {
            tracing::error!(correlation_id, code=?code, "Asset storage request is unavailable");
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "asset_storage_unavailable",
                "El almacenamiento seguro está temporalmente indisponible.",
            )
        }
        _ => {
            tracing::error!(correlation_id, code=?code, "Asset storage request failed");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "asset_request_failed",
                "No se pudo completar la operación del archivo.",
            )
        }
    }
}

fn asset_json(asset: xovyra_platform::grpc::AssetMessage) -> serde_json::Value {
    serde_json::json!({
        "asset_id": asset.asset_id,
        "workspace_id": asset.workspace_id,
        "owner_user_id": asset.owner_user_id,
        "display_name": asset.display_name,
        "content_type": asset.content_type,
        "size_bytes": asset.size_bytes,
        "lifecycle_state": asset.lifecycle_state,
        "row_version": asset.row_version,
        "created_at_unix": asset.created_at_unix,
    })
}
