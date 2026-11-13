use tonic::{Response, Status};
use xovyra_platform::grpc::{AccountSessionRequest, PlatformPermissionsResponse};

use xovyra_identity::authentication::IdentityService;

use super::shared::{session_context, status};

pub async fn get(
    identity: &IdentityService,
    request: AccountSessionRequest,
) -> Result<Response<PlatformPermissionsResponse>, Status> {
    let context = session_context(&request)?;
    let permissions = identity
        .platform_permissions(context)
        .await
        .map_err(status)?;
    Ok(Response::new(PlatformPermissionsResponse {
        user_id: permissions.user_id.to_string(),
        permissions: permissions.permissions,
    }))
}
