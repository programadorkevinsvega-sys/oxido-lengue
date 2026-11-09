use tonic::Status;
use uuid::Uuid;
use xovyra_platform::grpc::identity_authentication_client::IdentityAuthenticationClient;
use xovyra_platform::grpc::tenancy_teams_client::TenancyTeamsClient;
use xovyra_platform::grpc::{
    AccountSessionRequest, AuthenticateSessionRequest, AuthenticationAudience,
    CheckWorkspacePermissionRequest,
};
pub async fn authorize_workspace(
    channel: tonic::transport::Channel,
    session: Option<AccountSessionRequest>,
    workspace: &str,
    permission: &str,
) -> Result<Uuid, Status> {
    let session = session.ok_or_else(|| Status::unauthenticated("Tenant session is required"))?;
    if AuthenticationAudience::try_from(session.audience).ok()
        != Some(AuthenticationAudience::Tenant)
    {
        return Err(Status::unauthenticated("Tenant session is required"));
    }
    let workspace_id = Uuid::parse_str(workspace)
        .map_err(|_| Status::invalid_argument("Workspace identifier is invalid"))?;
    let mut identity = IdentityAuthenticationClient::new(channel.clone());
    let authenticated = identity
        .authenticate_session(AuthenticateSessionRequest {
            session_secret: session.session_secret,
            audience: AuthenticationAudience::Tenant as i32,
            correlation_id: session.correlation_id.clone(),
            csrf_secret: session.csrf_secret,
        })
        .await
        .map_err(|status| match status.code() {
            tonic::Code::Unauthenticated => Status::unauthenticated("Tenant session was rejected"),
            _ => Status::unavailable("Identity service is temporarily unavailable"),
        })?
        .into_inner();
    let actor = Uuid::parse_str(&authenticated.user_id)
        .map_err(|_| Status::internal("Authenticated user identifier is invalid"))?;
    let mut tenancy = TenancyTeamsClient::new(channel);
    let permission = tenancy
        .check_workspace_permission(CheckWorkspacePermissionRequest {
            workspace_id: workspace_id.to_string(),
            actor_user_id: actor.to_string(),
            permission: permission.to_owned(),
            correlation_id: session.correlation_id,
        })
        .await
        .map_err(|status| {
            tracing::error!(
                event = "billing.authorization.dependency_failed", component =
                "commercial_billing", operation = "check_workspace_permission",
                dependency = "tenancy_access", grpc_code =? status.code()
            );
            Status::unavailable("Workspace authorization is temporarily unavailable")
        })?
        .into_inner();
    if !permission.allowed {
        return Err(Status::permission_denied(
            "Workspace billing permission is required",
        ));
    }
    Ok(workspace_id)
}
