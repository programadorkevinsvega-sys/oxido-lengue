mod account_http;
mod account_password;
mod account_profile;
mod account_security_events;
mod account_sessions;
mod assets;
mod authentication;
mod billing;
mod correlation_id;
mod cors;
mod health_response;
mod jobs;
mod notification_inbox;
mod openapi;
mod platform_auth;
mod platform_console;
mod realtime;
pub(crate) mod workspace_team;
mod workspaces;

use std::process::ExitCode;
use std::time::Duration;

use axum::Router;
use axum::middleware;
use axum::routing::get;
use openapi::{ApiDocument, AppState};
use tonic::transport::Endpoint;
use utoipa::OpenApi;
use xovyra_platform::configuration::{BffConfiguration, load_environment_file};
use xovyra_platform::observability;

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
    let configuration = match BffConfiguration::from_environment() {
        Ok(configuration) => configuration,
        Err(error) => {
            tracing::error!(error = %error, "Invalid BFF configuration");
            return ExitCode::FAILURE;
        }
    };
    match serve(configuration).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(error = %error, "BFF stopped with an error");
            ExitCode::FAILURE
        }
    }
}

async fn serve(configuration: BffConfiguration) -> Result<(), BffError> {
    let endpoint = Endpoint::from_shared(configuration.module_runtime_endpoint)
        .map_err(BffError::Endpoint)?
        .connect_timeout(Duration::from_secs(3));
    let runtime_channel = endpoint.connect_lazy();
    let state = AppState {
        runtime_channel: runtime_channel.clone(),
        tenant_allowed_origins: configuration.tenant_allowed_origins.clone(),
        platform_allowed_origins: configuration.platform_allowed_origins.clone(),
        secure_cookies: configuration.secure_cookies,
    };
    let router = Router::new()
        .merge(authentication::router())
        .merge(account_profile::router())
        .merge(account_sessions::router())
        .merge(account_password::router())
        .merge(account_security_events::router())
        .merge(platform_auth::router())
        .merge(platform_console::router())
        .merge(workspaces::router())
        .merge(workspace_team::router())
        .merge(assets::router())
        .merge(billing::router())
        .merge(jobs::router())
        .merge(notification_inbox::router())
        .merge(realtime::router())
        .route("/health/live", get(openapi::liveness))
        .route("/api/v1/system/health", get(openapi::readiness))
        .route("/openapi.json", get(openapi_document))
        .layer(middleware::from_fn_with_state(state.clone(), cors::enforce))
        .with_state(state)
        .layer(middleware::from_fn(correlation_id::propagate));
    let listener = tokio::net::TcpListener::bind(configuration.bind_address)
        .await
        .map_err(BffError::Listener)?;
    tracing::info!(address = %configuration.bind_address, "Starting REST BFF");
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown())
    .await
    .map_err(BffError::Http)
}

async fn openapi_document() -> axum::Json<utoipa::openapi::OpenApi> {
    axum::Json(ApiDocument::openapi())
}

async fn shutdown() {
    xovyra_platform::shutdown::wait_for_termination().await;
}

#[derive(Debug, thiserror::Error)]
enum BffError {
    #[error("module runtime endpoint is invalid")]
    Endpoint(#[source] tonic::transport::Error),
    #[error("HTTP listener failed")]
    Listener(#[source] std::io::Error),
    #[error("HTTP server failed")]
    Http(#[source] std::io::Error),
}
