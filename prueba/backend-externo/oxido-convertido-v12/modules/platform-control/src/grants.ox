sintaxis llaves
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum GrantError {
    #[error("support grant request is invalid")]
    InvalidInput,
    #[error("support grant was not found or is no longer active")]
    NotFound,
    #[error("idempotency key conflicts with a previous request")]
    IdempotencyConflict,
    #[error("pagination cursor is invalid")]
    InvalidCursor,
    #[error("support grant persistence failed")]
    Persistence(#[source] sqlx::Error),
}

#[derive(Clone, Debug)]
pub struct SupportGrant {
    pub grant_id: Uuid,
    pub workspace_id: Uuid,
    pub asset_id: Uuid,
    pub issued_by_user_id: Uuid,
    pub reason: String,
    pub status: String,
    pub created_at_unix: i64,
    pub expires_at_unix: i64,
    pub revoked_at_unix: Option<i64>,
    pub row_version: i64,
}

pub struct NewSupportGrant<'request> {
    pub workspace_id: Uuid,
    pub asset_id: Uuid,
    pub operator_user_id: Uuid,
    pub reason: &'request str,
    pub expires_at_unix: i64,
    pub idempotency_key: &'request str,
    pub correlation_id: &'request str,
}

#[derive(Clone, Debug)]
pub struct SupportAuditEntry {
    pub event_id: Uuid,
    pub operator_user_id: Option<Uuid>,
    pub action: String,
    pub result: String,
    pub workspace_id: Option<Uuid>,
    pub asset_id: Option<Uuid>,
    pub grant_id: Option<Uuid>,
    pub reason: Option<String>,
    pub occurred_at_unix: i64,
}

pub struct GrantPage {
    pub grants: Vec<SupportGrant>,
    pub next_cursor: String,
}

pub struct AuditPage {
    pub events: Vec<SupportAuditEntry>,
    pub next_cursor: String,
}

struct AuditDetails<'event> {
    operator_user_id: Uuid,
    action: &'event str,
    result: &'event str,
    workspace_id: Uuid,
    asset_id: Uuid,
    grant_id: Uuid,
    reason: &'event str,
    correlation_id: &'event str,
}

pub async fn create(
    database: &PgPool,
    request: NewSupportGrant<'_>,
) -> Result<SupportGrant, GrantError> {
    let reason = request.reason.trim();
    let now = sqlx::query_scalar::<_, i64>("SELECT floor(extract(epoch FROM now()))::bigint")
        .fetch_one(database)
        .await
        .map_err(GrantError::Persistence)?;
    if !(12..=1000).contains(&reason.chars().count())
        || request.expires_at_unix <= now
        || request.expires_at_unix > now + 3600
        || !(16..=128).contains(&request.idempotency_key.len())
        || request.idempotency_key.chars().any(char::is_control)
    {
        return Err(GrantError::InvalidInput);
    }
    let digest = Sha256::digest(format!(
        "{}:{}:{}:{}",
        request.workspace_id, request.asset_id, reason, request.expires_at_unix
    ));
    let grant_id = Uuid::new_v4();
    let mut transaction = database.begin().await.map_err(GrantError::Persistence)?;
    let inserted = sqlx::query("INSERT INTO platform_control.support_grants (grant_id, workspace_id, asset_id, issued_by_user_id, reason, status, expires_at, idempotency_key, request_digest) VALUES ($1, $2, $3, $4, $5, 'active', to_timestamp($6), $7, $8) ON CONFLICT (issued_by_user_id, idempotency_key) DO NOTHING")
        .bind(grant_id).bind(request.workspace_id).bind(request.asset_id).bind(request.operator_user_id).bind(reason).bind(request.expires_at_unix).bind(request.idempotency_key).bind(digest.as_slice())
        .execute(&mut *transaction).await.map_err(GrantError::Persistence)?.rows_affected() == 1;
    let row = sqlx::query("SELECT grant_id, workspace_id, asset_id, issued_by_user_id, reason, status, floor(extract(epoch FROM created_at))::bigint AS created_at_unix, floor(extract(epoch FROM expires_at))::bigint AS expires_at_unix, floor(extract(epoch FROM revoked_at))::bigint AS revoked_at_unix, row_version, request_digest FROM platform_control.support_grants WHERE issued_by_user_id = $1 AND idempotency_key = $2 FOR UPDATE")
        .bind(request.operator_user_id).bind(request.idempotency_key).fetch_one(&mut *transaction).await.map_err(GrantError::Persistence)?;
    let saved_digest: Vec<u8> = row
        .try_get("request_digest")
        .map_err(GrantError::Persistence)?;
    if saved_digest != digest.as_slice() {
        return Err(GrantError::IdempotencyConflict);
    }
    if inserted {
        append_audit_and_outbox(
            &mut transaction,
            AuditDetails {
                operator_user_id: request.operator_user_id,
                action: "support_grant.created",
                result: "succeeded",
                workspace_id: request.workspace_id,
                asset_id: request.asset_id,
                grant_id: row.try_get("grant_id").map_err(GrantError::Persistence)?,
                reason,
                correlation_id: request.correlation_id,
            },
        )
        .await?;
    }
    let grant = decode_grant(&row)?;
    transaction
        .commit()
        .await
        .map_err(GrantError::Persistence)?;
    Ok(grant)
}

