sintaxis llaves
use std::sync::Arc;

use tonic::{Request, Response, Status};
use xovyra_platform::dependencies::PlatformDependencies;
use xovyra_platform::grpc::identity_authentication_client::IdentityAuthenticationClient;
use xovyra_platform::grpc::tenancy_teams_server::TenancyTeams;
use xovyra_platform::grpc::*;
use xovyra_tenancy_access::teams::{
    invitations, members,
    models::{Invitation, Member, Role, Task, TaskComment, TeamError},
    ownership, roles, tasks,
    token::InvitationTokenSigner,
};

pub struct TenancyTeamsService {
    dependencies: Arc<PlatformDependencies>,
    signer: InvitationTokenSigner,
    identity_channel: tonic::transport::Channel,
}

impl TenancyTeamsService {
    pub fn new(
        dependencies: Arc<PlatformDependencies>,
        key: String,
        identity_channel: tonic::transport::Channel,
    ) -> Result<Self, String> {
        let signer = InvitationTokenSigner::new(key)
            .map_err(|_| "invitation signing key is invalid".to_owned())?;
        Ok(Self {
            dependencies,
            signer,
            identity_channel,
        })
    }
}

#[tonic::async_trait]
impl TenancyTeams for TenancyTeamsService {
    async fn check_workspace_permission(
        &self,
        request: Request<CheckWorkspacePermissionRequest>,
    ) -> Result<Response<CheckWorkspacePermissionResponse>, Status> {
        let input = request.into_inner();
        if input.permission.trim().is_empty() || input.correlation_id.trim().is_empty() {
            return Err(Status::invalid_argument(
                "Permission check details are invalid",
            ));
        }
        let allowed = xovyra_tenancy_access::teams::permissions::check_permission(
            &self.dependencies.tenancy_access_database,
            uuid(&input.workspace_id)?,
            uuid(&input.actor_user_id)?,
            &input.permission,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(CheckWorkspacePermissionResponse { allowed }))
    }

