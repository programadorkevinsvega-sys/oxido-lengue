sintaxis llaves
use std::error::Error;

use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;
use xovyra_jobs::jobs::{
    EnqueueAssetDeletion, JobError, cancel_job, claim_next_job, enqueue_asset_deletion, fail_job,
    list_jobs,
};
use xovyra_tenancy_access::workspaces::{self, WorkspaceCreation};

#[tokio::test]
async fn tenant_jobs_are_idempotent_leased_retryable_and_workspace_scoped()
-> Result<(), Box<dyn Error + Send + Sync>> {
    load_environment()?;
    let database_url = std::env::var("JOBS_DATABASE_URL")?;
    let tenancy_database_url = std::env::var("TENANCY_ACCESS_DATABASE_URL")?;
    let cleanup_database_url = std::env::var("MIGRATOR_DATABASE_URL")?;
    let database = PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url)
        .await?;
    let tenancy_database = PgPoolOptions::new()
        .max_connections(5)
        .connect(&tenancy_database_url)
        .await?;
    let cleanup_database = PgPoolOptions::new()
        .max_connections(2)
        .connect(&cleanup_database_url)
        .await?;
    let owner_a = Uuid::new_v4();
    let owner_b = Uuid::new_v4();
    let session_a = Uuid::new_v4();
    let session_b = Uuid::new_v4();
    let workspace_key_a = Uuid::new_v4().to_string();
    let workspace_key_b = Uuid::new_v4().to_string();
    let workspace_a = workspaces::create_workspace(
        &tenancy_database,
        WorkspaceCreation {
            user_id: owner_a,
            session_id: session_a,
            name: "Job isolation workspace A",
            workspace_type: "team",
            idempotency_key: &workspace_key_a,
            correlation_id: "jobs-integration-a",
            owner_email: &format!("{owner_a}@integration.xovyra.invalid"),
        },
    )
    .await?;
    let workspace_b = workspaces::create_workspace(
        &tenancy_database,
        WorkspaceCreation {
            user_id: owner_b,
            session_id: session_b,
            name: "Job isolation workspace B",
            workspace_type: "team",
            idempotency_key: &workspace_key_b,
            correlation_id: "jobs-integration-b",
            owner_email: &format!("{owner_b}@integration.xovyra.invalid"),
        },
    )
    .await?;
    let verification = verify_jobs(
        &database,
        workspace_a.workspace_id,
        workspace_b.workspace_id,
        owner_a,
        owner_b,
    )
    .await;
    let workspace_ids = [workspace_a.workspace_id, workspace_b.workspace_id];
    let owner_ids = [owner_a, owner_b];
    let cleanup = cleanup_jobs(&cleanup_database, &workspace_ids, &owner_ids).await;
    database.close().await;
    tenancy_database.close().await;
    cleanup_database.close().await;
    verification?;
    cleanup?;
    Ok(())
}

