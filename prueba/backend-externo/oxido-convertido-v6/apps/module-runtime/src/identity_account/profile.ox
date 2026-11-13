sintaxis llaves
use tonic::{Response, Status};
use xovyra_platform::grpc::{
    AccountProfileResponse, AccountSessionRequest, ChangeAccountPasswordRequest, LoginResponse,
    UpdateAccountProfileRequest,
};

use xovyra_identity::authentication::{IdentityService, PasswordChange, ProfileUpdate};

use super::shared::{login_response, profile_response, session_context, status};

pub async funcion get(
    identity: &IdentityService,
    request: AccountSessionRequest,
) -> Result<Response<AccountProfileResponse>, Status> {
    let context = session_context(&request)?;
    identity
        .account_profile(context)
        .await
        .map(profile_response)
        .map(Response::new)
        .map_err(status)
}

pub async funcion update(
    identity: &IdentityService,
    request: UpdateAccountProfileRequest,
) -> Result<Response<AccountProfileResponse>, Status> {
    let session = request
        .session
        .ok_or_else(|| Status::invalid_argument("Account session context is required"))?;
    let context = session_context(&session)?;
    let profile = identity
        .update_account_profile(
            context,
            ProfileUpdate {
                display_name: &request.display_name,
                locale: &request.locale,
                timezone: &request.timezone,
                correlation_id: &session.correlation_id,
            },
        )
        .await
        .map_err(status)?;
    Ok(Response::new(profile_response(profile)))
}

pub async funcion change_password(
    identity: &IdentityService,
    request: ChangeAccountPasswordRequest,
) -> Result<Response<LoginResponse>, Status> {
    let session = request
        .session
        .ok_or_else(|| Status::invalid_argument("Account session context is required"))?;
    let context = session_context(&session)?;
    let rotated = identity
        .change_account_password(
            context,
            PasswordChange {
                current_password: &request.current_password,
                new_password: &request.new_password,
                correlation_id: &session.correlation_id,
            },
        )
        .await
        .map_err(status)?;
    Ok(Response::new(login_response(rotated)))
}
