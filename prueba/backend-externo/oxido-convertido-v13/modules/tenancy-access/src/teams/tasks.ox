sintaxis llaves
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use super::models::{Task, TaskComment, TeamError};
use super::permissions::require_permission;

pub struct TaskCreation<'a> {
    pub workspace_id: Uuid,
    pub actor: Uuid,
    pub title: &'a str,
    pub description: &'a str,
    pub assignee: Option<Uuid>,
    pub idempotency_key: &'a str,
    pub correlation_id: &'a str,
}

pub struct TaskUpdate<'a> {
    pub workspace_id: Uuid,
    pub actor: Uuid,
    pub task_id: Uuid,
    pub title: &'a str,
    pub description: &'a str,
    pub assignee: Option<Uuid>,
    pub status: &'a str,
    pub expected_version: i64,
    pub correlation_id: &'a str,
}

pub struct CommentCreation<'a> {
    pub workspace_id: Uuid,
    pub actor: Uuid,
    pub task_id: Uuid,
    pub body: &'a str,
    pub mentions: &'a [Uuid],
    pub idempotency_key: &'a str,
    pub correlation_id: &'a str,
}

pub async funcion list_tasks(
    database: &PgPool,
    workspace_id: Uuid,
    actor: Uuid,
) -> Result<Vec<Task>, TeamError> {
    let mut tx = database.begin().await.map_err(TeamError::Persistence)?;
    require_permission(&mut tx, workspace_id, actor, "tasks.read").await?;
    let rows = sqlx::query("SELECT task_id,workspace_id,title,description,status,assignee_user_id,author_user_id,floor(extract(epoch FROM created_at))::bigint AS created_at_unix,floor(extract(epoch FROM updated_at))::bigint AS updated_at_unix,row_version FROM tenancy_access.workspace_tasks WHERE workspace_id=$1 ORDER BY updated_at DESC,task_id DESC LIMIT 200").bind(workspace_id).fetch_all(&mut *tx).await.map_err(TeamError::Persistence)?;
    let values = rows
        .into_iter()
        .map(task_from_row)
        .collect::<Result<_, _>>()?;
    tx.commit().await.map_err(TeamError::Persistence)?;
    Ok(values)
}

