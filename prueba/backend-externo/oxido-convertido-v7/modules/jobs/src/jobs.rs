use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum JobError {
    #[error("job request is invalid")]
    InvalidInput,
    #[error("job is not available")]
    NotFound,
    #[error("job state does not allow this operation")]
    InvalidState,
    #[error("job idempotency key conflicts with a previous request")]
    IdempotencyConflict,
    #[error("job database operation failed")]
    Persistence(#[source] sqlx::Error),
    #[error("job event serialization failed")]
    Serialization(#[source] serde_json::Error),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Job {
    pub job_id: Uuid,
    pub workspace_id: Uuid,
    pub owner_user_id: Uuid,
    pub job_type: String,
    pub resource_id: Uuid,
    pub status: String,
    pub attempt_count: i32,
    pub row_version: i64,
    pub created_at_unix: i64,
    pub updated_at_unix: i64,
}

pub struct EnqueueAssetDeletion<'request> {
    pub workspace_id: Uuid,
    pub owner_user_id: Uuid,
    pub asset_id: Uuid,
    pub event_id: Uuid,
    pub correlation_id: &'request str,
}

pub async fn enqueue_asset_deletion(
    database: &PgPool,
    request: EnqueueAssetDeletion<'_>,
) -> Result<Job, JobError> {
    if request.workspace_id.is_nil()
        || request.owner_user_id.is_nil()
        || request.asset_id.is_nil()
        || request.event_id.is_nil()
        || request.correlation_id.trim().is_empty()
        || request.correlation_id.len() > 128
    {
        return Err(JobError::InvalidInput);
    }
    let mut transaction = database.begin().await.map_err(JobError::Persistence)?;
    let inserted = sqlx::query("INSERT INTO jobs.jobs (job_id,workspace_id,owner_user_id,idempotency_key,job_type,resource_id,status) VALUES ($1,$2,$3,$4,'asset.delete',$5,'queued') ON CONFLICT (workspace_id,idempotency_key) DO NOTHING RETURNING job_id,workspace_id,owner_user_id,job_type,resource_id,status,attempt_count,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix,floor(extract(epoch FROM updated_at))::bigint AS updated_at_unix")
        .bind(Uuid::new_v4())
        .bind(request.workspace_id)
        .bind(request.owner_user_id)
        .bind(request.event_id.to_string())
        .bind(request.asset_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(JobError::Persistence)?;
    let job = match inserted {
        Some(row) => {
            let job = job_from_row(row)?;
            append_event(
                &mut transaction,
                &job,
                "jobs.job.queued",
                request.correlation_id,
            )
            .await?;
            job
        }
        None => {
            let row = sqlx::query("SELECT job_id,workspace_id,owner_user_id,job_type,resource_id,status,attempt_count,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix,floor(extract(epoch FROM updated_at))::bigint AS updated_at_unix FROM jobs.jobs WHERE workspace_id=$1 AND idempotency_key=$2")
                .bind(request.workspace_id)
                .bind(request.event_id.to_string())
                .fetch_optional(&mut *transaction)
                .await
                .map_err(JobError::Persistence)?
                .ok_or(JobError::IdempotencyConflict)?;
            let job = job_from_row(row)?;
            if job.resource_id != request.asset_id || job.owner_user_id != request.owner_user_id {
                return Err(JobError::IdempotencyConflict);
            }
            job
        }
    };
    transaction.commit().await.map_err(JobError::Persistence)?;
    Ok(job)
}

pub async fn claim_next_job(
    database: &PgPool,
    worker_id: &str,
    lease_seconds: i64,
) -> Result<Option<Job>, JobError> {
    if worker_id.trim().is_empty() || worker_id.len() > 128 || !(5..=300).contains(&lease_seconds) {
        return Err(JobError::InvalidInput);
    }
    let mut transaction = database.begin().await.map_err(JobError::Persistence)?;
    let row = sqlx::query("SELECT job_id,workspace_id,owner_user_id,job_type,resource_id,status,attempt_count,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix,floor(extract(epoch FROM updated_at))::bigint AS updated_at_unix FROM jobs.jobs WHERE ((status='queued' AND next_attempt_at<=now()) OR (status='running' AND lease_expires_at<=now())) ORDER BY next_attempt_at,created_at,job_id FOR UPDATE SKIP LOCKED LIMIT 1")
        .fetch_optional(&mut *transaction)
        .await
        .map_err(JobError::Persistence)?;
    let Some(row) = row else {
        transaction.commit().await.map_err(JobError::Persistence)?;
        return Ok(None);
    };
    let job = job_from_row(row)?;
    let row = sqlx::query("UPDATE jobs.jobs SET status='running',lease_owner=$2,lease_expires_at=now()+($3 * interval '1 second'),attempt_count=attempt_count+1,row_version=row_version+1,updated_at=now() WHERE job_id=$1 RETURNING job_id,workspace_id,owner_user_id,job_type,resource_id,status,attempt_count,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix,floor(extract(epoch FROM updated_at))::bigint AS updated_at_unix")
        .bind(job.job_id)
        .bind(worker_id)
        .bind(lease_seconds)
        .fetch_one(&mut *transaction)
        .await
        .map_err(JobError::Persistence)?;
    let claimed = job_from_row(row)?;
    append_event(&mut transaction, &claimed, "jobs.job.started", "job-worker").await?;
    transaction.commit().await.map_err(JobError::Persistence)?;
    Ok(Some(claimed))
}

pub async fn complete_job(
    database: &PgPool,
    job_id: Uuid,
    worker_id: &str,
    correlation_id: &str,
) -> Result<Job, JobError> {
    let mut transaction = database.begin().await.map_err(JobError::Persistence)?;
    let row = sqlx::query("UPDATE jobs.jobs SET status='succeeded',lease_owner=NULL,lease_expires_at=NULL,failure_code=NULL,row_version=row_version+1,updated_at=now() WHERE job_id=$1 AND status='running' AND lease_owner=$2 RETURNING job_id,workspace_id,owner_user_id,job_type,resource_id,status,attempt_count,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix,floor(extract(epoch FROM updated_at))::bigint AS updated_at_unix")
        .bind(job_id)
        .bind(worker_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(JobError::Persistence)?
        .ok_or(JobError::InvalidState)?;
    let job = job_from_row(row)?;
    append_event(&mut transaction, &job, "jobs.job.succeeded", correlation_id).await?;
    transaction.commit().await.map_err(JobError::Persistence)?;
    Ok(job)
}

pub async fn fail_job(
    database: &PgPool,
    job_id: Uuid,
    worker_id: &str,
    failure_code: &str,
    correlation_id: &str,
) -> Result<Job, JobError> {
    if failure_code.trim().is_empty() || failure_code.len() > 96 {
        return Err(JobError::InvalidInput);
    }
    let mut transaction = database.begin().await.map_err(JobError::Persistence)?;
    let row = sqlx::query("UPDATE jobs.jobs SET status=CASE WHEN attempt_count>=5 THEN 'failed' ELSE 'queued' END,lease_owner=NULL,lease_expires_at=NULL,failure_code=$3,next_attempt_at=now()+LEAST(300,POWER(2,attempt_count)::integer)*interval '1 second',row_version=row_version+1,updated_at=now() WHERE job_id=$1 AND status='running' AND lease_owner=$2 RETURNING job_id,workspace_id,owner_user_id,job_type,resource_id,status,attempt_count,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix,floor(extract(epoch FROM updated_at))::bigint AS updated_at_unix")
        .bind(job_id)
        .bind(worker_id)
        .bind(failure_code)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(JobError::Persistence)?
        .ok_or(JobError::InvalidState)?;
    let job = job_from_row(row)?;
    let event_type = if job.status == "failed" {
        "jobs.job.failed"
    } else {
        "jobs.job.retry_scheduled"
    };
    append_event(&mut transaction, &job, event_type, correlation_id).await?;
    transaction.commit().await.map_err(JobError::Persistence)?;
    Ok(job)
}

pub async fn cancel_job(
    database: &PgPool,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    job_id: Uuid,
    can_cancel_any: bool,
    correlation_id: &str,
) -> Result<Job, JobError> {
    let mut transaction = database.begin().await.map_err(JobError::Persistence)?;
    let row = sqlx::query("UPDATE jobs.jobs SET status='cancelled',lease_owner=NULL,lease_expires_at=NULL,row_version=row_version+1,updated_at=now() WHERE workspace_id=$1 AND job_id=$2 AND status='queued' AND ($3 OR owner_user_id=$4) RETURNING job_id,workspace_id,owner_user_id,job_type,resource_id,status,attempt_count,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix,floor(extract(epoch FROM updated_at))::bigint AS updated_at_unix")
        .bind(workspace_id)
        .bind(job_id)
        .bind(can_cancel_any)
        .bind(actor_user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(JobError::Persistence)?
        .ok_or(JobError::NotFound)?;
    let job = job_from_row(row)?;
    append_event(&mut transaction, &job, "jobs.job.cancelled", correlation_id).await?;
    transaction.commit().await.map_err(JobError::Persistence)?;
    Ok(job)
}

pub async fn list_jobs(
    database: &PgPool,
    workspace_id: Uuid,
    page_size: i64,
) -> Result<Vec<Job>, JobError> {
    if workspace_id.is_nil() || !(1..=100).contains(&page_size) {
        return Err(JobError::InvalidInput);
    }
    let rows = sqlx::query("SELECT job_id,workspace_id,owner_user_id,job_type,resource_id,status,attempt_count,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix,floor(extract(epoch FROM updated_at))::bigint AS updated_at_unix FROM jobs.jobs WHERE workspace_id=$1 ORDER BY created_at DESC,job_id LIMIT $2")
        .bind(workspace_id)
        .bind(page_size)
        .fetch_all(database)
        .await
        .map_err(JobError::Persistence)?;
    rows.into_iter().map(job_from_row).collect()
}

async fn append_event(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job: &Job,
    event_type: &str,
    correlation_id: &str,
) -> Result<(), JobError> {
    if correlation_id.trim().is_empty() || correlation_id.len() > 128 {
        return Err(JobError::InvalidInput);
    }
    let event_id = Uuid::new_v4();
    let payload = serde_json::json!({
        "workspace_id": job.workspace_id,
        "owner_user_id": job.owner_user_id,
        "job_id": job.job_id,
        "resource_id": job.resource_id,
        "aggregate_id": job.job_id,
        "aggregate_version": job.row_version,
        "correlation_id": correlation_id,
        "status": job.status,
    });
    let payload = serde_json::to_string(&payload).map_err(JobError::Serialization)?;
    sqlx::query("INSERT INTO jobs.outbox_events (event_id,event_type,event_version,occurred_at,payload) VALUES ($1,$2,1,now(),$3::jsonb)")
        .bind(event_id)
        .bind(event_type)
        .bind(payload)
        .execute(&mut **transaction)
        .await
        .map_err(JobError::Persistence)?;
    Ok(())
}

fn job_from_row(row: sqlx::postgres::PgRow) -> Result<Job, JobError> {
    Ok(Job {
        job_id: row.try_get("job_id").map_err(JobError::Persistence)?,
        workspace_id: row.try_get("workspace_id").map_err(JobError::Persistence)?,
        owner_user_id: row
            .try_get("owner_user_id")
            .map_err(JobError::Persistence)?,
        job_type: row.try_get("job_type").map_err(JobError::Persistence)?,
        resource_id: row.try_get("resource_id").map_err(JobError::Persistence)?,
        status: row.try_get("status").map_err(JobError::Persistence)?,
        attempt_count: row
            .try_get("attempt_count")
            .map_err(JobError::Persistence)?,
        row_version: row.try_get("row_version").map_err(JobError::Persistence)?,
        created_at_unix: row
            .try_get("created_at_unix")
            .map_err(JobError::Persistence)?,
        updated_at_unix: row
            .try_get("updated_at_unix")
            .map_err(JobError::Persistence)?,
    })
}
