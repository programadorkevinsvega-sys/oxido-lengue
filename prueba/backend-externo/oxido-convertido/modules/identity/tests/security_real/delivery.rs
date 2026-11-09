use std::error::Error;
use std::time::Duration;

use async_nats::jetstream::stream::Stream;
use sqlx::PgPool;
use uuid::Uuid;
use xovyra_platform::outbox::{JetStreamEventPublisher, OutboxRelay};

pub async fn wait_for_registration_event(
    stream: &mut Stream,
    database: &PgPool,
    relay: &OutboxRelay,
    publisher: &JetStreamEventPublisher,
    first_sequence: u64,
    expected_user_id: Uuid,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut next_sequence = first_sequence;
    while tokio::time::Instant::now() < deadline {
        let last_sequence = stream.info().await?.state.last_sequence;
        while next_sequence <= last_sequence {
            let message = stream.get_raw_message(next_sequence).await?;
            next_sequence += 1;
            if message.subject.as_str() != "identity.account.registered.v1" {
                continue;
            }
            let envelope: serde_json::Value = serde_json::from_slice(&message.payload)?;
            let user_id = envelope
                .get("payload")
                .and_then(|payload| payload.get("user_id"))
                .and_then(serde_json::Value::as_str);
            if user_id == Some(expected_user_id.to_string().as_str()) {
                return Ok(());
            }
        }
        let mut connection = database.acquire().await?;
        relay.relay_one(&mut connection, publisher).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let event_state = sqlx::query_as::<_, (bool, i32)>(
        "SELECT published_at IS NOT NULL, delivery_attempts FROM identity.outbox_events WHERE payload->>'user_id' = $1 AND event_type = 'identity.account.registered'",
    )
    .bind(expected_user_id.to_string())
    .fetch_optional(database)
    .await?;
    let pending_count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM identity.outbox_events WHERE published_at IS NULL",
    )
    .fetch_one(database)
    .await?;
    Err(format!(
        "JetStream registration delivery timed out: event_state={event_state:?}, pending_outbox_rows={pending_count}"
    )
    .into())
}
