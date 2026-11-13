use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use super::models::TeamError;

pub async fn check_permission(
    database: &sqlx::PgPool,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    permission: &str,
) -> Result<bool, TeamError> {
    if permission.trim().is_empty() || permission.len() > 96 {
        return Err(TeamError::InvalidInput);
    }
    let result = sqlx::query_scalar::<_, bool>(
        "SELECT COALESCE(role.permissions @> ARRAY[$3]::text[], FALSE) FROM tenancy_access.workspace_memberships membership JOIN tenancy_access.workspace_roles role ON role.workspace_id = membership.workspace_id AND role.role_key = membership.role JOIN tenancy_access.workspaces workspace ON workspace.workspace_id = membership.workspace_id WHERE membership.workspace_id = $1 AND membership.user_id = $2 AND membership.status = 'active' AND workspace.status = 'active'",
    )
    .bind(workspace_id)
    .bind(actor_user_id)
    .bind(permission)
    .fetch_optional(database)
    .await
    .map_err(TeamError::Persistence)?;
    Ok(result.unwrap_or(false))
}

pub async fn require_permission(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    permission: &str,
) -> Result<String, TeamError> {
    let row = sqlx::query(
        "SELECT membership.role, workspace.workspace_type, workspace.status, role.permissions FROM tenancy_access.workspace_memberships membership JOIN tenancy_access.workspace_roles role ON role.workspace_id = membership.workspace_id AND role.role_key = membership.role JOIN tenancy_access.workspaces workspace ON workspace.workspace_id = membership.workspace_id WHERE membership.workspace_id = $1 AND membership.user_id = $2 AND membership.status = 'active' FOR UPDATE OF membership, workspace",
    )
    .bind(workspace_id)
    .bind(actor_user_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(TeamError::Persistence)?
    .ok_or(TeamError::NotFound)?;
    let status: String = row.try_get("status").map_err(TeamError::Persistence)?;
    let workspace_type: String = row
        .try_get("workspace_type")
        .map_err(TeamError::Persistence)?;
    if status != "active" {
        return Err(TeamError::InvalidState);
    }
    if workspace_type != "team" {
        return Err(TeamError::InvalidState);
    }
    let role: String = row.try_get("role").map_err(TeamError::Persistence)?;
    let permissions: Vec<String> = row.try_get("permissions").map_err(TeamError::Persistence)?;
    if !permissions.iter().any(|granted| granted == permission) {
        return Err(TeamError::Forbidden);
    }
    Ok(role)
}

pub async fn append_team_event(
    transaction: &mut Transaction<'_, Postgres>,
    event: TeamEvent<'_>,
) -> Result<(), TeamError> {
    let TeamEvent {
        workspace_id,
        actor_user_id,
        action,
        row_version,
        correlation_id,
        event_type,
        payload,
    } = event;
    let mut payload = payload.clone();
    let payload_fields = payload.as_object_mut().ok_or(TeamError::InvalidInput)?;
    let workspace_id_text = workspace_id.to_string();
    if payload_fields
        .get("workspace_id")
        .is_some_and(|value| value.as_str() != Some(workspace_id_text.as_str()))
    {
        return Err(TeamError::InvalidInput);
    }
    if row_version <= 0 || correlation_id.trim().is_empty() || correlation_id.len() > 128 {
        return Err(TeamError::InvalidInput);
    }
    payload_fields.insert("workspace_id".to_owned(), Value::String(workspace_id_text));
    payload_fields.insert(
        "correlation_id".to_owned(),
        Value::String(correlation_id.to_owned()),
    );
    payload_fields.insert("aggregate_version".to_owned(), Value::from(row_version));
    if let Some(actor_user_id) = actor_user_id {
        payload_fields.insert(
            "actor_user_id".to_owned(),
            Value::String(actor_user_id.to_string()),
        );
    }
    if !payload_fields.contains_key("aggregate_id") {
        let aggregate_id = [
            "task_id",
            "comment_id",
            "invitation_id",
            "user_id",
            "workspace_id",
        ]
        .into_iter()
        .find_map(|field| payload_fields.get(field).and_then(Value::as_str));
        if let Some(aggregate_id) = aggregate_id {
            payload_fields.insert(
                "aggregate_id".to_owned(),
                Value::String(aggregate_id.to_owned()),
            );
        }
    }
    let event_id = Uuid::new_v4();
    sqlx::query("INSERT INTO tenancy_access.workspace_audit_events (event_id, workspace_id, actor_user_id, action, row_version, correlation_id, occurred_at) VALUES ($1, $2, $3, $4, $5, $6, now())")
        .bind(event_id)
        .bind(workspace_id)
        .bind(actor_user_id)
        .bind(action)
        .bind(row_version)
        .bind(correlation_id)
        .execute(&mut **transaction)
        .await
        .map_err(TeamError::Persistence)?;
    sqlx::query("INSERT INTO tenancy_access.outbox_events (event_id, event_type, event_version, occurred_at, payload) VALUES ($1, $2, 1, now(), $3)")
        .bind(event_id)
        .bind(event_type)
        .bind(payload)
        .execute(&mut **transaction)
        .await
        .map_err(TeamError::Persistence)?;
    Ok(())
}

pub struct TeamEvent<'a> {
    pub workspace_id: Uuid,
    pub actor_user_id: Option<Uuid>,
    pub action: &'a str,
    pub row_version: i64,
    pub correlation_id: &'a str,
    pub event_type: &'a str,
    pub payload: &'a Value,
}
