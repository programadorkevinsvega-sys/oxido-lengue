use std::sync::Arc;

use tonic::{Request, Response, Status};
use xovyra_platform::dependencies::PlatformDependencies;
use xovyra_platform::grpc::identity_account_client::IdentityAccountClient;
use xovyra_platform::grpc::tenancy_access_server::TenancyAccess;
use xovyra_platform::grpc::{
    ActiveWorkspaceRequest, ActiveWorkspaceResponse, CreateWorkspaceRequest,
    DeleteWorkspaceRequest, GetWorkspaceRequest, ListWorkspacesRequest, ListWorkspacesResponse,
    RestoreWorkspaceRequest, SetActiveWorkspaceRequest, UpdateWorkspaceRequest,
    WorkspaceDeletionResponse, WorkspaceResponse, WorkspaceSummary,
};
use xovyra_tenancy_access::workspaces::{self, Workspace, WorkspaceError};

pub struct TenancyAccessService {
    dependencies: Arc<PlatformDependencies>,
    identity_endpoint: String,
}

impl TenancyAccessService {
    pub fn new(dependencies: Arc<PlatformDependencies>, identity_endpoint: String) -> Self {
        Self {
            dependencies,
            identity_endpoint,
        }
    }

    async fn authorize_platform(
        &self,
        session: Option<xovyra_platform::grpc::AccountSessionRequest>,
        permission: &str,
    ) -> Result<(), Status> {
        let session =
            session.ok_or_else(|| Status::unauthenticated("Platform session is required"))?;
        let correlation_id = session.correlation_id.clone();
        if xovyra_platform::grpc::AuthenticationAudience::try_from(session.audience).ok()
            != Some(xovyra_platform::grpc::AuthenticationAudience::Platform)
        {
            return Err(Status::unauthenticated("Platform session is required"));
        }
        let mut identity = IdentityAccountClient::connect(self.identity_endpoint.clone())
            .await
            .map_err(|_| {
                tracing::error!(
                    event = "platform.authorization.identity_connect.failed",
                    error_kind = "grpc_connection_failure",
                    error_code = "platform_identity_unavailable",
                    component = "tenancy_access_runtime",
                    operation = "authorize_platform_operator",
                    dependency = "identity_grpc",
                    grpc_code = ?tonic::Code::Unavailable,
                    outcome = "failed",
                    correlation_id,
                    "Platform operator authorization could not connect to Identity"
                );
                Status::unavailable("Platform authorization is temporarily unavailable")
            })?;
        let permissions = identity
            .get_platform_permissions(session)
            .await
            .inspect_err(|status| {
                tracing::error!(
                    event = "platform.authorization.permission_lookup.failed",
                    error_kind = "grpc_dependency_failure",
                    error_code = "platform_identity_permission_lookup_failed",
                    component = "tenancy_access_runtime",
                    operation = "authorize_platform_operator",
                    dependency = "identity_grpc",
                    grpc_code = ?status.code(),
                    outcome = "failed",
                    correlation_id,
                    "Platform operator permission lookup failed"
                );
            })?
            .into_inner();
        if !permissions
            .permissions
            .iter()
            .any(|granted| granted == permission)
        {
            return Err(Status::permission_denied(
                "Platform operator permission is required",
            ));
        }
        Ok(())
    }
}

