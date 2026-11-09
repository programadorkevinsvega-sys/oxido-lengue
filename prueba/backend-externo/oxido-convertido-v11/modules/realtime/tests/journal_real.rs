use std::error::Error;

use sqlx::{PgPool, Row, postgres::PgPoolOptions, postgres::PgRow};
use uuid::Uuid;
use xovyra_asset_storage::assets::{PendingAsset, create_pending_asset};
use xovyra_jobs::jobs::{EnqueueAssetDeletion, enqueue_asset_deletion};
use xovyra_tenancy_access::workspaces::{self, WorkspaceCreation};

#[tokio::test]
async fn journal_is_durable_scoped_and_idempotent_in_postgres()
-> Result<(), Box<dyn Error + Send + Sync>> {
    if let Err(error) = dotenvy::dotenv() {
        if error.not_found() {
            return Err("backend .env is required for the real realtime integration test".into());
        }
        return Err(error.into());
    }
    let database_url = std::env::var("REALTIME_DATABASE_URL")?;
    let database = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await?;
    let event_id = Uuid::new_v4();
    let replayed_event_id = Uuid::new_v4();
    let scope_id = Uuid::new_v4();
    let other_scope_id = Uuid::new_v4();
    let aggregate_id = Uuid::new_v4();
    let event = serde_json::json!({
        "event_id": event_id,
        "event_type": "tenancy_access.task.updated",
        "event_version": 1,
        "correlation_id": "realtime-real-integration",
        "payload": {
            "workspace_id": scope_id,
            "task_id": aggregate_id,
            "aggregate_version": 2,
        }
    });
    let verification = async {
        xovyra_realtime::journal::persist_domain_event(&database, &event).await?;
        xovyra_realtime::journal::persist_domain_event(&database, &event).await?;
        let first_page =
            xovyra_realtime::journal::list_after(&database, "tenant", scope_id, 0, 20).await?;
        let first_cursor = first_page.events[0].stream_cursor;
        sqlx::query("DELETE FROM realtime.stream_cursors WHERE audience='tenant' AND scope_id=$1")
            .bind(scope_id)
            .execute(&database)
            .await
            .map_err(xovyra_realtime::journal::JournalError::Persistence)?;
        let mut replayed_event = event.clone();
        replayed_event["event_id"] = serde_json::json!(replayed_event_id);
        replayed_event["payload"]["aggregate_version"] = serde_json::json!(3);
        xovyra_realtime::journal::persist_domain_event(&database, &replayed_event).await?;
        let page =
            xovyra_realtime::journal::list_after(&database, "tenant", scope_id, 0, 20).await?;
        let other_scope_page =
            xovyra_realtime::journal::list_after(&database, "tenant", other_scope_id, 0, 20)
                .await?;
        assert_eq!(page.events.len(), 2);
        assert!(other_scope_page.events.is_empty());
        assert_eq!(page.events[0].event_id, event_id);
        assert_eq!(page.events[0].schema_version, 1);
        assert_eq!(page.events[0].aggregate_version, 2);
        assert_eq!(page.events[1].event_id, replayed_event_id);
        assert_eq!(page.events[1].stream_cursor, first_cursor + 1);
        assert_eq!(page.next_cursor, page.events[1].stream_cursor);
        Ok::<(), xovyra_realtime::journal::JournalError>(())
    }
    .await;
    let cleanup = sqlx::query("DELETE FROM realtime.event_journal WHERE event_id=$1")
        .bind(event_id)
        .execute(&database)
        .await;
    let replayed_cleanup = sqlx::query("DELETE FROM realtime.event_journal WHERE event_id=$1")
        .bind(replayed_event_id)
        .execute(&database)
        .await;
    let cursor_cleanup =
        sqlx::query("DELETE FROM realtime.stream_cursors WHERE audience='tenant' AND scope_id=$1")
            .bind(scope_id)
            .execute(&database)
            .await;
    let other_cursor_cleanup =
        sqlx::query("DELETE FROM realtime.stream_cursors WHERE audience='tenant' AND scope_id=$1")
            .bind(other_scope_id)
            .execute(&database)
            .await;
    database.close().await;
    verification?;
    cleanup?;
    replayed_cleanup?;
    cursor_cleanup?;
    other_cursor_cleanup?;
    Ok(())
}

