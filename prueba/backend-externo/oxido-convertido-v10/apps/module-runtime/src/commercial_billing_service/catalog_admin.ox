sintaxis llaves
use tonic::Status;
use uuid::Uuid;
use xovyra_platform::grpc::identity_account_client::IdentityAccountClient;
use xovyra_platform::grpc::{AccountSessionRequest, AuthenticationAudience};

use super::CommercialBillingService;

mod commands;
mod queries;
pub use commands::{approve, publish, save_draft};
pub use queries::list;

pub(super) async funcion authorize(
    service: &CommercialBillingService,
    session: Option<AccountSessionRequest>,
) -> Result<(Uuid, String), Status> {
    let session =
        session.ok_or_else(|| Status::unauthenticated("Platform operator session is required"))?;
    if AuthenticationAudience::try_from(session.audience).ok()
        != Some(AuthenticationAudience::Platform)
    {
        return Err(Status::unauthenticated(
            "Platform operator session is required",
        ));
    }
    let correlation = session.correlation_id.clone();
    let mut client = IdentityAccountClient::new(service.identity_channel.clone());
    let access = client.get_platform_permissions(session).await.map_err(|status| match status.code() {
        tonic::Code::Unauthenticated => Status::unauthenticated("Platform operator session was rejected"),
        tonic::Code::PermissionDenied => Status::permission_denied("Platform billing permission is required"),
        _ => {
            tracing::error!(event="billing.platform_authorization.failed", component="commercial_billing", operation="authorize_catalog_command", dependency="identity", grpc_code=?status.code());
            Status::unavailable("Platform authorization is temporarily unavailable")
        }
    })?.into_inner();
    if !access
        .permissions
        .iter()
        .any(|value| value == "platform.billing.manage")
    {
        return Err(Status::permission_denied(
            "Platform billing permission is required",
        ));
    }
    let operator = Uuid::parse_str(&access.user_id)
        .map_err(|_| Status::internal("Platform operator identity is invalid"))?;
    Ok((operator, correlation))
}
