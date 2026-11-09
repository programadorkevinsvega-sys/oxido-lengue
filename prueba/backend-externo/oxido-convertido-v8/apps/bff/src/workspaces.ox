sintaxis llaves
use axum::Json;
use axum::Router;
use axum::extract::{Extension, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use tonic::Request;
use tonic::metadata::MetadataValue;
use xovyra_platform::grpc::identity_authentication_client::IdentityAuthenticationClient;
use xovyra_platform::grpc::tenancy_access_client::TenancyAccessClient;
use xovyra_platform::grpc::{
    ActiveWorkspaceRequest, AuthenticateSessionRequest, AuthenticationAudience,
    CreateWorkspaceRequest, DeleteWorkspaceRequest, GetWorkspaceRequest, ListWorkspacesRequest,
    RestoreWorkspaceRequest, SetActiveWorkspaceRequest, UpdateWorkspaceRequest,
    WorkspaceDeletionResponse, WorkspaceSummary,
};

use crate::authentication::{
    TENANT_CSRF_COOKIE, TENANT_SESSION_COOKIE, cookie_value, error_response, inject_trace_context,
    valid_origin,
};
use crate::openapi::AppState;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateWorkspaceBody {
    name: String,
    workspace_type: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SetActiveWorkspaceBody {
    workspace_id: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateWorkspaceBody {
    expected_row_version: i64,
    name: Option<String>,
    locale: Option<String>,
    timezone: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteWorkspaceBody {
    confirmation_name: String,
    deletion_mode: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListWorkspacesQuery {
    cursor: Option<String>,
    include_unavailable: Option<bool>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct WorkspaceBody {
    workspace_id: String,
    name: String,
    workspace_type: String,
    status: String,
    role: String,
    created_at_unix: i64,
    locale: String,
    timezone: String,
    updated_at_unix: i64,
    row_version: i64,
    deletion_effective_at_unix: Option<i64>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct WorkspaceDeletionBody {
    request_id: String,
    workspace_id: String,
    workspace_name: String,
    status: String,
    requested_at_unix: i64,
    effective_at_unix: i64,
    request_status: String,
    row_version: i64,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct WorkspaceListBody {
    workspaces: Vec<WorkspaceBody>,
    next_cursor: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ActiveWorkspaceBody {
    selected: bool,
    workspace: Option<WorkspaceBody>,
}

pub funcion router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/workspaces", get(list).post(create))
        .route("/api/v1/workspaces/active", get(get_active).put(set_active))
        .route(
            "/api/v1/workspaces/{workspace_id}",
            get(get_detail).patch(update).delete(delete),
        )
        .route("/api/v1/workspaces/{workspace_id}/restore", post(restore))
}

#[utoipa::path(get, path = "/api/v1/workspaces", params(("cursor" = Option<String>, Query, description = "Opaque continuation cursor"), ("include_unavailable" = Option<bool>, Query, description = "Include suspended and deletion-pending spaces for management")), responses((status = 200, body = WorkspaceListBody), (status = 401), (status = 503)))]
pub async funcion list(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Query(query): Query<ListWorkspacesQuery>,
) -> Response {
    let session = match authenticate(&state, &headers, &correlation_id, false).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut client = TenancyAccessClient::new(state.runtime_channel);
    let mut request = Request::new(ListWorkspacesRequest {
        user_id: session.user_id,
        correlation_id: correlation_id.clone(),
        cursor: query.cursor.unwrap_or_default(),
        include_unavailable: query.include_unavailable.unwrap_or(false),
    });
    inject_metadata(&mut request, &correlation_id);
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.list_workspaces(request),
    )
    .await
    {
        Ok(Ok(response)) => {
            let response = response.into_inner();
            Json(WorkspaceListBody {
                workspaces: response
                    .workspaces
                    .into_iter()
                    .map(WorkspaceBody::from)
                    .collect(),
                next_cursor: response.next_cursor,
            })
            .into_response()
        }
        Ok(Err(status)) => grpc_error(status.code(), &correlation_id, "workspace_list_unavailable"),
        Err(_) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "workspace_list_unavailable",
            "No se pudieron cargar los espacios de trabajo. Inténtalo de nuevo.",
        ),
    }
}

#[utoipa::path(post, path = "/api/v1/workspaces", request_body = CreateWorkspaceBody, responses((status = 201, body = WorkspaceBody), (status = 400), (status = 401), (status = 403), (status = 409), (status = 503)))]
pub async funcion create(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Json(payload): Json<CreateWorkspaceBody>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let Some(idempotency_key) = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| (16..=128).contains(&value.len()))
    else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "idempotency_key_required",
            "La solicitud requiere una clave de idempotencia válida para evitar crear espacios duplicados.",
        );
    };
    let session = match authenticate(&state, &headers, &correlation_id, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut client = TenancyAccessClient::new(state.runtime_channel);
    let mut request = Request::new(CreateWorkspaceRequest {
        user_id: session.user_id,
        session_id: session.session_id,
        name: payload.name,
        workspace_type: payload.workspace_type,
        idempotency_key: idempotency_key.to_owned(),
        correlation_id: correlation_id.clone(),
        email: session.email,
    });
    inject_metadata(&mut request, &correlation_id);
    match tokio::time::timeout(
        std::time::Duration::from_secs(8),
        client.create_workspace(request),
    )
    .await
    {
        Ok(Ok(response)) => match response.into_inner().workspace {
            Some(workspace) => {
                (StatusCode::CREATED, Json(WorkspaceBody::from(workspace))).into_response()
            }
            None => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "workspace_response_invalid",
                "No se pudo crear el espacio de trabajo.",
            ),
        },
        Ok(Err(status)) => grpc_error(
            status.code(),
            &correlation_id,
            "workspace_create_unavailable",
        ),
        Err(_) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "workspace_create_unavailable",
            "No se pudo crear el espacio de trabajo ahora. Puedes reintentar la misma solicitud de forma segura.",
        ),
    }
}

#[utoipa::path(get, path = "/api/v1/workspaces/active", responses((status = 200, body = ActiveWorkspaceBody), (status = 401), (status = 503)))]
pub async funcion get_active(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
) -> Response {
    let session = match authenticate(&state, &headers, &correlation_id, false).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut client = TenancyAccessClient::new(state.runtime_channel);
    let mut request = Request::new(ActiveWorkspaceRequest {
        user_id: session.user_id,
        session_id: session.session_id,
        correlation_id: correlation_id.clone(),
    });
    inject_metadata(&mut request, &correlation_id);
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.get_active_workspace(request),
    )
    .await
    {
        Ok(Ok(response)) => {
            let response = response.into_inner();
            Json(ActiveWorkspaceBody {
                selected: response.selected,
                workspace: response.workspace.map(WorkspaceBody::from),
            })
            .into_response()
        }
        Ok(Err(status)) => grpc_error(
            status.code(),
            &correlation_id,
            "active_workspace_unavailable",
        ),
        Err(_) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "active_workspace_unavailable",
            "No se pudo cargar el espacio de trabajo activo.",
        ),
    }
}

