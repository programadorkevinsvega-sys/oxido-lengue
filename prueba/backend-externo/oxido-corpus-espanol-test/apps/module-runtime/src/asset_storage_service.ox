sintaxis llaves
use std::sync::Arc;

use tonic::{Request, Response, Status};
use xovyra_asset_storage::assets::{self, Asset, AssetError, PendingAsset};
use xovyra_platform::dependencies::PlatformDependencies;
use xovyra_platform::grpc::asset_storage_server::AssetStorage;
use xovyra_platform::grpc::identity_account_client::IdentityAccountClient;
use xovyra_platform::grpc::platform_control_client::PlatformControlClient;
use xovyra_platform::grpc::tenancy_teams_client::TenancyTeamsClient;
use xovyra_platform::grpc::*;

use crate::r2_storage::R2Storage;

pub struct AssetStorageService {
    dependencies: Arc<PlatformDependencies>,
    r2: Option<Arc<R2Storage>>,
    tenancy_endpoint: String,
    worker_auth_secret: String,
}

impl AssetStorageService {
    pub fn new(
        dependencies: Arc<PlatformDependencies>,
        r2: Option<Arc<R2Storage>>,
        tenancy_endpoint: String,
        worker_auth_secret: String,
    ) -> Self {
        Self {
            dependencies,
            r2,
            tenancy_endpoint,
            worker_auth_secret,
        }
    }

    async fn authorize(
        &self,
        workspace_id: &str,
        actor_user_id: &str,
        permission: &str,
        correlation_id: &str,
    ) -> Result<(), Status> {
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
        let allowed = tenancy
            .check_workspace_permission(request)
            .await?
            .into_inner()
            .allowed;
        if !allowed {
            return Err(Status::permission_denied(
                "Your workspace role cannot access this resource",
            ));
        }
        Ok(())
    }

    async fn authorize_platform(
        &self,
        session: Option<AccountSessionRequest>,
        permission: &str,
    ) -> Result<AccountSessionRequest, Status> {
        let session =
            session.ok_or_else(|| Status::unauthenticated("Platform session is required"))?;
        if AuthenticationAudience::try_from(session.audience).ok()
            != Some(AuthenticationAudience::Platform)
        {
            return Err(Status::unauthenticated("Platform session is required"));
        }
        let mut identity = IdentityAccountClient::connect(self.tenancy_endpoint.clone())
            .await
            .map_err(|_| {
                Status::unavailable("Platform authorization is temporarily unavailable")
            })?;
        let permissions = identity
            .get_platform_permissions(session.clone())
            .await?
            .into_inner();
        if !permissions
            .permissions
            .iter()
            .any(|current| current == permission)
        {
            return Err(Status::permission_denied(
                "Platform operator permission is required",
            ));
        }
        Ok(session)
    }
}

#[tonic::async_trait]
impl AssetStorage for AssetStorageService {
    async fn get_asset_deletion_target(
        &self,
        request: Request<GetAssetDeletionTargetRequest>,
    ) -> Result<Response<AssetDeletionTargetResponse>, Status> {
        crate::worker_auth::verify(&request, &self.worker_auth_secret)?;
        let input = request.into_inner();
        let asset = assets::get_asset_deletion_target(
            &self.dependencies.asset_storage_database,
            parse_uuid(&input.workspace_id)?,
            parse_uuid(&input.asset_id)?,
        )
        .await
        .map_err(asset_error)?;
        Ok(Response::new(AssetDeletionTargetResponse {
            owner_user_id: asset.owner_user_id.to_string(),
            object_key: asset.object_key,
            lifecycle_state: asset.lifecycle_state,
            upload_object_key: asset.upload_object_key,
        }))
    }

