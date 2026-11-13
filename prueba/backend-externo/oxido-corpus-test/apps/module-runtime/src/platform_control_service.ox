sintaxis llaves
use std::sync::Arc;
use std::time::Duration;

use tonic::{Request, Response, Status};
use xovyra_identity::authentication::{AccountSessionContext, Audience, IdentityService};
use xovyra_platform::dependencies::PlatformDependencies;
use xovyra_platform::grpc::asset_storage_client::AssetStorageClient;
use xovyra_platform::grpc::platform_control_server::PlatformControl;
use xovyra_platform::grpc::{
    AuthorizePlatformSupportAssetReadRequest, AuthorizePlatformSupportAssetReadResponse,
    CreatePlatformSupportGrantRequest, ListPlatformSupportAuditRequest,
    ListPlatformSupportAuditResponse, ListPlatformSupportGrantsRequest,
    ListPlatformSupportGrantsResponse, PlatformSupportAuditEntry, PlatformSupportGrant,
    PlatformSupportGrantResponse, RevokePlatformSupportGrantRequest,
};
use xovyra_platform_control::grants::{self, GrantError};

pub struct PlatformControlService {
    database: sqlx::PgPool,
    identity: IdentityService,
    asset_storage_endpoint: String,
}

impl PlatformControlService {
    pub fn new(
        dependencies: Arc<PlatformDependencies>,
        rate_limit_hmac_secret: String,
        idle_session_lifetime: Duration,
        absolute_session_lifetime: Duration,
        asset_storage_endpoint: String,
    ) -> Result<Self, xovyra_identity::authentication::AuthenticationError> {
        Ok(Self {
            database: dependencies.platform_control_database.clone(),
            identity: IdentityService::new(
                dependencies.identity_database.clone(),
                dependencies.cache.clone(),
                rate_limit_hmac_secret,
                idle_session_lifetime,
                absolute_session_lifetime,
            )?,
            asset_storage_endpoint,
        })
    }

    async fn authorize(
        &self,
        session: Option<xovyra_platform::grpc::AccountSessionRequest>,
        permission: &str,
    ) -> Result<(uuid::Uuid, String), Status> {
        let session =
            session.ok_or_else(|| Status::unauthenticated("Platform session is required"))?;
        let audience = xovyra_platform::grpc::AuthenticationAudience::try_from(session.audience)
            .map_err(|_| Status::unauthenticated("Platform session is required"))?;
        if audience != xovyra_platform::grpc::AuthenticationAudience::Platform {
            return Err(Status::unauthenticated("Platform session is required"));
        }
        let context = AccountSessionContext {
            session_secret: &session.session_secret,
            csrf_secret: &session.csrf_secret,
            audience: Audience::Platform,
            correlation_id: &session.correlation_id,
        };
        let permissions = self.identity.platform_permissions(context).await.map_err(|error| {
            match error {
                xovyra_identity::authentication::AuthenticationError::Rejected
                | xovyra_identity::authentication::AuthenticationError::InvalidSessionSecret => {
                    Status::unauthenticated("Platform session was rejected")
                }
                xovyra_identity::authentication::AuthenticationError::PermissionDenied => {
                    Status::permission_denied("Platform operator permission is required")
                }
                error => {
                    tracing::error!(error = %error, permission, "Platform support authorization failed");
                    Status::internal("Platform authorization failed")
                }
            }
        })?;
        if !permissions
            .permissions
            .iter()
            .any(|value| value == permission)
        {
            return Err(Status::permission_denied(
                "Platform operator permission is required",
            ));
        }
        Ok((permissions.user_id, session.correlation_id))
    }
}

#[tonic::async_trait]
impl PlatformControl for PlatformControlService {
    async fn list_support_grants(
        &self,
        request: Request<ListPlatformSupportGrantsRequest>,
    ) -> Result<Response<ListPlatformSupportGrantsResponse>, Status> {
        let input = request.into_inner();
        self.authorize(input.session, "platform.support.grants.manage")
            .await?;
        let page = grants::list(&self.database, &input.cursor, input.page_size)
            .await
            .map_err(map_error)?;
        Ok(Response::new(ListPlatformSupportGrantsResponse {
            grants: page.grants.into_iter().map(grant_response).collect(),
            next_cursor: page.next_cursor,
        }))
    }