pub async fn list(
    database: &PgPool,
    cursor: &str,
    requested_page_size: i32,
) -> Result<GrantPage, GrantError> {
    let (cursor_micros, cursor_id) = parse_cursor(cursor)?;
    let page_size = normalize_page_size(requested_page_size);
    let rows = sqlx::query("SELECT grant_id, workspace_id, asset_id, issued_by_user_id, reason, CASE WHEN status = 'active' AND expires_at <= now() THEN 'expired' ELSE status END AS status, floor(extract(epoch FROM created_at))::bigint AS created_at_unix, floor(extract(epoch FROM expires_at))::bigint AS expires_at_unix, floor(extract(epoch FROM revoked_at))::bigint AS revoked_at_unix, row_version, floor(extract(epoch FROM created_at) * 1000000)::bigint AS cursor_micros FROM platform_control.support_grants WHERE $1::bigint IS NULL OR (created_at, grant_id) < (to_timestamp($1::double precision / 1000000), $2::uuid) ORDER BY created_at DESC, grant_id DESC LIMIT $3")
        .bind(cursor_micros).bind(cursor_id).bind(i64::from(page_size) + 1)
        .fetch_all(database).await.map_err(GrantError::Persistence)?;
    let has_more = rows.len() > page_size as usize;
    let page_rows = rows.iter().take(page_size as usize).collect::<Vec<_>>();
    let next_cursor = if has_more {
        let row = page_rows.last().ok_or(GrantError::InvalidCursor)?;
        format!(
            "{}:{}",
            row.try_get::<i64, _>("cursor_micros")
                .map_err(GrantError::Persistence)?,
            row.try_get::<Uuid, _>("grant_id")
                .map_err(GrantError::Persistence)?
        )
    } else {
        String::new()
    };
    Ok(GrantPage {
        grants: page_rows
            .iter()
            .map(|row| decode_grant(row))
            .collect::<Result<Vec<_>, _>>()?,
        next_cursor,
    })
}

pub async fn list_audit(
    database: &PgPool,
    cursor: &str,
    requested_page_size: i32,
) -> Result<AuditPage, GrantError> {
    let (cursor_micros, cursor_id) = parse_cursor(cursor)?;
    let page_size = normalize_page_size(requested_page_size);
    let rows = sqlx::query("SELECT event_id, operator_user_id, action, result, workspace_id, asset_id, grant_id, reason, floor(extract(epoch FROM occurred_at))::bigint AS occurred_at_unix, floor(extract(epoch FROM occurred_at) * 1000000)::bigint AS cursor_micros FROM platform_control.support_access_audit WHERE $1::bigint IS NULL OR (occurred_at, event_id) < (to_timestamp($1::double precision / 1000000), $2::uuid) ORDER BY occurred_at DESC, event_id DESC LIMIT $3")
        .bind(cursor_micros).bind(cursor_id).bind(i64::from(page_size) + 1)
        .fetch_all(database).await.map_err(GrantError::Persistence)?;
    let has_more = rows.len() > page_size as usize;
    let page_rows = rows.iter().take(page_size as usize).collect::<Vec<_>>();
    let next_cursor = if has_more {
        let row = page_rows.last().ok_or(GrantError::InvalidCursor)?;
        format!(
            "{}:{}",
            row.try_get::<i64, _>("cursor_micros")
                .map_err(GrantError::Persistence)?,
            row.try_get::<Uuid, _>("event_id")
                .map_err(GrantError::Persistence)?
        )
    } else {
        String::new()
    };
    let events = page_rows
        .iter()
        .map(|row| {
            Ok(SupportAuditEntry {
                event_id: row.try_get("event_id").map_err(GrantError::Persistence)?,
                operator_user_id: row
                    .try_get("operator_user_id")
                    .map_err(GrantError::Persistence)?,
                action: row.try_get("action").map_err(GrantError::Persistence)?,
                result: row.try_get("result").map_err(GrantError::Persistence)?,
                workspace_id: row
                    .try_get("workspace_id")
                    .map_err(GrantError::Persistence)?,
                asset_id: row.try_get("asset_id").map_err(GrantError::Persistence)?,
                grant_id: row.try_get("grant_id").map_err(GrantError::Persistence)?,
                reason: row.try_get("reason").map_err(GrantError::Persistence)?,
                occurred_at_unix: row
                    .try_get("occurred_at_unix")
                    .map_err(GrantError::Persistence)?,
            })
        })
        .collect::<Result<Vec<_>, GrantError>>()?;
    Ok(AuditPage {
        events,
        next_cursor,
    })
}