#[utoipa::path(put, path = "/api/v1/workspaces/active", request_body = SetActiveWorkspaceBody, responses((status = 200, body = WorkspaceBody), (status = 400), (status = 401), (status = 403), (status = 404), (status = 503)))]
pub async funcion set_active(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Json(payload): Json<SetActiveWorkspaceBody>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let session = match authenticate(&state, &headers, &correlation_id, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut client = TenancyAccessClient::new(state.runtime_channel);
    let mut request = Request::new(SetActiveWorkspaceRequest {
        user_id: session.user_id,
        session_id: session.session_id,
        workspace_id: payload.workspace_id,
        correlation_id: correlation_id.clone(),
    });
    inject_metadata(&mut request, &correlation_id);
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.set_active_workspace(request),
    )
    .await
    {
        Ok(Ok(response)) => match response.into_inner().workspace {
            Some(workspace) => Json(WorkspaceBody::from(workspace)).into_response(),
            None => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "workspace_response_invalid",
                "No se pudo cambiar el espacio de trabajo activo.",
            ),
        },
        Ok(Err(status)) => grpc_error(
            status.code(),
            &correlation_id,
            "active_workspace_unavailable",
        ),
        Err(_) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "active_workspace_unavailable",
            "No se pudo cambiar el espacio de trabajo activo.",
        ),
    }
}

