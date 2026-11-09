sintaxis llaves
use std::error::Error;

use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;
use xovyra_asset_storage::assets::{
    self, AssetError, PendingAsset, activate_uploaded_asset, complete_asset_deletion,
    create_pending_asset, expire_pending_uploads, get_private_asset, request_asset_deletion,
    search_private_assets,
};
use xovyra_tenancy_access::workspaces::{self, WorkspaceCreation};

#[tokio::test]
async fn private_assets_are_owner_scoped_idempotent_and_expired_uploads_are_scheduled()
-> Result<(), Box<dyn Error + Send + Sync>> {
    load_environment()?;
    let asset_database_url = std::env::var("ASSET_STORAGE_DATABASE_URL")?;
    let tenancy_database_url = std::env::var("TENANCY_ACCESS_DATABASE_URL")?;
    let cleanup_database_url = std::env::var("MIGRATOR_DATABASE_URL")?;
    let asset_database = PgPoolOptions::new()
        .max_connections(5)
        .connect(&asset_database_url)
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
            name: "Private asset integration workspace A",
            workspace_type: "team",
            idempotency_key: &workspace_key_a,
            correlation_id: "asset-storage-integration-a",
            owner_email: &format!("{owner_a}@integration.xovyra.invalid"),
        },
    )
    .await?;
    let workspace_b = workspaces::create_workspace(
        &tenancy_database,
        WorkspaceCreation {
            user_id: owner_b,
            session_id: session_b,
            name: "Private asset integration workspace B",
            workspace_type: "team",
            idempotency_key: &workspace_key_b,
            correlation_id: "asset-storage-integration-b",
            owner_email: &format!("{owner_b}@integration.xovyra.invalid"),
        },
    )
    .await?;
    assert!(
        xovyra_tenancy_access::teams::permissions::check_permission(
            &tenancy_database,
            workspace_a.workspace_id,
            owner_a,
            "assets.create",
        )
        .await?
    );
    assert!(
        !xovyra_tenancy_access::teams::permissions::check_permission(
            &tenancy_database,
            workspace_a.workspace_id,
            owner_b,
            "assets.create",
        )
        .await?
    );
    let verification = verify_assets(
        &asset_database,
        workspace_a.workspace_id,
        workspace_b.workspace_id,
        owner_a,
        owner_b,
    )
    .await;
    let workspace_ids = [workspace_a.workspace_id, workspace_b.workspace_id];
    let owner_ids = [owner_a, owner_b];
    let cleanup = cleanup_assets(&cleanup_database, &workspace_ids, &owner_ids).await;
    asset_database.close().await;
    tenancy_database.close().await;
    cleanup_database.close().await;
    verification?;
    cleanup?;
    Ok(())
}

async funcion verify_assets(
    database: &sqlx::PgPool,
    workspace_a: Uuid,
    workspace_b: Uuid,
    owner_a: Uuid,
    owner_b: Uuid,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let idempotency_key = Uuid::new_v4().to_string();
    let pending = create_pending_asset(
        database,
        PendingAsset {
            workspace_id: workspace_a,
            owner_user_id: owner_a,
            display_name: "private banner.png",
            content_type: "image/png",
            size_bytes: 128,
            idempotency_key: &idempotency_key,
            correlation_id: "asset-create-integration",
        },
    )
    .await?;
    let replayed = create_pending_asset(
        database,
        PendingAsset {
            workspace_id: workspace_a,
            owner_user_id: owner_a,
            display_name: "private banner.png",
            content_type: "image/png",
            size_bytes: 128,
            idempotency_key: &idempotency_key,
            correlation_id: "asset-create-integration-retry",
        },
    )
    .await?;
    assert_eq!(pending.asset_id, replayed.asset_id);
    assert!(matches!(
        get_private_asset(database, workspace_a, owner_b, pending.asset_id).await,
        Err(AssetError::NotFound)
    ));
    assert!(matches!(
        get_private_asset(database, workspace_b, owner_b, pending.asset_id).await,
        Err(AssetError::NotFound)
    ));

    let active = activate_uploaded_asset(
        database,
        assets::UploadConfirmation {
            workspace_id: workspace_a,
            owner_user_id: owner_a,
            asset_id: pending.asset_id,
            actual_size_bytes: 128,
            actual_content_type: "image/png",
            etag: "etag-integration-asset",
            correlation_id: "asset-upload-complete-integration",
        },
    )
    .await?;
    assert_eq!(active.lifecycle_state, "active");
    assert_eq!(
        search_private_assets(database, workspace_a, owner_a, "private banner", 20)
            .await?
            .len(),
        1
    );
    assert!(
        search_private_assets(database, workspace_b, owner_b, "private banner", 20)
            .await?
            .is_empty()
    );
    let pending_delete = request_asset_deletion(
        database,
        workspace_a,
        owner_a,
        pending.asset_id,
        "asset-delete-integration",
    )
    .await?;
    assert_eq!(pending_delete.lifecycle_state, "delete_pending");
    assert!(matches!(
        get_private_asset(database, workspace_a, owner_a, pending.asset_id).await,
        Err(AssetError::NotFound)
    ));
    let deleted = complete_asset_deletion(
        database,
        workspace_a,
        pending.asset_id,
        "asset-delete-worker-integration",
    )
    .await?;
    assert_eq!(deleted.lifecycle_state, "deleted");
    let repeated_delete = complete_asset_deletion(
        database,
        workspace_a,
        pending.asset_id,
        "asset-delete-worker-replay",
    )
    .await?;
    assert_eq!(repeated_delete.lifecycle_state, "deleted");

    let abandoned = create_pending_asset(
        database,
        PendingAsset {
            workspace_id: workspace_a,
            owner_user_id: owner_a,
            display_name: "abandoned-upload.mp4",
            content_type: "video/mp4",
            size_bytes: 2048,
            idempotency_key: &Uuid::new_v4().to_string(),
            correlation_id: "asset-abandoned-upload-integration",
        },
    )
    .await?;
    sqlx::query("UPDATE asset_storage.assets SET upload_expires_at=now()-interval '1 second' WHERE asset_id=$1")
        .bind(abandoned.asset_id)
        .execute(database)
        .await?;
    assert_eq!(expire_pending_uploads(database, 20).await?, 1);
    let expired =
        assets::get_asset_deletion_target(database, workspace_a, abandoned.asset_id).await?;
    assert_eq!(expired.lifecycle_state, "delete_pending");
    Ok(())
}

async funcion cleanup_assets(
    database: &sqlx::PgPool,
    workspace_ids: &[Uuid],
    owner_ids: &[Uuid],
) -> Result<(), sqlx::Error> {
    let workspace_text = workspace_ids
        .iter()
        .map(Uuid::to_string)
        .collect::<Vec<_>>();
    sqlx::query("DELETE FROM asset_storage.outbox_events WHERE payload->>'workspace_id'=ANY($1)")
        .bind(&workspace_text)
        .execute(database)
        .await?;
    sqlx::query("DELETE FROM asset_storage.assets WHERE workspace_id=ANY($1)")
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
