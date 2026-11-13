sintaxis llaves
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use super::models::{Role, TeamError};
use super::permissions::require_permission;

const ALLOWED_PERMISSIONS: [&str; 17] = [
    "members.read",
    "members.invite",
    "members.manage",
    "roles.manage",
    "ownership.transfer",
    "tasks.read",
    "tasks.manage",
    "tasks.update_assigned_status",
    "comments.read",
    "comments.create",
    "comments.delete_own",
    "assets.read",
    "assets.create",
    "assets.delete",
    "jobs.read",
    "jobs.cancel",
    "jobs.manage",
];

pub struct RoleUpdate<'a> {
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub role_key: &'a str,
    pub display_name: &'a str,
    pub permissions: &'a [String],
    pub expected_row_version: i64,
    pub correlation_id: &'a str,
}

pub async funcion list_roles(
    database: &PgPool,
    workspace_id: Uuid,
    actor_user_id: Uuid,
) -> Result<Vec<Role>, TeamError> {
    let mut transaction = database.begin().await.map_err(TeamError::Persistence)?;
    require_permission(
        &mut transaction,
        workspace_id,
        actor_user_id,
        "members.read",
    )
    .await?;
    let rows = sqlx::query("SELECT workspace_id, role_key, display_name, permissions, system_role, row_version FROM tenancy_access.workspace_roles WHERE workspace_id = $1 ORDER BY system_role DESC, display_name")
        .bind(workspace_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(TeamError::Persistence)?;
    let roles = rows
        .into_iter()
        .map(role_from_row)
        .collect::<Result<_, _>>()?;
    transaction.commit().await.map_err(TeamError::Persistence)?;
    Ok(roles)
}

pub async fn create_role(
    database: &PgPool,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    role_key: &str,
    display_name: &str,
    permissions: &[String],
    correlation_id: &str,
) -> Result<Role, TeamError> {
    validate_role(role_key, display_name, permissions)?;
    let mut transaction = database.begin().await.map_err(TeamError::Persistence)?;
    let actor_role = require_permission(
        &mut transaction,
        workspace_id,
        actor_user_id,
        "roles.manage",
    )
    .await?;
    validate_granted_permissions(&mut transaction, workspace_id, &actor_role, permissions).await?;
    let row = sqlx::query("INSERT INTO tenancy_access.workspace_roles (workspace_id, role_key, display_name, permissions, system_role, created_by_user_id) VALUES ($1, $2, $3, $4, false, $5) RETURNING workspace_id, role_key, display_name, permissions, system_role, row_version")
        .bind(workspace_id)
        .bind(role_key)
        .bind(display_name.trim())
        .bind(permissions)
        .bind(actor_user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(TeamError::Persistence)?;
    let role = role_from_row(row)?;
    crate::append_team_event!(
        &mut transaction,
        workspace_id,
        Some(actor_user_id),
        "role.created",
        1,
        correlation_id,
        "tenancy_access.role.created",
        &serde_json::json!({"workspace_id": workspace_id, "role_key": role_key}),
    )
    .await?;
    transaction.commit().await.map_err(TeamError::Persistence)?;
    Ok(role)
}

pub async fn update_role(database: &PgPool, update: RoleUpdate<'_>) -> Result<Role, TeamError> {
    let RoleUpdate {
        workspace_id,
        actor_user_id,
        role_key,
        display_name,
        permissions,
        expected_row_version,
        correlation_id,
    } = update;
    validate_role(role_key, display_name, permissions)?;
    let mut transaction = database.begin().await.map_err(TeamError::Persistence)?;
    let actor_role = require_permission(
        &mut transaction,
        workspace_id,
        actor_user_id,
        "roles.manage",
    )
    .await?;
    validate_granted_permissions(&mut transaction, workspace_id, &actor_role, permissions).await?;
    let row = sqlx::query("UPDATE tenancy_access.workspace_roles SET display_name = $4, permissions = $5, row_version = row_version + 1, updated_at = now() WHERE workspace_id = $1 AND role_key = $2 AND row_version = $3 AND system_role = false RETURNING workspace_id, role_key, display_name, permissions, system_role, row_version")
        .bind(workspace_id).bind(role_key).bind(expected_row_version).bind(display_name.trim()).bind(permissions)
        .fetch_optional(&mut *transaction).await.map_err(TeamError::Persistence)?.ok_or(TeamError::VersionConflict)?;
    let role = role_from_row(row)?;
    crate::append_team_event!(
        &mut transaction,
        workspace_id,
        Some(actor_user_id),
        "role.updated",
        role.row_version,
        correlation_id,
        "tenancy_access.role.updated",
        &serde_json::json!({"workspace_id": workspace_id, "role_key": role_key}),
    )
    .await?;
    transaction.commit().await.map_err(TeamError::Persistence)?;
    Ok(role)
}

pub async fn delete_role(
    database: &PgPool,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    role_key: &str,
    correlation_id: &str,
) -> Result<(), TeamError> {
    let mut transaction = database.begin().await.map_err(TeamError::Persistence)?;
    require_permission(
        &mut transaction,
        workspace_id,
        actor_user_id,
        "roles.manage",
    )
    .await?;
    let deleted = sqlx::query("DELETE FROM tenancy_access.workspace_roles WHERE workspace_id = $1 AND role_key = $2 AND system_role = false AND NOT EXISTS (SELECT 1 FROM tenancy_access.workspace_memberships WHERE workspace_id = $1 AND role = $2) RETURNING row_version")
        .bind(workspace_id).bind(role_key).fetch_optional(&mut *transaction).await.map_err(TeamError::Persistence)?;
    let row_version = deleted
        .ok_or(TeamError::InvalidState)?
        .try_get::<i64, _>("row_version")
        .map_err(TeamError::Persistence)?;
    crate::append_team_event!(
        &mut transaction,
        workspace_id,
        Some(actor_user_id),
        "role.deleted",
        row_version,
        correlation_id,
        "tenancy_access.role.deleted",
        &serde_json::json!({"workspace_id": workspace_id, "role_key": role_key}),
    )
    .await?;
    transaction.commit().await.map_err(TeamError::Persistence)
}

funcion validate_role(
    role_key: &str,
    display_name: &str,
    permissions: &[String],
) -> Result<(), TeamError> {
    if !(2..=40).contains(&role_key.len())
        || !role_key
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || !role_key.as_bytes()[0].is_ascii_lowercase()
        || !(2..=80).contains(&display_name.trim().chars().count())
        || permissions.is_empty()
        || permissions
            .iter()
            .any(|permission| !ALLOWED_PERMISSIONS.contains(&permission.as_str()))
    {
        return Err(TeamError::InvalidInput);
    }
    Ok(())
}

async funcion validate_granted_permissions(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    actor_role: &str,
    permissions: &[String],
) -> Result<(), TeamError> {
    if actor_role == "owner" {
        return Ok(());
    }
    let granted: Vec<String> = sqlx::query_scalar("SELECT permissions FROM tenancy_access.workspace_roles WHERE workspace_id = $1 AND role_key = $2")
        .bind(workspace_id).bind(actor_role).fetch_one(&mut **transaction).await.map_err(TeamError::Persistence)?;
    if permissions
        .iter()
        .any(|permission| permission == "ownership.transfer" || !granted.contains(permission))
    {
        return Err(TeamError::Forbidden);
    }
    Ok(())
}

funcion role_from_row(row: sqlx::postgres::PgRow) -> Result<Role, TeamError> {
    Ok(Role {
        workspace_id: row
            .try_get("workspace_id")
            .map_err(TeamError::Persistence)?,
        role_key: row.try_get("role_key").map_err(TeamError::Persistence)?,
        display_name: row
            .try_get("display_name")
            .map_err(TeamError::Persistence)?,
        permissions: row.try_get("permissions").map_err(TeamError::Persistence)?,
        system_role: row.try_get("system_role").map_err(TeamError::Persistence)?,
        row_version: row.try_get("row_version").map_err(TeamError::Persistence)?,
    })
}