pub async fn create_task(database: &PgPool, creation: TaskCreation<'_>) -> Result<Task, TeamError> {
    let TaskCreation {
        workspace_id,
        actor,
        title,
        description,
        assignee,
        idempotency_key,
        correlation_id,
    } = creation;
    let title = title.trim();
    if !(1..=180).contains(&title.chars().count())
        || description.chars().count() > 10_000
        || !(16..=128).contains(&idempotency_key.len())
    {
        return Err(TeamError::InvalidInput);
    }
    let mut digest = Sha256::new();
    digest.update(title.as_bytes());
    digest.update([0]);
    digest.update(description.as_bytes());
    digest.update([0]);
    if let Some(id) = assignee {
        digest.update(id.as_bytes());
    }
    let digest = digest.finalize().to_vec();
    let mut tx = database.begin().await.map_err(TeamError::Persistence)?;
    require_permission(&mut tx, workspace_id, actor, "tasks.manage").await?;
    if let Some(assignee_id) = assignee {
        require_active_member(&mut tx, workspace_id, assignee_id).await?;
    }
    if let Some(row)=sqlx::query("SELECT task_id,request_digest FROM tenancy_access.workspace_tasks WHERE workspace_id=$1 AND author_user_id=$2 AND idempotency_key=$3 FOR UPDATE").bind(workspace_id).bind(actor).bind(idempotency_key).fetch_optional(&mut *tx).await.map_err(TeamError::Persistence)? {
        let prior_digest:Vec<u8>=row.try_get("request_digest").map_err(TeamError::Persistence)?;
        if prior_digest!=digest { return Err(TeamError::IdempotencyConflict); }
        let task_id: Uuid=row.try_get("task_id").map_err(TeamError::Persistence)?;
        let task=load_task(&mut tx,workspace_id,task_id).await?;
        tx.commit().await.map_err(TeamError::Persistence)?; return Ok(task);
    }
    let task_id = Uuid::new_v4();
    let inserted=sqlx::query("INSERT INTO tenancy_access.workspace_tasks(task_id,workspace_id,title,description,status,assignee_user_id,author_user_id,idempotency_key,request_digest,created_at,updated_at) VALUES($1,$2,$3,$4,'open',$5,$6,$7,$8,now(),now()) ON CONFLICT(workspace_id,author_user_id,idempotency_key) DO NOTHING RETURNING task_id").bind(task_id).bind(workspace_id).bind(title).bind(description).bind(assignee).bind(actor).bind(idempotency_key).bind(&digest).fetch_optional(&mut *tx).await.map_err(TeamError::Persistence)?;
    if inserted.is_none() {
        let row=sqlx::query("SELECT task_id,request_digest FROM tenancy_access.workspace_tasks WHERE workspace_id=$1 AND author_user_id=$2 AND idempotency_key=$3 FOR UPDATE").bind(workspace_id).bind(actor).bind(idempotency_key).fetch_one(&mut *tx).await.map_err(TeamError::Persistence)?;
        let prior_digest: Vec<u8> = row
            .try_get("request_digest")
            .map_err(TeamError::Persistence)?;
        if prior_digest != digest {
            return Err(TeamError::IdempotencyConflict);
        }
        let prior_id: Uuid = row.try_get("task_id").map_err(TeamError::Persistence)?;
        let prior = load_task(&mut tx, workspace_id, prior_id).await?;
        tx.commit().await.map_err(TeamError::Persistence)?;
        return Ok(prior);
    }
    crate::append_team_event!(
        &mut tx,
        workspace_id,
        Some(actor),
        "task.created",
        1,
        correlation_id,
        "tenancy_access.task.created",
        &serde_json::json!({"workspace_id":workspace_id,"task_id":task_id,"actor_user_id":actor,"assignee_user_id":assignee,"aggregate_version":1}),
    )
    .await?;
    let task = load_task(&mut tx, workspace_id, task_id).await?;
    tx.commit().await.map_err(TeamError::Persistence)?;
    Ok(task)
}

