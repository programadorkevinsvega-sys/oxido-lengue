#[path = "identity_account/mod.rs"]
mod identity_account;
use std::sync::Arc;
use std::time::Duration;
use tonic::{Request, Response, Status};
use xovyra_platform::dependencies::PlatformDependencies;
use xovyra_platform::grpc::identity_account_server::IdentityAccount;
use xovyra_platform::grpc::{
    AccountProfileResponse, AccountSessionRequest, ChangeAccountPasswordRequest,
    ListAccountSecurityEventsRequest, ListAccountSecurityEventsResponse,
    ListAccountSessionsRequest, ListAccountSessionsResponse, PlatformPermissionsResponse,
    RevokeAccountSessionRequest, RevokeAccountSessionResponse, RevokeOtherAccountSessionsResponse,
    UpdateAccountProfileRequest,
};
pub struct IdentityAccountService {
    pub(super) identity: xovyra_identity::authentication::IdentityService,
}
impl IdentityAccountService {
    pub fn new(
        dependencies: Arc<PlatformDependencies>,
        rate_limit_hmac_secret: String,
        idle_session_lifetime: Duration,
        absolute_session_lifetime: Duration,
    ) -> Result<Self, xovyra_identity::authentication::AuthenticationError> {
        Ok(Self {
            identity: xovyra_identity::authentication::IdentityService::new(
                dependencies.identity_database.clone(),
                dependencies.cache.clone(),
                rate_limit_hmac_secret,
                idle_session_lifetime,
                absolute_session_lifetime,
            )?,
        })
    }
}
#[tonic::async_trait]
impl IdentityAccount for IdentityAccountService {
    async fn get_profile(
        &self,
        request: Request<AccountSessionRequest>,
    ) -> Result<Response<AccountProfileResponse>, Status> {
        identity_account::profile::get(&self.identity, request.into_inner()).await
    }
    async fn update_profile(
        &self,
        request: Request<UpdateAccountProfileRequest>,
    ) -> Result<Response<AccountProfileResponse>, Status> {
        identity_account::profile::update(&self.identity, request.into_inner()).await
    }
    async fn list_sessions(
        &self,
        request: Request<ListAccountSessionsRequest>,
    ) -> Result<Response<ListAccountSessionsResponse>, Status> {
        identity_account::sessions::list(&self.identity, request.into_inner()).await
    }
    async fn revoke_session(
        &self,
        request: Request<RevokeAccountSessionRequest>,
    ) -> Result<Response<RevokeAccountSessionResponse>, Status> {
        identity_account::sessions::revoke(&self.identity, request.into_inner()).await
    }
    async fn revoke_other_sessions(
        &self,
        request: Request<AccountSessionRequest>,
    ) -> Result<Response<RevokeOtherAccountSessionsResponse>, Status> {
        identity_account::sessions::revoke_others(&self.identity, request.into_inner()).await
    }
    async fn change_password(
        &self,
        request: Request<ChangeAccountPasswordRequest>,
    ) -> Result<Response<xovyra_platform::grpc::LoginResponse>, Status> {
        identity_account::profile::change_password(&self.identity, request.into_inner()).await
    }
    async fn list_security_events(
        &self,
        request: Request<ListAccountSecurityEventsRequest>,
    ) -> Result<Response<ListAccountSecurityEventsResponse>, Status> {
        identity_account::security_events::list(&self.identity, request.into_inner()).await
    }
    async fn get_platform_permissions(
        &self,
        request: Request<AccountSessionRequest>,
    ) -> Result<Response<PlatformPermissionsResponse>, Status> {
        identity_account::platform_permissions::get(&self.identity, request.into_inner()).await
    }
}