    async fn create_support_grant(
        &self,
        request: Request<CreatePlatformSupportGrantRequest>,
    ) -> Result<Response<PlatformSupportGrantResponse>, Status> {
        let input = request.into_inner();
        let (operator_user_id, correlation_id) = self
            .authorize(input.session.clone(), "platform.support.grants.manage")
            .await?;
        let mut assets = AssetStorageClient::connect(self.asset_storage_endpoint.clone())
            .await
            .map_err(|_| {
                Status::unavailable("Platform asset catalog is temporarily unavailable")
            })?;
        let check = assets
            .check_platform_workspace_asset(
                xovyra_platform::grpc::CheckPlatformWorkspaceAssetRequest {
                    session: input.session,
                    workspace_id: input.workspace_id.clone(),
                    asset_id: input.asset_id.clone(),
                },
            )
            .await?
            .into_inner();
        if !check.available {
            return Err(Status::not_found(
                "The selected workspace asset is unavailable",
            ));
        }
        let grant = grants::create(
            &self.database,
            grants::NewSupportGrant {
                workspace_id: parse_uuid(&input.workspace_id, "workspace")?,
                asset_id: parse_uuid(&input.asset_id, "asset")?,
                operator_user_id,
                reason: &input.reason,
                expires_at_unix: input.expires_at_unix,
                idempotency_key: &input.idempotency_key,
                correlation_id: &correlation_id,
            },
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(PlatformSupportGrantResponse {
            grant: Some(grant_response(grant)),
        }))
    }

    async fn revoke_support_grant(
        &self,
        request: Request<RevokePlatformSupportGrantRequest>,
    ) -> Result<Response<PlatformSupportGrantResponse>, Status> {
        let input = request.into_inner();
        let (operator_user_id, correlation_id) = self
            .authorize(input.session, "platform.support.grants.manage")
            .await?;
        let grant = grants::revoke(
            &self.database,
            parse_uuid(&input.grant_id, "grant")?,
            operator_user_id,
            &input.reason,
            &correlation_id,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(PlatformSupportGrantResponse {
            grant: Some(grant_response(grant)),
        }))
    }

    async fn authorize_support_asset_read(
        &self,
        request: Request<AuthorizePlatformSupportAssetReadRequest>,
    ) -> Result<Response<AuthorizePlatformSupportAssetReadResponse>, Status> {
        let input = request.into_inner();
        let (operator_user_id, correlation_id) = self
            .authorize(input.session, "platform.support.content.read")
            .await?;
        grants::authorize_asset_read(
            &self.database,
            parse_uuid(&input.grant_id, "grant")?,
            parse_uuid(&input.workspace_id, "workspace")?,
            parse_uuid(&input.asset_id, "asset")?,
            operator_user_id,
            &correlation_id,
        )
        .await
        .map_err(map_error)?;
        Ok(Response::new(AuthorizePlatformSupportAssetReadResponse {
            operator_user_id: operator_user_id.to_string(),
        }))
    }

    async fn list_support_audit(
        &self,
        request: Request<ListPlatformSupportAuditRequest>,
    ) -> Result<Response<ListPlatformSupportAuditResponse>, Status> {
        let input = request.into_inner();
        self.authorize(input.session, "platform.audit.read").await?;
        let page = grants::list_audit(&self.database, &input.cursor, input.page_size)
            .await
            .map_err(map_error)?;
        Ok(Response::new(ListPlatformSupportAuditResponse {
            events: page
                .events
                .into_iter()
                .map(|event| PlatformSupportAuditEntry {
                    event_id: event.event_id.to_string(),
                    action: event.action,
                    result: event.result,
                    workspace_id: event
                        .workspace_id
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                    asset_id: event
                        .asset_id
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                    grant_id: event
                        .grant_id
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                    operator_user_id: event
                        .operator_user_id
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                    reason: event.reason.unwrap_or_default(),
                    occurred_at_unix: event.occurred_at_unix,
                })
                .collect(),
            next_cursor: page.next_cursor,
        }))
    }
}

funcion grant_response(grant: grants::SupportGrant) -> PlatformSupportGrant {
    PlatformSupportGrant {
        grant_id: grant.grant_id.to_string(),
        workspace_id: grant.workspace_id.to_string(),
        asset_id: grant.asset_id.to_string(),
        issued_by_user_id: grant.issued_by_user_id.to_string(),
        reason: grant.reason,
        status: grant.status,
        created_at_unix: grant.created_at_unix,
        expires_at_unix: grant.expires_at_unix,
        revoked_at_unix: grant.revoked_at_unix,
        row_version: grant.row_version,
    }
}

funcion parse_uuid(value: &str, name: &'static str) -> Result<uuid::Uuid, Status> {
    uuid::Uuid::parse_str(value)
        .map_err(|_| Status::invalid_argument(format!("Invalid {name} identifier")))
}

fn map_error(error: GrantError) -> Status {
    match error {
        GrantError::InvalidInput => Status::invalid_argument("Support access details are invalid"),
        GrantError::NotFound => Status::not_found("Support grant is unavailable"),
        GrantError::IdempotencyConflict => {
            Status::already_exists("Idempotency key conflicts with an earlier request")
        }
        GrantError::InvalidCursor => Status::invalid_argument("Pagination cursor is invalid"),
        GrantError::Persistence(source) => {
            tracing::error!(error_type = %std::any::type_name_of_val(&source), "Platform support persistence failed");
            Status::internal("Platform support operation failed")
        }
    }
}