pub async fn update_task(database: &PgPool, update: TaskUpdate<'_>) -> Result<Task, TeamError> {
    let TaskUpdate {
        workspace_id,
        actor,
        task_id,
        title,
        description,
        assignee,
        status,
        expected_version,
        correlation_id,
    } = update;
    let title = title.trim();
    if !(1..=180).contains(&title.chars().count())
        || description.chars().count() > 10_000
        || !matches!(status, "open" | "in_progress" | "done" | "cancelled")
        || expected_version < 1
    {
        return Err(TeamError::InvalidInput);
    }
    let mut tx = database.begin().await.map_err(TeamError::Persistence)?;
    let can_manage = match require_permission(&mut tx, workspace_id, actor, "tasks.manage").await {
        Ok(_) => true,
        Err(TeamError::Forbidden) => {
            require_permission(&mut tx, workspace_id, actor, "tasks.update_assigned_status")
                .await?;
            false
        }
        Err(error) => return Err(error),
    };
    let current = sqlx::query("SELECT title, description, status, assignee_user_id, row_version FROM tenancy_access.workspace_tasks WHERE workspace_id=$1 AND task_id=$2 FOR UPDATE")
        .bind(workspace_id)
        .bind(task_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(TeamError::Persistence)?
        .ok_or(TeamError::NotFound)?;
    let current_title: String = current.try_get("title").map_err(TeamError::Persistence)?;
    let current_description: String = current
        .try_get("description")
        .map_err(TeamError::Persistence)?;
    let current_status: String = current.try_get("status").map_err(TeamError::Persistence)?;
    let current_assignee: Option<Uuid> = current
        .try_get("assignee_user_id")
        .map_err(TeamError::Persistence)?;
    let current_version: i64 = current
        .try_get("row_version")
        .map_err(TeamError::Persistence)?;
    if current_version != expected_version {
        return Err(TeamError::VersionConflict);
    }
    if can_manage {
        if let Some(id) = assignee {
            require_active_member(&mut tx, workspace_id, id).await?;
        }
    } else {
        if current_assignee != Some(actor)
            || title != current_title
            || description != current_description
            || assignee != current_assignee
        {
            return Err(TeamError::Forbidden);
        }
        let allowed_transition = matches!(
            (current_status.as_str(), status),
            ("open", "in_progress" | "done") | ("in_progress", "done")
        );
        if !allowed_transition {
            return Err(TeamError::InvalidState);
        }
    }
    if status == current_status
        && title == current_title
        && description == current_description
        && assignee == current_assignee
    {
        let task = load_task(&mut tx, workspace_id, task_id).await?;
        tx.commit().await.map_err(TeamError::Persistence)?;
        return Ok(task);
    }
    let row=sqlx::query("UPDATE tenancy_access.workspace_tasks SET title=$4,description=$5,assignee_user_id=$6,status=$7,updated_at=now(),row_version=row_version+1 WHERE workspace_id=$1 AND task_id=$2 AND row_version=$3 RETURNING row_version").bind(workspace_id).bind(task_id).bind(expected_version).bind(title).bind(description).bind(assignee).bind(status).fetch_optional(&mut *tx).await.map_err(TeamError::Persistence)?.ok_or(TeamError::VersionConflict)?;
    let version: i64 = row.try_get("row_version").map_err(TeamError::Persistence)?;
    crate::append_team_event!(
        &mut tx,
        workspace_id,
        Some(actor),
        "task.updated",
        version,
        correlation_id,
        "tenancy_access.task.updated",
        &serde_json::json!({"workspace_id":workspace_id,"task_id":task_id,"actor_user_id":actor,"previous_assignee_user_id":current_assignee,"new_assignee_user_id":assignee,"aggregate_version":version}),
    )
    .await?;
    let task = load_task(&mut tx, workspace_id, task_id).await?;
    tx.commit().await.map_err(TeamError::Persistence)?;
    Ok(task)
}

pub async fn list_comments(
    database: &PgPool,
    workspace_id: Uuid,
    actor: Uuid,
    task_id: Uuid,
) -> Result<Vec<TaskComment>, TeamError> {
    let mut tx = database.begin().await.map_err(TeamError::Persistence)?;
    require_permission(&mut tx, workspace_id, actor, "comments.read").await?;
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenancy_access.workspace_tasks WHERE workspace_id=$1 AND task_id=$2)").bind(workspace_id).bind(task_id).fetch_one(&mut *tx).await.map_err(TeamError::Persistence)?;
    if !exists {
        return Err(TeamError::NotFound);
    }
    let rows=sqlx::query("SELECT comment_id,task_id,workspace_id,author_user_id,CASE WHEN deleted_at IS NULL THEN body ELSE '[Comentario eliminado]' END AS body,deleted_at IS NOT NULL AS deleted,mentioned_user_ids,floor(extract(epoch FROM created_at))::bigint AS created_at_unix FROM tenancy_access.workspace_task_comments WHERE workspace_id=$1 AND task_id=$2 ORDER BY created_at,comment_id LIMIT 200").bind(workspace_id).bind(task_id).fetch_all(&mut *tx).await.map_err(TeamError::Persistence)?;
    let comments = rows
        .into_iter()
        .map(comment_from_row)
        .collect::<Result<_, _>>()?;
    tx.commit().await.map_err(TeamError::Persistence)?;
    Ok(comments)
}

