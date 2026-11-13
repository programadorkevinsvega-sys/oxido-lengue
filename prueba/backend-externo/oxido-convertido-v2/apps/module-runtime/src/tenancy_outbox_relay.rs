use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::watch;
use xovyra_platform::outbox::{JetStreamEventPublisher, OutboxRelay};

pub async fn run(
    database: PgPool,
    jetstream: async_nats::jetstream::Context,
    mut shutdown: watch::Receiver<bool>,
) {
    let context = xovyra_tenancy_access::module_public_api::bounded_context();
    let relay = match OutboxRelay::new(context) {
        Ok(relay) => relay,
        Err(error) => {
            tracing::error!(error = %error, "Tenancy Outbox relay configuration is invalid");
            return;
        }
    };
    let publisher = JetStreamEventPublisher::new(jetstream);
    let mut polling_interval = tokio::time::interval(Duration::from_millis(500));
    let mut retention_interval = tokio::time::interval(Duration::from_secs(10));
    loop {
        tokio::select! {
            scheduled_poll = polling_interval.tick() => {
                let mut connection = match database.acquire().await {
                    Ok(connection) => connection,
                    Err(error) => {
                        tracing::warn!(error_type = %std::any::type_name_of_val(&error), "Tenancy Outbox could not acquire its database connection");
                        continue;
                    }
                };
                match relay.relay_one(&mut connection, &publisher).await {
                    Ok(true) => tracing::debug!(relay_lag_ms = scheduled_poll.elapsed().as_millis(), "Tenancy event published to JetStream"),
                    Ok(false) => {}
                    Err(error) => tracing::warn!(error_type = %std::any::type_name_of_val(&error), "Tenancy Outbox publication will retry"),
                }
            }
            _ = retention_interval.tick() => {
                match xovyra_tenancy_access::workspaces::purge_due_workspaces(&database, None).await {
                    Ok(purged) if purged > 0 => tracing::info!(purged, "Expired tenant workspaces were permanently removed"),
                    Ok(_) => {}
                    Err(error) => tracing::error!(error_type = %std::any::type_name_of_val(&error), "Workspace retention pass failed and will retry"),
                }
            }
            shutdown_result = shutdown.changed() => {
                match shutdown_result {
                    Ok(()) if *shutdown.borrow() => return,
                    Ok(()) => {}
                    Err(error) => {
                        tracing::warn!(error = %error, "Tenancy Outbox relay shutdown channel closed");
                        return;
                    }
                }
            }
        }
    }
}
