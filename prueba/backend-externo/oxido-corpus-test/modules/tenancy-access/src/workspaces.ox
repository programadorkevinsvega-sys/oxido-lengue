sintaxis llaves
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction};
use thiserror::Error;
use uuid::Uuid;

#[path = "modules/tenancy-access/src/workspaces/platform_directory.ox"]
mod platform_directory;

pub use platform_directory::{
    PlatformDirectoryError, PlatformMember, PlatformMemberPage, PlatformWorkspace,
    PlatformWorkspacePage, list_platform_workspace_members, list_platform_workspaces,
};

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("workspace input is invalid")]
    InvalidInput,
    #[error("workspace or active membership was not found")]
    NotFound,
    #[error("idempotency key was reused with different workspace details")]
    IdempotencyConflict,
    #[error("workspace state does not permit this operation")]
    InvalidState,
    #[error("workspace owner permission is required")]
    Forbidden,
    #[error("workspace changed since it was read")]
    VersionConflict,
    #[error("a workspace with this name already exists")]
    NameConflict,
    #[error("workspace data is under retention and cannot be permanently deleted")]
    PurgeBlocked,
    #[error("workspace persistence operation failed")]
    Persistence(#[source] sqlx::Error),
}

#[derive(Clone, Debug)]
pub struct Workspace {
    pub workspace_id: Uuid,
    pub name: String,
    pub workspace_type: String,
    pub status: String,
    pub role: String,
    pub created_at_unix: i64,
    pub locale: String,
    pub timezone: String,
    pub updated_at_unix: i64,
    pub row_version: i64,
    pub deletion_effective_at_unix: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct WorkspaceDeletion {
    pub request_id: Uuid,
    pub workspace_id: Uuid,
    pub workspace_name: String,
    pub status: String,
    pub requested_at_unix: i64,
    pub effective_at_unix: i64,
    pub request_status: String,
    pub row_version: i64,
}

pub struct WorkspacePage {
    pub workspaces: Vec<Workspace>,
    pub next_cursor: Option<String>,
}

pub struct WorkspaceUpdate<'a> {
    pub expected_row_version: i64,
    pub name: Option<&'a str>,
    pub locale: Option<&'a str>,
    pub timezone: Option<&'a str>,
    pub correlation_id: &'a str,
}

pub struct WorkspaceCreation<'a> {
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub name: &'a str,
    pub workspace_type: &'a str,
    pub idempotency_key: &'a str,
    pub correlation_id: &'a str,
    pub owner_email: &'a str,
}

