sintaxis llaves
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use super::models::{Member, TeamError};
use super::permissions::require_permission;

pub struct MemberChange<'a> {
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub member_user_id: Uuid,
    pub role_key: &'a str,
    pub status: &'a str,
    pub expected_row_version: i64,
    pub correlation_id: &'a str,
}

pub async fn list_members(
    database: &PgPool,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    cursor: Option<&str>,
    page_size: i64,
) -> Result<(Vec<Member>, Option<String>), TeamError> {
    let limit = page_size.clamp(1, 100);
    let (timestamp, cursor_id) = decode_cursor(cursor)?;
    let mut transaction = database.begin().await.map_err(TeamError::Persistence)?;
    require_permission(
        &mut transaction,
        workspace_id,
        actor_user_id,
        "members.read",
    )
    .await?;
    let rows = sqlx::query("SELECT workspace_id, user_id, email_normalized, role, status, floor(extract(epoch FROM joined_at))::bigint AS joined_at_unix, row_version FROM tenancy_access.workspace_memberships WHERE workspace_id = $1 AND ($2::bigint IS NULL OR (floor(extract(epoch FROM joined_at))::bigint, user_id) > ($2, $3)) ORDER BY floor(extract(epoch FROM joined_at))::bigint, user_id LIMIT $4")
        .bind(workspace_id).bind(timestamp).bind(cursor_id).bind(limit + 1).fetch_all(&mut *transaction).await.map_err(TeamError::Persistence)?;
    let mut members = rows
        .into_iter()
        .map(member_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = members.len() as i64 > limit;
    if has_more {
        members.pop();
    }
    let next_cursor = if has_more {
        members
            .last()
            .map(|member| encode_cursor(member.joined_at_unix, member.user_id))
    } else {
        None
    };
    transaction.commit().await.map_err(TeamError::Persistence)?;
    Ok((members, next_cursor))
}

pub async fn change_member(
    database: &PgPool,
    change: MemberChange<'_>,
) -> Result<Member, TeamError> {
    let MemberChange {
        workspace_id,
        actor_user_id,
        member_user_id,
        role_key,
        status,
        expected_row_version,
        correlation_id,
    } = change;
    if !matches!(status, "active" | "suspended") || expected_row_version < 1 {
        return Err(TeamError::InvalidInput);
    }
    let mut transaction = database.begin().await.map_err(TeamError::Persistence)?;
    let actor_role = require_permission(
        &mut transaction,
        workspace_id,
        actor_user_id,
        "members.manage",
    )
    .await?;
    let target = sqlx::query("SELECT role,status FROM tenancy_access.workspace_memberships WHERE workspace_id = $1 AND user_id = $2 FOR UPDATE")
        .bind(workspace_id).bind(member_user_id).fetch_optional(&mut *transaction).await.map_err(TeamError::Persistence)?.ok_or(TeamError::NotFound)?;
    let target_role: String = target.try_get("role").map_err(TeamError::Persistence)?;
    let target_status: String = target.try_get("status").map_err(TeamError::Persistence)?;
    if target_role == "owner"
        || (actor_role != "owner"
            && (member_user_id == actor_user_id
                || target_role == "admin"
                || role_key == "owner"
                || role_key == "admin"))
    {
        return Err(TeamError::Forbidden);
    }
    ensure_assignable_role(
        &mut transaction,
        workspace_id,
        role_key,
        actor_role.as_str(),
    )
    .await?;
    if target_status == status && target_role == role_key {
        let member = find_member(&mut transaction, workspace_id, member_user_id).await?;
        transaction.commit().await.map_err(TeamError::Persistence)?;
        return Ok(member);
    }
    let row = sqlx::query("UPDATE tenancy_access.workspace_memberships SET role = $4, status = $5, row_version = row_version + 1 WHERE workspace_id = $1 AND user_id = $2 AND row_version = $3 RETURNING workspace_id, user_id, email_normalized, role, status, floor(extract(epoch FROM joined_at))::bigint AS joined_at_unix, row_version")
        .bind(workspace_id).bind(member_user_id).bind(expected_row_version).bind(role_key).bind(status).fetch_optional(&mut *transaction).await.map_err(TeamError::Persistence)?.ok_or(TeamError::VersionConflict)?;
    if status != "active" {
        clear_active_selections(&mut transaction, workspace_id, member_user_id).await?;
    }
    let member = member_from_row(row)?;
    crate::append_team_event!(&mut transaction, workspace_id, Some(actor_user_id), if status == "suspended" { "member.suspended" } else if target_role != role_key { "member.role_changed" } else { "member.reactivated" }, member.row_version, correlation_id, "tenancy_access.member.updated", &serde_json::json!({"workspace_id": workspace_id, "user_id": member_user_id, "actor_user_id": actor_user_id, "previous_role": target_role, "previous_status": target_status, "role": role_key, "status": status})).await?;
    transaction.commit().await.map_err(TeamError::Persistence)?;
    Ok(member)
}

pub async fn revoke_member(
    database: &PgPool,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    member_user_id: Uuid,
    correlation_id: &str,
) -> Result<Member, TeamError> {
    let mut transaction = database.begin().await.map_err(TeamError::Persistence)?;
    let actor_role = require_permission(
        &mut transaction,
        workspace_id,
        actor_user_id,
        "members.manage",
    )
    .await?;
    let target = sqlx::query("SELECT role,status FROM tenancy_access.workspace_memberships WHERE workspace_id = $1 AND user_id = $2 FOR UPDATE")
        .bind(workspace_id).bind(member_user_id).fetch_optional(&mut *transaction).await.map_err(TeamError::Persistence)?.ok_or(TeamError::NotFound)?;
    let target_role: String = target.try_get("role").map_err(TeamError::Persistence)?;
    let target_status: String = target.try_get("status").map_err(TeamError::Persistence)?;
    if target_role == "owner"
        || (actor_role != "owner" && (member_user_id == actor_user_id || target_role == "admin"))
    {
        return Err(TeamError::Forbidden);
    }
    if target_status == "revoked" {
        let member = find_member(&mut transaction, workspace_id, member_user_id).await?;
        transaction.commit().await.map_err(TeamError::Persistence)?;
        return Ok(member);
    }
    let row = sqlx::query("UPDATE tenancy_access.workspace_memberships SET status = 'revoked', row_version = row_version + 1 WHERE workspace_id = $1 AND user_id = $2 RETURNING workspace_id, user_id, email_normalized, role, status, floor(extract(epoch FROM joined_at))::bigint AS joined_at_unix, row_version")
        .bind(workspace_id).bind(member_user_id).fetch_optional(&mut *transaction).await.map_err(TeamError::Persistence)?.ok_or(TeamError::NotFound)?;
    clear_active_selections(&mut transaction, workspace_id, member_user_id).await?;
    let member = member_from_row(row)?;
    crate::append_team_event!(
        &mut transaction,
        workspace_id,
        Some(actor_user_id),
        "member.revoked",
        member.row_version,
        correlation_id,
        "tenancy_access.member.revoked",
        &serde_json::json!({"workspace_id": workspace_id, "user_id": member_user_id, "actor_user_id": actor_user_id}),
    )
    .await?;
    transaction.commit().await.map_err(TeamError::Persistence)?;
    Ok(member)
}

async fn ensure_assignable_role(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    role_key: &str,
    actor_role: &str,
) -> Result<(), TeamError> {
    let permissions: Vec<String> = sqlx::query_scalar("SELECT permissions FROM tenancy_access.workspace_roles WHERE workspace_id = $1 AND role_key = $2")
        .bind(workspace_id).bind(role_key).fetch_optional(&mut **transaction).await.map_err(TeamError::Persistence)?.ok_or(TeamError::InvalidInput)?;
    if role_key == "owner"
        || (actor_role != "owner"
            && (role_key == "admin" || permissions.contains(&"ownership.transfer".to_owned())))
    {
        return Err(TeamError::Forbidden);
    }
    if actor_role != "owner" {
        let actor_permissions:Vec<String>=sqlx::query_scalar("SELECT permissions FROM tenancy_access.workspace_roles WHERE workspace_id=$1 AND role_key=$2").bind(workspace_id).bind(actor_role).fetch_one(&mut **transaction).await.map_err(TeamError::Persistence)?;
        if permissions
            .iter()
            .any(|permission| !actor_permissions.contains(permission))
        {
            return Err(TeamError::Forbidden);
        }
    }
    Ok(())
}

async fn clear_active_selections(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    member_user_id: Uuid,
) -> Result<(), TeamError> {
    sqlx::query("DELETE FROM tenancy_access.active_workspace_selections WHERE workspace_id = $1 AND user_id = $2").bind(workspace_id).bind(member_user_id).execute(&mut **transaction).await.map_err(TeamError::Persistence)?;
    Ok(())
}

pub async fn find_member(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    user_id: Uuid,
) -> Result<Member, TeamError> {
    let row = sqlx::query("SELECT workspace_id, user_id, email_normalized, role, status, floor(extract(epoch FROM joined_at))::bigint AS joined_at_unix, row_version FROM tenancy_access.workspace_memberships WHERE workspace_id=$1 AND user_id=$2")
        .bind(workspace_id).bind(user_id).fetch_optional(&mut **transaction).await.map_err(TeamError::Persistence)?.ok_or(TeamError::NotFound)?;
    member_from_row(row)
}

fn member_from_row(row: sqlx::postgres::PgRow) -> Result<Member, TeamError> {
    Ok(Member {
        workspace_id: row
            .try_get("workspace_id")
            .map_err(TeamError::Persistence)?,
        user_id: row.try_get("user_id").map_err(TeamError::Persistence)?,
        email: row
            .try_get("email_normalized")
            .map_err(TeamError::Persistence)?,
        role_key: row.try_get("role").map_err(TeamError::Persistence)?,
        status: row.try_get("status").map_err(TeamError::Persistence)?,
        joined_at_unix: row
            .try_get("joined_at_unix")
            .map_err(TeamError::Persistence)?,
        row_version: row.try_get("row_version").map_err(TeamError::Persistence)?,
    })
}

fn decode_cursor(cursor: Option<&str>) -> Result<(Option<i64>, Option<Uuid>), TeamError> {
    let Some(cursor) = cursor.filter(|value| !value.is_empty()) else {
        return Ok((None, None));
    };
    let value = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| TeamError::InvalidInput)?;
    let value = String::from_utf8(value).map_err(|_| TeamError::InvalidInput)?;
    let (timestamp, user_id) = value.split_once('.').ok_or(TeamError::InvalidInput)?;
    Ok((
        Some(timestamp.parse().map_err(|_| TeamError::InvalidInput)?),
        Some(Uuid::parse_str(user_id).map_err(|_| TeamError::InvalidInput)?),
    ))
}

fn encode_cursor(timestamp: i64, user_id: Uuid) -> String {
    URL_SAFE_NO_PAD.encode(format!("{timestamp}.{user_id}"))
}