#[tonic::async_trait]
impl TenancyAccess for TenancyAccessService {
    async fn list_platform_workspaces(
        &self,
        request: Request<xovyra_platform::grpc::ListPlatformWorkspacesRequest>,
    ) -> Result<Response<xovyra_platform::grpc::ListPlatformWorkspacesResponse>, Status> {
        let input = request.into_inner();
        let correlation_id = input
            .session
            .as_ref()
            .map(|session| session.correlation_id.clone())
            .unwrap_or_default();
        self.authorize_platform(input.session, "platform.organizations.read")
            .await?;
        let page = workspaces::list_platform_workspaces(
            &self.dependencies.tenancy_access_database,
            &input.search,
            &input.cursor,
            input.page_size,
        )
        .await
        .map_err(|error| {
            platform_directory_error(error, "list_platform_workspaces", &correlation_id)
        })?;
        Ok(Response::new(
            xovyra_platform::grpc::ListPlatformWorkspacesResponse {
                workspaces: page
                    .workspaces
                    .into_iter()
                    .map(
                        |workspace| xovyra_platform::grpc::PlatformWorkspaceSummary {
                            workspace_id: workspace.workspace_id.to_string(),
                            name: workspace.name,
                            workspace_type: workspace.workspace_type,
                            status: workspace.status,
                            created_at_unix: workspace.created_at_unix,
                            member_count: workspace.member_count,
                            row_version: workspace.row_version,
                        },
                    )
                    .collect(),
                next_cursor: page.next_cursor.unwrap_or_default(),
            },
        ))
    }

    async fn list_platform_workspace_members(
        &self,
        request: Request<xovyra_platform::grpc::ListPlatformWorkspaceMembersRequest>,
    ) -> Result<Response<xovyra_platform::grpc::ListPlatformWorkspaceMembersResponse>, Status> {
        let input = request.into_inner();
        let correlation_id = input
            .session
            .as_ref()
            .map(|session| session.correlation_id.clone())
            .unwrap_or_default();
        self.authorize_platform(input.session, "platform.organizations.read")
            .await?;
        let workspace_id = parse_uuid(&input.workspace_id, "workspace")?;
        let page = workspaces::list_platform_workspace_members(
            &self.dependencies.tenancy_access_database,
            workspace_id,
            &input.cursor,
            input.page_size,
        )
        .await
        .map_err(|error| {
            platform_directory_error(error, "list_platform_workspace_members", &correlation_id)
        })?;
        Ok(Response::new(
            xovyra_platform::grpc::ListPlatformWorkspaceMembersResponse {
                members: page
                    .members
                    .into_iter()
                    .map(
                        |member| xovyra_platform::grpc::PlatformWorkspaceMemberSummary {
                            user_id: member.user_id.to_string(),
                            role: member.role,
                            status: member.status,
                            joined_at_unix: member.joined_at_unix,
                        },
                    )
                    .collect(),
                next_cursor: page.next_cursor.unwrap_or_default(),
            },
        ))
    }

    async fn list_workspaces(
        &self,
        request: Request<ListWorkspacesRequest>,
    ) -> Result<Response<ListWorkspacesResponse>, Status> {
        let input = request.into_inner();
        let user_id = parse_uuid(&input.user_id, "authenticated user")?;
        let page = workspaces::list_workspaces_page(
            &self.dependencies.tenancy_access_database,
            user_id,
            if input.cursor.is_empty() {
                None
            } else {
                Some(input.cursor.as_str())
            },
            input.include_unavailable,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(ListWorkspacesResponse {
            workspaces: page.workspaces.into_iter().map(workspace_summary).collect(),
            next_cursor: page.next_cursor.unwrap_or_default(),
        }))
    }

