use sqlx::PgPool;
use std::time::Duration;
use tokio::sync::watch;
use xovyra_platform::outbox::{JetStreamEventPublisher, OutboxRelay};
pub async fn run(
    database: PgPool,
    jetstream: async_nats::jetstream::Context,
    mut shutdown: watch::Receiver<bool>,
) {
    let relay =
        match OutboxRelay::new(xovyra_commercial_billing::module_public_api::bounded_context()) {
            Ok(relay) => relay,
            Err(error) => {
                tracing::error!(
                    event = "commercial_billing.outbox.configuration_failed", component =
                    "module_runtime", operation = "configure_billing_relay", error_code =
                    "outbox_context_invalid", error =% error
                );
                return;
            }
        };
    let publisher = JetStreamEventPublisher::new(jetstream);
    let mut interval = tokio::time::interval(Duration::from_millis(500));
    loop {
        tokio::select! {
            scheduled = interval.tick() => { let mut connection = match database
            .acquire(). await { Ok(connection) => connection, Err(error) => {
            tracing::error!(event = "commercial_billing.outbox.database_unavailable",
            component = "module_runtime", operation = "acquire_billing_outbox",
            dependency = "postgresql", elapsed_ms = scheduled.elapsed().as_millis(),
            error_type =% std::any::type_name_of_val(& error)); continue; } }; match
            relay.relay_one(& mut connection, & publisher). await { Ok(true) =>
            tracing::debug!(event = "commercial_billing.outbox.event_published",
            component = "module_runtime", operation = "relay_billing_event", dependency =
            "jetstream"), Ok(false) => {} Err(error) => tracing::error!(event =
            "commercial_billing.outbox.publish_failed", component = "module_runtime",
            operation = "relay_billing_event", dependency = "jetstream", error =% error),
            } } result = shutdown.changed() => match result { Ok(()) if * shutdown
            .borrow() => return, Ok(()) => {} Err(error) => { tracing::error!(event =
            "commercial_billing.outbox.shutdown_failed", component = "module_runtime",
            operation = "stop_billing_relay", error =% error); return; } }
        }
    }
}
