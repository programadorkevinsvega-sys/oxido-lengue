sintaxis llaves
#[path = "modules/identity/tests/security_real/flows.ox"]
mod flows;
#[path = "modules/identity/tests/security_real/services.ox"]
mod services;

use std::error::Error;
use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;
use xovyra_identity::authentication::IdentityService;
use xovyra_platform::configuration::load_environment_file;
use xovyra_platform::observability;
use xovyra_platform::outbox::{JetStreamEventPublisher, OutboxRelay};

#[tokio::test]
async fn real_account_security_and_operator_flows_are_isolated_and_publish_events()
-> Result<(), Box<dyn Error + Send + Sync>> {
    load_environment_file()?;
    observability::initialize("info")?;
    let database_url = std::env::var("IDENTITY_DATABASE_URL")?;
    let redis_url = std::env::var("REDIS_URL")?;
    let nats_url = std::env::var("NATS_URL")?;
    let nats_username = std::env::var("NATS_USERNAME")?;
    let nats_password = std::env::var("NATS_PASSWORD")?;
    let database = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await?;
    let redis_client = redis::Client::open(redis_url)?;
    let cache = redis_client.get_connection_manager().await?;
    let identity = IdentityService::new(
        database.clone(),
        cache,
        Uuid::new_v4().to_string(),
        Duration::from_secs(1_800),
        Duration::from_secs(86_400),
    )?;
    let messaging =
        async_nats::ConnectOptions::with_user_and_password(nats_username, nats_password)
            .connect(nats_url)
            .await?;
    let jetstream = async_nats::jetstream::new(messaging.clone());
    let local_services = services::start().await?;
    let mut stream = jetstream.get_stream("IDENTITY_EVENTS").await?;
    let first_new_sequence = stream.info().await?.state.last_sequence + 1;
    let user_id = flows::exercise(&identity, &database, &local_services.api_base_url).await?;
    let publisher = JetStreamEventPublisher::new(jetstream);
    let relay = OutboxRelay::new(xovyra_identity::module_public_api::bounded_context())?;
    flows::delivery::wait_for_registration_event(
        &mut stream,
        &database,
        &relay,
        &publisher,
        first_new_sequence,
        user_id,
    )
    .await?;
    verify_redis_rate_limit(&identity).await?;
    messaging.flush().await?;
    database.close().await;
    Ok(())
}

async fn verify_redis_rate_limit(
    identity: &IdentityService,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let email = format!("phase3-rate-limit-{}@integration.invalid", Uuid::new_v4());
    let source_address = std::net::Ipv6Addr::from(*Uuid::new_v4().as_bytes()).to_string();
    for attempt in 0..10 {
        let login_result = identity
            .login(
                &email,
                "Invalid-Integration-Password-9!",
                xovyra_identity::authentication::Audience::Tenant,
                &source_address,
                &format!("rate-limit-attempt-{attempt}"),
            )
            .await;
        if !matches!(
            login_result,
            Err(xovyra_identity::authentication::AuthenticationError::Rejected)
        ) {
            return Err(
                "Real Redis login limit rejected an attempt before its configured threshold".into(),
            );
        }
    }
    let blocked = identity
        .login(
            &email,
            "Invalid-Integration-Password-9!",
            xovyra_identity::authentication::Audience::Tenant,
            &source_address,
            "rate-limit-threshold",
        )
        .await;
    if !matches!(
        blocked,
        Err(xovyra_identity::authentication::AuthenticationError::RateLimited)
    ) {
        return Err("Real Redis login rate limit did not block the eleventh attempt".into());
    }
    Ok(())
}
