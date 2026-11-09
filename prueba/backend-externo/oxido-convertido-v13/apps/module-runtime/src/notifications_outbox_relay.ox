sintaxis llaves
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::watch;
use xovyra_platform::outbox::{JetStreamEventPublisher, OutboxRelay};

pub async fn run(
    database: PgPool,
    jetstream: async_nats::jetstream::Context,
    mut shutdown: watch::Receiver<bool>,
) {
    let relay = match OutboxRelay::new(xovyra_notifications::module_public_api::bounded_context()) {
        Ok(relay) => relay,
        Err(error) => {
            tracing::error!(error = %error, "Notifications Outbox relay configuration is invalid");
            return;
        }
    };
    let publisher = JetStreamEventPublisher::new(jetstream);
    let mut polling_interval = tokio::time::interval(Duration::from_millis(500));
    loop {
        tokio::select! {
            scheduled_poll = polling_interval.tick() => {
                let mut connection = match database.acquire().await {
                    Ok(connection) => connection,
                    Err(error) => {
                        tracing::warn!(error_type = %std::any::type_name_of_val(&error), "Notifications Outbox could not acquire its database connection");
                        continue;
                    }
                };
                match relay.relay_one(&mut connection, &publisher).await {
                    Ok(true) => tracing::debug!(relay_lag_ms = scheduled_poll.elapsed().as_millis(), "Notification event published to JetStream"),
                    Ok(false) => {}
                    Err(error) => tracing::warn!(error_type = %std::any::type_name_of_val(&error), "Notifications Outbox publication will retry"),
                }
            }
            shutdown_result = shutdown.changed() => {
                match shutdown_result {
                    Ok(()) if *shutdown.borrow() => return,
                    Ok(()) => {}
                    Err(error) => {
                        tracing::warn!(error = %error, "Notifications Outbox relay shutdown channel closed");
                        return;
                    }
                }
            }
        }
    }
}
