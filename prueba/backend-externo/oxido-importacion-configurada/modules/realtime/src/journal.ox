sintaxis llaves
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum JournalError {
    #[error("event is invalid")]
    InvalidEvent,
    #[error("journal persistence failed")]
    Persistence(#[source] sqlx::Error),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RealtimeEvent {
    pub event_id: Uuid,
    pub stream_cursor: i64,
    pub event_type: String,
    pub schema_version: i32,
    pub audience: String,
    pub scope_id: Uuid,
    pub aggregate_id: Uuid,
    pub aggregate_version: i64,
    pub occurred_at_unix: i64,
    pub correlation_id: String,
}

#[derive(Clone, Debug)]
pub struct EventPage {
    pub events: Vec<RealtimeEvent>,
    pub next_cursor: i64,
}

pub async fn persist_domain_event(pool: &PgPool, event: &Value) -> Result<(), JournalError> {
    let event_id = event
        .get("event_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(JournalError::InvalidEvent)?;
    let event_type = event
        .get("event_type")
        .and_then(Value::as_str)
        .ok_or(JournalError::InvalidEvent)?;
    let payload = event.get("payload").ok_or(JournalError::InvalidEvent)?;
    let audience = match event_type {
        value if value.starts_with("notifications.notification.") => "tenant_user",
        value
            if value.starts_with("platform_control.")
                || value.starts_with("commercial_billing.catalog.") =>
        {
            "platform"
        }
        value
            if value.starts_with("tenancy_access.")
                || value.starts_with("asset_storage.")
                || value.starts_with("jobs.")
                || value.starts_with("commercial_billing.subscription.") =>
        {
            "tenant"
        }
        _ => return Err(JournalError::InvalidEvent),
    };
    let scope_id = match audience {
        "tenant_user" => required_id(payload, "recipient_user_id")?,
        "tenant" => required_id(payload, "workspace_id")?,
        "platform" => required_id(payload, "platform_scope_id")?,
        _ => return Err(JournalError::InvalidEvent),
    };
    let aggregate_id = payload
        .get("aggregate_id")
        .or_else(|| payload.get("task_id"))
        .or_else(|| payload.get("asset_id"))
        .or_else(|| payload.get("job_id"))
        .or_else(|| payload.get("notification_id"))
        .or_else(|| payload.get("comment_id"))
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(JournalError::InvalidEvent)?;
    let aggregate_version = payload
        .get("aggregate_version")
        .and_then(Value::as_i64)
        .filter(|version| *version > 0)
        .ok_or(JournalError::InvalidEvent)?;
    let schema_version = event
        .get("event_version")
        .and_then(Value::as_i64)
        .ok_or(JournalError::InvalidEvent)?;
    if !(1..=i16::MAX as i64).contains(&schema_version) {
        return Err(JournalError::InvalidEvent);
    }
    let correlation_id = event
        .get("correlation_id")
        .and_then(Value::as_str)
        .or_else(|| payload.get("correlation_id").and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty() && value.len() <= 128)
        .or_else(|| event.get("event_id").and_then(Value::as_str))
        .ok_or(JournalError::InvalidEvent)?;
    let mut transaction = pool.begin().await.map_err(JournalError::Persistence)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(event_id.to_string())
        .execute(&mut *transaction)
        .await
        .map_err(JournalError::Persistence)?;
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT stream_cursor FROM realtime.event_journal WHERE event_id=$1",
    )
    .bind(event_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(JournalError::Persistence)?;
    if existing.is_some() {
        transaction
            .commit()
            .await
            .map_err(JournalError::Persistence)?;
        return Ok(());
    }
    let stream_cursor = sqlx::query_scalar::<_, i64>("INSERT INTO realtime.stream_cursors (audience,scope_id,last_cursor) SELECT $1,$2,COALESCE(MAX(stream_cursor),0)+1 FROM realtime.event_journal WHERE audience=$1 AND scope_id=$2 ON CONFLICT (audience,scope_id) DO UPDATE SET last_cursor=realtime.stream_cursors.last_cursor+1 RETURNING last_cursor")
        .bind(audience)
        .bind(scope_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(JournalError::Persistence)?;
    sqlx::query("INSERT INTO realtime.event_journal (stream_cursor,event_id,event_type,schema_version,audience,scope_id,aggregate_id,aggregate_version,correlation_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(stream_cursor)
        .bind(event_id)
        .bind(event_type)
        .bind(schema_version as i16)
        .bind(audience)
        .bind(scope_id)
        .bind(aggregate_id)
        .bind(aggregate_version)
        .bind(correlation_id)
        .execute(&mut *transaction)
        .await
        .map_err(JournalError::Persistence)?;
    transaction
        .commit()
        .await
        .map_err(JournalError::Persistence)?;
    Ok(())
}

pub async fn list_after(
    pool: &PgPool,
    audience: &str,
    scope_id: Uuid,
    after_cursor: i64,
    limit: i64,
) -> Result<EventPage, JournalError> {
    if !matches!(audience, "tenant" | "tenant_user" | "platform")
        || after_cursor < 0
        || !(1..=500).contains(&limit)
    {
        return Err(JournalError::InvalidEvent);
    }
    let rows = sqlx::query("SELECT event_id,stream_cursor,event_type,schema_version,audience,scope_id,aggregate_id,aggregate_version,floor(extract(epoch FROM occurred_at))::bigint AS occurred_at_unix,correlation_id FROM realtime.event_journal WHERE audience=$1 AND scope_id=$2 AND stream_cursor>$3 ORDER BY stream_cursor ASC LIMIT $4")
        .bind(audience)
        .bind(scope_id)
        .bind(after_cursor)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(JournalError::Persistence)?;
    let events = rows
        .into_iter()
        .map(event_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    let next_cursor = events
        .last()
        .map_or(after_cursor, |event| event.stream_cursor);
    Ok(EventPage {
        events,
        next_cursor,
    })
}

fn required_id(payload: &Value, field: &str) -> Result<Uuid, JournalError> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(JournalError::InvalidEvent)
}

fn event_from_row(row: sqlx::postgres::PgRow) -> Result<RealtimeEvent, JournalError> {
    Ok(RealtimeEvent {
        event_id: row.try_get("event_id").map_err(JournalError::Persistence)?,
        stream_cursor: row
            .try_get("stream_cursor")
            .map_err(JournalError::Persistence)?,
        event_type: row
            .try_get("event_type")
            .map_err(JournalError::Persistence)?,
        schema_version: i32::from(
            row.try_get::<i16, _>("schema_version")
                .map_err(JournalError::Persistence)?,
        ),
        audience: row.try_get("audience").map_err(JournalError::Persistence)?,
        scope_id: row.try_get("scope_id").map_err(JournalError::Persistence)?,
        aggregate_id: row
            .try_get("aggregate_id")
            .map_err(JournalError::Persistence)?,
        aggregate_version: row
            .try_get("aggregate_version")
            .map_err(JournalError::Persistence)?,
        occurred_at_unix: row
            .try_get("occurred_at_unix")
            .map_err(JournalError::Persistence)?,
        correlation_id: row
            .try_get("correlation_id")
            .map_err(JournalError::Persistence)?,
    })
}
