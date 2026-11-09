mod asset_deletion_worker;
mod asset_storage_outbox_relay;
mod asset_storage_service;
mod commercial_billing_outbox_relay;
mod commercial_billing_service;
mod identity_account_service;
mod identity_outbox_relay;
mod jobs_outbox_relay;
mod notification_inbox_service;
mod notifications_outbox_relay;
mod notifications_service;
mod platform_control_outbox_relay;
mod platform_control_service;
mod r2_storage;
mod readiness_service;
mod realtime_event_worker;
mod realtime_service;
mod scratch_cleanup_worker;
mod tenancy_invitation_worker;
mod tenancy_outbox_relay;
mod tenancy_service;
mod tenancy_teams_service;
mod tenant_jobs_service;
mod worker_auth;

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use tonic::transport::Server;
use xovyra_platform::configuration::{RuntimeConfiguration, load_environment_file};
use xovyra_platform::dependencies::{
    DependencyConfiguration, DependencyError, PlatformDependencies,
};
use xovyra_platform::grpc::asset_storage_server::AssetStorageServer;
use xovyra_platform::grpc::commercial_billing_server::CommercialBillingServer;
use xovyra_platform::grpc::identity_account_server::IdentityAccountServer;
use xovyra_platform::grpc::identity_authentication_server::IdentityAuthenticationServer;
use xovyra_platform::grpc::notification_inbox_server::NotificationInboxServer;
use xovyra_platform::grpc::notifications_delivery_server::NotificationsDeliveryServer;
use xovyra_platform::grpc::platform_control_server::PlatformControlServer;
use xovyra_platform::grpc::realtime_gateway_server::RealtimeGatewayServer;
use xovyra_platform::grpc::runtime_readiness_server::RuntimeReadinessServer;
use xovyra_platform::grpc::tenancy_access_server::TenancyAccessServer;
use xovyra_platform::grpc::tenancy_teams_server::TenancyTeamsServer;
use xovyra_platform::grpc::tenant_jobs_server::TenantJobsServer;
use xovyra_platform::observability;

const CONTEXTS: [xovyra_shared_kernel::BoundedContext; 19] = [
    xovyra_identity::module_public_api::bounded_context(),
    xovyra_tenancy_access::module_public_api::bounded_context(),
    xovyra_commercial_billing::module_public_api::bounded_context(),
    xovyra_usage_ledger::module_public_api::bounded_context(),
    xovyra_provider_catalog::module_public_api::bounded_context(),
    xovyra_integrations::module_public_api::bounded_context(),
    xovyra_projects::module_public_api::bounded_context(),
    xovyra_financial_research::module_public_api::bounded_context(),
    xovyra_generation::module_public_api::bounded_context(),
    xovyra_asset_storage::module_public_api::bounded_context(),
    xovyra_jobs::module_public_api::bounded_context(),
    xovyra_media_pipeline::module_public_api::bounded_context(),
    xovyra_approvals::module_public_api::bounded_context(),
    xovyra_publishing::module_public_api::bounded_context(),
    xovyra_analytics::module_public_api::bounded_context(),
    xovyra_notifications::module_public_api::bounded_context(),
    xovyra_realtime::module_public_api::bounded_context(),
    xovyra_audit::module_public_api::bounded_context(),
    xovyra_platform_control::module_public_api::bounded_context(),
];

#[tokio::main]
async fn main() -> ExitCode {
    let environment_result = load_environment_file();
    if let Err(error) = observability::initialize("info") {
        tracing::error!(error = %error, "Cannot initialize structured logging");
        return ExitCode::FAILURE;
    }
    if let Err(error) = environment_result {
        tracing::error!(error = %error, "Cannot load local environment configuration");
        return ExitCode::FAILURE;
    }
    let configuration = match RuntimeConfiguration::from_environment() {
        Ok(configuration) => configuration,
        Err(error) => {
            tracing::error!(error = %error, "Invalid runtime configuration");
            return ExitCode::FAILURE;
        }
    };
    match start(configuration).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(error = %error, "Module runtime stopped with an error");
            ExitCode::FAILURE
        }
    }
}