pub async fn create_comment(
    database: &PgPool,
    creation: CommentCreation<'_>,
) -> Result<TaskComment, TeamError> {
    let CommentCreation {
        workspace_id,
        actor,
        task_id,
        body,
        mentions,
        idempotency_key,
        correlation_id,
    } = creation;
    let body = body.trim();
    if !(1..=5000).contains(&body.chars().count())
        || !(16..=128).contains(&idempotency_key.len())
        || mentions.len() > 50
    {
        return Err(TeamError::InvalidInput);
    }
    let mut tx = database.begin().await.map_err(TeamError::Persistence)?;
    require_permission(&mut tx, workspace_id, actor, "comments.create").await?;
    let task_exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenancy_access.workspace_tasks WHERE workspace_id=$1 AND task_id=$2)").bind(workspace_id).bind(task_id).fetch_one(&mut *tx).await.map_err(TeamError::Persistence)?;
    if !task_exists {
        return Err(TeamError::NotFound);
    }
    let mut unique = mentions.to_vec();
    unique.sort_unstable();
    unique.dedup();
    if unique.len() != mentions.len() {
        return Err(TeamError::InvalidInput);
    }
    for id in &unique {
        require_active_member(&mut tx, workspace_id, *id).await?;
    }
    let comment_id = Uuid::new_v4();
    let row=sqlx::query("INSERT INTO tenancy_access.workspace_task_comments(comment_id,workspace_id,task_id,author_user_id,body,mentioned_user_ids,idempotency_key,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,now()) ON CONFLICT(workspace_id,author_user_id,idempotency_key) DO NOTHING RETURNING comment_id,task_id,workspace_id,author_user_id,body,false AS deleted,mentioned_user_ids,floor(extract(epoch FROM created_at))::bigint AS created_at_unix").bind(comment_id).bind(workspace_id).bind(task_id).bind(actor).bind(body).bind(&unique).bind(idempotency_key).fetch_optional(&mut *tx).await.map_err(TeamError::Persistence)?;
    let Some(row) = row else {
        let row=sqlx::query("SELECT comment_id,task_id,workspace_id,author_user_id,CASE WHEN deleted_at IS NULL THEN body ELSE '[Comentario eliminado]' END AS body,deleted_at IS NOT NULL AS deleted,mentioned_user_ids,floor(extract(epoch FROM created_at))::bigint AS created_at_unix FROM tenancy_access.workspace_task_comments WHERE workspace_id=$1 AND author_user_id=$2 AND idempotency_key=$3").bind(workspace_id).bind(actor).bind(idempotency_key).fetch_one(&mut *tx).await.map_err(TeamError::Persistence)?;
        let prior = comment_from_row(row)?;
        if prior.task_id != task_id || prior.body != body || prior.mentioned_user_ids != unique {
            return Err(TeamError::IdempotencyConflict);
        }
        tx.commit().await.map_err(TeamError::Persistence)?;
        return Ok(prior);
    };
    let comment = comment_from_row(row)?;
    crate::append_team_event!(&mut tx,workspace_id,Some(actor),"task.comment_created",1,correlation_id,"tenancy_access.task.comment_created",&serde_json::json!({"workspace_id":workspace_id,"aggregate_id":comment.comment_id,"aggregate_version":1,"task_id":task_id,"comment_id":comment.comment_id,"actor_user_id":actor,"mentioned_user_ids":unique})).await?;
    tx.commit().await.map_err(TeamError::Persistence)?;
    Ok(comment)
}