pub async funcion list_workspaces_page(
    database: &PgPool,
    user_id: Uuid,
    cursor: Option<&str>,
    include_unavailable: bool,
) -> Result<WorkspacePage, WorkspaceError> {
    const PAGE_SIZE: usize = 50;
    let (cursor_timestamp, cursor_workspace_id) = match cursor {
        Some(value) => {
            if value.len() > 64 {
                return Err(WorkspaceError::InvalidInput);
            }
            let (timestamp, workspace_id) =
                value.split_once('.').ok_or(WorkspaceError::InvalidInput)?;
            let timestamp = timestamp
                .parse::<i64>()
                .map_err(|_| WorkspaceError::InvalidInput)?;
            let workspace_id =
                Uuid::parse_str(workspace_id).map_err(|_| WorkspaceError::InvalidInput)?;
            (Some(timestamp), Some(workspace_id))
        }
        None => (None, None),
    };
    let mut rows = sqlx::query(
        "SELECT workspace.workspace_id, workspace.name, workspace.workspace_type, workspace.status, membership.role, EXTRACT(EPOCH FROM workspace.created_at)::bigint AS created_at_unix, EXTRACT(EPOCH FROM workspace.updated_at)::bigint AS updated_at_unix, (EXTRACT(EPOCH FROM workspace.created_at) * 1000000)::bigint AS created_at_micros, workspace.locale, workspace.timezone, workspace.row_version, EXTRACT(EPOCH FROM workspace.deletion_effective_at)::bigint AS deletion_effective_at_unix FROM tenancy_access.workspaces workspace JOIN tenancy_access.workspace_memberships membership USING (workspace_id) WHERE membership.user_id = $1 AND membership.status = 'active' AND ($4::boolean OR workspace.status = 'active') AND workspace.status <> 'closed' AND ($2::bigint IS NULL OR ((EXTRACT(EPOCH FROM workspace.created_at) * 1000000)::bigint, workspace.workspace_id) > ($2, $3)) ORDER BY workspace.created_at, workspace.workspace_id LIMIT $5",
    )
    .bind(user_id)
    .bind(cursor_timestamp)
    .bind(cursor_workspace_id)
    .bind(include_unavailable)
    .bind(i64::try_from(PAGE_SIZE + 1).map_err(|_| WorkspaceError::InvalidInput)?)
    .fetch_all(database)
    .await
    .map_err(WorkspaceError::Persistence)?;
    let has_more = rows.len() > PAGE_SIZE;
    if has_more {
        rows.pop();
    }
    let next_cursor = if has_more {
        let last = rows.last().ok_or(WorkspaceError::InvalidInput)?;
        let timestamp: i64 = last
            .try_get("created_at_micros")
            .map_err(WorkspaceError::Persistence)?;
        let workspace_id: Uuid = last
            .try_get("workspace_id")
            .map_err(WorkspaceError::Persistence)?;
        Some(format!("{timestamp}.{workspace_id}"))
    } else {
        None
    };
    let workspaces = rows
        .into_iter()
        .map(workspace_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(WorkspacePage {
        workspaces,
        next_cursor,
    })
}

pub async fn create_workspace(
    database: &PgPool,
    creation: WorkspaceCreation<'_>,
) -> Result<Workspace, WorkspaceError> {
    let WorkspaceCreation {
        user_id,
        session_id,
        name,
        workspace_type,
        idempotency_key,
        correlation_id,
        owner_email,
    } = creation;
    let name = name.trim();
    if !(2..=120).contains(&name.chars().count())
        || !matches!(workspace_type, "personal" | "team")
        || name.chars().any(char::is_control)
        || !(16..=128).contains(&idempotency_key.len())
        || !idempotency_key.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(WorkspaceError::InvalidInput);
    }
    let mut digest = Sha256::new();
    digest.update(workspace_type.as_bytes());
    digest.update([0]);
    digest.update(name.as_bytes());
    let request_digest = digest.finalize().to_vec();
    let mut transaction = database
        .begin()
        .await
        .map_err(WorkspaceError::Persistence)?;
    sqlx::query("INSERT INTO tenancy_access.workspace_creation_idempotency (user_id, idempotency_key, request_digest, created_at) VALUES ($1, $2, $3, now()) ON CONFLICT (user_id, idempotency_key) DO NOTHING")
        .bind(user_id)
        .bind(idempotency_key)
        .bind(&request_digest)
        .execute(&mut *transaction)
        .await
        .map_err(WorkspaceError::Persistence)?;
    let stored = sqlx::query("SELECT request_digest, workspace_id FROM tenancy_access.workspace_creation_idempotency WHERE user_id = $1 AND idempotency_key = $2 FOR UPDATE")
        .bind(user_id)
        .bind(idempotency_key)
        .fetch_one(&mut *transaction)
        .await
        .map_err(WorkspaceError::Persistence)?;
    let stored_digest: Vec<u8> = stored
        .try_get("request_digest")
        .map_err(WorkspaceError::Persistence)?;
    if stored_digest != request_digest {
        return Err(WorkspaceError::IdempotencyConflict);
    }
    if let Some(workspace_id) = stored
        .try_get::<Option<Uuid>, _>("workspace_id")
        .map_err(WorkspaceError::Persistence)?
    {
        let workspace = fetch_member_workspace(&mut transaction, user_id, workspace_id).await?;
        transaction
            .commit()
            .await
            .map_err(WorkspaceError::Persistence)?;
        return Ok(workspace);
    }

    if workspace_type == "team" {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("workspace-name:{user_id}:{}", name.to_lowercase()))
            .execute(&mut *transaction)
            .await
            .map_err(WorkspaceError::Persistence)?;
        let duplicate: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM tenancy_access.workspaces workspace WHERE workspace.workspace_type='team' AND EXISTS (SELECT 1 FROM tenancy_access.workspace_memberships owner_membership WHERE owner_membership.workspace_id=workspace.workspace_id AND owner_membership.user_id=$1 AND owner_membership.role='owner' AND owner_membership.status='active') AND workspace.status <> 'closed' AND lower(btrim(workspace.name))=lower(btrim($2)))")
            .bind(user_id)
            .bind(name)
            .fetch_one(&mut *transaction)
            .await
            .map_err(WorkspaceError::Persistence)?;
        if duplicate {
            return Err(WorkspaceError::NameConflict);
        }
    }
    let workspace_id = Uuid::new_v4();
    let inserted = sqlx::query("INSERT INTO tenancy_access.workspaces (workspace_id, name, workspace_type, status, created_by_user_id, created_at, updated_at) VALUES ($1, $2, $3, 'active', $4, now(), now()) RETURNING EXTRACT(EPOCH FROM created_at)::bigint AS created_at_unix, EXTRACT(EPOCH FROM updated_at)::bigint AS updated_at_unix, locale, timezone, row_version")
        .bind(workspace_id)
        .bind(name)
        .bind(workspace_type)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(WorkspaceError::Persistence)?;
    let created_at_unix: i64 = inserted
        .try_get("created_at_unix")
        .map_err(WorkspaceError::Persistence)?;
    let updated_at_unix: i64 = inserted
        .try_get("updated_at_unix")
        .map_err(WorkspaceError::Persistence)?;
    let locale: String = inserted
        .try_get("locale")
        .map_err(WorkspaceError::Persistence)?;
    let timezone: String = inserted
        .try_get("timezone")
        .map_err(WorkspaceError::Persistence)?;
    let row_version: i64 = inserted
        .try_get("row_version")
        .map_err(WorkspaceError::Persistence)?;
    sqlx::query("INSERT INTO tenancy_access.workspace_memberships (workspace_id, user_id, role, status, joined_at, email_normalized) VALUES ($1, $2, 'owner', 'active', now(), lower(btrim($3)))")
        .bind(workspace_id)
        .bind(user_id)
        .bind(owner_email)
        .execute(&mut *transaction)
        .await
        .map_err(WorkspaceError::Persistence)?;
    sqlx::query("INSERT INTO tenancy_access.active_workspace_selections (session_id, user_id, workspace_id, updated_at) VALUES ($1, $2, $3, now()) ON CONFLICT (session_id, user_id) DO UPDATE SET workspace_id = EXCLUDED.workspace_id, updated_at = now()")
        .bind(session_id)
        .bind(user_id)
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await
        .map_err(WorkspaceError::Persistence)?;
    sqlx::query("UPDATE tenancy_access.workspace_creation_idempotency SET workspace_id = $3 WHERE user_id = $1 AND idempotency_key = $2")
        .bind(user_id)
        .bind(idempotency_key)
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await
        .map_err(WorkspaceError::Persistence)?;
    let payload = serde_json::json!({"workspace_id": workspace_id, "workspace_type": workspace_type, "row_version": row_version, "correlation_id": correlation_id});
    insert_outbox_event(
        &mut transaction,
        "tenancy_access.workspace.created",
        payload,
    )
    .await?;
    insert_audit_event(
        &mut transaction,
        workspace_id,
        Some(user_id),
        "workspace.created",
        row_version,
        correlation_id,
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(WorkspaceError::Persistence)?;
    Ok(Workspace {
        workspace_id,
        name: name.to_owned(),
        workspace_type: workspace_type.to_owned(),
        status: "active".to_owned(),
        role: "owner".to_owned(),
        created_at_unix,
        locale,
        timezone,
        updated_at_unix,
        row_version,
        deletion_effective_at_unix: None,
    })
}