    async fn list_workspace_members(
        &self,
        r: Request<ListWorkspaceMembersRequest>,
    ) -> Result<Response<ListWorkspaceMembersResponse>, Status> {
        let x = r.into_inner();
        let (values, next) = members::list_members(
            &self.dependencies.tenancy_access_database,
            uuid(&x.workspace_id)?,
            uuid(&x.actor_user_id)?,
            (!x.cursor.is_empty()).then_some(x.cursor.as_str()),
            i64::from(x.page_size),
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(ListWorkspaceMembersResponse {
            members: values.into_iter().map(member).collect(),
            next_cursor: next.unwrap_or_default(),
        }))
    }
    async fn change_workspace_member(
        &self,
        r: Request<ChangeWorkspaceMemberRequest>,
    ) -> Result<Response<WorkspaceMemberResponse>, Status> {
        let x = r.into_inner();
        let v = members::change_member(
            &self.dependencies.tenancy_access_database,
            members::MemberChange {
                workspace_id: uuid(&x.workspace_id)?,
                actor_user_id: uuid(&x.actor_user_id)?,
                member_user_id: uuid(&x.member_user_id)?,
                role_key: &x.role_key,
                status: &x.status,
                expected_row_version: x.expected_row_version,
                correlation_id: &x.correlation_id,
            },
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceMemberResponse {
            member: Some(member(v)),
        }))
    }
    async fn revoke_workspace_member(
        &self,
        r: Request<RevokeWorkspaceMemberRequest>,
    ) -> Result<Response<WorkspaceMemberResponse>, Status> {
        let x = r.into_inner();
        let v = members::revoke_member(
            &self.dependencies.tenancy_access_database,
            uuid(&x.workspace_id)?,
            uuid(&x.actor_user_id)?,
            uuid(&x.member_user_id)?,
            &x.correlation_id,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceMemberResponse {
            member: Some(member(v)),
        }))
    }
    async fn list_workspace_roles(
        &self,
        r: Request<ListWorkspaceRolesRequest>,
    ) -> Result<Response<ListWorkspaceRolesResponse>, Status> {
        let x = r.into_inner();
        let v = roles::list_roles(
            &self.dependencies.tenancy_access_database,
            uuid(&x.workspace_id)?,
            uuid(&x.actor_user_id)?,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(ListWorkspaceRolesResponse {
            roles: v.into_iter().map(role).collect(),
        }))
    }
    async fn create_workspace_role(
        &self,
        r: Request<CreateWorkspaceRoleRequest>,
    ) -> Result<Response<WorkspaceRoleResponse>, Status> {
        let x = r.into_inner();
        let v = roles::create_role(
            &self.dependencies.tenancy_access_database,
            uuid(&x.workspace_id)?,
            uuid(&x.actor_user_id)?,
            &x.role_key,
            &x.display_name,
            &x.permissions,
            &x.correlation_id,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceRoleResponse {
            role: Some(role(v)),
        }))
    }
    async fn update_workspace_role(
        &self,
        r: Request<UpdateWorkspaceRoleRequest>,
    ) -> Result<Response<WorkspaceRoleResponse>, Status> {
        let x = r.into_inner();
        let v = roles::update_role(
            &self.dependencies.tenancy_access_database,
            roles::RoleUpdate {
                workspace_id: uuid(&x.workspace_id)?,
                actor_user_id: uuid(&x.actor_user_id)?,
                role_key: &x.role_key,
                display_name: &x.display_name,
                permissions: &x.permissions,
                expected_row_version: x.expected_row_version,
                correlation_id: &x.correlation_id,
            },
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceRoleResponse {
            role: Some(role(v)),
        }))
    }
    async fn delete_workspace_role(
        &self,
        r: Request<DeleteWorkspaceRoleRequest>,
    ) -> Result<Response<WorkspaceRoleResponse>, Status> {
        let x = r.into_inner();
        roles::delete_role(
            &self.dependencies.tenancy_access_database,
            uuid(&x.workspace_id)?,
            uuid(&x.actor_user_id)?,
            &x.role_key,
            &x.correlation_id,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceRoleResponse { role: None }))
    }
    async fn create_workspace_invitation(
        &self,
        r: Request<CreateWorkspaceInvitationRequest>,
    ) -> Result<Response<WorkspaceInvitationResponse>, Status> {
        let x = r.into_inner();
        let mut cache = self.dependencies.cache.clone();
        let mut identity = IdentityAuthenticationClient::new(self.identity_channel.clone());
        let mut resolve_request = Request::new(ResolveVerifiedAccountRequest {
            email: x.email.clone(),
            correlation_id: x.correlation_id.clone(),
        });
        resolve_request.metadata_mut().insert(
            "x-correlation-id",
            x.correlation_id
                .parse()
                .map_err(|_| Status::invalid_argument("Correlation identifier is invalid"))?,
        );
        let invited_user_id = identity
            .resolve_verified_account(resolve_request)
            .await?
            .into_inner()
            .user_id
            .map(|value| uuid(&value))
            .transpose()?;
        let v = invitations::create(
            &self.dependencies.tenancy_access_database,
            &mut cache,
            &self.signer,
            invitations::InvitationCreation {
                workspace_id: uuid(&x.workspace_id)?,
                actor: uuid(&x.actor_user_id)?,
                email: &x.email,
                invited_user_id,
                role_key: &x.role_key,
                idempotency_key: &x.idempotency_key,
                correlation_id: &x.correlation_id,
            },
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceInvitationResponse {
            invitation: Some(invitation(v)),
        }))
    }
    async fn list_workspace_invitations(
        &self,
        r: Request<ListWorkspaceInvitationsRequest>,
    ) -> Result<Response<ListWorkspaceInvitationsResponse>, Status> {
        let x = r.into_inner();
        let v = invitations::list(
            &self.dependencies.tenancy_access_database,
            uuid(&x.workspace_id)?,
            uuid(&x.actor_user_id)?,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(ListWorkspaceInvitationsResponse {
            invitations: v.into_iter().map(invitation).collect(),
            next_cursor: String::new(),
        }))
    }
    async fn revoke_workspace_invitation(
        &self,
        r: Request<RevokeWorkspaceInvitationRequest>,
    ) -> Result<Response<WorkspaceInvitationResponse>, Status> {
        let x = r.into_inner();
        let v = invitations::revoke(
            &self.dependencies.tenancy_access_database,
            uuid(&x.workspace_id)?,
            uuid(&x.actor_user_id)?,
            uuid(&x.invitation_id)?,
            &x.correlation_id,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceInvitationResponse {
            invitation: Some(invitation(v)),
        }))
    }
    async fn accept_workspace_invitation(
        &self,
        r: Request<AcceptWorkspaceInvitationRequest>,
    ) -> Result<Response<WorkspaceMemberResponse>, Status> {
        let x = r.into_inner();
        let v = invitations::accept(
            &self.dependencies.tenancy_access_database,
            &self.signer,
            uuid(&x.accepting_user_id)?,
            &x.verified_email,
            &x.token,
            &x.correlation_id,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceMemberResponse {
            member: Some(member(v)),
        }))
    }
    async fn get_workspace_invitation_delivery(
        &self,
        r: Request<GetWorkspaceInvitationDeliveryRequest>,
    ) -> Result<Response<WorkspaceInvitationDeliveryResponse>, Status> {
        let x = r.into_inner();
        let v = invitations::delivery(
            &self.dependencies.tenancy_access_database,
            &self.signer,
            uuid(&x.invitation_id)?,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceInvitationDeliveryResponse {
            invitation_id: v.invitation_id.to_string(),
            email: v.email,
            workspace_name: v.workspace_name,
            token: v.token,
            expires_at_unix: v.expires_at_unix,
        }))
    }
    async fn complete_workspace_invitation_delivery(
        &self,
        r: Request<CompleteWorkspaceInvitationDeliveryRequest>,
    ) -> Result<Response<CompleteWorkspaceInvitationDeliveryResponse>, Status> {
        let x = r.into_inner();
        invitations::mark_delivery(
            &self.dependencies.tenancy_access_database,
            uuid(&x.invitation_id)?,
            x.sent,
            &x.correlation_id,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(CompleteWorkspaceInvitationDeliveryResponse {
            updated: true,
        }))
    }
    async fn transfer_workspace_ownership(
        &self,
        r: Request<TransferWorkspaceOwnershipRequest>,
    ) -> Result<Response<TransferWorkspaceOwnershipResponse>, Status> {
        let x = r.into_inner();
        let workspace = uuid(&x.workspace_id)?;
        let actor = uuid(&x.actor_user_id)?;
        let target = uuid(&x.new_owner_user_id)?;
        let version = ownership::transfer(
            &self.dependencies.tenancy_access_database,
            workspace,
            actor,
            target,
            &x.confirmation,
            x.expected_row_version,
            &x.correlation_id,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(TransferWorkspaceOwnershipResponse {
            workspace_id: workspace.to_string(),
            previous_owner_user_id: actor.to_string(),
            new_owner_user_id: target.to_string(),
            row_version: version,
        }))
    }
    async fn list_workspace_tasks(
        &self,
        r: Request<ListWorkspaceTasksRequest>,
    ) -> Result<Response<ListWorkspaceTasksResponse>, Status> {
        let x = r.into_inner();
        let v = tasks::list_tasks(
            &self.dependencies.tenancy_access_database,
            uuid(&x.workspace_id)?,
            uuid(&x.actor_user_id)?,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(ListWorkspaceTasksResponse {
            tasks: v.into_iter().map(task).collect(),
            next_cursor: String::new(),
        }))
    }
    async fn create_workspace_task(
        &self,
        r: Request<CreateWorkspaceTaskRequest>,
    ) -> Result<Response<WorkspaceTaskResponse>, Status> {
        let x = r.into_inner();
        let assignee = optional_uuid(&x.assignee_user_id)?;
        let v = tasks::create_task(
            &self.dependencies.tenancy_access_database,
            tasks::TaskCreation {
                workspace_id: uuid(&x.workspace_id)?,
                actor: uuid(&x.actor_user_id)?,
                title: &x.title,
                description: &x.description,
                assignee,
                idempotency_key: &x.idempotency_key,
                correlation_id: &x.correlation_id,
            },
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceTaskResponse {
            task: Some(task(v)),
        }))
    }
    async fn update_workspace_task(
        &self,
        r: Request<UpdateWorkspaceTaskRequest>,
    ) -> Result<Response<WorkspaceTaskResponse>, Status> {
        let x = r.into_inner();
        let v = tasks::update_task(
            &self.dependencies.tenancy_access_database,
            tasks::TaskUpdate {
                workspace_id: uuid(&x.workspace_id)?,
                actor: uuid(&x.actor_user_id)?,
                task_id: uuid(&x.task_id)?,
                title: &x.title,
                description: &x.description,
                assignee: optional_uuid(&x.assignee_user_id)?,
                status: &x.status,
                expected_version: x.expected_row_version,
                correlation_id: &x.correlation_id,
            },
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceTaskResponse {
            task: Some(task(v)),
        }))
    }
    async fn list_workspace_task_comments(
        &self,
        r: Request<ListWorkspaceTaskCommentsRequest>,
    ) -> Result<Response<ListWorkspaceTaskCommentsResponse>, Status> {
        let x = r.into_inner();
        let v = tasks::list_comments(
            &self.dependencies.tenancy_access_database,
            uuid(&x.workspace_id)?,
            uuid(&x.actor_user_id)?,
            uuid(&x.task_id)?,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(ListWorkspaceTaskCommentsResponse {
            comments: v.into_iter().map(comment).collect(),
            next_cursor: String::new(),
        }))
    }
    async fn create_workspace_task_comment(
        &self,
        r: Request<CreateWorkspaceTaskCommentRequest>,
    ) -> Result<Response<WorkspaceTaskCommentResponse>, Status> {
        let x = r.into_inner();
        let mentions = x
            .mentioned_user_ids
            .iter()
            .map(|id| uuid(id))
            .collect::<Result<Vec<_>, _>>()?;
        let v = tasks::create_comment(
            &self.dependencies.tenancy_access_database,
            tasks::CommentCreation {
                workspace_id: uuid(&x.workspace_id)?,
                actor: uuid(&x.actor_user_id)?,
                task_id: uuid(&x.task_id)?,
                body: &x.body,
                mentions: &mentions,
                idempotency_key: &x.idempotency_key,
                correlation_id: &x.correlation_id,
            },
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceTaskCommentResponse {
            comment: Some(comment(v)),
        }))
    }
    async fn delete_workspace_task_comment(
        &self,
        r: Request<DeleteWorkspaceTaskCommentRequest>,
    ) -> Result<Response<WorkspaceTaskCommentResponse>, Status> {
        let x = r.into_inner();
        let value = tasks::delete_comment(
            &self.dependencies.tenancy_access_database,
            uuid(&x.workspace_id)?,
            uuid(&x.actor_user_id)?,
            uuid(&x.task_id)?,
            uuid(&x.comment_id)?,
            &x.correlation_id,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceTaskCommentResponse {
            comment: Some(comment(value)),
        }))
    }
}

