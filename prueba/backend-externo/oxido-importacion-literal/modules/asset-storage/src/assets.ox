sintaxis llaves
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

const MAX_ASSET_SIZE_BYTES: i64 = 2_147_483_648;

#[derive(Debug, Error)]
pub enum AssetError {
    #[error("asset request is invalid")]
    InvalidInput,
    #[error("asset is not available")]
    NotFound,
    #[error("asset state does not allow this operation")]
    InvalidState,
    #[error("asset idempotency key conflicts with a previous request")]
    IdempotencyConflict,
    #[error("asset storage database operation failed")]
    Persistence(#[source] sqlx::Error),
    #[error("asset event payload could not be serialized")]
    Serialization(#[source] serde_json::Error),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Asset {
    pub asset_id: Uuid,
    pub workspace_id: Uuid,
    pub owner_user_id: Uuid,
    pub object_key: String,
    pub upload_object_key: String,
    pub display_name: String,
    pub content_type: String,
    pub declared_size_bytes: i64,
    pub stored_size_bytes: Option<i64>,
    pub etag: Option<String>,
    pub lifecycle_state: String,
    pub row_version: i64,
    pub created_at_unix: i64,
}

#[derive(Clone, Debug)]
pub struct PlatformAsset {
    pub asset_id: Uuid,
    pub workspace_id: Uuid,
    pub object_key: String,
    pub display_name: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub lifecycle_state: String,
    pub created_at_unix: i64,
}

pub struct PlatformAssetPage {
    pub assets: Vec<PlatformAsset>,
    pub next_cursor: Option<String>,
}

pub async fn list_platform_assets(
    database: &PgPool,
    workspace_id: Uuid,
    cursor: &str,
    page_size: i32,
) -> Result<PlatformAssetPage, AssetError> {
    if !(0..=200).contains(&page_size) || cursor.len() > 36 {
        return Err(AssetError::InvalidInput);
    }
    let limit = if page_size == 0 {
        51
    } else {
        i64::from(page_size) + 1
    };
    let cursor_id = if cursor.is_empty() {
        None
    } else {
        Some(Uuid::parse_str(cursor).map_err(|_| AssetError::InvalidInput)?)
    };
    let rows = sqlx::query("SELECT asset_id, workspace_id, object_key, display_name, content_type, coalesce(stored_size_bytes, declared_size_bytes) AS size_bytes, lifecycle_state, floor(extract(epoch FROM created_at))::bigint AS created_at_unix FROM asset_storage.assets WHERE workspace_id = $1 AND lifecycle_state = 'active' AND ($2::uuid IS NULL OR asset_id > $2) ORDER BY asset_id LIMIT $3")
        .bind(workspace_id).bind(cursor_id).bind(limit).fetch_all(database).await.map_err(AssetError::Persistence)?;
    let mut assets = rows
        .iter()
        .map(platform_asset_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    let requested = if page_size == 0 {
        50
    } else {
        usize::try_from(page_size).map_err(|_| AssetError::InvalidInput)?
    };
    let has_more = assets.len() > requested;
    if has_more {
        assets.pop();
    }
    let next_cursor = if has_more {
        assets.last().map(|asset| asset.asset_id.to_string())
    } else {
        None
    };
    Ok(PlatformAssetPage {
        assets,
        next_cursor,
    })
}

pub async fn get_platform_asset(
    database: &PgPool,
    workspace_id: Uuid,
    asset_id: Uuid,
) -> Result<PlatformAsset, AssetError> {
    let row = sqlx::query("SELECT asset_id, workspace_id, object_key, display_name, content_type, coalesce(stored_size_bytes, declared_size_bytes) AS size_bytes, lifecycle_state, floor(extract(epoch FROM created_at))::bigint AS created_at_unix FROM asset_storage.assets WHERE workspace_id = $1 AND asset_id = $2 AND lifecycle_state = 'active'")
        .bind(workspace_id).bind(asset_id).fetch_optional(database).await.map_err(AssetError::Persistence)?.ok_or(AssetError::NotFound)?;
    platform_asset_from_row(&row)
}

fn platform_asset_from_row(row: &sqlx::postgres::PgRow) -> Result<PlatformAsset, AssetError> {
    Ok(PlatformAsset {
        asset_id: row.try_get("asset_id").map_err(AssetError::Persistence)?,
        workspace_id: row
            .try_get("workspace_id")
            .map_err(AssetError::Persistence)?,
        object_key: row.try_get("object_key").map_err(AssetError::Persistence)?,
        display_name: row
            .try_get("display_name")
            .map_err(AssetError::Persistence)?,
        content_type: row
            .try_get("content_type")
            .map_err(AssetError::Persistence)?,
        size_bytes: row.try_get("size_bytes").map_err(AssetError::Persistence)?,
        lifecycle_state: row
            .try_get("lifecycle_state")
            .map_err(AssetError::Persistence)?,
        created_at_unix: row
            .try_get("created_at_unix")
            .map_err(AssetError::Persistence)?,
    })
}

pub struct PendingAsset<'request> {
    pub workspace_id: Uuid,
    pub owner_user_id: Uuid,
    pub display_name: &'request str,
    pub content_type: &'request str,
    pub size_bytes: i64,
    pub idempotency_key: &'request str,
    pub correlation_id: &'request str,
}

pub struct UploadConfirmation<'request> {
    pub workspace_id: Uuid,
    pub owner_user_id: Uuid,
    pub asset_id: Uuid,
    pub actual_size_bytes: i64,
    pub actual_content_type: &'request str,
    pub etag: &'request str,
    pub correlation_id: &'request str,
}

pub async fn create_pending_asset(
    database: &PgPool,
    request: PendingAsset<'_>,
) -> Result<Asset, AssetError> {
    validate_pending_asset(&request)?;
    let asset_id = Uuid::new_v4();
    let object_key = format!(
        "private/{}/{}/{}",
        request.workspace_id, request.owner_user_id, asset_id
    );
    let upload_object_key = format!(
        "staging/{}/{}/{}/{}",
        request.workspace_id,
        request.owner_user_id,
        asset_id,
        Uuid::new_v4()
    );
    let mut transaction = database.begin().await.map_err(AssetError::Persistence)?;
    let inserted = sqlx::query("INSERT INTO asset_storage.assets (asset_id,workspace_id,owner_user_id,object_key,upload_object_key,display_name,content_type,declared_size_bytes,lifecycle_state,idempotency_key) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'upload_pending',$9) ON CONFLICT (workspace_id,owner_user_id,idempotency_key) DO NOTHING RETURNING asset_id,workspace_id,owner_user_id,object_key,upload_object_key,display_name,content_type,declared_size_bytes,stored_size_bytes,etag,lifecycle_state,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix")
        .bind(asset_id)
        .bind(request.workspace_id)
        .bind(request.owner_user_id)
        .bind(&object_key)
        .bind(&upload_object_key)
        .bind(request.display_name.trim())
        .bind(request.content_type)
        .bind(request.size_bytes)
        .bind(request.idempotency_key)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AssetError::Persistence)?;
    let asset = match inserted {
        Some(row) => {
            let asset = asset_from_row(row)?;
            append_event(
                &mut transaction,
                &asset,
                "asset_storage.asset.upload_requested",
                request.correlation_id,
            )
            .await?;
            asset
        }
        None => {
            let row = sqlx::query("SELECT asset_id,workspace_id,owner_user_id,object_key,upload_object_key,display_name,content_type,declared_size_bytes,stored_size_bytes,etag,lifecycle_state,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix FROM asset_storage.assets WHERE workspace_id=$1 AND owner_user_id=$2 AND idempotency_key=$3")
                .bind(request.workspace_id)
                .bind(request.owner_user_id)
                .bind(request.idempotency_key)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(AssetError::Persistence)?
                .ok_or(AssetError::IdempotencyConflict)?;
            let asset = asset_from_row(row)?;
            if asset.display_name != request.display_name.trim()
                || asset.content_type != request.content_type
                || asset.declared_size_bytes != request.size_bytes
            {
                return Err(AssetError::IdempotencyConflict);
            }
            asset
        }
    };
    transaction
        .commit()
        .await
        .map_err(AssetError::Persistence)?;
    Ok(asset)
}

