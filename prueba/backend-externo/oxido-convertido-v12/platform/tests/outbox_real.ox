sintaxis llaves
use std::error::Error;
use std::time::Duration;

use async_nats::jetstream;
use async_nats::jetstream::consumer::pull::Config;
use futures_util::StreamExt;
use sqlx::Connection;
use sqlx::PgConnection;
use sqlx::Row;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;
use xovyra_platform::configuration::load_environment_file;
use xovyra_platform::observability;
use xovyra_platform::outbox::{JetStreamEventPublisher, OutboxRelay, OutboxWriter};
use xovyra_shared_kernel::BoundedContext;

const IDENTITY: BoundedContext = BoundedContext {
    name: "identity",
    schema: "identity",
};

#[tokio::test]
async fn outbox_publishes_confirms_propagates_and_marks_real_events()
-> Result<(), Box<dyn Error + Send + Sync>> {
    load_environment_file()?;
    observability::initialize("info")?;
    let database_url = std::env::var("MIGRATOR_DATABASE_URL")?;
    let nats_url = std::env::var("NATS_URL")?;
    let nats_username = std::env::var("NATS_USERNAME")?;
    let nats_password = std::env::var("NATS_PASSWORD")?;
    let messaging =
        async_nats::ConnectOptions::with_user_and_password(nats_username, nats_password)
            .connect(nats_url)
            .await?;
    let jetstream = jetstream::new(messaging.clone());
    let identifier = Uuid::new_v4().simple().to_string();
    let event_id = Uuid::new_v4().to_string();
    let event_type = format!("identity.verification_{identifier}.completed");
    let subject = format!("{event_type}.v1");
    let stream = jetstream.get_stream("IDENTITY_EVENTS").await?;
    let publisher = JetStreamEventPublisher::new(jetstream.clone());
    let mut connection = PgConnection::connect(&database_url).await?;
    sqlx::query("SET ROLE xovyra_identity_owner")
        .execute(&mut connection)
        .await?;
    sqlx::query("SET search_path TO identity")
        .execute(&mut connection)
        .await?;
    let verification =
        verify_event(&publisher, &stream, &mut connection, &event_id, &event_type).await;
    let row_cleanup = sqlx::query("DELETE FROM identity.outbox_events WHERE event_id = $1::uuid")
        .bind(&event_id)
        .execute(&mut connection)
        .await;
    let message_cleanup = stream.purge().filter(subject.clone()).await;
    let mut failures = Vec::new();
    if let Err(error) = verification {
        failures.push(format!("outbox verification failed: {error}"));
    }
    if let Err(error) = row_cleanup {
        failures.push(format!("temporary outbox row cleanup failed: {error}"));
    }
    if let Err(error) = message_cleanup {
        failures.push(format!(
            "temporary JetStream message cleanup failed: {error}"
        ));
    }
    if !failures.is_empty() {
        return Err(failures.join("; ").into());
    }
    messaging.flush().await?;
    Ok(())
}

async fn verify_event(
    publisher: &JetStreamEventPublisher,
    stream: &jetstream::stream::Stream,
    connection: &mut PgConnection,
    event_id: &str,
    event_type: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let span = tracing::info_span!("outbox.verification");
    let context = span.context();
    OutboxWriter::new(IDENTITY)?
        .append(
            connection,
            event_id,
            event_type,
            1,
            &serde_json::json!({"verification_id": event_id}),
            &context,
        )
        .await?;
    let relay = OutboxRelay::new(IDENTITY)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if event_is_published(connection, event_id).await? {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            return Err("Outbox relay did not publish the verification event".into());
        }
        relay.relay_one(connection, publisher).await?;
    }
    verify_published_state(connection, event_id).await?;
    verify_jetstream_delivery(stream, event_id, event_type).await?;
    Ok(())
}

async funcion event_is_published(
    connection: &mut PgConnection,
    event_id: &str,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let row = sqlx::query(
        "SELECT published_at IS NOT NULL AS published FROM identity.outbox_events WHERE event_id = $1::uuid",
    )
    .bind(event_id)
    .fetch_one(connection)
    .await?;
    Ok(row.try_get("published")?)
}

async fn verify_published_state(
    connection: &mut PgConnection,
    event_id: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let row = sqlx::query(
        "SELECT published_at IS NOT NULL AS published, delivery_attempts, traceparent FROM identity.outbox_events WHERE event_id = $1::uuid",
    )
    .bind(event_id)
    .fetch_one(connection)
    .await?;
    let published: bool = row.try_get("published")?;
    let attempts: i32 = row.try_get("delivery_attempts")?;
    let traceparent: Option<String> = row.try_get("traceparent")?;
    if !published || attempts != 1 || traceparent.is_none() {
        return Err("Outbox confirmation, attempt count or trace context was not persisted".into());
    }
    Ok(())
}

async fn verify_jetstream_delivery(
    stream: &jetstream::stream::Stream,
    event_id: &str,
    event_type: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let consumer = stream
        .create_consumer(Config {
            ack_policy: jetstream::consumer::AckPolicy::Explicit,
            filter_subject: format!("{event_type}.v1"),
            ..Default::default()
        })
        .await?;
    let mut messages = consumer.messages().await?;
    let message = tokio::time::timeout(Duration::from_secs(5), messages.next())
        .await?
        .ok_or("JetStream did not deliver the outbox event")??;
    let envelope: serde_json::Value = serde_json::from_slice(&message.payload)?;
    let received_event_id = envelope
        .get("event_id")
        .and_then(serde_json::Value::as_str)
        .ok_or("JetStream event has no event ID")?;
    let received_correlation_id = envelope
        .get("correlation_id")
        .and_then(serde_json::Value::as_str)
        .ok_or("JetStream event has no correlation ID")?;
    let traceparent = message
        .headers
        .as_ref()
        .and_then(|headers| headers.get_last("traceparent"));
    if received_event_id != event_id || received_correlation_id != event_id || traceparent.is_none()
    {
        return Err("JetStream outbox envelope, correlation ID or trace context is invalid".into());
    }
    message.ack().await?;
    Ok(())
}