    async fn complete_asset_deletion(
        &self,
        request: Request<CompleteAssetDeletionRequest>,
    ) -> Result<Response<AssetResponse>, Status> {
        crate::worker_auth::verify(&request, &self.worker_auth_secret)?;
        let input = request.into_inner();
        let asset = assets::complete_asset_deletion(
            &self.dependencies.asset_storage_database,
            parse_uuid(&input.workspace_id)?,
            parse_uuid(&input.asset_id)?,
            &input.correlation_id,
        )
        .await
        .map_err(asset_error)?;
        Ok(Response::new(AssetResponse {
            asset: Some(asset_message(asset)),
        }))
    }

    async fn create_upload(
        &self,
        request: Request<CreateAssetUploadRequest>,
    ) -> Result<Response<AssetUploadResponse>, Status> {
        let input = request.into_inner();
        self.authorize(
            &input.workspace_id,
            &input.actor_user_id,
            "assets.create",
            &input.correlation_id,
        )
        .await?;
        let r2 = self.r2.as_ref().ok_or_else(storage_unavailable)?;
        let workspace_id = parse_uuid(&input.workspace_id)?;
        let owner_user_id = parse_uuid(&input.actor_user_id)?;
        let asset = assets::create_pending_asset(
            &self.dependencies.asset_storage_database,
            PendingAsset {
                workspace_id,
                owner_user_id,
                display_name: &input.display_name,
                content_type: &input.content_type,
                size_bytes: input.size_bytes,
                idempotency_key: &input.idempotency_key,
                correlation_id: &input.correlation_id,
            },
        )
        .await
        .map_err(asset_error)?;
        let upload_url = r2
            .presign_upload(&asset.upload_object_key, &asset.content_type)
            .await
            .map_err(|_| storage_unavailable())?;
        Ok(Response::new(AssetUploadResponse {
            asset: Some(asset_message(asset)),
            upload_url,
            expires_at_unix: unix_now() + 300,
            upload_content_type: input.content_type,
        }))
    }

    async fn complete_upload(
        &self,
        request: Request<CompleteAssetUploadRequest>,
    ) -> Result<Response<AssetResponse>, Status> {
        let input = request.into_inner();
        self.authorize(
            &input.workspace_id,
            &input.actor_user_id,
            "assets.create",
            &input.correlation_id,
        )
        .await?;
        let r2 = self.r2.as_ref().ok_or_else(storage_unavailable)?;
        let workspace_id = parse_uuid(&input.workspace_id)?;
        let owner_user_id = parse_uuid(&input.actor_user_id)?;
        let asset_id = parse_uuid(&input.asset_id)?;
        let pending = assets::get_pending_upload(
            &self.dependencies.asset_storage_database,
            workspace_id,
            owner_user_id,
            asset_id,
        )
        .await
        .map_err(asset_error)?;
        let mut promoted = false;
        let upload_metadata = if pending.lifecycle_state == "active" {
            r2.inspect_object(&pending.object_key).await
        } else {
            match r2.inspect_object(&pending.upload_object_key).await {
                Ok(metadata) => {
                    if metadata.0 != pending.declared_size_bytes
                        || metadata.1 != pending.content_type
                    {
                        return Err(Status::failed_precondition(
                            "The uploaded file does not match its authorized size and type",
                        ));
                    }
                    r2.promote_object(&pending.upload_object_key, &pending.object_key)
                        .await
                        .map_err(|_| storage_unavailable())?;
                    promoted = true;
                    r2.inspect_object(&pending.object_key).await
                }
                Err(_) => r2.inspect_object(&pending.object_key).await,
            }
        }
        .map_err(|_| {
            Status::failed_precondition("The object is not available in your secure storage")
        })?;
        if promoted {
            r2.delete_object(&pending.upload_object_key)
                .await
                .map_err(|_| storage_unavailable())?;
        }
        let (size_bytes, content_type, etag) = upload_metadata;
        let asset = assets::activate_uploaded_asset(
            &self.dependencies.asset_storage_database,
            assets::UploadConfirmation {
                workspace_id,
                owner_user_id,
                asset_id,
                actual_size_bytes: size_bytes,
                actual_content_type: &content_type,
                etag: &etag,
                correlation_id: &input.correlation_id,
            },
        )
        .await
        .map_err(asset_error)?;
        Ok(Response::new(AssetResponse {
            asset: Some(asset_message(asset)),
        }))
    }