async fn start(configuration: RuntimeConfiguration) -> Result<(), RuntimeError> {
    let scratch = Arc::new(
        xovyra_jobs::scratch::ScratchSpace::new(
            &configuration.scratch_root,
            Duration::from_secs(configuration.scratch_retention_seconds),
        )
        .map_err(RuntimeError::ScratchConfiguration)?,
    );
    let dependencies = Arc::new(
        PlatformDependencies::connect(DependencyConfiguration {
            database_url: &configuration.database_url,
            identity_database_url: &configuration.identity_database_url,
            platform_control_database_url: &configuration.platform_control_database_url,
            tenancy_access_database_url: &configuration.tenancy_access_database_url,
            commercial_billing_database_url: &configuration.commercial_billing_database_url,
            notifications_database_url: &configuration.notifications_database_url,
            realtime_database_url: &configuration.realtime_database_url,
            asset_storage_database_url: &configuration.asset_storage_database_url,
            jobs_database_url: &configuration.jobs_database_url,
            redis_url: &configuration.redis_url,
            nats_url: &configuration.nats_url,
            nats_username: &configuration.nats_username,
            nats_password: &configuration.nats_password,
            contexts: &CONTEXTS,
        })
        .await
        .map_err(RuntimeError::Dependencies)?,
    );
    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_service_status("xovyra.runtime.v1", tonic_health::ServingStatus::Serving)
        .await;
    let readiness = RuntimeReadinessServer::new(readiness_service::ReadinessService::new(
        Arc::clone(&dependencies),
    ));
    let notifications_channel =
        tonic::transport::Endpoint::from_shared(configuration.internal_runtime_endpoint.clone())
            .map_err(RuntimeError::InternalEndpoint)?
            .connect_lazy();
    let notifications = NotificationsDeliveryServer::new(
        notifications_service::NotificationsDeliveryService::new(
            configuration.resend_api_key.clone(),
            configuration.resend_from_email.clone(),
            configuration.resend_webhook_secret.clone(),
            notifications_channel.clone(),
        )
        .map_err(RuntimeError::NotificationsConfiguration)?,
    );
    let notification_inbox = NotificationInboxServer::new(
        notification_inbox_service::NotificationInboxService::new(Arc::clone(&dependencies)),
    );
    let realtime = RealtimeGatewayServer::new(realtime_service::RealtimeGatewayService::new(
        Arc::clone(&dependencies),
        notifications_channel.clone(),
    ));
    let identity = IdentityAuthenticationServer::new(
        readiness_service::IdentityAuthenticationService::new(
            Arc::clone(&dependencies),
            configuration.rate_limit_hmac_secret.clone(),
            configuration.public_app_origin.clone(),
            notifications_channel.clone(),
            Duration::from_secs(configuration.session_idle_ttl_seconds),
            Duration::from_secs(configuration.session_absolute_ttl_seconds),
        )
        .map_err(RuntimeError::IdentityConfiguration)?,
    );
    let identity_account = IdentityAccountServer::new(
        identity_account_service::IdentityAccountService::new(
            Arc::clone(&dependencies),
            configuration.rate_limit_hmac_secret.clone(),
            Duration::from_secs(configuration.session_idle_ttl_seconds),
            Duration::from_secs(configuration.session_absolute_ttl_seconds),
        )
        .map_err(RuntimeError::IdentityConfiguration)?,
    );
    let platform_control = PlatformControlServer::new(
        platform_control_service::PlatformControlService::new(
            Arc::clone(&dependencies),
            configuration.rate_limit_hmac_secret.clone(),
            Duration::from_secs(configuration.session_idle_ttl_seconds),
            Duration::from_secs(configuration.session_absolute_ttl_seconds),
            configuration.internal_runtime_endpoint.clone(),
        )
        .map_err(RuntimeError::IdentityConfiguration)?,
    );
    let tenancy_access = TenancyAccessServer::new(tenancy_service::TenancyAccessService::new(
        Arc::clone(&dependencies),
        configuration.internal_runtime_endpoint.clone(),
    ));
    let tenancy_teams = TenancyTeamsServer::new(
        tenancy_teams_service::TenancyTeamsService::new(
            Arc::clone(&dependencies),
            configuration.tenancy_invitation_token_secret.clone(),
            notifications_channel.clone(),
        )
        .map_err(RuntimeError::TeamConfiguration)?,
    );
    let r2 = r2_storage::R2Storage::from_environment(
        configuration.r2_endpoint.as_deref(),
        configuration.r2_bucket.as_deref(),
        configuration.r2_access_key_id.as_deref(),
        configuration.r2_secret_access_key.as_deref(),
    )
    .map(Arc::new);
    let asset_storage = AssetStorageServer::new(asset_storage_service::AssetStorageService::new(
        Arc::clone(&dependencies),
        r2.clone(),
        configuration.internal_runtime_endpoint.clone(),
        configuration.asset_worker_auth_secret.clone(),
    ));
    let tenant_jobs = TenantJobsServer::new(tenant_jobs_service::TenantJobsService::new(
        Arc::clone(&dependencies),
        configuration.internal_runtime_endpoint.clone(),
    ));
    let commercial_billing =
        CommercialBillingServer::new(commercial_billing_service::CommercialBillingService::new(
            Arc::clone(&dependencies),
            notifications_channel,
            configuration.stripe_secret_key.clone(),
            configuration.stripe_webhook_secret.clone(),
            configuration.public_app_origin.clone(),
        ));
    let (relay_shutdown_sender, relay_shutdown_receiver) = tokio::sync::watch::channel(false);
    let relay_database = dependencies.identity_database.clone();
    let relay_jetstream = dependencies.jetstream.clone();
    let relay_task = tokio::spawn(identity_outbox_relay::run(
        relay_database,
        relay_jetstream,
        relay_shutdown_receiver.clone(),
    ));
    let tenancy_relay_database = dependencies.tenancy_access_database.clone();
    let tenancy_relay_jetstream = dependencies.jetstream.clone();
    let tenancy_relay_task = tokio::spawn(tenancy_outbox_relay::run(
        tenancy_relay_database,
        tenancy_relay_jetstream,
        relay_shutdown_receiver.clone(),
    ));
    let notifications_relay_task = tokio::spawn(notifications_outbox_relay::run(
        dependencies.notifications_database.clone(),
        dependencies.jetstream.clone(),
        relay_shutdown_receiver.clone(),
    ));
    let asset_relay_task = tokio::spawn(asset_storage_outbox_relay::run(
        dependencies.asset_storage_database.clone(),
        dependencies.jetstream.clone(),
        relay_shutdown_receiver.clone(),
    ));
    let jobs_relay_task = tokio::spawn(jobs_outbox_relay::run(
        dependencies.jobs_database.clone(),
        dependencies.jetstream.clone(),
        relay_shutdown_receiver.clone(),
    ));
    let platform_control_relay_task = tokio::spawn(platform_control_outbox_relay::run(
        dependencies.platform_control_database.clone(),
        dependencies.jetstream.clone(),
        relay_shutdown_receiver.clone(),
    ));
    let commercial_billing_relay_task = tokio::spawn(commercial_billing_outbox_relay::run(
        dependencies.commercial_billing_database.clone(),
        dependencies.jetstream.clone(),
        relay_shutdown_receiver.clone(),
    ));
    let asset_worker_task = tokio::spawn(asset_deletion_worker::run(
        Arc::clone(&dependencies),
        r2,
        Arc::clone(&scratch),
        configuration.internal_runtime_endpoint.clone(),
        configuration.asset_worker_auth_secret.clone(),
        relay_shutdown_receiver.clone(),
    ));
    let scratch_worker_task = tokio::spawn(scratch_cleanup_worker::run(
        scratch.as_ref().clone(),
        relay_shutdown_receiver.clone(),
    ));
    let invitation_worker_task = tokio::spawn(tenancy_invitation_worker::run(
        Arc::clone(&dependencies),
        configuration.internal_runtime_endpoint.clone(),
        configuration.public_app_origin.clone(),
        relay_shutdown_receiver.clone(),
    ));
    let realtime_worker_task = tokio::spawn(realtime_event_worker::run(
        Arc::clone(&dependencies),
        relay_shutdown_receiver.clone(),
    ));
    tracing::info!(address = %configuration.bind_address, context_count = CONTEXTS.len(), "Starting module runtime");
    let server_result = Server::builder()
        .layer(xovyra_platform::grpc_telemetry::GrpcTelemetryLayer)
        .add_service(health_service)
        .add_service(notifications)
        .add_service(notification_inbox)
        .add_service(realtime)
        .add_service(identity)
        .add_service(identity_account)
        .add_service(platform_control)
        .add_service(tenancy_access)
        .add_service(tenancy_teams)
        .add_service(asset_storage)
        .add_service(tenant_jobs)
        .add_service(commercial_billing)
        .add_service(readiness)
        .serve_with_shutdown(configuration.bind_address, shutdown())
        .await;
    relay_shutdown_sender.send_replace(true);
    relay_task.await.map_err(RuntimeError::OutboxTask)?;
    tenancy_relay_task
        .await
        .map_err(RuntimeError::TenancyOutboxTask)?;
    notifications_relay_task
        .await
        .map_err(RuntimeError::NotificationsOutboxTask)?;
    asset_relay_task
        .await
        .map_err(RuntimeError::AssetStorageOutboxTask)?;
    jobs_relay_task
        .await
        .map_err(RuntimeError::JobsOutboxTask)?;
    platform_control_relay_task
        .await
        .map_err(RuntimeError::PlatformControlOutboxTask)?;
    commercial_billing_relay_task
        .await
        .map_err(RuntimeError::CommercialBillingOutboxTask)?;
    asset_worker_task
        .await
        .map_err(RuntimeError::AssetWorkerTask)?;
    scratch_worker_task
        .await
        .map_err(RuntimeError::ScratchWorkerTask)?;
    invitation_worker_task
        .await
        .map_err(RuntimeError::InvitationWorkerTask)?;
    realtime_worker_task
        .await
        .map_err(RuntimeError::RealtimeWorkerTask)?;
    server_result.map_err(RuntimeError::Grpc)?;
    dependencies.database.close().await;
    Ok(())
}

