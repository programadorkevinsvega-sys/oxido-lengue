sintaxis llaves
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::models::TeamError;
use super::permissions::require_permission;

pub async fn transfer(
    database: &PgPool,
    workspace_id: Uuid,
    actor: Uuid,
    new_owner: Uuid,
    confirmation: &str,
    expected_version: i64,
    correlation_id: &str,
) -> Result<i64, TeamError> {
    if expected_version < 1 {
        return Err(TeamError::InvalidInput);
    }
    let mut tx = database.begin().await.map_err(TeamError::Persistence)?;
    let actor_role = require_permission(&mut tx, workspace_id, actor, "ownership.transfer").await?;
    if actor_role != "owner" || actor == new_owner {
        return Err(TeamError::Forbidden);
    }
    let workspace=sqlx::query("SELECT name,row_version,status FROM tenancy_access.workspaces WHERE workspace_id=$1 FOR UPDATE").bind(workspace_id).fetch_optional(&mut *tx).await.map_err(TeamError::Persistence)?.ok_or(TeamError::NotFound)?;
    let name: String = workspace.try_get("name").map_err(TeamError::Persistence)?;
    let version: i64 = workspace
        .try_get("row_version")
        .map_err(TeamError::Persistence)?;
    let status: String = workspace
        .try_get("status")
        .map_err(TeamError::Persistence)?;
    if status != "active" || version != expected_version {
        return Err(TeamError::VersionConflict);
    }
    if confirmation != format!("TRANSFER {name}") {
        return Err(TeamError::InvalidInput);
    }
    let target_active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenancy_access.workspace_memberships WHERE workspace_id=$1 AND user_id=$2 AND status='active')").bind(workspace_id).bind(new_owner).fetch_one(&mut *tx).await.map_err(TeamError::Persistence)?;
    if !target_active {
        return Err(TeamError::InvalidState);
    }
    sqlx::query("UPDATE tenancy_access.workspace_memberships SET role='admin',row_version=row_version+1 WHERE workspace_id=$1 AND user_id=$2 AND role='owner' AND status='active'").bind(workspace_id).bind(actor).execute(&mut *tx).await.map_err(TeamError::Persistence)?;
    sqlx::query("UPDATE tenancy_access.workspace_memberships SET role='owner',row_version=row_version+1 WHERE workspace_id=$1 AND user_id=$2 AND status='active'").bind(workspace_id).bind(new_owner).execute(&mut *tx).await.map_err(TeamError::Persistence)?;
    let next_version = version + 1;
    sqlx::query("UPDATE tenancy_access.workspaces SET row_version=$2,updated_at=now() WHERE workspace_id=$1").bind(workspace_id).bind(next_version).execute(&mut *tx).await.map_err(TeamError::Persistence)?;
    crate::append_team_event!(&mut tx,workspace_id,Some(actor),"ownership.transferred",next_version,correlation_id,"tenancy_access.ownership.transferred",&serde_json::json!({"workspace_id":workspace_id,"previous_owner_user_id":actor,"new_owner_user_id":new_owner})).await?;
    tx.commit().await.map_err(TeamError::Persistence)?;
    Ok(next_version)
}
