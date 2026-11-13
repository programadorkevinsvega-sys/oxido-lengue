sintaxis llaves
use std::collections::HashMap;

use async_nats::HeaderMap;
use sqlx::Connection;
use sqlx::PgConnection;
use sqlx::Row;
use thiserror::Error;
use xovyra_shared_kernel::BoundedContext;

use crate::outbox::publisher::EventPublisher;
use crate::propagation::{extract_trace_context, inject_trace_context};

#[derive(Debug, Error)]
pub enum OutboxError {
    #[error("outbox module schema is invalid")]
    InvalidSchema,
    #[error("outbox database operation failed")]
    Database(#[source] sqlx::Error),
    #[error("outbox event payload is not valid JSON")]
    InvalidPayload(#[source] serde_json::Error),
    #[error("outbox event could not be published to JetStream")]
    JetStream,
}

pub struct OutboxRelay {
    schema: String,
    context_name: &'static str,
}

impl OutboxRelay {
    pub fn new(context: BoundedContext) -> Result<Self, OutboxError> {
        if context.schema.is_empty()
            || !context.schema.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
            })
        {
            return Err(OutboxError::InvalidSchema);
        }
        Ok(Self {
            schema: context.schema.to_owned(),
            context_name: context.name,
        })
    }

    pub async fn relay_one(
        &self,
        connection: &mut PgConnection,
        publisher: &dyn EventPublisher,
    ) -> Result<bool, OutboxError> {
        self.relay_selected(connection, publisher, None).await
    }

    /// Relays one specific outbox event. This is useful for deterministic recovery and
    /// integration verification without consuming unrelated pending events in the context.
    pub async fn relay_event(
        &self,
        connection: &mut PgConnection,
        publisher: &dyn EventPublisher,
        event_id: &str,
    ) -> Result<bool, OutboxError> {
        self.relay_selected(connection, publisher, Some(event_id))
            .await
    }

    async fn relay_selected(
        &self,
        connection: &mut PgConnection,
        publisher: &dyn EventPublisher,
        selected_event_id: Option<&str>,
    ) -> Result<bool, OutboxError> {
        let mut transaction = connection.begin().await.map_err(OutboxError::Database)?;
        let statement = format!(
            "SELECT event_id::text, event_type, event_version, occurred_at::text, payload::text, traceparent, tracestate FROM \"{}\".outbox_events WHERE published_at IS NULL AND ($1::uuid IS NULL OR event_id=$1::uuid) ORDER BY occurred_at, event_id LIMIT 1 FOR UPDATE SKIP LOCKED",
            self.schema
        );
        let event = sqlx::query(sqlx::AssertSqlSafe(statement))
            .bind(selected_event_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(OutboxError::Database)?;
        let Some(event) = event else {
            transaction.commit().await.map_err(OutboxError::Database)?;
            return Ok(false);
        };
        let event_id: String = event.try_get("event_id").map_err(OutboxError::Database)?;
        let event_type: String = event.try_get("event_type").map_err(OutboxError::Database)?;
        let event_version: i16 = event
            .try_get("event_version")
            .map_err(OutboxError::Database)?;
        let occurred_at: String = event
            .try_get("occurred_at")
            .map_err(OutboxError::Database)?;
        let payload_text: String = event.try_get("payload").map_err(OutboxError::Database)?;
        let traceparent: Option<String> = event
            .try_get("traceparent")
            .map_err(OutboxError::Database)?;
        let tracestate: Option<String> =
            event.try_get("tracestate").map_err(OutboxError::Database)?;
        if !event_type.starts_with(&format!("{}.", self.context_name)) {
            return Err(OutboxError::InvalidSchema);
        }
        let payload: serde_json::Value =
            serde_json::from_str(&payload_text).map_err(OutboxError::InvalidPayload)?;
        let correlation_id = payload
            .get("correlation_id")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty() && value.len() <= 128)
            .unwrap_or(&event_id);
        let envelope = serde_json::json!({
            "event_id": event_id,
            "event_type": event_type,
            "event_version": event_version,
            "occurred_at": occurred_at,
            "correlation_id": correlation_id,
            "payload": payload,
        });
        let trace_context = extract_trace_context(trace_values(traceparent, tracestate));
        let propagated_values = inject_trace_context(&trace_context);
        let mut headers = HeaderMap::new();
        for (name, value) in propagated_values {
            headers.insert(name, value);
        }
        headers.insert("Nats-Msg-Id".to_owned(), event_id.clone());
        let subject = format!("{event_type}.v{event_version}");
        let serialized_envelope =
            serde_json::to_vec(&envelope).map_err(OutboxError::InvalidPayload)?;
        let publish_result = publisher
            .publish(&subject, headers, serialized_envelope)
            .await;
        match publish_result {
            Err(error) => {
                tracing::warn!(error_type = %std::any::type_name_of_val(&error), "Outbox event publication failed");
                self.mark_failed(&mut transaction, &event_id).await?;
                transaction.commit().await.map_err(OutboxError::Database)?;
                return Err(OutboxError::JetStream);
            }
            Ok(()) => self.mark_published(&mut transaction, &event_id).await?,
        }
        transaction.commit().await.map_err(OutboxError::Database)?;
        Ok(true)
    }

    async fn mark_published(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        event_id: &str,
    ) -> Result<(), OutboxError> {
        let statement = format!(
            "UPDATE \"{}\".outbox_events SET published_at = now(), delivery_attempts = delivery_attempts + 1, last_delivery_error = NULL WHERE event_id = $1::uuid",
            self.schema
        );
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .bind(event_id)
            .execute(&mut **transaction)
            .await
            .map_err(OutboxError::Database)?;
        Ok(())
    }

    async fn mark_failed(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        event_id: &str,
    ) -> Result<(), OutboxError> {
        let statement = format!(
            "UPDATE \"{}\".outbox_events SET delivery_attempts = delivery_attempts + 1, last_delivery_error = 'JetStream publication was not confirmed' WHERE event_id = $1::uuid",
            self.schema
        );
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .bind(event_id)
            .execute(&mut **transaction)
            .await
            .map_err(OutboxError::Database)?;
        Ok(())
    }
}

fn trace_values(
    traceparent: Option<String>,
    tracestate: Option<String>,
) -> HashMap<String, String> {
    let mut values = HashMap::new();
    if let Some(traceparent) = traceparent {
        values.insert("traceparent".to_owned(), traceparent);
    }
    if let Some(tracestate) = tracestate {
        values.insert("tracestate".to_owned(), tracestate);
    }
    values
}