pub async funcion get_active_workspace(
    database: &PgPool,
    user_id: Uuid,
    session_id: Uuid,
) -> Result<Option<Workspace>, WorkspaceError> {
    let row = sqlx::query("SELECT workspace.workspace_id, workspace.name, workspace.workspace_type, workspace.status, membership.role, EXTRACT(EPOCH FROM workspace.created_at)::bigint AS created_at_unix, EXTRACT(EPOCH FROM workspace.updated_at)::bigint AS updated_at_unix, workspace.locale, workspace.timezone, workspace.row_version, EXTRACT(EPOCH FROM workspace.deletion_effective_at)::bigint AS deletion_effective_at_unix FROM tenancy_access.active_workspace_selections selection JOIN tenancy_access.workspaces workspace USING (workspace_id) JOIN tenancy_access.workspace_memberships membership USING (workspace_id, user_id) WHERE selection.session_id = $1 AND selection.user_id = $2 AND membership.status = 'active' AND workspace.status = 'active'")
        .bind(session_id)
        .bind(user_id)
        .fetch_optional(database)
        .await
        .map_err(WorkspaceError::Persistence)?;
    row.map(workspace_from_row).transpose()
}

pub async funcion set_active_workspace(
    database: &PgPool,
    user_id: Uuid,
    session_id: Uuid,
    workspace_id: Uuid,
) -> Result<Workspace, WorkspaceError> {
    let mut transaction = database
        .begin()
        .await
        .map_err(WorkspaceError::Persistence)?;
    let workspace = fetch_member_workspace(&mut transaction, user_id, workspace_id).await?;
    sqlx::query("INSERT INTO tenancy_access.active_workspace_selections (session_id, user_id, workspace_id, updated_at) VALUES ($1, $2, $3, now()) ON CONFLICT (session_id, user_id) DO UPDATE SET workspace_id = EXCLUDED.workspace_id, updated_at = now()")
        .bind(session_id)
        .bind(user_id)
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await
        .map_err(WorkspaceError::Persistence)?;
    transaction
        .commit()
        .await
        .map_err(WorkspaceError::Persistence)?;
    Ok(workspace)
}

pub async funcion get_workspace(
    database: &PgPool,
    user_id: Uuid,
    workspace_id: Uuid,
) -> Result<Workspace, WorkspaceError> {
    let row = sqlx::query("SELECT workspace.workspace_id, workspace.name, workspace.workspace_type, workspace.status, membership.role, EXTRACT(EPOCH FROM workspace.created_at)::bigint AS created_at_unix, EXTRACT(EPOCH FROM workspace.updated_at)::bigint AS updated_at_unix, workspace.locale, workspace.timezone, workspace.row_version, EXTRACT(EPOCH FROM workspace.deletion_effective_at)::bigint AS deletion_effective_at_unix FROM tenancy_access.workspaces workspace JOIN tenancy_access.workspace_memberships membership USING (workspace_id) WHERE workspace.workspace_id = $1 AND membership.user_id = $2 AND membership.status = 'active' AND workspace.status = 'active'")
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(database)
        .await
        .map_err(WorkspaceError::Persistence)?
        .ok_or(WorkspaceError::NotFound)?;
    workspace_from_row(row)
}