    async fn create_workspace(
        &self,
        request: Request<CreateWorkspaceRequest>,
    ) -> Result<Response<WorkspaceResponse>, Status> {
        let input = request.into_inner();
        let user_id = parse_uuid(&input.user_id, "authenticated user")?;
        let session_id = parse_uuid(&input.session_id, "authenticated session")?;
        tracing::info!(correlation_id = %input.correlation_id, "Creating tenant workspace");
        let workspace = workspaces::create_workspace(
            &self.dependencies.tenancy_access_database,
            workspaces::WorkspaceCreation {
                user_id,
                session_id,
                name: &input.name,
                workspace_type: &input.workspace_type,
                idempotency_key: &input.idempotency_key,
                correlation_id: &input.correlation_id,
                owner_email: &input.email,
            },
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceResponse {
            workspace: Some(workspace_summary(workspace)),
        }))
    }

    async fn get_active_workspace(
        &self,
        request: Request<ActiveWorkspaceRequest>,
    ) -> Result<Response<ActiveWorkspaceResponse>, Status> {
        let input = request.into_inner();
        let user_id = parse_uuid(&input.user_id, "authenticated user")?;
        let session_id = parse_uuid(&input.session_id, "authenticated session")?;
        let workspace = workspaces::get_active_workspace(
            &self.dependencies.tenancy_access_database,
            user_id,
            session_id,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(ActiveWorkspaceResponse {
            selected: workspace.is_some(),
            workspace: workspace.map(workspace_summary),
        }))
    }

    async fn get_workspace(
        &self,
        request: Request<GetWorkspaceRequest>,
    ) -> Result<Response<WorkspaceResponse>, Status> {
        let input = request.into_inner();
        let user_id = parse_uuid(&input.user_id, "authenticated user")?;
        let workspace_id = parse_uuid(&input.workspace_id, "workspace")?;
        let workspace = workspaces::get_workspace(
            &self.dependencies.tenancy_access_database,
            user_id,
            workspace_id,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceResponse {
            workspace: Some(workspace_summary(workspace)),
        }))
    }

    async fn update_workspace(
        &self,
        request: Request<UpdateWorkspaceRequest>,
    ) -> Result<Response<WorkspaceResponse>, Status> {
        let input = request.into_inner();
        let user_id = parse_uuid(&input.user_id, "authenticated user")?;
        let workspace_id = parse_uuid(&input.workspace_id, "workspace")?;
        let workspace = workspaces::update_workspace(
            &self.dependencies.tenancy_access_database,
            user_id,
            workspace_id,
            workspaces::WorkspaceUpdate {
                expected_row_version: input.expected_row_version,
                name: input.name.as_deref(),
                locale: input.locale.as_deref(),
                timezone: input.timezone.as_deref(),
                correlation_id: &input.correlation_id,
            },
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceResponse {
            workspace: Some(workspace_summary(workspace)),
        }))
    }

    async fn delete_workspace(
        &self,
        request: Request<DeleteWorkspaceRequest>,
    ) -> Result<Response<WorkspaceDeletionResponse>, Status> {
        let input = request.into_inner();
        let user_id = parse_uuid(&input.user_id, "authenticated user")?;
        let workspace_id = parse_uuid(&input.workspace_id, "workspace")?;
        let deletion = workspaces::request_workspace_deletion(
            &self.dependencies.tenancy_access_database,
            user_id,
            workspace_id,
            &input.idempotency_key,
            &input.confirmation_name,
            &input.deletion_mode,
            &input.correlation_id,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceDeletionResponse {
            request_id: deletion.request_id.to_string(),
            workspace_id: deletion.workspace_id.to_string(),
            workspace_name: deletion.workspace_name,
            status: deletion.status,
            requested_at_unix: deletion.requested_at_unix,
            effective_at_unix: deletion.effective_at_unix,
            request_status: deletion.request_status,
            row_version: deletion.row_version,
        }))
    }

    async fn restore_workspace(
        &self,
        request: Request<RestoreWorkspaceRequest>,
    ) -> Result<Response<WorkspaceResponse>, Status> {
        let input = request.into_inner();
        let user_id = parse_uuid(&input.user_id, "authenticated user")?;
        let workspace_id = parse_uuid(&input.workspace_id, "workspace")?;
        let workspace = workspaces::restore_workspace(
            &self.dependencies.tenancy_access_database,
            user_id,
            workspace_id,
            &input.correlation_id,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceResponse {
            workspace: Some(workspace_summary(workspace)),
        }))
    }

    async fn set_active_workspace(
        &self,
        request: Request<SetActiveWorkspaceRequest>,
    ) -> Result<Response<WorkspaceResponse>, Status> {
        let input = request.into_inner();
        let user_id = parse_uuid(&input.user_id, "authenticated user")?;
        let session_id = parse_uuid(&input.session_id, "authenticated session")?;
        let workspace_id = parse_uuid(&input.workspace_id, "workspace")?;
        let workspace = workspaces::set_active_workspace(
            &self.dependencies.tenancy_access_database,
            user_id,
            session_id,
            workspace_id,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(WorkspaceResponse {
            workspace: Some(workspace_summary(workspace)),
        }))
    }
}

