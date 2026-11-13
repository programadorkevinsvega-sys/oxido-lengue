use tonic::Status;
use xovyra_platform::grpc::{
    AccountProfileResponse, AccountSessionRequest, AuthenticationAudience, LoginResponse,
};

use xovyra_identity::authentication::{
    AccountSessionContext, Audience, AuthenticationError, LoginSession,
};

pub(super) fn session_context(
    request: &AccountSessionRequest,
) -> Result<AccountSessionContext<'_>, Status> {
    let audience = match AuthenticationAudience::try_from(request.audience) {
        Ok(AuthenticationAudience::Tenant) => Audience::Tenant,
        Ok(AuthenticationAudience::Platform) => Audience::Platform,
        Ok(AuthenticationAudience::Unspecified) | Err(_) => {
            return Err(Status::invalid_argument(
                "Authentication audience is required",
            ));
        }
    };
    Ok(AccountSessionContext {
        session_secret: &request.session_secret,
        csrf_secret: &request.csrf_secret,
        audience,
        correlation_id: &request.correlation_id,
    })
}

pub(super) fn profile_response(
    profile: xovyra_identity::authentication::account::AccountProfile,
) -> AccountProfileResponse {
    AccountProfileResponse {
        user_id: profile.user_id.to_string(),
        email: profile.email,
        email_verified: profile.email_verified,
        display_name: profile.display_name,
        locale: profile.locale,
        timezone: profile.timezone,
        marketing_consent: profile.marketing_consent,
        updated_at_unix: profile.updated_at_unix,
    }
}

pub(super) fn login_response(session: LoginSession) -> LoginResponse {
    LoginResponse {
        session_id: session.session_id.to_string(),
        session_secret: session.session_secret,
        csrf_secret: session.csrf_secret,
        user_id: session.user_id.to_string(),
        idle_expires_at_unix: session.idle_expires_at,
        absolute_expires_at_unix: session.absolute_expires_at,
    }
}

pub(super) fn status(error: AuthenticationError) -> Status {
    match error {
        AuthenticationError::Rejected | AuthenticationError::InvalidSessionSecret => {
            Status::unauthenticated("Authentication was rejected")
        }
        AuthenticationError::RateLimited => Status::resource_exhausted("Rate limit exceeded"),
        AuthenticationError::EmailActionRateLimited { .. } => {
            Status::resource_exhausted("Email action rate limit exceeded")
        }
        AuthenticationError::InvalidProfile | AuthenticationError::InvalidCursor => {
            Status::invalid_argument("Account request is invalid")
        }
        AuthenticationError::PermissionDenied => {
            Status::permission_denied("Platform permission is required")
        }
        AuthenticationError::InvalidSourceAddress => {
            Status::invalid_argument("Request source is invalid")
        }
        error => {
            tracing::error!(error = %error, "Identity account operation failed");
            Status::internal("Identity operation failed")
        }
    }
}