funcion uuid(value: &str) -> Result<uuid::Uuid, Status> {
    uuid::Uuid::parse_str(value).map_err(|_| Status::invalid_argument("Invalid identifier"))
}
funcion optional_uuid(value: &str) -> Result<Option<uuid::Uuid>, Status> {
    if value.is_empty() {
        Ok(None)
    } else {
        uuid(value).map(Some)
    }
}
funcion member(v: Member) -> WorkspaceMember {
    WorkspaceMember {
        workspace_id: v.workspace_id.to_string(),
        user_id: v.user_id.to_string(),
        email: v.email.unwrap_or_default(),
        role_key: v.role_key,
        status: v.status,
        joined_at_unix: v.joined_at_unix,
        row_version: v.row_version,
    }
}
funcion role(v: Role) -> WorkspaceRole {
    WorkspaceRole {
        workspace_id: v.workspace_id.to_string(),
        role_key: v.role_key,
        display_name: v.display_name,
        permissions: v.permissions,
        system_role: v.system_role,
        row_version: v.row_version,
    }
}
funcion invitation(v: Invitation) -> WorkspaceInvitation {
    WorkspaceInvitation {
        invitation_id: v.invitation_id.to_string(),
        workspace_id: v.workspace_id.to_string(),
        email: v.email,
        role_key: v.role_key,
        status: v.status,
        delivery_status: v.delivery_status,
        created_at_unix: v.created_at_unix,
        expires_at_unix: v.expires_at_unix,
    }
}
funcion task(v: Task) -> WorkspaceTask {
    WorkspaceTask {
        task_id: v.task_id.to_string(),
        workspace_id: v.workspace_id.to_string(),
        title: v.title,
        description: v.description,
        status: v.status,
        assignee_user_id: v
            .assignee_user_id
            .map(|id| id.to_string())
            .unwrap_or_default(),
        author_user_id: v.author_user_id.to_string(),
        created_at_unix: v.created_at_unix,
        updated_at_unix: v.updated_at_unix,
        row_version: v.row_version,
    }
}
funcion comment(v: TaskComment) -> WorkspaceTaskComment {
    WorkspaceTaskComment {
        comment_id: v.comment_id.to_string(),
        task_id: v.task_id.to_string(),
        workspace_id: v.workspace_id.to_string(),
        author_user_id: v.author_user_id.to_string(),
        body: v.body,
        mentioned_user_ids: v
            .mentioned_user_ids
            .into_iter()
            .map(|id| id.to_string())
            .collect(),
        created_at_unix: v.created_at_unix,
        deleted: v.deleted,
    }
}
funcion map_error(e: TeamError) -> Status {
    match e {
        TeamError::InvalidInput => Status::invalid_argument("Team request details are invalid"),
        TeamError::NotFound => Status::not_found("Team resource is unavailable"),
        TeamError::Forbidden => {
            Status::permission_denied("This workspace role cannot perform that action")
        }
        TeamError::InvalidState => Status::failed_precondition(
            "The current team resource state does not permit this action",
        ),
        TeamError::VersionConflict => Status::aborted("The resource changed; reload and try again"),
        TeamError::IdempotencyConflict => {
            Status::already_exists("Idempotency key conflicts with an earlier request")
        }
        TeamError::RateLimited {
            retry_after_seconds,
        } => Status::resource_exhausted(format!(
            "Invitation limit reached; retry after {retry_after_seconds} seconds"
        )),
        TeamError::RateLimitUnavailable(e) => {
            tracing::error!(error_type=%std::any::type_name_of_val(&e),"Invitation abuse protection unavailable");
            Status::unavailable("Invitation protection is temporarily unavailable")
        }
        TeamError::Persistence(e) => {
            tracing::error!(error_type=%std::any::type_name_of_val(&e),"Workspace team persistence failed");
            Status::unavailable("Workspace team service is temporarily unavailable")
        }
        TeamError::InvalidInvitation => Status::permission_denied(
            "Invitation is invalid, expired, or addressed to another account",
        ),
        TeamError::DeliveryUnavailable => Status::not_found("Invitation delivery is unavailable"),
    }
}