#[tokio::test]
async fn platform_support_events_use_an_isolated_global_platform_scope()
-> Result<(), Box<dyn Error + Send + Sync>> {
    if let Err(error) = dotenvy::dotenv() {
        if error.not_found() {
            return Err("backend .env is required for the real realtime integration test".into());
        }
        return Err(error.into());
    }
    let database_url = std::env::var("REALTIME_DATABASE_URL")?;
    let database = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;
    let event_id = Uuid::new_v4();
    let aggregate_id = Uuid::new_v4();
    let platform_scope_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001")?;
    let workspace_id = Uuid::new_v4();
    let event = serde_json::json!({
        "event_id": event_id,
        "event_type": "platform_control.support_grant.created",
        "event_version": 1,
        "correlation_id": "platform-support-realtime-integration",
        "payload": {
            "platform_scope_id": platform_scope_id,
            "workspace_id": workspace_id,
            "aggregate_id": aggregate_id,
            "aggregate_version": 1,
            "action": "support_grant.created"
        }
    });
    let verification = async {
        let baseline_cursor = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(stream_cursor), 0) FROM realtime.event_journal WHERE audience='platform' AND scope_id=$1",
        )
        .bind(platform_scope_id)
        .fetch_one(&database)
        .await
        .map_err(xovyra_realtime::journal::JournalError::Persistence)?;
        xovyra_realtime::journal::persist_domain_event(&database, &event).await?;
        xovyra_realtime::journal::persist_domain_event(&database, &event).await?;
        let platform_page =
            xovyra_realtime::journal::list_after(
                &database,
                "platform",
                platform_scope_id,
                baseline_cursor,
                20,
            )
                .await?;
        let tenant_page =
            xovyra_realtime::journal::list_after(&database, "tenant", workspace_id, 0, 20).await?;
        let platform_event = platform_page
            .events
            .iter()
            .find(|entry| entry.event_id == event_id)
            .expect("platform event is missing from its isolated global scope");
        assert_eq!(platform_event.audience, "platform");
        assert!(tenant_page.events.is_empty());
        Ok::<(), xovyra_realtime::journal::JournalError>(())
    }
    .await;
    let event_cleanup = sqlx::query("DELETE FROM realtime.event_journal WHERE event_id=$1")
        .bind(event_id)
        .execute(&database)
        .await;
    database.close().await;
    verification?;
    event_cleanup?;
    Ok(())
}

#[tokio::test]
async fn subscription_outbox_events_are_projected_to_the_workspace_scope()
-> Result<(), Box<dyn Error + Send + Sync>> {
    if let Err(error) = dotenvy::dotenv() {
        if error.not_found() {
            return Err("backend .env is required for the real realtime integration test".into());
        }
        return Err(error.into());
    }
    let database_url = std::env::var("REALTIME_DATABASE_URL")?;
    let database = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;
    let event_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let subscription_id = Uuid::new_v4();
    let event = serde_json::json!({
        "event_id": event_id,
        "event_type": "commercial_billing.subscription.plan_change_scheduled",
        "event_version": 1,
        "correlation_id": "billing-subscription-realtime-integration",
        "payload": {
            "workspace_id": workspace_id,
            "aggregate_id": subscription_id,
            "aggregate_version": 4,
        }
    });
    let verification = async {
        xovyra_realtime::journal::persist_domain_event(&database, &event).await?;
        let tenant_page =
            xovyra_realtime::journal::list_after(&database, "tenant", workspace_id, 0, 20).await?;
        let platform_page =
            xovyra_realtime::journal::list_after(&database, "platform", workspace_id, 0, 20)
                .await?;
        let projected = tenant_page
            .events
            .iter()
            .find(|entry| entry.event_id == event_id)
            .expect("subscription event must be in the workspace tenant stream");
        assert_eq!(projected.audience, "tenant");
        assert_eq!(projected.scope_id, workspace_id);
        assert_eq!(projected.aggregate_id, subscription_id);
        assert!(
            platform_page
                .events
                .iter()
                .all(|entry| entry.event_id != event_id)
        );
        Ok::<(), xovyra_realtime::journal::JournalError>(())
    }
    .await;
    let cleanup = sqlx::query("DELETE FROM realtime.event_journal WHERE event_id=$1")
        .bind(event_id)
        .execute(&database)
        .await;
    let cursor_cleanup =
        sqlx::query("DELETE FROM realtime.stream_cursors WHERE audience='tenant' AND scope_id=$1")
            .bind(workspace_id)
            .execute(&database)
            .await;
    database.close().await;
    verification?;
    cleanup?;
    cursor_cleanup?;
    Ok(())
}

