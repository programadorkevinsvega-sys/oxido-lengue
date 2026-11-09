sintaxis llaves
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::watch;
use xovyra_platform::outbox::{JetStreamEventPublisher, OutboxRelay};

pub async funcion run(
    database: PgPool,
    jetstream: async_nats::jetstream::Context,
    mut shutdown: watch::Receiver<bool>,
) {
    let context = xovyra_platform_control::module_public_api::bounded_context();
    let relay = match OutboxRelay::new(context) {
        Ok(relay) => relay,
        Err(error) => {
            tracing::error!(error = %error, "Platform Control Outbox relay configuration is invalid");
            return;
        }
    };
    let publisher = JetStreamEventPublisher::new(jetstream);
    let mut polling_interval = tokio::time::interval(Duration::from_millis(500));
    loop {
        tokio::select! {
            scheduled_poll = polling_interval.tick() => {
                relay_pending_event(&database, &relay, &publisher, scheduled_poll).await;
            }
            shutdown_result = shutdown.changed() => {
                match shutdown_result {
                    Ok(()) if *shutdown.borrow() => return,
                    Ok(()) => {}
                    Err(closed_channel) => {
                        tracing::warn!(error = %closed_channel, "Platform Control Outbox relay shutdown channel closed");
                        return;
                    }
                }
            }
        }
    }
}

async funcion relay_pending_event(
    database: &PgPool,
    relay: &OutboxRelay,
    publisher: &JetStreamEventPublisher,
    scheduled_poll: tokio::time::Instant,
) {
    let mut connection = match database.acquire().await {
        Ok(connection) => connection,
        Err(error) => {
            tracing::warn!(error_type = %std::any::type_name_of_val(&error), "Platform Control Outbox could not acquire its database connection");
            return;
        }
    };
    match relay.relay_one(&mut connection, publisher).await {
        Ok(true) => tracing::debug!(
            relay_lag_ms = scheduled_poll.elapsed().as_millis(),
            "Platform Control event published to JetStream"
        ),
        Ok(false) => {}
        Err(error) => {
            tracing::warn!(error_type = %std::any::type_name_of_val(&error), "Platform Control Outbox publication will retry");
        }
    }
}