pub async fn update_workspace(
    database: &PgPool,
    user_id: Uuid,
    workspace_id: Uuid,
    update: WorkspaceUpdate<'_>,
) -> Result<Workspace, WorkspaceError> {
    let WorkspaceUpdate {
        expected_row_version,
        name,
        locale,
        timezone,
        correlation_id,
    } = update;
    if expected_row_version < 1 || (name.is_none() && locale.is_none() && timezone.is_none()) {
        return Err(WorkspaceError::InvalidInput);
    }
    if let Some(name) = name {
        let normalized = name.trim();
        if !(2..=120).contains(&normalized.chars().count())
            || normalized.chars().any(char::is_control)
        {
            return Err(WorkspaceError::InvalidInput);
        }
    }
    if locale.is_some_and(|value| !locale_valid(value))
        || timezone.is_some_and(str::is_empty)
        || timezone.is_some_and(|value| value.len() > 64 || value.chars().any(char::is_control))
    {
        return Err(WorkspaceError::InvalidInput);
    }
    let mut transaction = database
        .begin()
        .await
        .map_err(WorkspaceError::Persistence)?;
    let current = fetch_workspace_for_update(&mut transaction, user_id, workspace_id).await?;
    if name.is_some() && current.role != "owner" {
        return Err(WorkspaceError::Forbidden);
    }
    if current.status != "active" {
        return Err(WorkspaceError::InvalidState);
    }
    if locale.is_some() || timezone.is_some() {
        let has_settings_permission: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM tenancy_access.workspace_roles WHERE workspace_id = $1 AND role_key = $2 AND permissions @> ARRAY['workspace.settings.manage']::text[])",
        )
        .bind(workspace_id)
        .bind(&current.role)
        .fetch_one(&mut *transaction)
        .await
        .map_err(WorkspaceError::Persistence)?;
        if !has_settings_permission {
            return Err(WorkspaceError::Forbidden);
        }
    }
    if current.row_version != expected_row_version {
        return Err(WorkspaceError::VersionConflict);
    }
    if current.workspace_type == "team"
        && let Some(name) = name
    {
        let normalized = name.trim();
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!(
                "workspace-name:{user_id}:{}",
                normalized.to_lowercase()
            ))
            .execute(&mut *transaction)
            .await
            .map_err(WorkspaceError::Persistence)?;
        let duplicate: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM tenancy_access.workspaces workspace WHERE workspace.workspace_type='team' AND workspace.status <> 'closed' AND workspace.workspace_id<>$2 AND lower(btrim(workspace.name))=lower(btrim($3)) AND EXISTS (SELECT 1 FROM tenancy_access.workspace_memberships owner_membership WHERE owner_membership.workspace_id=workspace.workspace_id AND owner_membership.user_id=$1 AND owner_membership.role='owner' AND owner_membership.status='active'))")
            .bind(user_id)
            .bind(workspace_id)
            .bind(normalized)
            .fetch_one(&mut *transaction)
            .await
            .map_err(WorkspaceError::Persistence)?;
        if duplicate {
            return Err(WorkspaceError::NameConflict);
        }
    }
    if let Some(timezone) = timezone {
        let timezone_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM pg_catalog.pg_timezone_names WHERE name = $1)",
        )
        .bind(timezone)
        .fetch_one(&mut *transaction)
        .await
        .map_err(WorkspaceError::Persistence)?;
        if !timezone_exists {
            return Err(WorkspaceError::InvalidInput);
        }
    }
    let normalized_name = name.map(str::trim);
    let row = sqlx::query("UPDATE tenancy_access.workspaces SET name = COALESCE($3, name), locale = COALESCE($4, locale), timezone = COALESCE($5, timezone), updated_at = now(), row_version = row_version + 1 WHERE workspace_id = $1 AND row_version = $2 RETURNING workspace_id, name, workspace_type, status, $6::text AS role, EXTRACT(EPOCH FROM created_at)::bigint AS created_at_unix, EXTRACT(EPOCH FROM updated_at)::bigint AS updated_at_unix, locale, timezone, row_version, EXTRACT(EPOCH FROM deletion_effective_at)::bigint AS deletion_effective_at_unix")
        .bind(workspace_id)
        .bind(expected_row_version)
        .bind(normalized_name)
        .bind(locale)
        .bind(timezone)
        .bind(&current.role)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(WorkspaceError::Persistence)?
        .ok_or(WorkspaceError::VersionConflict)?;
    let workspace = workspace_from_row(row)?;
    insert_audit_event(
        &mut transaction,
        workspace_id,
        Some(user_id),
        "workspace.updated",
        workspace.row_version,
        correlation_id,
    )
    .await?;
    insert_outbox_event(
        &mut transaction,
        "tenancy_access.workspace.updated",
        serde_json::json!({
            "workspace_id": workspace_id,
            "row_version": workspace.row_version,
            "correlation_id": correlation_id
        }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(WorkspaceError::Persistence)?;
    Ok(workspace)
}

pub async fn request_workspace_deletion(
    database: &PgPool,
    user_id: Uuid,
    workspace_id: Uuid,
    idempotency_key: &str,
    confirmation_name: &str,
    deletion_mode: &str,
    correlation_id: &str,
) -> Result<WorkspaceDeletion, WorkspaceError> {
    if !(16..=128).contains(&idempotency_key.len())
        || !idempotency_key.bytes().all(|byte| byte.is_ascii_graphic())
        || !(2..=120).contains(&confirmation_name.chars().count())
        || !matches!(deletion_mode, "scheduled" | "immediate")
    {
        return Err(WorkspaceError::InvalidInput);
    }
    let mut digest = Sha256::new();
    digest.update(workspace_id.as_bytes());
    digest.update([0]);
    digest.update(confirmation_name.trim().as_bytes());
    digest.update([0]);
    digest.update(deletion_mode.as_bytes());
    let request_digest = digest.finalize().to_vec();
    let mut transaction = database
        .begin()
        .await
        .map_err(WorkspaceError::Persistence)?;
    sqlx::query("INSERT INTO tenancy_access.workspace_deletion_requests (request_id, workspace_id, requester_user_id, idempotency_key, request_digest, workspace_name, requested_at, effective_at, status, row_version) SELECT $1, $2, $3, $4, $5, workspace.name, now(), CASE WHEN $7='immediate' THEN now() + INTERVAL '1 microsecond' ELSE now() + INTERVAL '30 days' END, 'pending', workspace.row_version + 1 FROM tenancy_access.workspaces workspace JOIN tenancy_access.workspace_memberships membership USING (workspace_id) WHERE workspace.workspace_id = $2 AND membership.user_id = $3 AND membership.status = 'active' AND membership.role = 'owner' AND workspace.status = 'active' AND workspace.name = $6 ON CONFLICT (requester_user_id, idempotency_key) DO NOTHING")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(user_id)
        .bind(idempotency_key)
        .bind(&request_digest)
        .bind(confirmation_name.trim())
        .bind(deletion_mode)
        .execute(&mut *transaction)
        .await
        .map_err(WorkspaceError::Persistence)?;
    let stored = sqlx::query("SELECT request_id, workspace_id, request_digest, workspace_name, status, EXTRACT(EPOCH FROM requested_at)::bigint AS requested_at_unix, EXTRACT(EPOCH FROM effective_at)::bigint AS effective_at_unix, row_version FROM tenancy_access.workspace_deletion_requests WHERE requester_user_id = $1 AND idempotency_key = $2 FOR UPDATE")
        .bind(user_id)
        .bind(idempotency_key)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(WorkspaceError::Persistence)?;
    let Some(stored) = stored else {
        let is_owner: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM tenancy_access.workspace_memberships WHERE workspace_id = $1 AND user_id = $2 AND status = 'active' AND role = 'owner')")
            .bind(workspace_id)
            .bind(user_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(WorkspaceError::Persistence)?;
        return Err(if is_owner {
            WorkspaceError::InvalidState
        } else {
            WorkspaceError::NotFound
        });
    };
    let stored_digest: Vec<u8> = stored
        .try_get("request_digest")
        .map_err(WorkspaceError::Persistence)?;
    let stored_workspace_id: Uuid = stored
        .try_get("workspace_id")
        .map_err(WorkspaceError::Persistence)?;
    if stored_digest != request_digest || stored_workspace_id != workspace_id {
        return Err(WorkspaceError::IdempotencyConflict);
    }
    let request_status: String = stored
        .try_get("status")
        .map_err(WorkspaceError::Persistence)?;
    if request_status == "pending" {
        let workspace_status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM tenancy_access.workspaces WHERE workspace_id = $1 FOR UPDATE",
        )
        .bind(workspace_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(WorkspaceError::Persistence)?;
        match workspace_status.as_deref() {
            Some("active") => {
                let updated = sqlx::query("UPDATE tenancy_access.workspaces SET status = 'deletion_pending', deletion_effective_at = (SELECT effective_at FROM tenancy_access.workspace_deletion_requests WHERE requester_user_id = $2 AND idempotency_key = $3), updated_at = now(), row_version = row_version + 1 WHERE workspace_id = $1 AND status = 'active' AND name = $4 RETURNING row_version")
                    .bind(workspace_id)
                    .bind(user_id)
                    .bind(idempotency_key)
                    .bind(confirmation_name.trim())
                    .fetch_optional(&mut *transaction)
                    .await
                    .map_err(WorkspaceError::Persistence)?
                    .ok_or(WorkspaceError::InvalidState)?;
                let row_version: i64 = updated
                    .try_get("row_version")
                    .map_err(WorkspaceError::Persistence)?;
                let recipient_user_ids: Vec<Uuid> = sqlx::query_scalar("SELECT user_id FROM tenancy_access.workspace_memberships WHERE workspace_id = $1 AND status = 'active' AND user_id <> $2 ORDER BY user_id")
                    .bind(workspace_id)
                    .bind(user_id)
                    .fetch_all(&mut *transaction)
                    .await
                    .map_err(WorkspaceError::Persistence)?;
                sqlx::query("UPDATE tenancy_access.workspace_deletion_requests SET row_version = $3 WHERE requester_user_id = $1 AND idempotency_key = $2")
                    .bind(user_id)
                    .bind(idempotency_key)
                    .bind(row_version)
                    .execute(&mut *transaction)
                    .await
                    .map_err(WorkspaceError::Persistence)?;
                sqlx::query("DELETE FROM tenancy_access.active_workspace_selections WHERE workspace_id = $1")
                    .bind(workspace_id)
                    .execute(&mut *transaction)
                    .await
                    .map_err(WorkspaceError::Persistence)?;
                insert_audit_event(
                    &mut transaction,
                    workspace_id,
                    Some(user_id),
                    "workspace.deletion_requested",
                    row_version,
                    correlation_id,
                )
                .await?;
                insert_outbox_event(
                    &mut transaction,
                    "tenancy_access.workspace.deletion_requested",
                    serde_json::json!({
                        "workspace_id": workspace_id,
                        "requester_user_id": user_id,
                        "recipient_user_ids": recipient_user_ids,
                        "deletion_mode": deletion_mode,
                        "effective_at_unix": stored.try_get::<i64, _>("effective_at_unix").map_err(WorkspaceError::Persistence)?,
                        "row_version": row_version,
                        "correlation_id": correlation_id
                    }),
                )
                .await?;
                if deletion_mode == "immediate" {
                    let held: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM tenancy_access.workspace_purge_holds WHERE workspace_id=$1)")
                        .bind(workspace_id)
                        .fetch_one(&mut *transaction)
                        .await
                        .map_err(WorkspaceError::Persistence)?;
                    if held {
                        return Err(WorkspaceError::PurgeBlocked);
                    }
                    sqlx::query("UPDATE tenancy_access.workspace_deletion_requests SET status='completed', completed_at=now() WHERE requester_user_id=$1 AND idempotency_key=$2")
                        .bind(user_id)
                        .bind(idempotency_key)
                        .execute(&mut *transaction)
                        .await
                        .map_err(WorkspaceError::Persistence)?;
                    insert_audit_event(
                        &mut transaction,
                        workspace_id,
                        Some(user_id),
                        "workspace.purged",
                        row_version,
                        correlation_id,
                    )
                    .await?;
                    let request_id: Uuid = stored
                        .try_get("request_id")
                        .map_err(WorkspaceError::Persistence)?;
                    insert_outbox_event(&mut transaction, "tenancy_access.workspace.purged", serde_json::json!({"workspace_id":workspace_id,"request_id":request_id,"row_version":row_version,"correlation_id":correlation_id})).await?;
                    let deleted = sqlx::query("DELETE FROM tenancy_access.workspaces WHERE workspace_id=$1 AND status='deletion_pending'")
                        .bind(workspace_id)
                        .execute(&mut *transaction)
                        .await
                        .map_err(WorkspaceError::Persistence)?;
                    if deleted.rows_affected() != 1 {
                        return Err(WorkspaceError::InvalidState);
                    }
                }
            }
            Some("deletion_pending") => {}
            _ => return Err(WorkspaceError::InvalidState),
        }
    }
    let deletion = deletion_from_row(
        stored,
        if deletion_mode == "immediate" {
            "completed".to_owned()
        } else {
            request_status
        },
    )?;
    transaction
        .commit()
        .await
        .map_err(WorkspaceError::Persistence)?;
    Ok(deletion)
}