#[tokio::test]
async fn asset_and_job_outbox_events_are_projected_to_the_workspace_journal()
-> Result<(), Box<dyn Error + Send + Sync>> {
    if let Err(error) = dotenvy::dotenv() {
        if error.not_found() {
            return Err("backend .env is required for the real realtime integration test".into());
        }
        return Err(error.into());
    }
    let realtime_database_url = std::env::var("REALTIME_DATABASE_URL")?;
    let asset_database_url = std::env::var("ASSET_STORAGE_DATABASE_URL")?;
    let jobs_database_url = std::env::var("JOBS_DATABASE_URL")?;
    let tenancy_database_url = std::env::var("TENANCY_ACCESS_DATABASE_URL")?;
    let cleanup_database_url = std::env::var("MIGRATOR_DATABASE_URL")?;
    let realtime_database = PgPoolOptions::new()
        .max_connections(3)
        .connect(&realtime_database_url)
        .await?;
    let asset_database = PgPoolOptions::new()
        .max_connections(3)
        .connect(&asset_database_url)
        .await?;
    let jobs_database = PgPoolOptions::new()
        .max_connections(3)
        .connect(&jobs_database_url)
        .await?;
    let tenancy_database = PgPoolOptions::new()
        .max_connections(3)
        .connect(&tenancy_database_url)
        .await?;
    let cleanup_database = PgPoolOptions::new()
        .max_connections(3)
        .connect(&cleanup_database_url)
        .await?;
    let owner_user_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    let idempotency_key = Uuid::new_v4().to_string();
    let workspace = workspaces::create_workspace(
        &tenancy_database,
        WorkspaceCreation {
            user_id: owner_user_id,
            session_id,
            name: "Realtime asset job integration",
            workspace_type: "team",
            idempotency_key: &idempotency_key,
            correlation_id: "realtime-asset-job-integration",
            owner_email: &format!("{owner_user_id}@integration.xovyra.invalid"),
        },
    )
    .await?;
    let verification = async {
        let asset_idempotency_key = Uuid::new_v4().to_string();
        let asset = create_pending_asset(
            &asset_database,
            PendingAsset {
                workspace_id: workspace.workspace_id,
                owner_user_id,
                display_name: "realtime-upload.txt",
                content_type: "text/plain",
                size_bytes: 12,
                idempotency_key: &asset_idempotency_key,
                correlation_id: "realtime-asset-outbox",
            },
        )
        .await?;
        let deletion_event_id = Uuid::new_v4();
        let job = enqueue_asset_deletion(
            &jobs_database,
            EnqueueAssetDeletion {
                workspace_id: workspace.workspace_id,
                owner_user_id,
                asset_id: asset.asset_id,
                event_id: deletion_event_id,
                correlation_id: "realtime-job-outbox",
            },
        )
        .await?;
        let asset_event = load_outbox_event(
            &asset_database,
            "asset_storage.outbox_events",
            workspace.workspace_id,
            "asset_storage.asset.upload_requested",
        )
        .await?;
        let job_event = load_outbox_event(
            &jobs_database,
            "jobs.outbox_events",
            workspace.workspace_id,
            "jobs.job.queued",
        )
        .await?;
        for event in [&asset_event, &job_event] {
            xovyra_realtime::journal::persist_domain_event(&realtime_database, event).await?;
            xovyra_realtime::journal::persist_domain_event(&realtime_database, event).await?;
        }
        let page = xovyra_realtime::journal::list_after(
            &realtime_database,
            "tenant",
            workspace.workspace_id,
            0,
            20,
        )
        .await?;
        assert_eq!(page.events.len(), 2);
        assert!(page.events.iter().any(|event| {
            event.event_type == "asset_storage.asset.upload_requested"
                && event.aggregate_id == asset.asset_id
        }));
        assert!(page.events.iter().any(|event| {
            event.event_type == "jobs.job.queued" && event.aggregate_id == job.job_id
        }));
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;
    let cleanup = cleanup_asset_job_journal_integration(
        &cleanup_database,
        owner_user_id,
        workspace.workspace_id,
        &idempotency_key,
    )
    .await;
    realtime_database.close().await;
    asset_database.close().await;
    jobs_database.close().await;
    tenancy_database.close().await;
    cleanup_database.close().await;
    verification?;
    cleanup?;
    Ok(())
}

async fn load_outbox_event(
    database: &PgPool,
    table_name: &str,
    workspace_id: Uuid,
    event_type: &str,
) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>> {
    let row = match table_name {
        "asset_storage.outbox_events" => {
            sqlx::query("SELECT event_id,event_type,event_version,occurred_at::text AS occurred_at,payload FROM asset_storage.outbox_events WHERE payload->>'workspace_id'=$1 AND event_type=$2 ORDER BY occurred_at DESC LIMIT 1")
                .bind(workspace_id.to_string())
                .bind(event_type)
                .fetch_one(database)
                .await?
        }
        "jobs.outbox_events" => {
            sqlx::query("SELECT event_id,event_type,event_version,occurred_at::text AS occurred_at,payload FROM jobs.outbox_events WHERE payload->>'workspace_id'=$1 AND event_type=$2 ORDER BY occurred_at DESC LIMIT 1")
                .bind(workspace_id.to_string())
                .bind(event_type)
                .fetch_one(database)
                .await?
        }
        _ => return Err("unsupported outbox table".into()),
    };
    outbox_envelope(row)
}

fn outbox_envelope(row: PgRow) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>> {
    let event_id: Uuid = row.try_get("event_id")?;
    let event_type: String = row.try_get("event_type")?;
    let event_version: i16 = row.try_get("event_version")?;
    let occurred_at: String = row.try_get("occurred_at")?;
    let payload: serde_json::Value = row.try_get("payload")?;
    let correlation_id = payload
        .get("correlation_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    Ok(serde_json::json!({
        "event_id": event_id,
        "event_type": event_type,
        "event_version": event_version,
        "occurred_at": occurred_at,
        "correlation_id": correlation_id,
        "payload": payload,
    }))
}

