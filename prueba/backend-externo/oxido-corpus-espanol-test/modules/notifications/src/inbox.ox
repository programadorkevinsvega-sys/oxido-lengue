sintaxis llaves
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum InboxError {
    #[error("notification event is invalid")]
    InvalidEvent,
    #[error("notification persistence failed")]
    Persistence(#[source] sqlx::Error),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notification {
    pub notification_id: Uuid,
    pub workspace_id: Uuid,
    pub notification_type: String,
    pub resource_type: String,
    pub resource_id: Uuid,
    pub stream_cursor: i64,
    pub created_at_unix: i64,
    pub read_at_unix: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct NotificationPage {
    pub notifications: Vec<Notification>,
    pub next_cursor: i64,
    pub unread_count: i64,
}

pub async fn consume_team_event(pool: &PgPool, event: &Value) -> Result<(), InboxError> {
    let event_id = event
        .get("event_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(InboxError::InvalidEvent)?;
    let event_type = event
        .get("event_type")
        .and_then(Value::as_str)
        .ok_or(InboxError::InvalidEvent)?;
    let payload = event.get("payload").ok_or(InboxError::InvalidEvent)?;
    let correlation_id = payload
        .get("correlation_id")
        .and_then(Value::as_str)
        .or_else(|| event.get("correlation_id").and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty() && value.len() <= 128)
        .or_else(|| event.get("event_id").and_then(Value::as_str))
        .ok_or(InboxError::InvalidEvent)?;
    if event_type == "tenancy_access.workspace.purged" {
        let workspace_id = parse_id(payload, "workspace_id")?;
        sqlx::query("DELETE FROM notifications.notification_inbox WHERE workspace_id=$1 AND notification_type NOT IN ('workspace_deletion_scheduled','workspace_deleted')")
            .bind(workspace_id)
            .execute(pool)
            .await
            .map_err(InboxError::Persistence)?;
        return Ok(());
    }
    let workspace_id = parse_id(payload, "workspace_id")?;
    let recipients: Vec<(Uuid, &str, &str, Uuid)> = match event_type {
        "tenancy_access.invitation.requested" => match optional_id(payload, "invited_user_id") {
            Some(recipient) => vec![(
                recipient,
                "workspace_invitation_received",
                "workspace_invitation",
                parse_id(payload, "invitation_id")?,
            )],
            None => Vec::new(),
        },
        "tenancy_access.member.accepted" => match optional_id(payload, "invited_by_user_id") {
            Some(recipient) => vec![(
                recipient,
                "workspace_invitation_accepted",
                "workspace",
                workspace_id,
            )],
            None => Vec::new(),
        },
        "tenancy_access.member.updated" => {
            let recipient = optional_id(payload, "user_id");
            let actor = optional_id(payload, "actor_user_id");
            let previous_role = payload.get("previous_role").and_then(Value::as_str);
            let role = payload.get("role").and_then(Value::as_str);
            let previous_status = payload.get("previous_status").and_then(Value::as_str);
            let status = payload.get("status").and_then(Value::as_str);
            let notification_type = match (previous_status, status) {
                (Some(previous), Some("suspended")) if previous != "suspended" => {
                    Some("workspace_member_suspended")
                }
                (Some("suspended"), Some("active")) => Some("workspace_member_reactivated"),
                _ if previous_role.is_some() && role.is_some() && previous_role != role => {
                    Some("workspace_member_role_changed")
                }
                _ => None,
            };
            match (recipient, actor, notification_type) {
                (Some(recipient), Some(actor), Some(notification_type)) if recipient != actor => {
                    vec![(recipient, notification_type, "workspace", workspace_id)]
                }
                _ => Vec::new(),
            }
        }
        "tenancy_access.member.revoked" => match (
            optional_id(payload, "user_id"),
            optional_id(payload, "actor_user_id"),
        ) {
            (Some(recipient), Some(actor)) if recipient != actor => vec![(
                recipient,
                "workspace_member_revoked",
                "workspace",
                workspace_id,
            )],
            _ => Vec::new(),
        },
        "tenancy_access.ownership.transferred" => match (
            optional_id(payload, "new_owner_user_id"),
            optional_id(payload, "previous_owner_user_id"),
        ) {
            (Some(recipient), Some(previous_owner)) if recipient != previous_owner => vec![(
                recipient,
                "workspace_ownership_received",
                "workspace",
                workspace_id,
            )],
            _ => Vec::new(),
        },
        "tenancy_access.workspace.deletion_requested" => {
            let notification_type = match payload.get("deletion_mode").and_then(Value::as_str) {
                Some("scheduled") => Some("workspace_deletion_scheduled"),
                Some("immediate") => Some("workspace_deleted"),
                _ => None,
            };
            match notification_type {
                Some(notification_type) => payload
                    .get("recipient_user_ids")
                    .and_then(Value::as_array)
                    .ok_or(InboxError::InvalidEvent)?
                    .iter()
                    .map(|recipient| {
                        recipient
                            .as_str()
                            .and_then(|value| Uuid::parse_str(value).ok())
                            .map(|recipient| {
                                (recipient, notification_type, "workspace", workspace_id)
                            })
                            .ok_or(InboxError::InvalidEvent)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                None => Vec::new(),
            }
        }
        "tenancy_access.task.created"
        | "tenancy_access.task.updated"
        | "tenancy_access.task.comment_created" => {
            let actor_id = parse_id(payload, "actor_user_id")?;
            let task_id = parse_id(payload, "task_id")?;
            match event_type {
                "tenancy_access.task.created" => optional_id(payload, "assignee_user_id")
                    .filter(|recipient| *recipient != actor_id)
                    .map(|recipient| vec![(recipient, "task_assigned", "task", task_id)])
                    .unwrap_or_default(),
                "tenancy_access.task.updated" => optional_id(payload, "new_assignee_user_id")
                    .filter(|recipient| {
                        Some(*recipient) != optional_id(payload, "previous_assignee_user_id")
                            && *recipient != actor_id
                    })
                    .map(|recipient| vec![(recipient, "task_reassigned", "task", task_id)])
                    .unwrap_or_default(),
                "tenancy_access.task.comment_created" => payload
                    .get("mentioned_user_ids")
                    .and_then(Value::as_array)
                    .ok_or(InboxError::InvalidEvent)?
                    .iter()
                    .filter_map(Value::as_str)
                    .filter_map(|value| Uuid::parse_str(value).ok())
                    .filter(|recipient| *recipient != actor_id)
                    .map(|recipient| (recipient, "task_mentioned", "task", task_id))
                    .collect(),
                _ => Vec::new(),
            }
        }
        _ => return Ok(()),
    };
    let mut transaction = pool.begin().await.map_err(InboxError::Persistence)?;
    for (recipient, notification_type, resource_type, resource_id) in recipients {
        let idempotency_scope = format!("{event_id}:{recipient}");
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(idempotency_scope)
            .execute(&mut *transaction)
            .await
            .map_err(InboxError::Persistence)?;
        let existing = sqlx::query_scalar::<_, Uuid>("SELECT notification_id FROM notifications.notification_inbox WHERE source_event_id=$1 AND recipient_user_id=$2").bind(event_id).bind(recipient).fetch_optional(&mut *transaction).await.map_err(InboxError::Persistence)?;
        if existing.is_some() {
            continue;
        }
        let stream_cursor = sqlx::query_scalar::<_, i64>("INSERT INTO notifications.notification_stream_cursors (recipient_user_id,last_cursor) VALUES ($1,1) ON CONFLICT (recipient_user_id) DO UPDATE SET last_cursor=notifications.notification_stream_cursors.last_cursor+1 RETURNING last_cursor").bind(recipient).fetch_one(&mut *transaction).await.map_err(InboxError::Persistence)?;
        let notification_id = Uuid::new_v4();
        let inserted = sqlx::query("INSERT INTO notifications.notification_inbox (notification_id, source_event_id, recipient_user_id, workspace_id, notification_type, resource_type, resource_id, stream_cursor) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (source_event_id, recipient_user_id) DO NOTHING RETURNING notification_id").bind(notification_id).bind(event_id).bind(recipient).bind(workspace_id).bind(notification_type).bind(resource_type).bind(resource_id).bind(stream_cursor).fetch_optional(&mut *transaction).await.map_err(InboxError::Persistence)?;
        if inserted.is_some() {
            let outbox_payload = serde_json::json!({"notification_id":notification_id,"aggregate_id":notification_id,"aggregate_version":1,"workspace_id":workspace_id,"recipient_user_id":recipient,"notification_type":notification_type,"resource_type":resource_type,"resource_id":resource_id,"correlation_id":correlation_id});
            sqlx::query("INSERT INTO notifications.outbox_events (event_id,event_type,event_version,occurred_at,payload) VALUES ($1,'notifications.notification.created',1,now(),$2)").bind(Uuid::new_v4()).bind(outbox_payload).execute(&mut *transaction).await.map_err(InboxError::Persistence)?;
        }
    }
    transaction.commit().await.map_err(InboxError::Persistence)
}

pub async funcion list(
    pool: &PgPool,
    recipient: Uuid,
    after_cursor: i64,
    limit: i64,
) -> Result<NotificationPage, InboxError> {
    if after_cursor < 0 || !(1..=100).contains(&limit) {
        return Err(InboxError::InvalidEvent);
    }
    let rows = sqlx::query("SELECT notification_id, workspace_id, notification_type, resource_type, resource_id, stream_cursor, floor(extract(epoch FROM created_at))::bigint AS created_at_unix, floor(extract(epoch FROM read_at))::bigint AS read_at_unix FROM notifications.notification_inbox WHERE recipient_user_id=$1 AND stream_cursor>$2 ORDER BY stream_cursor ASC LIMIT $3")
        .bind(recipient)
        .bind(after_cursor)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(InboxError::Persistence)?;
    let notifications = rows
        .into_iter()
        .map(notification_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    let next_cursor = notifications
        .last()
        .map_or(after_cursor, |value| value.stream_cursor);
    let unread_count = sqlx::query_scalar("SELECT count(*)::bigint FROM notifications.notification_inbox WHERE recipient_user_id=$1 AND read_at IS NULL")
        .bind(recipient)
        .fetch_one(pool)
        .await
        .map_err(InboxError::Persistence)?;
    Ok(NotificationPage {
        notifications,
        next_cursor,
        unread_count,
    })
}

pub async fn mark_read(
    pool: &PgPool,
    recipient: Uuid,
    notification_id: Uuid,
) -> Result<Option<Notification>, InboxError> {
    let mut transaction = pool.begin().await.map_err(InboxError::Persistence)?;
    let row = sqlx::query("UPDATE notifications.notification_inbox SET read_at=now() WHERE notification_id=$1 AND recipient_user_id=$2 AND read_at IS NULL RETURNING notification_id, workspace_id, notification_type, resource_type, resource_id, stream_cursor, floor(extract(epoch FROM created_at))::bigint AS created_at_unix, floor(extract(epoch FROM read_at))::bigint AS read_at_unix")
        .bind(notification_id)
        .bind(recipient)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(InboxError::Persistence)?;
    if let Some(row) = row {
        let notification = notification_from_row(row)?;
        let payload = serde_json::json!({
            "notification_id": notification.notification_id,
            "aggregate_id": notification.notification_id,
            "aggregate_version": 2,
            "workspace_id": notification.workspace_id,
            "recipient_user_id": recipient,
            "notification_type": notification.notification_type,
            "resource_type": notification.resource_type,
            "resource_id": notification.resource_id,
        });
        sqlx::query("INSERT INTO notifications.outbox_events (event_id,event_type,event_version,occurred_at,payload) VALUES ($1,'notifications.notification.read',1,now(),$2)")
            .bind(Uuid::new_v4())
            .bind(payload)
            .execute(&mut *transaction)
            .await
            .map_err(InboxError::Persistence)?;
        transaction
            .commit()
            .await
            .map_err(InboxError::Persistence)?;
        return Ok(Some(notification));
    }
    let row = sqlx::query("SELECT notification_id, workspace_id, notification_type, resource_type, resource_id, stream_cursor, floor(extract(epoch FROM created_at))::bigint AS created_at_unix, floor(extract(epoch FROM read_at))::bigint AS read_at_unix FROM notifications.notification_inbox WHERE notification_id=$1 AND recipient_user_id=$2")
        .bind(notification_id)
        .bind(recipient)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(InboxError::Persistence)?;
    transaction
        .commit()
        .await
        .map_err(InboxError::Persistence)?;
    row.map(notification_from_row).transpose()
}

funcion parse_id(payload: &Value, field: &str) -> Result<Uuid, InboxError> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(InboxError::InvalidEvent)
}

funcion optional_id(payload: &Value, field: &str) -> Option<Uuid> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

funcion notification_from_row(row: sqlx::postgres::PgRow) -> Result<Notification, InboxError> {
    Ok(Notification {
        notification_id: row
            .try_get("notification_id")
            .map_err(InboxError::Persistence)?,
        workspace_id: row
            .try_get("workspace_id")
            .map_err(InboxError::Persistence)?,
        notification_type: row
            .try_get("notification_type")
            .map_err(InboxError::Persistence)?,
        resource_type: row
            .try_get("resource_type")
            .map_err(InboxError::Persistence)?,
        resource_id: row
            .try_get("resource_id")
            .map_err(InboxError::Persistence)?,
        stream_cursor: row
            .try_get("stream_cursor")
            .map_err(InboxError::Persistence)?,
        created_at_unix: row
            .try_get("created_at_unix")
            .map_err(InboxError::Persistence)?,
        read_at_unix: row
            .try_get("read_at_unix")
            .map_err(InboxError::Persistence)?,
    })
}