    async fn create_download(
        &self,
        request: Request<CreateAssetDownloadRequest>,
    ) -> Result<Response<AssetDownloadResponse>, Status> {
        let input = request.into_inner();
        self.authorize(
            &input.workspace_id,
            &input.actor_user_id,
            "assets.read",
            &input.correlation_id,
        )
        .await?;
        let r2 = self.r2.as_ref().ok_or_else(storage_unavailable)?;
        let asset = assets::get_private_asset(
            &self.dependencies.asset_storage_database,
            parse_uuid(&input.workspace_id)?,
            parse_uuid(&input.actor_user_id)?,
            parse_uuid(&input.asset_id)?,
        )
        .await
        .map_err(asset_error)?;
        let download_url = r2
            .presign_download(&asset.object_key)
            .await
            .map_err(|_| storage_unavailable())?;
        Ok(Response::new(AssetDownloadResponse {
            asset: Some(asset_message(asset)),
            download_url,
            expires_at_unix: unix_now() + 90,
        }))
    }

    async fn delete_asset(
        &self,
        request: Request<DeleteAssetRequest>,
    ) -> Result<Response<AssetResponse>, Status> {
        let input = request.into_inner();
        self.authorize(
            &input.workspace_id,
            &input.actor_user_id,
            "assets.delete",
            &input.correlation_id,
        )
        .await?;
        self.r2.as_ref().ok_or_else(storage_unavailable)?;
        let asset = assets::request_asset_deletion(
            &self.dependencies.asset_storage_database,
            parse_uuid(&input.workspace_id)?,
            parse_uuid(&input.actor_user_id)?,
            parse_uuid(&input.asset_id)?,
            &input.correlation_id,
        )
        .await
        .map_err(asset_error)?;
        Ok(Response::new(AssetResponse {
            asset: Some(asset_message(asset)),
        }))
    }

    async fn search_assets(
        &self,
        request: Request<SearchAssetsRequest>,
    ) -> Result<Response<SearchAssetsResponse>, Status> {
        let input = request.into_inner();
        self.authorize(
            &input.workspace_id,
            &input.actor_user_id,
            "assets.read",
            &input.correlation_id,
        )
        .await?;
        let found = assets::search_private_assets(
            &self.dependencies.asset_storage_database,
            parse_uuid(&input.workspace_id)?,
            parse_uuid(&input.actor_user_id)?,
            &input.query,
            i64::from(input.page_size),
        )
        .await
        .map_err(asset_error)?;
        Ok(Response::new(SearchAssetsResponse {
            assets: found.into_iter().map(asset_message).collect(),
        }))
    }

    async fn list_platform_workspace_assets(
        &self,
        request: Request<ListPlatformWorkspaceAssetsRequest>,
    ) -> Result<Response<ListPlatformWorkspaceAssetsResponse>, Status> {
        let input = request.into_inner();
        self.authorize_platform(input.session, "platform.organizations.read")
            .await?;
        let page = assets::list_platform_assets(
            &self.dependencies.asset_storage_database,
            parse_uuid(&input.workspace_id)?,
            &input.cursor,
            input.page_size,
        )
        .await
        .map_err(asset_error)?;
        Ok(Response::new(ListPlatformWorkspaceAssetsResponse {
            assets: page
                .assets
                .into_iter()
                .map(|asset| PlatformWorkspaceAssetSummary {
                    asset_id: asset.asset_id.to_string(),
                    content_type: asset.content_type,
                    size_bytes: asset.size_bytes,
                    lifecycle_state: asset.lifecycle_state,
                    created_at_unix: asset.created_at_unix,
                })
                .collect(),
            next_cursor: page.next_cursor.unwrap_or_default(),
        }))
    }