async fn cleanup_asset_job_journal_integration(
    database: &PgPool,
    owner_user_id: Uuid,
    workspace_id: Uuid,
    idempotency_key: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    sqlx::query("DELETE FROM realtime.event_journal WHERE audience='tenant' AND scope_id=$1")
        .bind(workspace_id)
        .execute(database)
        .await?;
    sqlx::query("DELETE FROM realtime.stream_cursors WHERE audience='tenant' AND scope_id=$1")
        .bind(workspace_id)
        .execute(database)
        .await?;
    sqlx::query("DELETE FROM asset_storage.outbox_events WHERE payload->>'workspace_id'=$1")
        .bind(workspace_id.to_string())
        .execute(database)
        .await?;
    sqlx::query("DELETE FROM asset_storage.assets WHERE workspace_id=$1")
        .bind(workspace_id)
        .execute(database)
        .await?;
    sqlx::query("DELETE FROM jobs.outbox_events WHERE payload->>'workspace_id'=$1")
        .bind(workspace_id.to_string())
        .execute(database)
        .await?;
    sqlx::query("DELETE FROM jobs.jobs WHERE workspace_id=$1")
        .bind(workspace_id)
        .execute(database)
        .await?;
    sqlx::query("DELETE FROM tenancy_access.outbox_events WHERE payload->>'workspace_id'=$1")
        .bind(workspace_id.to_string())
        .execute(database)
        .await?;
    sqlx::query("DELETE FROM tenancy_access.workspace_creation_idempotency WHERE user_id=$1 AND idempotency_key=$2")
        .bind(owner_user_id)
        .bind(idempotency_key)
        .execute(database)
        .await?;
    sqlx::query("DELETE FROM tenancy_access.workspaces WHERE workspace_id=$1")
        .bind(workspace_id)
        .execute(database)
        .await?;
    Ok(())
}