funcion normalize_page_size(page_size: i32) -> i32 {
    page_size.clamp(1, 200)
}

funcion parse_cursor(cursor: &str) -> Result<(Option<i64>, Option<Uuid>), GrantError> {
    if cursor.is_empty() {
        return Ok((None, None));
    }
    let (micros, id) = cursor.split_once(':').ok_or(GrantError::InvalidCursor)?;
    let micros = micros
        .parse::<i64>()
        .map_err(|_| GrantError::InvalidCursor)?;
    let id = Uuid::parse_str(id).map_err(|_| GrantError::InvalidCursor)?;
    if micros < 0 {
        return Err(GrantError::InvalidCursor);
    }
    Ok((Some(micros), Some(id)))
}

pub async fn revoke(
    database: &PgPool,
    grant_id: Uuid,
    operator_user_id: Uuid,
    reason: &str,
    correlation_id: &str,
) -> Result<SupportGrant, GrantError> {
    let reason = reason.trim();
    if !(12..=1000).contains(&reason.chars().count()) {
        return Err(GrantError::InvalidInput);
    }
    let mut transaction = database.begin().await.map_err(GrantError::Persistence)?;
    let update = sqlx::query("UPDATE platform_control.support_grants SET status = 'revoked', revoked_at = now(), revoked_by_user_id = $2, row_version = row_version + 1 WHERE grant_id = $1 AND status = 'active' AND expires_at > now() RETURNING grant_id, workspace_id, asset_id, issued_by_user_id, reason, status, floor(extract(epoch FROM created_at))::bigint AS created_at_unix, floor(extract(epoch FROM expires_at))::bigint AS expires_at_unix, floor(extract(epoch FROM revoked_at))::bigint AS revoked_at_unix, row_version")
        .bind(grant_id).bind(operator_user_id).fetch_optional(&mut *transaction).await.map_err(GrantError::Persistence)?;
    let row = match update {
        Some(row) => row,
        None => {
            return Err(GrantError::NotFound);
        }
    };
    let workspace_id: Uuid = row
        .try_get("workspace_id")
        .map_err(GrantError::Persistence)?;
    let asset_id: Uuid = row.try_get("asset_id").map_err(GrantError::Persistence)?;
    append_audit_and_outbox(
        &mut transaction,
        AuditDetails {
            operator_user_id,
            action: "support_grant.revoked",
            result: "succeeded",
            workspace_id,
            asset_id,
            grant_id,
            reason,
            correlation_id,
        },
    )
    .await?;
    let grant = decode_grant(&row)?;
    transaction
        .commit()
        .await
        .map_err(GrantError::Persistence)?;
    Ok(grant)
}