fn workspace_summary(workspace: Workspace) -> WorkspaceSummary {
    WorkspaceSummary {
        workspace_id: workspace.workspace_id.to_string(),
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

fn parse_uuid(value: &str, description: &'static str) -> Result<uuid::Uuid, Status> {
    uuid::Uuid::parse_str(value)
        .map_err(|_| Status::invalid_argument(format!("Invalid {description} identifier")))
}

fn platform_directory_error(
    error: xovyra_tenancy_access::workspaces::PlatformDirectoryError,
    operation: &'static str,
    correlation_id: &str,
) -> Status {
    match error {
        xovyra_tenancy_access::workspaces::PlatformDirectoryError::InvalidInput => {
            Status::invalid_argument("Platform workspace query is invalid")
        }
        xovyra_tenancy_access::workspaces::PlatformDirectoryError::Persistence(source) => {
            let sqlx_error_kind = match &source {
                sqlx::Error::Database(_) => "database",
                sqlx::Error::Io(_) => "database_transport",
                sqlx::Error::PoolTimedOut => "database_pool_timeout",
                sqlx::Error::PoolClosed => "database_pool_closed",
                sqlx::Error::ColumnDecode { .. } | sqlx::Error::Decode(_) => "row_decode",
                sqlx::Error::ColumnNotFound(_) => "column_not_found",
                _ => "other",
            };
            let database_error = source.as_database_error();
            let database_code = database_error
                .and_then(|database_error| database_error.code())
                .map(|code| code.into_owned());
            let database_constraint = database_error
                .and_then(|database_error| database_error.constraint())
                .map(str::to_owned);
            let database_table = database_error
                .and_then(|database_error| database_error.table())
                .map(str::to_owned);
            tracing::error!(
                event = "platform.workspace_directory.query.failed",
                error_kind = "persistence_failure",
                error_code = "platform_workspace_directory_unavailable",
                sqlx_error_kind,
                database_code = database_code.as_deref().unwrap_or("not_available"),
                database_constraint = database_constraint.as_deref().unwrap_or("not_available"),
                database_table = database_table.as_deref().unwrap_or("not_available"),
                operation,
                dependency = "postgresql",
                outcome = "failed",
                correlation_id,
                "Platform workspace directory query failed"
            );
            let user_message = match operation {
                "list_platform_workspace_members" => {
                    "Platform workspace members are temporarily unavailable"
                }
                _ => "Platform workspace directory is temporarily unavailable",
            };
            Status::unavailable(user_message)
        }
    }
}

fn map_error(error: WorkspaceError) -> Status {
    match error {
        WorkspaceError::InvalidInput => Status::invalid_argument("Workspace details are invalid"),
        WorkspaceError::NotFound => Status::not_found("Workspace is unavailable"),
        WorkspaceError::IdempotencyConflict => {
            Status::already_exists("Idempotency key conflicts with an earlier request")
        }
        WorkspaceError::NameConflict => {
            Status::already_exists("A team workspace with this name already exists")
        }
        WorkspaceError::PurgeBlocked => {
            Status::failed_precondition("Workspace data is under retention")
        }
        WorkspaceError::InvalidState => {
            Status::failed_precondition("Workspace does not permit this operation")
        }
        WorkspaceError::Forbidden => Status::permission_denied("Workspace permission is required"),
        WorkspaceError::VersionConflict => Status::aborted("Workspace changed since it was read"),
        WorkspaceError::Persistence(source) => {
            if let Some(database_error) = source.as_database_error() {
                tracing::error!(
                    database_code = ?database_error.code(),
                    constraint = ?database_error.constraint(),
                    "Tenancy database operation failed"
                );
            } else {
                tracing::error!(error_type = %std::any::type_name_of_val(&source), "Tenancy database operation failed");
            }
            Status::internal("Workspace operation failed")
        }
    }
}
