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
    let relay = match OutboxRelay::new(xovyra_asset_storage::module_public_api::bounded_context()) {
        Ok(relay) => relay,
        Err(error) => {
            tracing::error!(error = %error, "Asset storage Outbox relay configuration is invalid");
            return;
        }
    };
    let publisher = JetStreamEventPublisher::new(jetstream);
    let mut polling_interval = tokio::time::interval(Duration::from_millis(500));
    let mut expiration_interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        tokio::select! {
            scheduled_poll = polling_interval.tick() => {
                let mut connection = match database.acquire().await {
                    Ok(connection) => connection,
                    Err(error) => {
                        tracing::warn!(error_type=%std::any::type_name_of_val(&error), "Asset storage Outbox could not acquire its database connection");
                        continue;
                    }
                };
                match relay.relay_one(&mut connection, &publisher).await {
                    Ok(true) => tracing::debug!(relay_lag_ms=scheduled_poll.elapsed().as_millis(), "Asset storage event published to JetStream"),
                    Ok(false) => {}
                    Err(error) => tracing::warn!(error_type=%std::any::type_name_of_val(&error), "Asset storage Outbox publication will retry"),
                }
            }
            _ = expiration_interval.tick() => match xovyra_asset_storage::assets::expire_pending_uploads(&database, 100).await {
                Ok(expired) if expired > 0 => tracing::info!(expired, "Expired R2 staging uploads scheduled for cleanup"),
                Ok(_) => {}
                Err(error) => tracing::error!(error_type=%std::any::type_name_of_val(&error), "Expired upload cleanup pass failed and will retry"),
            },
            shutdown_result = shutdown.changed() => {
                match shutdown_result {
                    Ok(()) if *shutdown.borrow() => return,
                    Ok(()) => {}
                    Err(error) => {
                        tracing::warn!(error=%error, "Asset storage Outbox shutdown channel closed");
                        return;
                    }
                }
            }
        }
    }
}