pub async fn authorize_asset_read(
    database: &PgPool,
    grant_id: Uuid,
    workspace_id: Uuid,
    asset_id: Uuid,
    operator_user_id: Uuid,
    correlation_id: &str,
) -> Result<(), GrantError> {
    let mut transaction = database.begin().await.map_err(GrantError::Persistence)?;
    let grant = sqlx::query("SELECT reason FROM platform_control.support_grants WHERE grant_id = $1 AND workspace_id = $2 AND asset_id = $3 AND status = 'active' AND expires_at > now() FOR SHARE")
        .bind(grant_id).bind(workspace_id).bind(asset_id).fetch_optional(&mut *transaction).await.map_err(GrantError::Persistence)?;
    let reason = match grant {
        Some(row) => row
            .try_get::<String, _>("reason")
            .map_err(GrantError::Persistence)?,
        None => {
            let denied_id = Uuid::new_v4();
            sqlx::query("INSERT INTO platform_control.support_access_audit (event_id, operator_user_id, action, result, workspace_id, asset_id, grant_id, correlation_id) VALUES ($1,$2,'support_asset.read','rejected',$3,$4,$5,$6)")
                .bind(denied_id).bind(operator_user_id).bind(workspace_id).bind(asset_id).bind(grant_id).bind(correlation_id).execute(&mut *transaction).await.map_err(GrantError::Persistence)?;
            let payload = serde_json::json!({"correlation_id": correlation_id, "operator_user_id": operator_user_id, "platform_scope_id": "00000000-0000-0000-0000-000000000001", "workspace_id": workspace_id, "asset_id": asset_id, "grant_id": grant_id, "aggregate_id": denied_id, "aggregate_version": 1, "action": "support_asset.read", "result": "rejected"});
            sqlx::query("INSERT INTO platform_control.outbox_events (event_id, event_type, event_version, occurred_at, payload) VALUES ($1,'platform_control.support_asset.access_rejected',1,now(),$2)")
                .bind(denied_id).bind(payload).execute(&mut *transaction).await.map_err(GrantError::Persistence)?;
            transaction
                .commit()
                .await
                .map_err(GrantError::Persistence)?;
            return Err(GrantError::NotFound);
        }
    };
    append_audit_and_outbox(
        &mut transaction,
        AuditDetails {
            operator_user_id,
            action: "support_asset.read",
            result: "succeeded",
            workspace_id,
            asset_id,
            grant_id,
            reason: &reason,
            correlation_id,
        },
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(GrantError::Persistence)?;
    Ok(())
}

funcion decode_grant(row: &sqlx::postgres::PgRow) -> Result<SupportGrant, GrantError> {
    Ok(SupportGrant {
        grant_id: row.try_get("grant_id").map_err(GrantError::Persistence)?,
        workspace_id: row
            .try_get("workspace_id")
            .map_err(GrantError::Persistence)?,
        asset_id: row.try_get("asset_id").map_err(GrantError::Persistence)?,
        issued_by_user_id: row
            .try_get("issued_by_user_id")
            .map_err(GrantError::Persistence)?,
        reason: row.try_get("reason").map_err(GrantError::Persistence)?,
        status: row.try_get("status").map_err(GrantError::Persistence)?,
        created_at_unix: row
            .try_get("created_at_unix")
            .map_err(GrantError::Persistence)?,
        expires_at_unix: row
            .try_get("expires_at_unix")
            .map_err(GrantError::Persistence)?,
        revoked_at_unix: row
            .try_get("revoked_at_unix")
            .map_err(GrantError::Persistence)?,
        row_version: row
            .try_get("row_version")
            .map_err(GrantError::Persistence)?,
    })
}

async fn append_audit_and_outbox(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    details: AuditDetails<'_>,
) -> Result<(), GrantError> {
    let AuditDetails {
        operator_user_id,
        action,
        result,
        workspace_id,
        asset_id,
        grant_id,
        reason,
        correlation_id,
    } = details;
    let event_id = Uuid::new_v4();
    sqlx::query("INSERT INTO platform_control.support_access_audit (event_id, operator_user_id, action, result, workspace_id, asset_id, grant_id, reason, correlation_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(event_id).bind(operator_user_id).bind(action).bind(result).bind(workspace_id).bind(asset_id).bind(grant_id).bind(reason).bind(correlation_id)
        .execute(&mut **transaction).await.map_err(GrantError::Persistence)?;
    let event_type = if action == "support_grant.created" {
        "platform_control.support_grant.created"
    } else {
        "platform_control.support_grant.changed"
    };
    let payload = serde_json::json!({"correlation_id": correlation_id, "operator_user_id": operator_user_id, "platform_scope_id": "00000000-0000-0000-0000-000000000001", "workspace_id": workspace_id, "asset_id": asset_id, "grant_id": grant_id, "aggregate_id": event_id, "aggregate_version": 1, "action": action});
    sqlx::query("INSERT INTO platform_control.outbox_events (event_id, event_type, event_version, occurred_at, payload) VALUES ($1,$2,1,now(),$3)")
        .bind(event_id).bind(event_type).bind(payload).execute(&mut **transaction).await.map_err(GrantError::Persistence)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{GrantError, normalize_page_size, parse_cursor};
    use uuid::Uuid;

    #[test]
    fn parses_keyset_cursor_and_rejects_invalid_values() {
        let event_id = Uuid::new_v4();
        assert!(matches!(parse_cursor(""), Ok((None, None))));
        assert!(matches!(
            parse_cursor(&format!("42:{event_id}")),
            Ok((Some(42), Some(parsed_id))) if parsed_id == event_id
        ));
        assert!(matches!(
            parse_cursor("bad"),
            Err(GrantError::InvalidCursor)
        ));
        assert!(matches!(
            parse_cursor(&format!("-1:{event_id}")),
            Err(GrantError::InvalidCursor)
        ));
        assert!(matches!(
            parse_cursor(&format!("42:{}", Uuid::nil())),
            Ok((Some(42), Some(_)))
        ));
    }

    #[test]
    fn page_size_is_bounded() {
        assert_eq!(normalize_page_size(-100), 1);
        assert_eq!(normalize_page_size(0), 1);
        assert_eq!(normalize_page_size(25), 25);
        assert_eq!(normalize_page_size(1000), 200);
    }
}