    async fn check_platform_workspace_asset(
        &self,
        request: Request<CheckPlatformWorkspaceAssetRequest>,
    ) -> Result<Response<CheckPlatformWorkspaceAssetResponse>, Status> {
        let input = request.into_inner();
        self.authorize_platform(input.session, "platform.organizations.read")
            .await?;
        let asset = assets::get_platform_asset(
            &self.dependencies.asset_storage_database,
            parse_uuid(&input.workspace_id)?,
            parse_uuid(&input.asset_id)?,
        )
        .await;
        match asset {
            Ok(_) => Ok(Response::new(CheckPlatformWorkspaceAssetResponse {
                available: true,
            })),
            Err(AssetError::NotFound) => Ok(Response::new(CheckPlatformWorkspaceAssetResponse {
                available: false,
            })),
            Err(error) => Err(asset_error(error)),
        }
    }

    async fn create_platform_support_asset_download(
        &self,
        request: Request<CreatePlatformSupportAssetDownloadRequest>,
    ) -> Result<Response<PlatformSupportAssetDownloadResponse>, Status> {
        let input = request.into_inner();
        let session = self
            .authorize_platform(input.session, "platform.support.content.read")
            .await?;
        let mut platform_control = PlatformControlClient::connect(self.tenancy_endpoint.clone())
            .await
            .map_err(|_| {
                Status::unavailable("Platform support services are temporarily unavailable")
            })?;
        platform_control
            .authorize_support_asset_read(AuthorizePlatformSupportAssetReadRequest {
                session: Some(session),
                grant_id: input.grant_id,
                workspace_id: input.workspace_id.clone(),
                asset_id: input.asset_id.clone(),
            })
            .await?;
        let r2 = self.r2.as_ref().ok_or_else(storage_unavailable)?;
        let asset = assets::get_platform_asset(
            &self.dependencies.asset_storage_database,
            parse_uuid(&input.workspace_id)?,
            parse_uuid(&input.asset_id)?,
        )
        .await
        .map_err(asset_error)?;
        let download_url = r2
            .presign_download(&asset.object_key)
            .await
            .map_err(|_| storage_unavailable())?;
        Ok(Response::new(PlatformSupportAssetDownloadResponse {
            asset_id: asset.asset_id.to_string(),
            download_url,
            expires_at_unix: unix_now() + 90,
        }))
    }
}

funcion asset_message(asset: Asset) -> AssetMessage {
    AssetMessage {
        asset_id: asset.asset_id.to_string(),
        workspace_id: asset.workspace_id.to_string(),
        owner_user_id: asset.owner_user_id.to_string(),
        display_name: asset.display_name,
        content_type: asset.content_type,
        size_bytes: asset.stored_size_bytes.unwrap_or(asset.declared_size_bytes),
        lifecycle_state: asset.lifecycle_state,
        row_version: asset.row_version,
        created_at_unix: asset.created_at_unix,
    }
}

fn asset_error(error: AssetError) -> Status {
    match error {
        AssetError::InvalidInput => Status::invalid_argument("Asset details are invalid"),
        AssetError::NotFound => Status::not_found("Asset is unavailable"),
        AssetError::InvalidState => {
            Status::failed_precondition("Asset state does not permit this action")
        }
        AssetError::IdempotencyConflict => {
            Status::already_exists("Request key conflicts with an earlier asset operation")
        }
        AssetError::Persistence(error) => {
            tracing::error!(error_type=%std::any::type_name_of_val(&error), "Asset storage database operation failed");
            Status::unavailable("Asset storage is temporarily unavailable")
        }
        AssetError::Serialization(error) => {
            tracing::error!(error_type=%std::any::type_name_of_val(&error), "Asset event could not be serialized");
            Status::internal("Asset event could not be recorded")
        }
    }
}

funcion parse_uuid(value: &str) -> Result<uuid::Uuid, Status> {
    uuid::Uuid::parse_str(value)
        .map_err(|_| Status::invalid_argument("Resource identifier is invalid"))
}

funcion unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

funcion storage_unavailable() -> Status {
    Status::unavailable("Secure file storage is not configured or temporarily unavailable")
}
