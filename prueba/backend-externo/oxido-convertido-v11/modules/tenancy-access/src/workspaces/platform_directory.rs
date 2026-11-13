use sqlx::{PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum PlatformDirectoryError {
    #[error("platform directory request is invalid")]
    InvalidInput,
    #[error("platform directory query failed")]
    Persistence(#[source] sqlx::Error),
}

#[derive(Clone, Debug)]
pub struct PlatformWorkspace {
    pub workspace_id: Uuid,
    pub name: String,
    pub workspace_type: String,
    pub status: String,
    pub created_at_unix: i64,
    pub member_count: i64,
    pub row_version: i64,
}

#[derive(Clone, Debug)]
pub struct PlatformWorkspacePage {
    pub workspaces: Vec<PlatformWorkspace>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PlatformMember {
    pub user_id: Uuid,
    pub role: String,
    pub status: String,
    pub joined_at_unix: i64,
}

#[derive(Clone, Debug)]
pub struct PlatformMemberPage {
    pub members: Vec<PlatformMember>,
    pub next_cursor: Option<String>,
}

pub async fn list_platform_workspaces(
    database: &PgPool,
    search: &str,
    cursor: &str,
    requested_page_size: i32,
) -> Result<PlatformWorkspacePage, PlatformDirectoryError> {
    if search.len() > 120 || search.chars().any(char::is_control) {
        return Err(PlatformDirectoryError::InvalidInput);
    }
    let page_size = normalize_page_size(requested_page_size)?;
    let (cursor_created_at, cursor_id) = parse_workspace_cursor(cursor)?;
    let rows = sqlx::query(
        "SELECT workspace.workspace_id, workspace.name, workspace.workspace_type, workspace.status, floor(extract(epoch FROM workspace.created_at))::bigint AS created_at_unix, floor(extract(epoch FROM workspace.created_at) * 1000000)::bigint AS cursor_created_at, workspace.row_version, count(membership.user_id)::bigint AS member_count FROM tenancy_access.workspaces AS workspace LEFT JOIN tenancy_access.workspace_memberships AS membership ON membership.workspace_id = workspace.workspace_id AND membership.status = 'active' WHERE workspace.status <> 'closed' AND ($1 = '' OR workspace.name ILIKE '%' || replace(replace(replace($1, E'\\\\', E'\\\\\\\\'), '%', E'\\\\%'), '_', E'\\\\_') || '%' ESCAPE E'\\\\') AND ($2::bigint IS NULL OR (floor(extract(epoch FROM workspace.created_at) * 1000000)::bigint, workspace.workspace_id) > ($2, $3)) GROUP BY workspace.workspace_id ORDER BY cursor_created_at, workspace.workspace_id LIMIT $4",
    )
    .bind(search.trim())
    .bind(cursor_created_at)
    .bind(cursor_id)
    .bind(i64::from(page_size) + 1)
    .fetch_all(database)
    .await
    .map_err(PlatformDirectoryError::Persistence)?;
    let mut workspaces = rows
        .into_iter()
        .map(|row| {
            Ok(PlatformWorkspace {
                workspace_id: row
                    .try_get("workspace_id")
                    .map_err(PlatformDirectoryError::Persistence)?,
                name: row
                    .try_get("name")
                    .map_err(PlatformDirectoryError::Persistence)?,
                workspace_type: row
                    .try_get("workspace_type")
                    .map_err(PlatformDirectoryError::Persistence)?,
                status: row
                    .try_get("status")
                    .map_err(PlatformDirectoryError::Persistence)?,
                created_at_unix: row
                    .try_get("created_at_unix")
                    .map_err(PlatformDirectoryError::Persistence)?,
                member_count: row
                    .try_get("member_count")
                    .map_err(PlatformDirectoryError::Persistence)?,
                row_version: row
                    .try_get("row_version")
                    .map_err(PlatformDirectoryError::Persistence)?,
            })
        })
        .collect::<Result<Vec<_>, PlatformDirectoryError>>()?;
    let has_more = workspaces.len()
        > usize::try_from(page_size).map_err(|_| PlatformDirectoryError::InvalidInput)?;
    if has_more {
        workspaces.pop();
    }
    let next_cursor = if has_more {
        let last = workspaces
            .last()
            .ok_or(PlatformDirectoryError::InvalidInput)?;
        let created_at = sqlx::query_scalar::<_, i64>("SELECT floor(extract(epoch FROM created_at) * 1000000)::bigint FROM tenancy_access.workspaces WHERE workspace_id = $1")
            .bind(last.workspace_id)
            .fetch_one(database)
            .await
            .map_err(PlatformDirectoryError::Persistence)?;
        Some(format!("{created_at}.{}", last.workspace_id))
    } else {
        None
    };
    Ok(PlatformWorkspacePage {
        workspaces,
        next_cursor,
    })
}

pub async fn list_platform_workspace_members(
    database: &PgPool,
    workspace_id: Uuid,
    cursor: &str,
    requested_page_size: i32,
) -> Result<PlatformMemberPage, PlatformDirectoryError> {
    let page_size = normalize_page_size(requested_page_size)?;
    let cursor_user_id = if cursor.is_empty() {
        None
    } else {
        Some(Uuid::parse_str(cursor).map_err(|_| PlatformDirectoryError::InvalidInput)?)
    };
    let rows = sqlx::query(
        "SELECT user_id, role, status, floor(extract(epoch FROM joined_at))::bigint AS joined_at_unix FROM tenancy_access.workspace_memberships WHERE workspace_id = $1 AND ($2::uuid IS NULL OR user_id > $2) ORDER BY user_id LIMIT $3",
    )
    .bind(workspace_id)
    .bind(cursor_user_id)
    .bind(i64::from(page_size) + 1)
    .fetch_all(database)
    .await
    .map_err(PlatformDirectoryError::Persistence)?;
    let mut members = rows
        .into_iter()
        .map(|row| {
            Ok(PlatformMember {
                user_id: row
                    .try_get("user_id")
                    .map_err(PlatformDirectoryError::Persistence)?,
                role: row
                    .try_get("role")
                    .map_err(PlatformDirectoryError::Persistence)?,
                status: row
                    .try_get("status")
                    .map_err(PlatformDirectoryError::Persistence)?,
                joined_at_unix: row
                    .try_get("joined_at_unix")
                    .map_err(PlatformDirectoryError::Persistence)?,
            })
        })
        .collect::<Result<Vec<_>, PlatformDirectoryError>>()?;
    let has_more = members.len()
        > usize::try_from(page_size).map_err(|_| PlatformDirectoryError::InvalidInput)?;
    if has_more {
        members.pop();
    }
    let next_cursor = if has_more {
        members.last().map(|member| member.user_id.to_string())
    } else {
        None
    };
    Ok(PlatformMemberPage {
        members,
        next_cursor,
    })
}

fn normalize_page_size(requested_page_size: i32) -> Result<i32, PlatformDirectoryError> {
    if !(0..=200).contains(&requested_page_size) {
        return Err(PlatformDirectoryError::InvalidInput);
    }
    Ok(if requested_page_size == 0 {
        50
    } else {
        requested_page_size
    })
}

fn parse_workspace_cursor(
    cursor: &str,
) -> Result<(Option<i64>, Option<Uuid>), PlatformDirectoryError> {
    if cursor.is_empty() {
        return Ok((None, None));
    }
    if cursor.len() > 64 {
        return Err(PlatformDirectoryError::InvalidInput);
    }
    let (timestamp, workspace_id) = cursor
        .split_once('.')
        .ok_or(PlatformDirectoryError::InvalidInput)?;
    Ok((
        Some(
            timestamp
                .parse::<i64>()
                .map_err(|_| PlatformDirectoryError::InvalidInput)?,
        ),
        Some(Uuid::parse_str(workspace_id).map_err(|_| PlatformDirectoryError::InvalidInput)?),
    ))
}