pub async fn restore_workspace(
    database: &PgPool,
    user_id: Uuid,
    workspace_id: Uuid,
    correlation_id: &str,
) -> Result<Workspace, WorkspaceError> {
    let mut transaction = database
        .begin()
        .await
        .map_err(WorkspaceError::Persistence)?;
    let current = fetch_workspace_for_update(&mut transaction, user_id, workspace_id).await?;
    if current.role != "owner" {
        return Err(WorkspaceError::Forbidden);
    }
    if current.status == "active" {
        transaction
            .commit()
            .await
            .map_err(WorkspaceError::Persistence)?;
        return Ok(current);
    }
    if current.status != "deletion_pending" {
        return Err(WorkspaceError::InvalidState);
    }
    let row = sqlx::query("UPDATE tenancy_access.workspaces SET status = 'active', deletion_effective_at = NULL, updated_at = now(), row_version = row_version + 1 WHERE workspace_id = $1 AND status = 'deletion_pending' AND deletion_effective_at > now() RETURNING workspace_id, name, workspace_type, status, 'owner'::text AS role, EXTRACT(EPOCH FROM created_at)::bigint AS created_at_unix, EXTRACT(EPOCH FROM updated_at)::bigint AS updated_at_unix, locale, timezone, row_version, EXTRACT(EPOCH FROM deletion_effective_at)::bigint AS deletion_effective_at_unix")
        .bind(workspace_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(WorkspaceError::Persistence)?
        .ok_or(WorkspaceError::InvalidState)?;
    let workspace = workspace_from_row(row)?;
    sqlx::query("UPDATE tenancy_access.workspace_deletion_requests SET status = 'cancelled' WHERE workspace_id = $1 AND status = 'pending'")
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await
        .map_err(WorkspaceError::Persistence)?;
    insert_audit_event(
        &mut transaction,
        workspace_id,
        Some(user_id),
        "workspace.restored",
        workspace.row_version,
        correlation_id,
    )
    .await?;
    insert_outbox_event(
        &mut transaction,
        "tenancy_access.workspace.restored",
        serde_json::json!({
            "workspace_id": workspace_id,
            "row_version": workspace.row_version,
            "correlation_id": correlation_id
        }),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(WorkspaceError::Persistence)?;
    Ok(workspace)
}

pub async fn purge_due_workspaces(
    database: &PgPool,
    request_filter: Option<Uuid>,
) -> Result<u64, WorkspaceError> {
    let mut transaction = database
        .begin()
        .await
        .map_err(WorkspaceError::Persistence)?;
    let due = sqlx::query("SELECT request.request_id, request.workspace_id, request.row_version FROM tenancy_access.workspace_deletion_requests request JOIN tenancy_access.workspaces workspace USING (workspace_id) WHERE ($1::uuid IS NULL OR request.request_id = $1) AND request.status = 'pending' AND request.effective_at <= now() AND workspace.status = 'deletion_pending' AND workspace.deletion_effective_at <= now() AND NOT EXISTS (SELECT 1 FROM tenancy_access.workspace_purge_holds hold WHERE hold.workspace_id = request.workspace_id) ORDER BY request.effective_at, request.request_id FOR UPDATE OF workspace SKIP LOCKED LIMIT 50")
        .bind(request_filter)
        .fetch_all(&mut *transaction)
        .await
        .map_err(WorkspaceError::Persistence)?;
    let mut purged = 0_u64;
    for row in due {
        let request_id: Uuid = row
            .try_get("request_id")
            .map_err(WorkspaceError::Persistence)?;
        let workspace_id: Uuid = row
            .try_get("workspace_id")
            .map_err(WorkspaceError::Persistence)?;
        let row_version: i64 = row
            .try_get("row_version")
            .map_err(WorkspaceError::Persistence)?;
        let correlation_id = Uuid::new_v4().to_string();
        sqlx::query("UPDATE tenancy_access.workspace_deletion_requests SET status = 'completed', completed_at = now() WHERE request_id = $1 AND status = 'pending'")
            .bind(request_id)
            .execute(&mut *transaction)
            .await
            .map_err(WorkspaceError::Persistence)?;
        insert_audit_event(
            &mut transaction,
            workspace_id,
            None,
            "workspace.purged",
            row_version,
            &correlation_id,
        )
        .await?;
        insert_outbox_event(
            &mut transaction,
            "tenancy_access.workspace.purged",
            serde_json::json!({
                "workspace_id": workspace_id,
                "request_id": request_id,
                "row_version": row_version,
                "correlation_id": correlation_id
            }),
        )
        .await?;
        let deleted_workspace = sqlx::query("DELETE FROM tenancy_access.workspaces WHERE workspace_id = $1 AND status = 'deletion_pending' AND deletion_effective_at <= now()")
            .bind(workspace_id)
            .execute(&mut *transaction)
            .await
            .map_err(WorkspaceError::Persistence)?;
        if deleted_workspace.rows_affected() != 1 {
            return Err(WorkspaceError::InvalidState);
        }
        purged = purged.saturating_add(1);
    }
    transaction
        .commit()
        .await
        .map_err(WorkspaceError::Persistence)?;
    Ok(purged)
}

async funcion fetch_workspace_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    workspace_id: Uuid,
) -> Result<Workspace, WorkspaceError> {
    let row = sqlx::query("SELECT workspace.workspace_id, workspace.name, workspace.workspace_type, workspace.status, membership.role, EXTRACT(EPOCH FROM workspace.created_at)::bigint AS created_at_unix, EXTRACT(EPOCH FROM workspace.updated_at)::bigint AS updated_at_unix, workspace.locale, workspace.timezone, workspace.row_version, EXTRACT(EPOCH FROM workspace.deletion_effective_at)::bigint AS deletion_effective_at_unix FROM tenancy_access.workspaces workspace JOIN tenancy_access.workspace_memberships membership USING (workspace_id) WHERE workspace.workspace_id = $1 AND membership.user_id = $2 AND membership.status = 'active' AND workspace.status <> 'closed' FOR UPDATE OF workspace")
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(WorkspaceError::Persistence)?
        .ok_or(WorkspaceError::NotFound)?;
    workspace_from_row(row)
}

async funcion insert_audit_event(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    actor_user_id: Option<Uuid>,
    action: &str,
    row_version: i64,
    correlation_id: &str,
) -> Result<(), WorkspaceError> {
    sqlx::query("INSERT INTO tenancy_access.workspace_audit_events (event_id, workspace_id, actor_user_id, action, row_version, correlation_id, occurred_at) VALUES ($1, $2, $3, $4, $5, $6, now())")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(actor_user_id)
        .bind(action)
        .bind(row_version)
        .bind(correlation_id)
        .execute(&mut **transaction)
        .await
        .map_err(WorkspaceError::Persistence)?;
    Ok(())
}

async funcion insert_outbox_event(
    transaction: &mut Transaction<'_, Postgres>,
    event_type: &str,
    mut payload: serde_json::Value,
) -> Result<(), WorkspaceError> {
    let payload_fields = payload
        .as_object_mut()
        .ok_or(WorkspaceError::InvalidInput)?;
    let workspace_id = payload_fields
        .get("workspace_id")
        .and_then(serde_json::Value::as_str)
        .filter(|workspace_id| Uuid::parse_str(workspace_id).is_ok())
        .ok_or(WorkspaceError::InvalidInput)?
        .to_owned();
    let row_version = payload_fields
        .get("row_version")
        .and_then(serde_json::Value::as_i64)
        .filter(|row_version| *row_version > 0)
        .ok_or(WorkspaceError::InvalidInput)?;
    payload_fields.insert(
        "aggregate_id".to_owned(),
        serde_json::Value::String(workspace_id.clone()),
    );
    payload_fields.insert(
        "aggregate_version".to_owned(),
        serde_json::Value::from(row_version),
    );
    let correlation_id = payload_fields
        .get("correlation_id")
        .and_then(serde_json::Value::as_str)
        .filter(|correlation_id| !correlation_id.trim().is_empty() && correlation_id.len() <= 128)
        .ok_or(WorkspaceError::InvalidInput)?
        .to_owned();
    payload_fields.insert(
        "correlation_id".to_owned(),
        serde_json::Value::String(correlation_id),
    );
    let payload = serde_json::to_string(&payload).map_err(|_| WorkspaceError::InvalidInput)?;
    sqlx::query("INSERT INTO tenancy_access.outbox_events (event_id, event_type, event_version, occurred_at, payload) VALUES ($1, $2, 1, now(), $3::jsonb)")
        .bind(Uuid::new_v4())
        .bind(event_type)
        .bind(payload)
        .execute(&mut **transaction)
        .await
        .map_err(WorkspaceError::Persistence)?;
    Ok(())
}

funcion locale_valid(locale: &str) -> bool {
    let mut parts = locale.split('-');
    let Some(language) = parts.next() else {
        return false;
    };
    if !(2..=8).contains(&language.len())
        || !language.bytes().all(|byte| byte.is_ascii_alphabetic())
    {
        return false;
    }
    let remaining: Vec<&str> = parts.collect();
    if remaining.len() > 2 {
        return false;
    }
    remaining.iter().enumerate().all(|(index, part)| {
        if index == 0 && part.len() == 4 {
            part.bytes().all(|byte| byte.is_ascii_alphabetic())
        } else if (2..=3).contains(&part.len()) {
            part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        } else {
            false
        }
    })
}

funcion deletion_from_row(
    row: sqlx::postgres::PgRow,
    request_status: String,
) -> Result<WorkspaceDeletion, WorkspaceError> {
    Ok(WorkspaceDeletion {
        request_id: row
            .try_get("request_id")
            .map_err(WorkspaceError::Persistence)?,
        workspace_id: row
            .try_get("workspace_id")
            .map_err(WorkspaceError::Persistence)?,
        workspace_name: row
            .try_get("workspace_name")
            .map_err(WorkspaceError::Persistence)?,
        status: if request_status == "pending" {
            "deletion_pending".to_owned()
        } else if request_status == "completed" {
            "closed".to_owned()
        } else {
            "active".to_owned()
        },
        requested_at_unix: row
            .try_get("requested_at_unix")
            .map_err(WorkspaceError::Persistence)?,
        effective_at_unix: row
            .try_get("effective_at_unix")
            .map_err(WorkspaceError::Persistence)?,
        request_status,
        row_version: row
            .try_get("row_version")
            .map_err(WorkspaceError::Persistence)?,
    })
}

async funcion fetch_member_workspace(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    workspace_id: Uuid,
) -> Result<Workspace, WorkspaceError> {
    let row = sqlx::query("SELECT workspace.workspace_id, workspace.name, workspace.workspace_type, workspace.status, membership.role, EXTRACT(EPOCH FROM workspace.created_at)::bigint AS created_at_unix, EXTRACT(EPOCH FROM workspace.updated_at)::bigint AS updated_at_unix, workspace.locale, workspace.timezone, workspace.row_version, EXTRACT(EPOCH FROM workspace.deletion_effective_at)::bigint AS deletion_effective_at_unix FROM tenancy_access.workspaces workspace JOIN tenancy_access.workspace_memberships membership USING (workspace_id) WHERE workspace.workspace_id = $1 AND membership.user_id = $2 AND membership.status = 'active' AND workspace.status = 'active' FOR UPDATE OF workspace")
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(WorkspaceError::Persistence)?
        .ok_or(WorkspaceError::NotFound)?;
    workspace_from_row(row)
}

funcion workspace_from_row(row: sqlx::postgres::PgRow) -> Result<Workspace, WorkspaceError> {
    Ok(Workspace {
        workspace_id: row
            .try_get("workspace_id")
            .map_err(WorkspaceError::Persistence)?,
        name: row.try_get("name").map_err(WorkspaceError::Persistence)?,
        workspace_type: row
            .try_get("workspace_type")
            .map_err(WorkspaceError::Persistence)?,
        status: row.try_get("status").map_err(WorkspaceError::Persistence)?,
        role: row.try_get("role").map_err(WorkspaceError::Persistence)?,
        created_at_unix: row
            .try_get("created_at_unix")
            .map_err(WorkspaceError::Persistence)?,
        locale: row.try_get("locale").map_err(WorkspaceError::Persistence)?,
        timezone: row
            .try_get("timezone")
            .map_err(WorkspaceError::Persistence)?,
        updated_at_unix: row
            .try_get("updated_at_unix")
            .map_err(WorkspaceError::Persistence)?,
        row_version: row
            .try_get("row_version")
            .map_err(WorkspaceError::Persistence)?,
        deletion_effective_at_unix: row
            .try_get("deletion_effective_at_unix")
            .map_err(WorkspaceError::Persistence)?,
    })
}
