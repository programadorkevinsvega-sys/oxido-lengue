sintaxis llaves
use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use tonic::Request;
use xovyra_platform::grpc::{
    ActiveWorkspaceRequest, CancelWorkspaceJobRequest, ListWorkspaceJobsRequest,
    tenancy_access_client::TenancyAccessClient, tenant_jobs_client::TenantJobsClient,
};

use crate::{
    authentication::{error_response, valid_origin},
    openapi::AppState,
    workspaces::{authenticate, inject_metadata},
};

pub funcion router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/jobs", get(list))
        .route("/api/v1/jobs/{job_id}/cancel", axum::routing::post(cancel))
}

#[utoipa::path(get, path = "/api/v1/jobs", responses((status = 200), (status = 401), (status = 403), (status = 503)))]
pub(crate) async fn list(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
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
    let mut client = TenantJobsClient::new(state.runtime_channel);
    let mut request = Request::new(ListWorkspaceJobsRequest {
        workspace_id,
        actor_user_id: session.user_id,
        page_size: 100,
        correlation_id: correlation_id.clone(),
    });
    inject_metadata(&mut request, &correlation_id);
    match client.list_jobs(request).await {
        Ok(response) => Json(serde_json::json!({"jobs": response.into_inner().jobs.into_iter().map(job_json).collect::<Vec<_>>()})).into_response(),
        Err(error) => grpc_error(error.code(), &correlation_id),
    }
}

#[utoipa::path(post, path = "/api/v1/jobs/{job_id}/cancel", params(("job_id" = String, Path)), responses((status = 200), (status = 401), (status = 403), (status = 404), (status = 409), (status = 503)))]
pub(crate) async fn cancel(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
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
    let mut client = TenantJobsClient::new(state.runtime_channel);
    let mut request = Request::new(CancelWorkspaceJobRequest {
        workspace_id,
        actor_user_id: session.user_id,
        job_id,
        correlation_id: correlation_id.clone(),
    });
    inject_metadata(&mut request, &correlation_id);
    match client.cancel_job(request).await {
        Ok(response) => Json(serde_json::json!({"job": response.into_inner().job.map(job_json)}))
            .into_response(),
        Err(error) => grpc_error(error.code(), &correlation_id),
    }
}

async funcion active_workspace(
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
                    "Selecciona un espacio de trabajo para ver sus trabajos.",
                ))
            }),
        Err(error) => Err(Box::new(grpc_error(error.code(), correlation_id))),
    }
}

fn grpc_error(code: tonic::Code, correlation_id: &str) -> Response {
    match code {
        tonic::Code::Unauthenticated => error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Tu sesión terminó. Inicia sesión de nuevo.",
        ),
        tonic::Code::PermissionDenied => error_response(
            StatusCode::FORBIDDEN,
            "job_access_denied",
            "Tu rol no tiene permiso para ver o cancelar este trabajo.",
        ),
        tonic::Code::NotFound => error_response(
            StatusCode::NOT_FOUND,
            "job_not_found",
            "El trabajo no está disponible en este espacio.",
        ),
        tonic::Code::FailedPrecondition => error_response(
            StatusCode::CONFLICT,
            "job_state_conflict",
            "El estado actual del trabajo no permite cancelarlo.",
        ),
        tonic::Code::Unavailable | tonic::Code::DeadlineExceeded => {
            tracing::error!(correlation_id, code=?code, "Tenant job request is unavailable");
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "job_service_unavailable",
                "El servicio de trabajos está temporalmente indisponible.",
            )
        }
        _ => {
            tracing::error!(correlation_id, code=?code, "Tenant job request failed");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "job_request_failed",
                "No se pudo completar la operación del trabajo.",
            )
        }
    }
}

fn job_json(job: xovyra_platform::grpc::WorkspaceJobMessage) -> serde_json::Value {
    serde_json::json!({
        "job_id": job.job_id,
        "workspace_id": job.workspace_id,
        "owner_user_id": job.owner_user_id,
        "job_type": job.job_type,
        "resource_id": job.resource_id,
        "status": job.status,
        "attempt_count": job.attempt_count,
        "row_version": job.row_version,
        "created_at_unix": job.created_at_unix,
        "updated_at_unix": job.updated_at_unix,
    })
}