pub async fn activate_uploaded_asset(
    database: &PgPool,
    confirmation: UploadConfirmation<'_>,
) -> Result<Asset, AssetError> {
    let UploadConfirmation {
        workspace_id,
        owner_user_id,
        asset_id,
        actual_size_bytes,
        actual_content_type,
        etag,
        correlation_id,
    } = confirmation;
    if actual_size_bytes < 1 || etag.trim().is_empty() || etag.len() > 256 {
        return Err(AssetError::InvalidInput);
    }
    validate_content_type(actual_content_type)?;
    let mut transaction = database.begin().await.map_err(AssetError::Persistence)?;
    let row = sqlx::query("SELECT asset_id,workspace_id,owner_user_id,object_key,upload_object_key,display_name,content_type,declared_size_bytes,stored_size_bytes,etag,lifecycle_state,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix FROM asset_storage.assets WHERE workspace_id=$1 AND owner_user_id=$2 AND asset_id=$3 FOR UPDATE")
        .bind(workspace_id)
        .bind(owner_user_id)
        .bind(asset_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AssetError::Persistence)?
        .ok_or(AssetError::NotFound)?;
    let current_asset = asset_from_row(row)?;
    if current_asset.lifecycle_state == "active"
        && current_asset.stored_size_bytes == Some(actual_size_bytes)
        && current_asset.etag.as_deref() == Some(etag)
    {
        transaction
            .commit()
            .await
            .map_err(AssetError::Persistence)?;
        return Ok(current_asset);
    }
    if current_asset.lifecycle_state != "upload_pending"
        || current_asset.declared_size_bytes != actual_size_bytes
        || current_asset.content_type != actual_content_type
    {
        return Err(AssetError::InvalidState);
    }
    let updated = sqlx::query("UPDATE asset_storage.assets SET lifecycle_state='active',stored_size_bytes=$4,etag=$5,row_version=row_version+1,updated_at=now() WHERE workspace_id=$1 AND owner_user_id=$2 AND asset_id=$3 RETURNING asset_id,workspace_id,owner_user_id,object_key,upload_object_key,display_name,content_type,declared_size_bytes,stored_size_bytes,etag,lifecycle_state,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix")
        .bind(workspace_id)
        .bind(owner_user_id)
        .bind(asset_id)
        .bind(actual_size_bytes)
        .bind(etag)
        .fetch_one(&mut *transaction)
        .await
        .map_err(AssetError::Persistence)?;
    let asset = asset_from_row(updated)?;
    append_event(
        &mut transaction,
        &asset,
        "asset_storage.asset.created",
        correlation_id,
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(AssetError::Persistence)?;
    Ok(asset)
}

pub async fn get_private_asset(
    database: &PgPool,
    workspace_id: Uuid,
    owner_user_id: Uuid,
    asset_id: Uuid,
) -> Result<Asset, AssetError> {
    let row = sqlx::query("SELECT asset_id,workspace_id,owner_user_id,object_key,upload_object_key,display_name,content_type,declared_size_bytes,stored_size_bytes,etag,lifecycle_state,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix FROM asset_storage.assets WHERE workspace_id=$1 AND owner_user_id=$2 AND asset_id=$3 AND lifecycle_state='active'")
        .bind(workspace_id)
        .bind(owner_user_id)
        .bind(asset_id)
        .fetch_optional(database)
        .await
        .map_err(AssetError::Persistence)?
        .ok_or(AssetError::NotFound)?;
    asset_from_row(row)
}

pub async fn get_pending_upload(
    database: &PgPool,
    workspace_id: Uuid,
    owner_user_id: Uuid,
    asset_id: Uuid,
) -> Result<Asset, AssetError> {
    let row = sqlx::query("SELECT asset_id,workspace_id,owner_user_id,object_key,upload_object_key,upload_object_key,display_name,content_type,declared_size_bytes,stored_size_bytes,etag,lifecycle_state,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix FROM asset_storage.assets WHERE workspace_id=$1 AND owner_user_id=$2 AND asset_id=$3 AND lifecycle_state IN ('upload_pending','active')")
        .bind(workspace_id)
        .bind(owner_user_id)
        .bind(asset_id)
        .fetch_optional(database)
        .await
        .map_err(AssetError::Persistence)?
        .ok_or(AssetError::NotFound)?;
    asset_from_row(row)
}

pub async fn get_asset_deletion_target(
    database: &PgPool,
    workspace_id: Uuid,
    asset_id: Uuid,
) -> Result<Asset, AssetError> {
    let row = sqlx::query("SELECT asset_id,workspace_id,owner_user_id,object_key,upload_object_key,upload_object_key,display_name,content_type,declared_size_bytes,stored_size_bytes,etag,lifecycle_state,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix FROM asset_storage.assets WHERE workspace_id=$1 AND asset_id=$2 AND lifecycle_state IN ('delete_pending','deleted')")
        .bind(workspace_id)
        .bind(asset_id)
        .fetch_optional(database)
        .await
        .map_err(AssetError::Persistence)?
        .ok_or(AssetError::NotFound)?;
    asset_from_row(row)
}

pub async fn request_asset_deletion(
    database: &PgPool,
    workspace_id: Uuid,
    owner_user_id: Uuid,
    asset_id: Uuid,
    correlation_id: &str,
) -> Result<Asset, AssetError> {
    let mut transaction = database.begin().await.map_err(AssetError::Persistence)?;
    let current = sqlx::query("SELECT asset_id,workspace_id,owner_user_id,object_key,upload_object_key,upload_object_key,display_name,content_type,declared_size_bytes,stored_size_bytes,etag,lifecycle_state,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix FROM asset_storage.assets WHERE workspace_id=$1 AND owner_user_id=$2 AND asset_id=$3 FOR UPDATE")
        .bind(workspace_id)
        .bind(owner_user_id)
        .bind(asset_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AssetError::Persistence)?
        .ok_or(AssetError::NotFound)?;
    let current = asset_from_row(current)?;
    if current.lifecycle_state == "delete_pending" || current.lifecycle_state == "deleted" {
        transaction
            .commit()
            .await
            .map_err(AssetError::Persistence)?;
        return Ok(current);
    }
    if current.lifecycle_state != "active" && current.lifecycle_state != "failed" {
        return Err(AssetError::InvalidState);
    }
    let updated = sqlx::query("UPDATE asset_storage.assets SET lifecycle_state='delete_pending',deleted_at=now(),row_version=row_version+1,updated_at=now() WHERE workspace_id=$1 AND owner_user_id=$2 AND asset_id=$3 RETURNING asset_id,workspace_id,owner_user_id,object_key,upload_object_key,display_name,content_type,declared_size_bytes,stored_size_bytes,etag,lifecycle_state,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix")
        .bind(workspace_id)
        .bind(owner_user_id)
        .bind(asset_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(AssetError::Persistence)?;
    let asset = asset_from_row(updated)?;
    append_event(
        &mut transaction,
        &asset,
        "asset_storage.asset.deletion_requested",
        correlation_id,
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(AssetError::Persistence)?;
    Ok(asset)
}

pub async fn complete_asset_deletion(
    database: &PgPool,
    workspace_id: Uuid,
    asset_id: Uuid,
    correlation_id: &str,
) -> Result<Asset, AssetError> {
    let mut transaction = database.begin().await.map_err(AssetError::Persistence)?;
    let existing = sqlx::query("SELECT asset_id,workspace_id,owner_user_id,object_key,upload_object_key,upload_object_key,display_name,content_type,declared_size_bytes,stored_size_bytes,etag,lifecycle_state,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix FROM asset_storage.assets WHERE workspace_id=$1 AND asset_id=$2 AND lifecycle_state='deleted'")
        .bind(workspace_id)
        .bind(asset_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AssetError::Persistence)?;
    if let Some(row) = existing {
        let asset = asset_from_row(row)?;
        transaction
            .commit()
            .await
            .map_err(AssetError::Persistence)?;
        return Ok(asset);
    }
    let updated = sqlx::query("UPDATE asset_storage.assets SET lifecycle_state='deleted',row_version=row_version+1,updated_at=now() WHERE workspace_id=$1 AND asset_id=$2 AND lifecycle_state='delete_pending' RETURNING asset_id,workspace_id,owner_user_id,object_key,upload_object_key,display_name,content_type,declared_size_bytes,stored_size_bytes,etag,lifecycle_state,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix")
        .bind(workspace_id)
        .bind(asset_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AssetError::Persistence)?
        .ok_or(AssetError::NotFound)?;
    let asset = asset_from_row(updated)?;
    append_event(
        &mut transaction,
        &asset,
        "asset_storage.asset.deleted",
        correlation_id,
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(AssetError::Persistence)?;
    Ok(asset)
}

pub async fn search_private_assets(
    database: &PgPool,
    workspace_id: Uuid,
    owner_user_id: Uuid,
    query: &str,
    page_size: i64,
) -> Result<Vec<Asset>, AssetError> {
    let search = query.trim();
    if search.len() > 160 || !(1..=100).contains(&page_size) {
        return Err(AssetError::InvalidInput);
    }
    let rows = sqlx::query("SELECT asset_id,workspace_id,owner_user_id,object_key,upload_object_key,upload_object_key,display_name,content_type,declared_size_bytes,stored_size_bytes,etag,lifecycle_state,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix FROM asset_storage.assets WHERE workspace_id=$1 AND owner_user_id=$2 AND lifecycle_state='active' AND ($3='' OR search_document @@ plainto_tsquery('simple',$3)) ORDER BY created_at DESC,asset_id LIMIT $4")
        .bind(workspace_id)
        .bind(owner_user_id)
        .bind(search)
        .bind(page_size)
        .fetch_all(database)
        .await
        .map_err(AssetError::Persistence)?;
    rows.into_iter().map(asset_from_row).collect()
}

pub async fn expire_pending_uploads(database: &PgPool, batch_size: i64) -> Result<i64, AssetError> {
    if !(1..=500).contains(&batch_size) {
        return Err(AssetError::InvalidInput);
    }
    let mut transaction = database.begin().await.map_err(AssetError::Persistence)?;
    let rows = sqlx::query("SELECT asset_id,workspace_id,owner_user_id,object_key,upload_object_key,display_name,content_type,declared_size_bytes,stored_size_bytes,etag,lifecycle_state,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix FROM asset_storage.assets WHERE lifecycle_state='upload_pending' AND upload_expires_at<=now() ORDER BY upload_expires_at,asset_id FOR UPDATE SKIP LOCKED LIMIT $1")
        .bind(batch_size)
        .fetch_all(&mut *transaction)
        .await
        .map_err(AssetError::Persistence)?;
    let mut expired = 0_i64;
    for row in rows {
        let current = asset_from_row(row)?;
        let updated = sqlx::query("UPDATE asset_storage.assets SET lifecycle_state='delete_pending',deleted_at=now(),row_version=row_version+1,updated_at=now() WHERE asset_id=$1 AND lifecycle_state='upload_pending' RETURNING asset_id,workspace_id,owner_user_id,object_key,upload_object_key,display_name,content_type,declared_size_bytes,stored_size_bytes,etag,lifecycle_state,row_version,floor(extract(epoch FROM created_at))::bigint AS created_at_unix")
            .bind(current.asset_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(AssetError::Persistence)?;
        if let Some(row) = updated {
            let asset = asset_from_row(row)?;
            let correlation_id = format!("asset-expire-{}", asset.asset_id);
            append_event(
                &mut transaction,
                &asset,
                "asset_storage.asset.deletion_requested",
                &correlation_id,
            )
            .await?;
            expired += 1;
        }
    }
    transaction
        .commit()
        .await
        .map_err(AssetError::Persistence)?;
    Ok(expired)
}

async fn append_event(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    asset: &Asset,
    event_type: &str,
    correlation_id: &str,
) -> Result<(), AssetError> {
    if correlation_id.trim().is_empty() || correlation_id.len() > 128 {
        return Err(AssetError::InvalidInput);
    }
    let event_id = Uuid::new_v4();
    let payload = serde_json::json!({
        "workspace_id": asset.workspace_id,
        "owner_user_id": asset.owner_user_id,
        "asset_id": asset.asset_id,
        "aggregate_id": asset.asset_id,
        "aggregate_version": asset.row_version,
        "correlation_id": correlation_id,
    });
    let payload = serde_json::to_string(&payload).map_err(AssetError::Serialization)?;
    sqlx::query("INSERT INTO asset_storage.outbox_events (event_id,event_type,event_version,occurred_at,payload) VALUES ($1,$2,1,now(),$3::jsonb)")
        .bind(event_id)
        .bind(event_type)
        .bind(payload)
        .execute(&mut **transaction)
        .await
        .map_err(AssetError::Persistence)?;
    Ok(())
}

fn validate_pending_asset(request: &PendingAsset<'_>) -> Result<(), AssetError> {
    if request.workspace_id.is_nil()
        || request.owner_user_id.is_nil()
        || request.display_name.trim().is_empty()
        || request.display_name.trim().len() > 255
        || request.size_bytes < 1
        || request.size_bytes > MAX_ASSET_SIZE_BYTES
        || request.idempotency_key.len() < 16
        || request.idempotency_key.len() > 128
        || request.correlation_id.trim().is_empty()
        || request.correlation_id.len() > 128
    {
        return Err(AssetError::InvalidInput);
    }
    validate_content_type(request.content_type)
}

fn validate_content_type(content_type: &str) -> Result<(), AssetError> {
    let safe_media_type = content_type.split(';').next().unwrap_or_default().trim();
    let supported = safe_media_type.starts_with("image/")
        || safe_media_type.starts_with("video/")
        || safe_media_type.starts_with("audio/")
        || matches!(
            safe_media_type,
            "application/pdf" | "application/octet-stream" | "text/plain"
        );
    if !supported
        || safe_media_type.eq_ignore_ascii_case("image/svg+xml")
        || safe_media_type.eq_ignore_ascii_case("text/html")
        || safe_media_type.len() > 160
    {
        return Err(AssetError::InvalidInput);
    }
    Ok(())
}

fn asset_from_row(row: sqlx::postgres::PgRow) -> Result<Asset, AssetError> {
    Ok(Asset {
        asset_id: row.try_get("asset_id").map_err(AssetError::Persistence)?,
        workspace_id: row
            .try_get("workspace_id")
            .map_err(AssetError::Persistence)?,
        owner_user_id: row
            .try_get("owner_user_id")
            .map_err(AssetError::Persistence)?,
        object_key: row.try_get("object_key").map_err(AssetError::Persistence)?,
        upload_object_key: row
            .try_get("upload_object_key")
            .map_err(AssetError::Persistence)?,
        display_name: row
            .try_get("display_name")
            .map_err(AssetError::Persistence)?,
        content_type: row
            .try_get("content_type")
            .map_err(AssetError::Persistence)?,
        declared_size_bytes: row
            .try_get("declared_size_bytes")
            .map_err(AssetError::Persistence)?,
        stored_size_bytes: row
            .try_get("stored_size_bytes")
            .map_err(AssetError::Persistence)?,
        etag: row.try_get("etag").map_err(AssetError::Persistence)?,
        lifecycle_state: row
            .try_get("lifecycle_state")
            .map_err(AssetError::Persistence)?,
        row_version: row
            .try_get("row_version")
            .map_err(AssetError::Persistence)?,
        created_at_unix: row
            .try_get("created_at_unix")
            .map_err(AssetError::Persistence)?,
    })
}