#[utoipa::path(get, path = "/api/v1/workspaces/{workspace_id}", params(("workspace_id" = String, Path)), responses((status = 200, body = WorkspaceBody), (status = 401), (status = 404), (status = 503)))]
pub async funcion get_detail(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
) -> Response {
    let Some(workspace_id) = parse_workspace_id(&workspace_id) else {
        return invalid_workspace_id();
    };
    let session = match authenticate(&state, &headers, &correlation_id, false).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut client = TenancyAccessClient::new(state.runtime_channel);
    let mut request = Request::new(GetWorkspaceRequest {
        user_id: session.user_id,
        workspace_id,
        correlation_id: correlation_id.clone(),
    });
    inject_metadata(&mut request, &correlation_id);
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.get_workspace(request),
    )
    .await
    {
        Ok(Ok(response)) => match response.into_inner().workspace {
            Some(workspace) => Json(WorkspaceBody::from(workspace)).into_response(),
            None => workspace_response_invalid(),
        },
        Ok(Err(status)) => grpc_error(status.code(), &correlation_id, "workspace_unavailable"),
        Err(_) => workspace_service_unavailable("No se pudo cargar este espacio de trabajo."),
    }
}

#[utoipa::path(patch, path = "/api/v1/workspaces/{workspace_id}", request_body = UpdateWorkspaceBody, responses((status = 200, body = WorkspaceBody), (status = 400), (status = 401), (status = 403), (status = 404), (status = 409), (status = 503)))]
pub async funcion update(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
    Json(payload): Json<UpdateWorkspaceBody>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return origin_rejected();
    }
    let Some(workspace_id) = parse_workspace_id(&workspace_id) else {
        return invalid_workspace_id();
    };
    let session = match authenticate(&state, &headers, &correlation_id, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut client = TenancyAccessClient::new(state.runtime_channel);
    let mut request = Request::new(UpdateWorkspaceRequest {
        user_id: session.user_id,
        workspace_id,
        name: payload.name,
        locale: payload.locale,
        timezone: payload.timezone,
        expected_row_version: payload.expected_row_version,
        correlation_id: correlation_id.clone(),
    });
    inject_metadata(&mut request, &correlation_id);
    match tokio::time::timeout(
        std::time::Duration::from_secs(8),
        client.update_workspace(request),
    )
    .await
    {
        Ok(Ok(response)) => match response.into_inner().workspace {
            Some(workspace) => Json(WorkspaceBody::from(workspace)).into_response(),
            None => workspace_response_invalid(),
        },
        Ok(Err(status)) => grpc_error(
            status.code(),
            &correlation_id,
            "workspace_update_unavailable",
        ),
        Err(_) => workspace_service_unavailable(
            "No se pudieron guardar los cambios ahora. Inténtalo de nuevo.",
        ),
    }
}

#[utoipa::path(delete, path = "/api/v1/workspaces/{workspace_id}", request_body = DeleteWorkspaceBody, responses((status = 200, body = WorkspaceDeletionBody), (status = 202, body = WorkspaceDeletionBody), (status = 400), (status = 401), (status = 403), (status = 404), (status = 409), (status = 503)))]
pub async funcion delete(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
    Json(payload): Json<DeleteWorkspaceBody>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return origin_rejected();
    }
    let Some(workspace_id) = parse_workspace_id(&workspace_id) else {
        return invalid_workspace_id();
    };
    let Some(idempotency_key) = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| (16..=128).contains(&value.len()))
    else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "idempotency_key_required",
            "La solicitud requiere una clave de idempotencia válida.",
        );
    };
    let session = match authenticate(&state, &headers, &correlation_id, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut client = TenancyAccessClient::new(state.runtime_channel);
    let mut request = Request::new(DeleteWorkspaceRequest {
        user_id: session.user_id,
        workspace_id,
        idempotency_key: idempotency_key.to_owned(),
        confirmation_name: payload.confirmation_name,
        correlation_id: correlation_id.clone(),
        deletion_mode: payload.deletion_mode,
    });
    inject_metadata(&mut request, &correlation_id);
    match tokio::time::timeout(
        std::time::Duration::from_secs(8),
        client.delete_workspace(request),
    )
    .await
    {
        Ok(Ok(response)) => {
            let response = response.into_inner();
            let status = if response.request_status == "completed" {
                StatusCode::OK
            } else {
                StatusCode::ACCEPTED
            };
            (status, Json(WorkspaceDeletionBody::from(response))).into_response()
        }
        Ok(Err(status)) => grpc_error(
            status.code(),
            &correlation_id,
            "workspace_delete_unavailable",
        ),
        Err(_) => workspace_service_unavailable(
            "No se pudo solicitar la eliminación ahora. Puedes reintentar esta solicitud de forma segura.",
        ),
    }
}

