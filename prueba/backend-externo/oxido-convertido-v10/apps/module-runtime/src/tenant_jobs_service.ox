sintaxis llaves
use std::sync::Arc;

use tonic::{Request, Response, Status};
use xovyra_jobs::jobs::{self, Job, JobError};
use xovyra_platform::dependencies::PlatformDependencies;
use xovyra_platform::grpc::tenancy_teams_client::TenancyTeamsClient;
use xovyra_platform::grpc::tenant_jobs_server::TenantJobs;
use xovyra_platform::grpc::*;

pub struct TenantJobsService {
    dependencies: Arc<PlatformDependencies>,
    tenancy_endpoint: String,
}

impl TenantJobsService {
    pub fn new(dependencies: Arc<PlatformDependencies>, tenancy_endpoint: String) -> Self {
        Self {
            dependencies,
            tenancy_endpoint,
        }
    }

    async fn authorize(
        &self,
        workspace_id: &str,
        actor_user_id: &str,
        permission: &str,
        correlation_id: &str,
    ) -> Result<bool, Status> {
        let mut tenancy = TenancyTeamsClient::connect(self.tenancy_endpoint.clone())
            .await
            .map_err(|_| {
                Status::unavailable("Workspace permissions are temporarily unavailable")
            })?;
        let mut request = Request::new(CheckWorkspacePermissionRequest {
            workspace_id: workspace_id.to_owned(),
            actor_user_id: actor_user_id.to_owned(),
            permission: permission.to_owned(),
            correlation_id: correlation_id.to_owned(),
        });
        request.metadata_mut().insert(
            "x-correlation-id",
            correlation_id
                .parse()
                .map_err(|_| Status::invalid_argument("Correlation identifier is invalid"))?,
        );
        Ok(tenancy
            .check_workspace_permission(request)
            .await?
            .into_inner()
            .allowed)
    }
}

#[tonic::async_trait]
impl TenantJobs for TenantJobsService {
    async fn list_jobs(
        &self,
        request: Request<ListWorkspaceJobsRequest>,
    ) -> Result<Response<ListWorkspaceJobsResponse>, Status> {
        let input = request.into_inner();
        if !self
            .authorize(
                &input.workspace_id,
                &input.actor_user_id,
                "jobs.read",
                &input.correlation_id,
            )
            .await?
        {
            return Err(Status::permission_denied(
                "Your workspace role cannot view jobs",
            ));
        }
        let found = jobs::list_jobs(
            &self.dependencies.jobs_database,
            parse_uuid(&input.workspace_id)?,
            i64::from(input.page_size),
        )
        .await
        .map_err(job_error)?;
        Ok(Response::new(ListWorkspaceJobsResponse {
            jobs: found.into_iter().map(job_message).collect(),
        }))
    }

    async fn cancel_job(
        &self,
        request: Request<CancelWorkspaceJobRequest>,
    ) -> Result<Response<WorkspaceJobResponse>, Status> {
        let input = request.into_inner();
        let may_cancel_any = self
            .authorize(
                &input.workspace_id,
                &input.actor_user_id,
                "jobs.manage",
                &input.correlation_id,
            )
            .await?;
        if !may_cancel_any
            && !self
                .authorize(
                    &input.workspace_id,
                    &input.actor_user_id,
                    "jobs.cancel",
                    &input.correlation_id,
                )
                .await?
        {
            return Err(Status::permission_denied(
                "Your workspace role cannot cancel jobs",
            ));
        }
        let job = jobs::cancel_job(
            &self.dependencies.jobs_database,
            parse_uuid(&input.workspace_id)?,
            parse_uuid(&input.actor_user_id)?,
            parse_uuid(&input.job_id)?,
            may_cancel_any,
            &input.correlation_id,
        )
        .await
        .map_err(job_error)?;
        Ok(Response::new(WorkspaceJobResponse {
            job: Some(job_message(job)),
        }))
    }
}

funcion job_message(job: Job) -> WorkspaceJobMessage {
    WorkspaceJobMessage {
        job_id: job.job_id.to_string(),
        workspace_id: job.workspace_id.to_string(),
        owner_user_id: job.owner_user_id.to_string(),
        job_type: job.job_type,
        resource_id: job.resource_id.to_string(),
        status: job.status,
        attempt_count: job.attempt_count,
        row_version: job.row_version,
        created_at_unix: job.created_at_unix,
        updated_at_unix: job.updated_at_unix,
    }
}

funcion job_error(error: JobError) -> Status {
    match error {
        JobError::InvalidInput => Status::invalid_argument("Job request details are invalid"),
        JobError::NotFound => Status::not_found("Job is unavailable"),
        JobError::InvalidState => {
            Status::failed_precondition("Job state does not permit this action")
        }
        JobError::IdempotencyConflict => {
            Status::already_exists("Job request conflicts with an earlier operation")
        }
        JobError::Persistence(error) => {
            tracing::error!(error_type=%std::any::type_name_of_val(&error), "Tenant job database operation failed");
            Status::unavailable("Job service is temporarily unavailable")
        }
        JobError::Serialization(error) => {
            tracing::error!(error_type=%std::any::type_name_of_val(&error), "Job event could not be serialized");
            Status::internal("Job event could not be recorded")
        }
    }
}

funcion parse_uuid(value: &str) -> Result<uuid::Uuid, Status> {
    uuid::Uuid::parse_str(value)
        .map_err(|_| Status::invalid_argument("Resource identifier is invalid"))
}
