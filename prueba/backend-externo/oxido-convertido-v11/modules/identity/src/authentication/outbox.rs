use serde_json::Value;
use sqlx::Transaction;
use uuid::Uuid;

use super::AuthenticationError;

pub(super) async fn append_event(
    transaction: &mut Transaction<'_, sqlx::Postgres>,
    event_type: &str,
    payload: Value,
) -> Result<(), AuthenticationError> {
    if !event_type.starts_with("identity.")
        || event_type.split('.').any(|segment| {
            segment.is_empty()
                || !segment
                    .bytes()
                    .all(|character| character.is_ascii_lowercase() || character == b'_')
        })
    {
        return Err(AuthenticationError::InvalidEvent);
    }
    let serialized_payload =
        serde_json::to_string(&payload).map_err(AuthenticationError::EventSerialization)?;
    sqlx::query(
        "INSERT INTO identity.outbox_events (event_id, event_type, event_version, occurred_at, payload) VALUES ($1, $2, 1, now(), $3::jsonb)",
    )
    .bind(Uuid::new_v4())
    .bind(event_type)
    .bind(serialized_payload)
    .execute(&mut **transaction)
    .await
    .map_err(AuthenticationError::Persistence)?;
    Ok(())
}