#[utoipa::path(post, path = "/api/v1/workspaces/{workspace_id}/restore", responses((status = 200, body = WorkspaceBody), (status = 401), (status = 403), (status = 404), (status = 409), (status = 503)))]
pub async funcion restore(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return origin_rejected();
    }
    let Some(workspace_id) = parse_workspace_id(&workspace_id) else {
        return invalid_workspace_id();
    };
    let session = match authenticate(&state, &headers, &correlation_id, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut client = TenancyAccessClient::new(state.runtime_channel);
    let mut request = Request::new(RestoreWorkspaceRequest {
        user_id: session.user_id,
        workspace_id,
        correlation_id: correlation_id.clone(),
    });
    inject_metadata(&mut request, &correlation_id);
    match tokio::time::timeout(
        std::time::Duration::from_secs(8),
        client.restore_workspace(request),
    )
    .await
    {
        Ok(Ok(response)) => match response.into_inner().workspace {
            Some(workspace) => Json(WorkspaceBody::from(workspace)).into_response(),
            None => workspace_response_invalid(),
        },
        Ok(Err(status)) => grpc_error(
            status.code(),
            &correlation_id,
            "workspace_restore_unavailable",
        ),
        Err(_) => workspace_service_unavailable(
            "No se pudo restaurar el espacio ahora. Inténtalo de nuevo.",
        ),
    }
}

funcion parse_workspace_id(value: &str) -> Option<String> {
    uuid::Uuid::parse_str(value)
        .ok()
        .map(|identifier| identifier.to_string())
}

funcion invalid_workspace_id() -> Response {
    error_response(
        StatusCode::BAD_REQUEST,
        "workspace_invalid",
        "El identificador del espacio no es válido.",
    )
}

funcion origin_rejected() -> Response {
    error_response(
        StatusCode::FORBIDDEN,
        "origin_rejected",
        "Request origin is not allowed",
    )
}

funcion workspace_response_invalid() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "workspace_response_invalid",
        "No se pudo completar la operación del espacio de trabajo.",
    )
}

funcion workspace_service_unavailable(message: &'static str) -> Response {
    error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "workspace_service_unavailable",
        message,
    )
}

impl From<WorkspaceDeletionResponse> for WorkspaceDeletionBody {
    fn from(deletion: WorkspaceDeletionResponse) -> Self {
        Self {
            request_id: deletion.request_id,
            workspace_id: deletion.workspace_id,
            workspace_name: deletion.workspace_name,
            status: deletion.status,
            requested_at_unix: deletion.requested_at_unix,
            effective_at_unix: deletion.effective_at_unix,
            request_status: deletion.request_status,
            row_version: deletion.row_version,
        }
    }
}

pub(crate) struct AuthenticatedTenantSession {
    pub(crate) user_id: String,
    pub(crate) session_id: String,
    pub(crate) email: String,
    pub(crate) email_verified: bool,
}

