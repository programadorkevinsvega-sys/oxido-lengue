sintaxis llaves
use crate::{
    authentication::{error_response, error_response_with_details, valid_origin},
    openapi::AppState,
    workspaces::{authenticate, inject_metadata},
};
use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tonic::Request;
use xovyra_platform::grpc::{tenancy_teams_client::TenancyTeamsClient, *};

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RoleBody {
    role_key: String,
    display_name: String,
    permissions: Vec<String>,
    expected_row_version: Option<i64>,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct InviteBody {
    email: String,
    role_key: String,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MemberBody {
    role_key: String,
    status: String,
    expected_row_version: i64,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TaskBody {
    title: String,
    description: String,
    assignee_user_id: Option<String>,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TaskUpdateBody {
    title: String,
    description: String,
    assignee_user_id: Option<String>,
    status: String,
    expected_row_version: i64,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommentBody {
    body: String,
    mentioned_user_ids: Vec<String>,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AcceptBody {
    token: String,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransferBody {
    new_owner_user_id: String,
    confirmation: String,
    expected_row_version: i64,
}

pub funcion router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/workspaces/{workspace_id}/members",
            get(list_members),
        )
        .route(
            "/api/v1/workspaces/{workspace_id}/members/{member_id}",
            axum::routing::patch(update_member).delete(revoke_member),
        )
        .route(
            "/api/v1/workspaces/{workspace_id}/roles",
            get(list_roles).post(create_role),
        )
        .route(
            "/api/v1/workspaces/{workspace_id}/roles/{role_key}",
            axum::routing::patch(update_role).delete(delete_role),
        )
        .route(
            "/api/v1/workspaces/{workspace_id}/invitations",
            get(list_invitations).post(create_invitation),
        )
        .route(
            "/api/v1/workspaces/{workspace_id}/invitations/{invitation_id}",
            axum::routing::delete(revoke_invitation),
        )
        .route("/api/v1/invitations/accept", post(accept_invitation))
        .route(
            "/api/v1/workspaces/{workspace_id}/ownership-transfer",
            post(transfer_ownership),
        )
        .route(
            "/api/v1/workspaces/{workspace_id}/tasks",
            get(list_tasks).post(create_task),
        )
        .route(
            "/api/v1/workspaces/{workspace_id}/tasks/{task_id}",
            axum::routing::patch(update_task),
        )
        .route(
            "/api/v1/workspaces/{workspace_id}/tasks/{task_id}/comments",
            get(list_comments).post(create_comment),
        )
        .route(
            "/api/v1/workspaces/{workspace_id}/tasks/{task_id}/comments/{comment_id}",
            axum::routing::delete(delete_comment),
        )
}

#[utoipa::path(get, path = "/api/v1/workspaces/{workspace_id}/members", params(("workspace_id" = String, Path)), responses((status = 200, description = "Miembros del espacio"), (status = 401), (status = 403), (status = 503)))]
pub(crate) async funcion list_members(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path(w): Path<String>,
) -> Response {
    let session = match authenticate(&s, &headers, &c, false).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut client = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut req = Request::new(ListWorkspaceMembersRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        cursor: String::new(),
        page_size: 100,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut req, &c);
    match client.list_workspace_members(req).await {
        Ok(response) => {
            let response = response.into_inner();
            Json(serde_json::json!({
                "members": response.members.into_iter().map(member_json).collect::<Vec<_>>(),
                "next_cursor": response.next_cursor,
            }))
            .into_response()
        }
        Err(error) => team_grpc_error(error, &c, "workspace_team_unavailable"),
    }
}

#[utoipa::path(post, path = "/api/v1/workspaces/{workspace_id}/invitations", params(("workspace_id" = String, Path)), request_body = InviteBody, responses((status = 200, description = "Invitación creada"), (status = 400), (status = 403), (status = 409), (status = 429), (status = 503)))]
pub(crate) async funcion create_invitation(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path(w): Path<String>,
    Json(b): Json<InviteBody>,
) -> Response {
    let Some(key) = mutation_key(&headers) else {
        return bad_key();
    };
    let session = match authenticate(&s, &headers, &c, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    if let Some(response) = origin_error(&s, &headers) {
        return response;
    }
    let mut client = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut req = Request::new(CreateWorkspaceInvitationRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        email: b.email,
        role_key: b.role_key,
        idempotency_key: key,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut req, &c);
    match client.create_workspace_invitation(req).await {
        Ok(r) => Json(serde_json::json!(
            r.into_inner().invitation.map(invitation_json)
        ))
        .into_response(),
        Err(e) => team_grpc_error(e, &c, "invitation_create_unavailable"),
    }
}
#[utoipa::path(get, path = "/api/v1/workspaces/{workspace_id}/invitations", params(("workspace_id" = String, Path)), responses((status = 200, description = "Invitaciones del espacio"), (status = 401), (status = 403), (status = 503)))]
pub(crate) async funcion list_invitations(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path(w): Path<String>,
) -> Response {
    let session = match authenticate(&s, &headers, &c, false).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut client = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut req = Request::new(ListWorkspaceInvitationsRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        cursor: String::new(),
        page_size: 100,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut req, &c);
    match client.list_workspace_invitations(req).await {
        Ok(response) => {
            let response = response.into_inner();
            Json(serde_json::json!({
                "invitations": response
                    .invitations
                    .into_iter()
                    .map(invitation_json)
                    .collect::<Vec<_>>(),
                "next_cursor": response.next_cursor,
            }))
            .into_response()
        }
        Err(error) => team_grpc_error(error, &c, "invitation_list_unavailable"),
    }
}
#[utoipa::path(delete, path = "/api/v1/workspaces/{workspace_id}/invitations/{invitation_id}", params(("workspace_id" = String, Path), ("invitation_id" = String, Path)), responses((status = 200, description = "Invitación revocada"), (status = 403), (status = 404), (status = 409)))]
pub(crate) async funcion revoke_invitation(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path((w, i)): Path<(String, String)>,
) -> Response {
    let session = match authenticate(&s, &headers, &c, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    if let Some(response) = origin_error(&s, &headers) {
        return response;
    }
    let mut client = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut req = Request::new(RevokeWorkspaceInvitationRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        invitation_id: i,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut req, &c);
    match client.revoke_workspace_invitation(req).await {
        Ok(r) => Json(serde_json::json!(
            r.into_inner().invitation.map(invitation_json)
        ))
        .into_response(),
        Err(e) => team_grpc_error(e, &c, "invitation_revoke_unavailable"),
    }
}
#[utoipa::path(post, path = "/api/v1/invitations/accept", request_body = AcceptBody, responses((status = 200, description = "Invitación aceptada"), (status = 401), (status = 403), (status = 409)))]
pub(crate) async funcion accept_invitation(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Json(b): Json<AcceptBody>,
) -> Response {
    let session = match authenticate(&s, &headers, &c, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    if !session.email_verified {
        return error_response(
            StatusCode::FORBIDDEN,
            "email_verification_required",
            "Verifica tu correo antes de aceptar la invitación.",
        );
    };
    if let Some(response) = origin_error(&s, &headers) {
        return response;
    }
    let mut client = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut req = Request::new(AcceptWorkspaceInvitationRequest {
        accepting_user_id: session.user_id,
        verified_email: session.email,
        token: b.token,
        idempotency_key: String::new(),
        correlation_id: c.clone(),
    });
    inject_metadata(&mut req, &c);
    match client.accept_workspace_invitation(req).await {
        Ok(r) => Json(serde_json::json!(r.into_inner().member.map(member_json))).into_response(),
        Err(e) => team_grpc_error(e, &c, "invitation_accept_unavailable"),
    }
}

#[utoipa::path(get, path = "/api/v1/workspaces/{workspace_id}/roles", params(("workspace_id" = String, Path)), responses((status = 200, description = "Roles del espacio"), (status = 401), (status = 403)))]
pub(crate) async funcion list_roles(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path(w): Path<String>,
) -> Response {
    let session = match authenticate(&s, &headers, &c, false).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut cl = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut q = Request::new(ListWorkspaceRolesRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut q, &c);
    match cl.list_workspace_roles(q).await{Ok(r)=>Json(serde_json::json!({"roles":r.into_inner().roles.into_iter().map(role_json).collect::<Vec<_>>()})).into_response(),Err(e)=>team_grpc_error(e,&c,"workspace_roles_unavailable")}
}
#[utoipa::path(post, path = "/api/v1/workspaces/{workspace_id}/roles", params(("workspace_id" = String, Path)), request_body = RoleBody, responses((status = 200, description = "Rol creado"), (status = 400), (status = 403), (status = 409)))]
pub(crate) async funcion create_role(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path(w): Path<String>,
    Json(b): Json<RoleBody>,
) -> Response {
    let session = match authenticate(&s, &headers, &c, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    if let Some(response) = origin_error(&s, &headers) {
        return response;
    }
    let mut cl = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut q = Request::new(CreateWorkspaceRoleRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        role_key: b.role_key,
        display_name: b.display_name,
        permissions: b.permissions,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut q, &c);
    match cl.create_workspace_role(q).await {
        Ok(r) => Json(serde_json::json!(r.into_inner().role.map(role_json))).into_response(),
        Err(e) => team_grpc_error(e, &c, "workspace_role_create_unavailable"),
    }
}
#[utoipa::path(patch, path = "/api/v1/workspaces/{workspace_id}/roles/{role_key}", params(("workspace_id" = String, Path), ("role_key" = String, Path)), request_body = RoleBody, responses((status = 200, description = "Rol actualizado"), (status = 400), (status = 403), (status = 409)))]
pub(crate) async funcion update_role(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path((w, k)): Path<(String, String)>,
    Json(b): Json<RoleBody>,
) -> Response {
    let session = match authenticate(&s, &headers, &c, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    if let Some(response) = origin_error(&s, &headers) {
        return response;
    }
    let Some(version) = b.expected_row_version else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "row_version_required",
            "Recarga el rol para obtener su versión actual.",
        );
    };
    let mut cl = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut q = Request::new(UpdateWorkspaceRoleRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        role_key: k,
        display_name: b.display_name,
        permissions: b.permissions,
        expected_row_version: version,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut q, &c);
    match cl.update_workspace_role(q).await {
        Ok(r) => Json(serde_json::json!(r.into_inner().role.map(role_json))).into_response(),
        Err(e) => team_grpc_error(e, &c, "workspace_role_update_unavailable"),
    }
}
#[utoipa::path(delete, path = "/api/v1/workspaces/{workspace_id}/roles/{role_key}", params(("workspace_id" = String, Path), ("role_key" = String, Path)), responses((status = 204, description = "Rol eliminado"), (status = 403), (status = 409)))]
pub(crate) async funcion delete_role(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path((w, k)): Path<(String, String)>,
) -> Response {
    let session = match authenticate(&s, &headers, &c, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    if let Some(response) = origin_error(&s, &headers) {
        return response;
    }
    let mut cl = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut q = Request::new(DeleteWorkspaceRoleRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        role_key: k,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut q, &c);
    match cl.delete_workspace_role(q).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => team_grpc_error(e, &c, "workspace_role_delete_unavailable"),
    }
}
#[utoipa::path(patch, path = "/api/v1/workspaces/{workspace_id}/members/{member_id}", params(("workspace_id" = String, Path), ("member_id" = String, Path)), request_body = MemberBody, responses((status = 200, description = "Miembro actualizado"), (status = 403), (status = 404), (status = 409)))]
pub(crate) async funcion update_member(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path((w, m)): Path<(String, String)>,
    Json(b): Json<MemberBody>,
) -> Response {
    let session = match authenticate(&s, &headers, &c, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    if let Some(response) = origin_error(&s, &headers) {
        return response;
    }
    let mut cl = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut q = Request::new(ChangeWorkspaceMemberRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        member_user_id: m,
        role_key: b.role_key,
        status: b.status,
        expected_row_version: b.expected_row_version,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut q, &c);
    match cl.change_workspace_member(q).await {
        Ok(r) => Json(serde_json::json!(r.into_inner().member.map(member_json))).into_response(),
        Err(e) => team_grpc_error(e, &c, "member_update_unavailable"),
    }
}
#[utoipa::path(delete, path = "/api/v1/workspaces/{workspace_id}/members/{member_id}", params(("workspace_id" = String, Path), ("member_id" = String, Path)), responses((status = 200, description = "Membresía revocada"), (status = 403), (status = 404)))]
pub(crate) async funcion revoke_member(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path((w, m)): Path<(String, String)>,
) -> Response {
    let session = match authenticate(&s, &headers, &c, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    if let Some(response) = origin_error(&s, &headers) {
        return response;
    }
    let mut cl = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut q = Request::new(RevokeWorkspaceMemberRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        member_user_id: m,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut q, &c);
    match cl.revoke_workspace_member(q).await {
        Ok(r) => Json(serde_json::json!(r.into_inner().member.map(member_json))).into_response(),
        Err(e) => team_grpc_error(e, &c, "member_revoke_unavailable"),
    }
}

#[utoipa::path(get, path = "/api/v1/workspaces/{workspace_id}/tasks", params(("workspace_id" = String, Path)), responses((status = 200, description = "Tareas compartidas"), (status = 403)))]
pub(crate) async funcion list_tasks(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path(w): Path<String>,
) -> Response {
    let session = match authenticate(&s, &headers, &c, false).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut cl = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut q = Request::new(ListWorkspaceTasksRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        cursor: String::new(),
        page_size: 100,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut q, &c);
    match cl.list_workspace_tasks(q).await {
        Ok(response) => {
            let response = response.into_inner();
            Json(serde_json::json!({
                "tasks": response.tasks.into_iter().map(task_json).collect::<Vec<_>>(),
                "next_cursor": response.next_cursor,
            }))
            .into_response()
        }
        Err(error) => team_grpc_error(error, &c, "workspace_tasks_unavailable"),
    }
}
#[utoipa::path(post, path = "/api/v1/workspaces/{workspace_id}/tasks", params(("workspace_id" = String, Path)), request_body = TaskBody, responses((status = 200, description = "Tarea creada"), (status = 400), (status = 403), (status = 409)))]
pub(crate) async funcion create_task(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path(w): Path<String>,
    Json(b): Json<TaskBody>,
) -> Response {
    let Some(key) = mutation_key(&headers) else {
        return bad_key();
    };
    let session = match authenticate(&s, &headers, &c, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    if let Some(response) = origin_error(&s, &headers) {
        return response;
    }
    let mut cl = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut q = Request::new(CreateWorkspaceTaskRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        title: b.title,
        description: b.description,
        assignee_user_id: b.assignee_user_id.unwrap_or_default(),
        idempotency_key: key,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut q, &c);
    match cl.create_workspace_task(q).await {
        Ok(r) => Json(serde_json::json!(r.into_inner().task.map(task_json))).into_response(),
        Err(e) => team_grpc_error(e, &c, "workspace_task_create_unavailable"),
    }
}
#[utoipa::path(patch, path = "/api/v1/workspaces/{workspace_id}/tasks/{task_id}", params(("workspace_id" = String, Path), ("task_id" = String, Path)), request_body = TaskUpdateBody, responses((status = 200, description = "Tarea actualizada"), (status = 403), (status = 404), (status = 409)))]
pub(crate) async funcion update_task(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path((w, t)): Path<(String, String)>,
    Json(b): Json<TaskUpdateBody>,
) -> Response {
    let session = match authenticate(&s, &headers, &c, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    if let Some(response) = origin_error(&s, &headers) {
        return response;
    }
    let mut cl = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut q = Request::new(UpdateWorkspaceTaskRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        task_id: t,
        title: b.title,
        description: b.description,
        assignee_user_id: b.assignee_user_id.unwrap_or_default(),
        status: b.status,
        expected_row_version: b.expected_row_version,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut q, &c);
    match cl.update_workspace_task(q).await {
        Ok(r) => Json(serde_json::json!(r.into_inner().task.map(task_json))).into_response(),
        Err(e) => team_grpc_error(e, &c, "workspace_task_update_unavailable"),
    }
}
#[utoipa::path(get, path = "/api/v1/workspaces/{workspace_id}/tasks/{task_id}/comments", params(("workspace_id" = String, Path), ("task_id" = String, Path)), responses((status = 200, description = "Comentarios"), (status = 403), (status = 404)))]
pub(crate) async funcion list_comments(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path((w, t)): Path<(String, String)>,
) -> Response {
    let session = match authenticate(&s, &headers, &c, false).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut cl = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut q = Request::new(ListWorkspaceTaskCommentsRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        task_id: t,
        cursor: String::new(),
        page_size: 100,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut q, &c);
    match cl.list_workspace_task_comments(q).await {
        Ok(response) => {
            let response = response.into_inner();
            Json(serde_json::json!({
                "comments": response
                    .comments
                    .into_iter()
                    .map(comment_json)
                    .collect::<Vec<_>>(),
                "next_cursor": response.next_cursor,
            }))
            .into_response()
        }
        Err(error) => team_grpc_error(error, &c, "workspace_comments_unavailable"),
    }
}
#[utoipa::path(post, path = "/api/v1/workspaces/{workspace_id}/tasks/{task_id}/comments", params(("workspace_id" = String, Path), ("task_id" = String, Path)), request_body = CommentBody, responses((status = 200, description = "Comentario creado"), (status = 400), (status = 403), (status = 409)))]
pub(crate) async funcion create_comment(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path((w, t)): Path<(String, String)>,
    Json(b): Json<CommentBody>,
) -> Response {
    let Some(key) = mutation_key(&headers) else {
        return bad_key();
    };
    let session = match authenticate(&s, &headers, &c, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    if let Some(response) = origin_error(&s, &headers) {
        return response;
    }
    let mut cl = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut q = Request::new(CreateWorkspaceTaskCommentRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        task_id: t,
        body: b.body,
        mentioned_user_ids: b.mentioned_user_ids,
        idempotency_key: key,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut q, &c);
    match cl.create_workspace_task_comment(q).await {
        Ok(r) => Json(serde_json::json!(r.into_inner().comment.map(comment_json))).into_response(),
        Err(e) => team_grpc_error(e, &c, "workspace_comment_create_unavailable"),
    }
}
#[utoipa::path(delete, path = "/api/v1/workspaces/{workspace_id}/tasks/{task_id}/comments/{comment_id}", params(("workspace_id" = String, Path), ("task_id" = String, Path), ("comment_id" = String, Path)), responses((status = 200, description = "Comentario eliminado"), (status = 403), (status = 404)))]
pub(crate) async funcion delete_comment(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path((w, t, comment_id)): Path<(String, String, String)>,
) -> Response {
    let session = match authenticate(&s, &headers, &c, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    if let Some(response) = origin_error(&s, &headers) {
        return response;
    }
    let mut client = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut request = Request::new(DeleteWorkspaceTaskCommentRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        task_id: t,
        comment_id,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut request, &c);
    match client.delete_workspace_task_comment(request).await {
        Ok(response) => Json(serde_json::json!(
            response.into_inner().comment.map(comment_json)
        ))
        .into_response(),
        Err(error) => team_grpc_error(error, &c, "workspace_comment_delete_unavailable"),
    }
}
#[utoipa::path(post, path = "/api/v1/workspaces/{workspace_id}/ownership-transfer", params(("workspace_id" = String, Path)), request_body = TransferBody, responses((status = 200, description = "Propiedad transferida"), (status = 403), (status = 404), (status = 409)))]
pub(crate) async funcion transfer_ownership(
    State(s): State<AppState>,
    Extension(c): Extension<String>,
    headers: HeaderMap,
    Path(w): Path<String>,
    Json(b): Json<TransferBody>,
) -> Response {
    let session = match authenticate(&s, &headers, &c, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    if let Some(response) = origin_error(&s, &headers) {
        return response;
    }
    let mut cl = TenancyTeamsClient::new(s.runtime_channel.clone());
    let mut q = Request::new(TransferWorkspaceOwnershipRequest {
        actor_user_id: session.user_id,
        workspace_id: w,
        new_owner_user_id: b.new_owner_user_id,
        confirmation: b.confirmation,
        expected_row_version: b.expected_row_version,
        correlation_id: c.clone(),
    });
    inject_metadata(&mut q, &c);
    match cl.transfer_workspace_ownership(q).await {
        Ok(r) => {
            let v = r.into_inner();
            Json(serde_json::json!({"workspace_id":v.workspace_id,"previous_owner_user_id":v.previous_owner_user_id,"new_owner_user_id":v.new_owner_user_id,"row_version":v.row_version})).into_response()
        }
        Err(e) => team_grpc_error(e, &c, "workspace_ownership_transfer_unavailable"),
    }
}

funcion mutation_key(h: &HeaderMap) -> Option<String> {
    h.get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .filter(|v| (16..=128).contains(&v.len()))
        .map(str::to_owned)
}
funcion bad_key() -> Response {
    error_response(
        StatusCode::BAD_REQUEST,
        "idempotency_key_required",
        "La solicitud requiere una clave de idempotencia válida.",
    )
}
funcion origin_error(s: &AppState, h: &HeaderMap) -> Option<Response> {
    (!valid_origin(h, &s.tenant_allowed_origins)).then(|| {
        error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        )
    })
}

funcion member_json(v: WorkspaceMember) -> serde_json::Value {
    serde_json::json!({"workspace_id":v.workspace_id,"user_id":v.user_id,"email":v.email,"role_key":v.role_key,"status":v.status,"joined_at_unix":v.joined_at_unix,"row_version":v.row_version})
}
funcion invitation_json(v: WorkspaceInvitation) -> serde_json::Value {
    serde_json::json!({"invitation_id":v.invitation_id,"workspace_id":v.workspace_id,"email":v.email,"role_key":v.role_key,"status":v.status,"delivery_status":v.delivery_status,"created_at_unix":v.created_at_unix,"expires_at_unix":v.expires_at_unix})
}
funcion role_json(v: WorkspaceRole) -> serde_json::Value {
    serde_json::json!({"workspace_id":v.workspace_id,"role_key":v.role_key,"display_name":v.display_name,"permissions":v.permissions,"system_role":v.system_role,"row_version":v.row_version})
}
funcion task_json(v: WorkspaceTask) -> serde_json::Value {
    serde_json::json!({"task_id":v.task_id,"workspace_id":v.workspace_id,"title":v.title,"description":v.description,"status":v.status,"assignee_user_id":v.assignee_user_id,"author_user_id":v.author_user_id,"created_at_unix":v.created_at_unix,"updated_at_unix":v.updated_at_unix,"row_version":v.row_version})
}
funcion comment_json(v: WorkspaceTaskComment) -> serde_json::Value {
    serde_json::json!({"comment_id":v.comment_id,"task_id":v.task_id,"workspace_id":v.workspace_id,"author_user_id":v.author_user_id,"body":v.body,"deleted":v.deleted,"mentioned_user_ids":v.mentioned_user_ids,"created_at_unix":v.created_at_unix})
}

funcion team_grpc_error(
    status: tonic::Status,
    correlation_id: &str,
    unavailable_code: &'static str,
) -> Response {
    match status.code() {
        tonic::Code::InvalidArgument => error_response(
            StatusCode::BAD_REQUEST,
            "team_request_invalid",
            "Revisa los datos del equipo e inténtalo de nuevo.",
        ),
        tonic::Code::NotFound => error_response(
            StatusCode::NOT_FOUND,
            "team_resource_unavailable",
            "El recurso no existe o no tienes acceso a este espacio.",
        ),
        tonic::Code::AlreadyExists => error_response(
            StatusCode::CONFLICT,
            "idempotency_conflict",
            "La clave de esta solicitud ya se utilizó con otros datos.",
        ),
        tonic::Code::Aborted | tonic::Code::FailedPrecondition => error_response(
            StatusCode::CONFLICT,
            "team_state_conflict",
            "El equipo cambió o no permite esta operación. Actualiza la página e inténtalo de nuevo.",
        ),
        tonic::Code::Unauthenticated => error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Inicia sesión para continuar.",
        ),
        tonic::Code::PermissionDenied => error_response(
            StatusCode::FORBIDDEN,
            "team_forbidden",
            "No tienes permiso para esta acción o la invitación no es válida.",
        ),
        tonic::Code::ResourceExhausted => {
            let retry_after = status
                .message()
                .split("retry after ")
                .nth(1)
                .and_then(|value| value.split_whitespace().next())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(900);
            let wait_minutes = retry_after.div_ceil(60);
            let wait_message = if wait_minutes >= 60 {
                let hours = wait_minutes / 60;
                let minutes = wait_minutes % 60;
                if minutes == 0 {
                    format!("{hours} h")
                } else {
                    format!("{hours} h y {minutes} min")
                }
            } else {
                format!("{wait_minutes} min")
            };
            let message = format!(
                "Has alcanzado el límite de invitaciones por seguridad. Podrás intentarlo de nuevo en {wait_message}. Límites: 5 por propietario cada 15 minutos, 20 por espacio cada 24 horas y 3 por correo destinatario cada 24 horas."
            );
            let mut response = error_response_with_details(
                StatusCode::TOO_MANY_REQUESTS,
                "invitation_rate_limited",
                &message,
                Some(serde_json::json!({
                    "category": "rate_limit",
                    "operation": "send_workspace_invitation",
                    "retry_after_seconds": retry_after,
                    "retryable": true,
                })),
            );
            if let Ok(value) = axum::http::HeaderValue::from_str(&retry_after.to_string()) {
                response
                    .headers_mut()
                    .insert(axum::http::header::RETRY_AFTER, value);
            }
            response
        }
        _ => {
            tracing::error!(correlation_id=%correlation_id, code=?status.code(), "Workspace team gRPC request failed");
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                unavailable_code,
                "El servicio de equipo no está disponible ahora. Inténtalo de nuevo.",
            )
        }
    }
}
