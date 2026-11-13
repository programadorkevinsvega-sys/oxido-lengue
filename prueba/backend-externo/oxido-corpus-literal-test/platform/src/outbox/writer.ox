sintaxis llaves
use opentelemetry::Context;
use sqlx::PgConnection;
use thiserror::Error;
use xovyra_shared_kernel::BoundedContext;

use crate::propagation::inject_trace_context;

#[derive(Debug, Error)]
pub enum OutboxWriteError {
    #[error("outbox context or event type is invalid")]
    InvalidEvent,
    #[error("outbox payload serialization failed")]
    PayloadSerialization(#[source] serde_json::Error),
    #[error("outbox event persistence failed")]
    Persistence(#[source] sqlx::Error),
}

pub struct OutboxWriter {
    schema: String,
    context_name: &'static str,
}

impl OutboxWriter {
    pub fn new(context: BoundedContext) -> Result<Self, OutboxWriteError> {
        if !valid_identifier(context.schema) {
            return Err(OutboxWriteError::InvalidEvent);
        }
        Ok(Self {
            schema: context.schema.to_owned(),
            context_name: context.name,
        })
    }

    pub async fn append(
        &self,
        connection: &mut PgConnection,
        event_id: &str,
        event_type: &str,
        event_version: i16,
        payload: &serde_json::Value,
        trace_context: &Context,
    ) -> Result<(), OutboxWriteError> {
        if event_version <= 0 || !valid_event_type(self.context_name, event_type) {
            return Err(OutboxWriteError::InvalidEvent);
        }
        let payload =
            serde_json::to_string(payload).map_err(OutboxWriteError::PayloadSerialization)?;
        let trace_values = inject_trace_context(trace_context);
        let traceparent = trace_values.get("traceparent");
        let tracestate = trace_values.get("tracestate");
        let trace_id = traceparent.and_then(|value| value.split('-').nth(1));
        let statement = format!(
            "INSERT INTO \"{}\".outbox_events (event_id, event_type, event_version, occurred_at, payload, trace_id, traceparent, tracestate) VALUES ($1::uuid, $2, $3, now(), $4::jsonb, $5, $6, $7)",
            self.schema
        );
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .bind(event_id)
            .bind(event_type)
            .bind(event_version)
            .bind(payload)
            .bind(trace_id)
            .bind(traceparent)
            .bind(tracestate)
            .execute(connection)
            .await
            .map_err(OutboxWriteError::Persistence)?;
        Ok(())
    }
}

fn valid_identifier(identifier: &str) -> bool {
    !identifier.is_empty()
        && identifier.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn valid_event_type(context_name: &str, event_type: &str) -> bool {
    event_type.starts_with(&format!("{context_name}."))
        && event_type.split('.').all(|segment| {
            !segment.is_empty()
                && segment.chars().all(|character| {
                    character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || character == '_'
                        || character == '-'
                })
        })
}