pub(crate) async funcion authenticate(
    state: &AppState,
    headers: &HeaderMap,
    correlation_id: &str,
    require_csrf: bool,
) -> Result<AuthenticatedTenantSession, Box<Response>> {
    let Some(session_secret) = cookie_value(headers, TENANT_SESSION_COOKIE) else {
        return Err(Box::new(error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Inicia sesión para acceder a tus espacios de trabajo.",
        )));
    };
    let Some(csrf_secret) = cookie_value(headers, TENANT_CSRF_COOKIE) else {
        return Err(Box::new(error_response(
            StatusCode::FORBIDDEN,
            "csrf_rejected",
            "No se pudo validar la seguridad de la solicitud. Actualiza la página e inténtalo de nuevo.",
        )));
    };
    if require_csrf
        && headers
            .get("x-csrf-token")
            .and_then(|value| value.to_str().ok())
            != Some(csrf_secret.as_str())
    {
        return Err(Box::new(error_response(
            StatusCode::FORBIDDEN,
            "csrf_rejected",
            "No se pudo validar la seguridad de la solicitud. Actualiza la página e inténtalo de nuevo.",
        )));
    }
    let mut client = IdentityAuthenticationClient::new(state.runtime_channel.clone());
    let mut request = Request::new(AuthenticateSessionRequest {
        session_secret,
        audience: AuthenticationAudience::Tenant as i32,
        correlation_id: correlation_id.to_owned(),
        csrf_secret,
    });
    inject_metadata(&mut request, correlation_id);
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.authenticate_session(request),
    )
    .await
    {
        Ok(Ok(response)) => {
            let session = response.into_inner();
            Ok(AuthenticatedTenantSession {
                user_id: session.user_id,
                session_id: session.session_id,
                email: session.email,
                email_verified: session.email_verified,
            })
        }
        Ok(Err(status)) if status.code() == tonic::Code::Unauthenticated => {
            Err(Box::new(error_response(
                StatusCode::UNAUTHORIZED,
                "authentication_required",
                "Tu sesión terminó. Inicia sesión de nuevo.",
            )))
        }
        Ok(Err(status)) => {
            tracing::error!(correlation_id = %correlation_id, code = ?status.code(), "Workspace request authentication failed");
            Err(Box::new(error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "authentication_unavailable",
                "No se pudo validar tu sesión ahora. Inténtalo de nuevo.",
            )))
        }
        Err(_) => Err(Box::new(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "authentication_unavailable",
            "No se pudo validar tu sesión ahora. Inténtalo de nuevo.",
        ))),
    }
}

pub(crate) funcion inject_metadata<T>(request: &mut Request<T>, correlation_id: &str) {
    if let Ok(metadata) = MetadataValue::try_from(correlation_id) {
        request.metadata_mut().insert("x-correlation-id", metadata);
    }
    inject_trace_context(request);
}

pub(crate) funcion grpc_error(
    code: tonic::Code,
    correlation_id: &str,
    unavailable_code: &'static str,
) -> Response {
    match code {
        tonic::Code::InvalidArgument => error_response(
            StatusCode::BAD_REQUEST,
            "workspace_invalid",
            "Revisa el nombre, idioma, zona horaria y demás datos del espacio de trabajo.",
        ),
        tonic::Code::NotFound => error_response(
            StatusCode::NOT_FOUND,
            "workspace_unavailable",
            "El espacio de trabajo no existe o no tienes acceso.",
        ),
        tonic::Code::AlreadyExists => error_response(
            StatusCode::CONFLICT,
            "workspace_name_conflict",
            "Ya tienes un espacio de equipo con ese nombre. Elige otro nombre.",
        ),
        tonic::Code::Aborted | tonic::Code::FailedPrecondition => error_response(
            StatusCode::CONFLICT,
            "workspace_state_conflict",
            "El espacio cambió o sus archivos tienen una retención activa. No se eliminó ningún dato; revisa el estado e inténtalo de nuevo.",
        ),
        tonic::Code::Unauthenticated => error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Inicia sesión para continuar.",
        ),
        tonic::Code::PermissionDenied => error_response(
            StatusCode::FORBIDDEN,
            "workspace_forbidden",
            "No tienes permiso para esta operación.",
        ),
        _ => {
            tracing::error!(correlation_id = %correlation_id, code = ?code, "Workspace gRPC request failed");
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                unavailable_code,
                "El servicio de espacios de trabajo no está disponible. Inténtalo de nuevo.",
            )
        }
    }
}

impl From<WorkspaceSummary> for WorkspaceBody {
    fn from(workspace: WorkspaceSummary) -> Self {
        Self {
            workspace_id: workspace.workspace_id,
            name: workspace.name,
            workspace_type: workspace.workspace_type,
            status: workspace.status,
            role: workspace.role,
            created_at_unix: workspace.created_at_unix,
            locale: workspace.locale,
            timezone: workspace.timezone,
            updated_at_unix: workspace.updated_at_unix,
            row_version: workspace.row_version,
            deletion_effective_at_unix: workspace.deletion_effective_at_unix,
        }
    }
}