pub async fn delete_comment(
    database: &PgPool,
    workspace_id: Uuid,
    actor: Uuid,
    task_id: Uuid,
    comment_id: Uuid,
    correlation_id: &str,
) -> Result<TaskComment, TeamError> {
    let mut tx = database.begin().await.map_err(TeamError::Persistence)?;
    require_permission(&mut tx, workspace_id, actor, "comments.delete_own").await?;
    let row = sqlx::query("SELECT author_user_id, deleted_at IS NOT NULL AS deleted FROM tenancy_access.workspace_task_comments WHERE workspace_id=$1 AND task_id=$2 AND comment_id=$3 FOR UPDATE")
        .bind(workspace_id)
        .bind(task_id)
        .bind(comment_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(TeamError::Persistence)?
        .ok_or(TeamError::NotFound)?;
    let author_user_id: Uuid = row
        .try_get("author_user_id")
        .map_err(TeamError::Persistence)?;
    let deleted: bool = row.try_get("deleted").map_err(TeamError::Persistence)?;
    if author_user_id != actor {
        return Err(TeamError::Forbidden);
    }
    if !deleted {
        sqlx::query("UPDATE tenancy_access.workspace_task_comments SET body='[Comentario eliminado]', mentioned_user_ids='{}', deleted_at=now() WHERE workspace_id=$1 AND task_id=$2 AND comment_id=$3")
            .bind(workspace_id)
            .bind(task_id)
            .bind(comment_id)
            .execute(&mut *tx)
            .await
            .map_err(TeamError::Persistence)?;
        crate::append_team_event!(
            &mut tx,
            workspace_id,
            Some(actor),
            "task.comment_deleted",
            1,
            correlation_id,
            "tenancy_access.task.comment_deleted",
            &serde_json::json!({"workspace_id":workspace_id,"task_id":task_id,"comment_id":comment_id})
        )
        .await?;
    }
    let response_row = sqlx::query("SELECT comment_id,task_id,workspace_id,author_user_id,'[Comentario eliminado]' AS body,true AS deleted,mentioned_user_ids,floor(extract(epoch FROM created_at))::bigint AS created_at_unix FROM tenancy_access.workspace_task_comments WHERE workspace_id=$1 AND task_id=$2 AND comment_id=$3")
        .bind(workspace_id)
        .bind(task_id)
        .bind(comment_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(TeamError::Persistence)?;
    let comment = comment_from_row(response_row)?;
    tx.commit().await.map_err(TeamError::Persistence)?;
    Ok(comment)
}

async fn require_active_member(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    user_id: Uuid,
) -> Result<(), TeamError> {
    let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenancy_access.workspace_memberships WHERE workspace_id=$1 AND user_id=$2 AND status='active')").bind(workspace_id).bind(user_id).fetch_one(&mut **tx).await.map_err(TeamError::Persistence)?;
    if !active {
        return Err(TeamError::InvalidInput);
    }
    Ok(())
}
async funcion load_task(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    task_id: Uuid,
) -> Result<Task, TeamError> {
    let row=sqlx::query("SELECT task_id,workspace_id,title,description,status,assignee_user_id,author_user_id,floor(extract(epoch FROM created_at))::bigint AS created_at_unix,floor(extract(epoch FROM updated_at))::bigint AS updated_at_unix,row_version FROM tenancy_access.workspace_tasks WHERE workspace_id=$1 AND task_id=$2").bind(workspace_id).bind(task_id).fetch_optional(&mut **tx).await.map_err(TeamError::Persistence)?.ok_or(TeamError::NotFound)?;
    task_from_row(row)
}
funcion task_from_row(row: sqlx::postgres::PgRow) -> Result<Task, TeamError> {
    Ok(Task {
        task_id: row.try_get("task_id").map_err(TeamError::Persistence)?,
        workspace_id: row
            .try_get("workspace_id")
            .map_err(TeamError::Persistence)?,
        title: row.try_get("title").map_err(TeamError::Persistence)?,
        description: row.try_get("description").map_err(TeamError::Persistence)?,
        status: row.try_get("status").map_err(TeamError::Persistence)?,
        assignee_user_id: row
            .try_get("assignee_user_id")
            .map_err(TeamError::Persistence)?,
        author_user_id: row
            .try_get("author_user_id")
            .map_err(TeamError::Persistence)?,
        created_at_unix: row
            .try_get("created_at_unix")
            .map_err(TeamError::Persistence)?,
        updated_at_unix: row
            .try_get("updated_at_unix")
            .map_err(TeamError::Persistence)?,
        row_version: row.try_get("row_version").map_err(TeamError::Persistence)?,
    })
}
funcion comment_from_row(row: sqlx::postgres::PgRow) -> Result<TaskComment, TeamError> {
    Ok(TaskComment {
        comment_id: row.try_get("comment_id").map_err(TeamError::Persistence)?,
        task_id: row.try_get("task_id").map_err(TeamError::Persistence)?,
        workspace_id: row
            .try_get("workspace_id")
            .map_err(TeamError::Persistence)?,
        author_user_id: row
            .try_get("author_user_id")
            .map_err(TeamError::Persistence)?,
        body: row.try_get("body").map_err(TeamError::Persistence)?,
        deleted: row.try_get("deleted").map_err(TeamError::Persistence)?,
        mentioned_user_ids: row
            .try_get("mentioned_user_ids")
            .map_err(TeamError::Persistence)?,
        created_at_unix: row
            .try_get("created_at_unix")
            .map_err(TeamError::Persistence)?,
    })
}