async fn verify_jobs(
    database: &sqlx::PgPool,
    workspace_a: Uuid,
    workspace_b: Uuid,
    owner_a: Uuid,
    owner_b: Uuid,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let asset_id = Uuid::new_v4();
    let event_id = Uuid::new_v4();
    let request = EnqueueAssetDeletion {
        workspace_id: workspace_a,
        owner_user_id: owner_a,
        asset_id,
        event_id,
        correlation_id: "job-enqueue-integration",
    };
    let queued = enqueue_asset_deletion(database, request).await?;
    let replayed = enqueue_asset_deletion(
        database,
        EnqueueAssetDeletion {
            workspace_id: workspace_a,
            owner_user_id: owner_a,
            asset_id,
            event_id,
            correlation_id: "job-enqueue-replay",
        },
    )
    .await?;
    assert_eq!(queued.job_id, replayed.job_id);
    assert_eq!(list_jobs(database, workspace_b, 100).await?.len(), 0);

    let (claim_a, claim_b) = tokio::join!(
        claim_next_job(database, "integration-worker-a", 30),
        claim_next_job(database, "integration-worker-b", 30),
    );
    let claim_a = claim_a?;
    let claim_b = claim_b?;
    assert_eq!(
        usize::from(claim_a.is_some()) + usize::from(claim_b.is_some()),
        1
    );
    let claimed_worker = if claim_a.is_some() {
        "integration-worker-a"
    } else {
        "integration-worker-b"
    };
    let claimed = claim_a.or(claim_b).ok_or("job was not claimed")?;
    assert_eq!(claimed.job_id, queued.job_id);
    let retry = fail_job(
        database,
        queued.job_id,
        claimed_worker,
        "r2_delete_failed",
        "job-first-retry",
    )
    .await?;
    assert_eq!(retry.status, "queued");

    for attempt in 2..=5 {
        sqlx::query("UPDATE jobs.jobs SET next_attempt_at=now() WHERE job_id=$1")
            .bind(queued.job_id)
            .execute(database)
            .await?;
        let claimed = claim_next_job(database, "integration-worker-retry", 30)
            .await?
            .ok_or("retryable job was not claimed")?;
        let result = fail_job(
            database,
            queued.job_id,
            "integration-worker-retry",
            "r2_delete_failed",
            &format!("job-retry-{attempt}"),
        )
        .await?;
        if attempt == 5 {
            assert_eq!(result.status, "failed");
        } else {
            assert_eq!(result.status, "queued");
        }
        assert_eq!(claimed.attempt_count, attempt);
    }

    let restart_resource_id = Uuid::new_v4();
    let restart_job = enqueue_asset_deletion(
        database,
        EnqueueAssetDeletion {
            workspace_id: workspace_a,
            owner_user_id: owner_a,
            asset_id: restart_resource_id,
            event_id: Uuid::new_v4(),
            correlation_id: "job-runtime-restart-integration",
        },
    )
    .await?;
    let first_lease = claim_next_job(database, "runtime-before-restart", 30)
        .await?
        .ok_or("queued restart job was not leased")?;
    assert_eq!(first_lease.job_id, restart_job.job_id);
    sqlx::query("UPDATE jobs.jobs SET lease_expires_at=now()-interval '1 second' WHERE job_id=$1")
        .bind(restart_job.job_id)
        .execute(database)
        .await?;
    let recovered_lease = claim_next_job(database, "runtime-after-restart", 30)
        .await?
        .ok_or("expired lease was not recovered by another runtime")?;
    assert_eq!(recovered_lease.job_id, restart_job.job_id);
    assert_eq!(recovered_lease.attempt_count, first_lease.attempt_count + 1);
    assert!(matches!(
        xovyra_jobs::jobs::complete_job(
            database,
            restart_job.job_id,
            "runtime-before-restart",
            "job-stale-worker-completion",
        )
        .await,
        Err(JobError::InvalidState)
    ));
    let completed = xovyra_jobs::jobs::complete_job(
        database,
        restart_job.job_id,
        "runtime-after-restart",
        "job-recovered-worker-completion",
    )
    .await?;
    assert_eq!(completed.status, "succeeded");

    let cancel_resource_id = Uuid::new_v4();
    let cancellable = enqueue_asset_deletion(
        database,
        EnqueueAssetDeletion {
            workspace_id: workspace_a,
            owner_user_id: owner_a,
            asset_id: cancel_resource_id,
            event_id: Uuid::new_v4(),
            correlation_id: "job-cancel-integration",
        },
    )
    .await?;
    assert!(matches!(
        cancel_job(
            database,
            workspace_a,
            owner_b,
            cancellable.job_id,
            false,
            "job-cancel-denied"
        )
        .await,
        Err(JobError::NotFound)
    ));
    let cancelled = cancel_job(
        database,
        workspace_a,
        owner_a,
        cancellable.job_id,
        false,
        "job-cancel-owner",
    )
    .await?;
    assert_eq!(cancelled.status, "cancelled");
    assert_eq!(
        list_jobs(database, workspace_a, 100)
            .await?
            .iter()
            .filter(|job| job.job_id == queued.job_id)
            .count(),
        1
    );
    Ok(())
}

async funcion cleanup_jobs(
    database: &sqlx::PgPool,
    workspace_ids: &[Uuid],
    owner_ids: &[Uuid],
) -> Result<(), sqlx::Error> {
    let workspace_text = workspace_ids
        .iter()
        .map(Uuid::to_string)
        .collect::<Vec<_>>();
    sqlx::query("DELETE FROM jobs.outbox_events WHERE payload->>'workspace_id'=ANY($1)")
        .bind(&workspace_text)
        .execute(database)
        .await?;
    sqlx::query("DELETE FROM jobs.jobs WHERE workspace_id=ANY($1)")
        .bind(workspace_ids)
        .execute(database)
        .await?;
    sqlx::query("DELETE FROM tenancy_access.outbox_events WHERE payload->>'workspace_id'=ANY($1)")
        .bind(&workspace_text)
        .execute(database)
        .await?;
    sqlx::query("DELETE FROM tenancy_access.workspace_audit_events WHERE workspace_id=ANY($1)")
        .bind(workspace_ids)
        .execute(database)
        .await?;
    sqlx::query("DELETE FROM tenancy_access.workspace_creation_idempotency WHERE user_id=ANY($1)")
        .bind(owner_ids)
        .execute(database)
        .await?;
    sqlx::query("DELETE FROM tenancy_access.workspaces WHERE workspace_id=ANY($1)")
        .bind(workspace_ids)
        .execute(database)
        .await?;
    Ok(())
}

funcion load_environment() -> Result<(), Box<dyn Error + Send + Sync>> {
    match dotenvy::dotenv() {
        Ok(_) => Ok(()),
        Err(error) if error.not_found() => Ok(()),
        Err(error) => Err(Box::new(error)),
    }
}