async fn shutdown() {
    xovyra_platform::shutdown::wait_for_termination().await;
}

#[derive(Debug, thiserror::Error)]
enum RuntimeError {
    #[error("critical dependency initialization failed")]
    Dependencies(#[source] DependencyError),
    #[error("gRPC runtime failed")]
    Grpc(#[source] tonic::transport::Error),
    #[error("identity authentication service could not be configured")]
    IdentityConfiguration(#[source] xovyra_identity::authentication::AuthenticationError),
    #[error("workspace team security configuration is invalid")]
    TeamConfiguration(String),
    #[error("internal module runtime endpoint is invalid")]
    InternalEndpoint(#[source] tonic::transport::Error),
    #[error("notifications provider configuration is invalid")]
    NotificationsConfiguration(#[source] xovyra_notifications::resend::ResendError),
    #[error("identity Outbox relay task failed")]
    OutboxTask(#[source] tokio::task::JoinError),
    #[error("tenancy Outbox relay task failed")]
    TenancyOutboxTask(#[source] tokio::task::JoinError),
    #[error("workspace invitation notification worker task failed")]
    InvitationWorkerTask(#[source] tokio::task::JoinError),
    #[error("realtime event worker task failed")]
    RealtimeWorkerTask(#[source] tokio::task::JoinError),
    #[error("notifications Outbox relay task failed")]
    NotificationsOutboxTask(#[source] tokio::task::JoinError),
    #[error("asset storage Outbox relay task failed")]
    AssetStorageOutboxTask(#[source] tokio::task::JoinError),
    #[error("tenant jobs Outbox relay task failed")]
    JobsOutboxTask(#[source] tokio::task::JoinError),
    #[error("platform control Outbox relay task failed")]
    PlatformControlOutboxTask(#[source] tokio::task::JoinError),
    #[error("commercial billing Outbox relay task failed")]
    CommercialBillingOutboxTask(#[source] tokio::task::JoinError),
    #[error("asset deletion worker task failed")]
    AssetWorkerTask(#[source] tokio::task::JoinError),
    #[error("scratch workspace could not be configured")]
    ScratchConfiguration(#[source] xovyra_jobs::scratch::ScratchError),
    #[error("scratch cleanup worker task failed")]
    ScratchWorkerTask(#[source] tokio::task::JoinError),
}
