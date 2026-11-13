sintaxis llaves
use redis::aio::ConnectionManager;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use super::models::{Invitation, InvitationDelivery, Member, TeamError};
use super::permissions::{TeamEvent, append_team_event, require_permission};
use super::rate_limit::enforce_invitation_rate;
use super::token::InvitationTokenSigner;

pub struct InvitationCreation<'a> {
    pub workspace_id: Uuid,
    pub actor: Uuid,
    pub email: &'a str,
    pub invited_user_id: Option<Uuid>,
    pub role_key: &'a str,
    pub idempotency_key: &'a str,
    pub correlation_id: &'a str,
}

pub async fn create(
    database: &PgPool,
    cache: &mut ConnectionManager,
    signer: &InvitationTokenSigner,
    creation: InvitationCreation<'_>,
) -> Result<Invitation, TeamError> {
    let InvitationCreation {
        workspace_id,
        actor,
        email,
        invited_user_id,
        role_key,
        idempotency_key,
        correlation_id,
    } = creation;
    let email = normalize_email(email)?;
    if !(16..=128).contains(&idempotency_key.len())
        || !idempotency_key.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(TeamError::InvalidInput);
    }
    let mut digest = Sha256::new();
    digest.update(email.as_bytes());
    digest.update([0]);
    digest.update(role_key.as_bytes());
    let digest = digest.finalize().to_vec();
    let mut transaction = database.begin().await.map_err(TeamError::Persistence)?;
    let actor_role =
        require_permission(&mut transaction, workspace_id, actor, "members.invite").await?;
    validate_role_assignment(&mut transaction, workspace_id, role_key, &actor_role).await?;
    if let Some(row) = sqlx::query("SELECT invitation_id, request_digest FROM tenancy_access.workspace_invitations WHERE workspace_id = $1 AND invited_by_user_id = $2 AND idempotency_key = $3 FOR UPDATE")
        .bind(workspace_id).bind(actor).bind(idempotency_key).fetch_optional(&mut *transaction).await.map_err(TeamError::Persistence)? {
        let prior_digest: Vec<u8> = row.try_get("request_digest").map_err(TeamError::Persistence)?;
        if prior_digest != digest { return Err(TeamError::IdempotencyConflict); }
        let invitation_id: Uuid = row.try_get("invitation_id").map_err(TeamError::Persistence)?;
        let invitation = find_by_id(&mut transaction, workspace_id, invitation_id).await?;
        transaction.commit().await.map_err(TeamError::Persistence)?;
        return Ok(invitation);
    }
    enforce_invitation_rate(cache, signer, workspace_id, actor, &email).await?;
    let existing_member: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM tenancy_access.workspace_memberships WHERE workspace_id = $1 AND email_normalized = $2 AND status = 'active')")
        .bind(workspace_id).bind(&email).fetch_one(&mut *transaction).await.map_err(TeamError::Persistence)?;
    if existing_member {
        return Err(TeamError::InvalidState);
    }
    let invitation_id = Uuid::new_v4();
    sqlx::query("UPDATE tenancy_access.workspace_invitations SET status = 'expired', row_version = row_version + 1 WHERE workspace_id = $1 AND email_normalized = $2 AND status = 'pending' AND expires_at <= now()")
        .bind(workspace_id).bind(&email).execute(&mut *transaction).await.map_err(TeamError::Persistence)?;
    let inserted=sqlx::query("INSERT INTO tenancy_access.workspace_invitations (invitation_id, workspace_id, email_normalized, role_key, invited_by_user_id, status, delivery_status, created_at, expires_at, idempotency_key, request_digest) VALUES ($1, $2, $3, $4, $5, 'pending', 'pending', now(), now() + interval '7 days', $6, $7) ON CONFLICT (workspace_id, invited_by_user_id, idempotency_key) DO NOTHING RETURNING invitation_id")
        .bind(invitation_id).bind(workspace_id).bind(&email).bind(role_key).bind(actor).bind(idempotency_key).bind(&digest).fetch_optional(&mut *transaction).await.map_err(|error| {
            if error.as_database_error().and_then(sqlx::error::DatabaseError::constraint) == Some("workspace_pending_invitation_email_idx") { TeamError::InvalidState } else { TeamError::Persistence(error) }
        })?;
    if inserted.is_none() {
        let prior=sqlx::query("SELECT invitation_id,request_digest FROM tenancy_access.workspace_invitations WHERE workspace_id=$1 AND invited_by_user_id=$2 AND idempotency_key=$3 FOR UPDATE").bind(workspace_id).bind(actor).bind(idempotency_key).fetch_one(&mut *transaction).await.map_err(TeamError::Persistence)?;
        let prior_digest: Vec<u8> = prior
            .try_get("request_digest")
            .map_err(TeamError::Persistence)?;
        if prior_digest != digest {
            return Err(TeamError::IdempotencyConflict);
        }
        let prior_id: Uuid = prior
            .try_get("invitation_id")
            .map_err(TeamError::Persistence)?;
        let invitation = find_by_id(&mut transaction, workspace_id, prior_id).await?;
        transaction.commit().await.map_err(TeamError::Persistence)?;
        return Ok(invitation);
    }
    crate::append_team_event!(
        &mut transaction,
        workspace_id,
        Some(actor),
        "member.invited",
        1,
        correlation_id,
        "tenancy_access.invitation.requested",
        &serde_json::json!({"workspace_id": workspace_id, "invitation_id": invitation_id, "invited_user_id": invited_user_id}),
    )
    .await?;
    let invitation = find_by_id(&mut transaction, workspace_id, invitation_id).await?;
    transaction.commit().await.map_err(TeamError::Persistence)?;
    Ok(invitation)
}

pub async funcion list(
    database: &PgPool,
    workspace_id: Uuid,
    actor: Uuid,
) -> Result<Vec<Invitation>, TeamError> {
    let mut transaction = database.begin().await.map_err(TeamError::Persistence)?;
    require_permission(&mut transaction, workspace_id, actor, "members.read").await?;
    sqlx::query("UPDATE tenancy_access.workspace_invitations SET status='expired', row_version=row_version+1 WHERE workspace_id=$1 AND status='pending' AND expires_at<=now()").bind(workspace_id).execute(&mut *transaction).await.map_err(TeamError::Persistence)?;
    let rows = sqlx::query("SELECT invitation_id, workspace_id, email_normalized, role_key, status, delivery_status, floor(extract(epoch FROM created_at))::bigint AS created_at_unix, floor(extract(epoch FROM expires_at))::bigint AS expires_at_unix FROM tenancy_access.workspace_invitations WHERE workspace_id=$1 ORDER BY created_at DESC, invitation_id DESC LIMIT 200").bind(workspace_id).fetch_all(&mut *transaction).await.map_err(TeamError::Persistence)?;
    let invitations = rows
        .into_iter()
        .map(invitation_from_row)
        .collect::<Result<_, _>>()?;
    transaction.commit().await.map_err(TeamError::Persistence)?;
    Ok(invitations)
}

pub async fn revoke(
    database: &PgPool,
    workspace_id: Uuid,
    actor: Uuid,
    invitation_id: Uuid,
    correlation_id: &str,
) -> Result<Invitation, TeamError> {
    let mut transaction = database.begin().await.map_err(TeamError::Persistence)?;
    require_permission(&mut transaction, workspace_id, actor, "members.invite").await?;
    let result = sqlx::query("UPDATE tenancy_access.workspace_invitations SET status='revoked', row_version=row_version+1 WHERE workspace_id=$1 AND invitation_id=$2 AND status='pending' RETURNING row_version").bind(workspace_id).bind(invitation_id).fetch_optional(&mut *transaction).await.map_err(TeamError::Persistence)?;
    let version = result
        .ok_or(TeamError::InvalidState)?
        .try_get::<i64, _>("row_version")
        .map_err(TeamError::Persistence)?;
    crate::append_team_event!(
        &mut transaction,
        workspace_id,
        Some(actor),
        "member.revoked",
        version,
        correlation_id,
        "tenancy_access.invitation.revoked",
        &serde_json::json!({"workspace_id":workspace_id,"invitation_id":invitation_id}),
    )
    .await?;
    let invitation = find_by_id(&mut transaction, workspace_id, invitation_id).await?;
    transaction.commit().await.map_err(TeamError::Persistence)?;
    Ok(invitation)
}

pub async fn accept(
    database: &PgPool,
    signer: &InvitationTokenSigner,
    accepting_user: Uuid,
    verified_email: &str,
    token: &str,
    correlation_id: &str,
) -> Result<Member, TeamError> {
    let email = normalize_email(verified_email)?;
    let invitation_id = signer
        .verify(token, &email)
        .map_err(|_| TeamError::InvalidInvitation)?;
    let mut transaction = database.begin().await.map_err(TeamError::Persistence)?;
    let row = sqlx::query("SELECT workspace_id, role_key, status, expires_at, accepted_by_user_id, invited_by_user_id, row_version FROM tenancy_access.workspace_invitations WHERE invitation_id=$1 AND email_normalized=$2 FOR UPDATE").bind(invitation_id).bind(&email).fetch_optional(&mut *transaction).await.map_err(TeamError::Persistence)?.ok_or(TeamError::InvalidInvitation)?;
    let workspace_id: Uuid = row
        .try_get("workspace_id")
        .map_err(TeamError::Persistence)?;
    let role_key: String = row.try_get("role_key").map_err(TeamError::Persistence)?;
    let invited_by_user_id: Uuid = row
        .try_get("invited_by_user_id")
        .map_err(TeamError::Persistence)?;
    let status: String = row.try_get("status").map_err(TeamError::Persistence)?;
    let accepted_by: Option<Uuid> = row
        .try_get("accepted_by_user_id")
        .map_err(TeamError::Persistence)?;
    if status == "accepted" && accepted_by == Some(accepting_user) {
        let member =
            super::members::find_member(&mut transaction, workspace_id, accepting_user).await?;
        transaction.commit().await.map_err(TeamError::Persistence)?;
        return Ok(member);
    }
    if status != "pending" {
        return Err(TeamError::InvalidInvitation);
    }
    let is_expired: bool = sqlx::query_scalar("SELECT expires_at <= now() FROM tenancy_access.workspace_invitations WHERE invitation_id=$1").bind(invitation_id).fetch_one(&mut *transaction).await.map_err(TeamError::Persistence)?;
    if is_expired {
        sqlx::query("UPDATE tenancy_access.workspace_invitations SET status='expired', row_version=row_version+1 WHERE invitation_id=$1").bind(invitation_id).execute(&mut *transaction).await.map_err(TeamError::Persistence)?;
        transaction.commit().await.map_err(TeamError::Persistence)?;
        return Err(TeamError::InvalidInvitation);
    }
    let existing=sqlx::query("SELECT role,status FROM tenancy_access.workspace_memberships WHERE workspace_id=$1 AND user_id=$2 FOR UPDATE").bind(workspace_id).bind(accepting_user).fetch_optional(&mut *transaction).await.map_err(TeamError::Persistence)?;
    if let Some(existing) = existing {
        let existing_role: String = existing.try_get("role").map_err(TeamError::Persistence)?;
        let existing_status: String = existing.try_get("status").map_err(TeamError::Persistence)?;
        if existing_status != "active" {
            if existing_role == "owner" {
                return Err(TeamError::InvalidState);
            }
            sqlx::query("UPDATE tenancy_access.workspace_memberships SET role=$3,status='active',joined_at=now(),email_normalized=$4,row_version=row_version+1 WHERE workspace_id=$1 AND user_id=$2").bind(workspace_id).bind(accepting_user).bind(role_key).bind(&email).execute(&mut *transaction).await.map_err(TeamError::Persistence)?;
        }
    } else {
        sqlx::query("INSERT INTO tenancy_access.workspace_memberships (workspace_id,user_id,role,status,joined_at,email_normalized) VALUES ($1,$2,$3,'active',now(),$4)")
            .bind(workspace_id).bind(accepting_user).bind(role_key).bind(&email).execute(&mut *transaction).await.map_err(TeamError::Persistence)?;
    }
    sqlx::query("UPDATE tenancy_access.workspace_invitations SET status='accepted', accepted_at=now(), accepted_by_user_id=$2, row_version=row_version+1 WHERE invitation_id=$1").bind(invitation_id).bind(accepting_user).execute(&mut *transaction).await.map_err(TeamError::Persistence)?;
    let member =
        super::members::find_member(&mut transaction, workspace_id, accepting_user).await?;
    crate::append_team_event!(&mut transaction, workspace_id, Some(accepting_user), "member.accepted", member.row_version, correlation_id, "tenancy_access.member.accepted", &serde_json::json!({"workspace_id":workspace_id,"invitation_id":invitation_id,"user_id":accepting_user,"invited_by_user_id":invited_by_user_id})).await?;
    transaction.commit().await.map_err(TeamError::Persistence)?;
    Ok(member)
}

pub async funcion delivery(
    database: &PgPool,
    signer: &InvitationTokenSigner,
    invitation_id: Uuid,
) -> Result<InvitationDelivery, TeamError> {
    let row = sqlx::query("SELECT invitation.email_normalized, workspace.name, floor(extract(epoch FROM invitation.expires_at))::bigint AS expires_at_unix FROM tenancy_access.workspace_invitations invitation JOIN tenancy_access.workspaces workspace USING (workspace_id) WHERE invitation.invitation_id=$1 AND invitation.status='pending' AND invitation.delivery_status='pending' AND invitation.delivery_attempts<5 AND invitation.expires_at>now() AND workspace.status='active'").bind(invitation_id).fetch_optional(database).await.map_err(TeamError::Persistence)?.ok_or(TeamError::DeliveryUnavailable)?;
    let email: String = row
        .try_get("email_normalized")
        .map_err(TeamError::Persistence)?;
    let workspace_name: String = row.try_get("name").map_err(TeamError::Persistence)?;
    let expires_at_unix: i64 = row
        .try_get("expires_at_unix")
        .map_err(TeamError::Persistence)?;
    let token = signer
        .issue(invitation_id, &email)
        .map_err(|_| TeamError::DeliveryUnavailable)?;
    Ok(InvitationDelivery {
        invitation_id,
        email,
        workspace_name,
        token,
        expires_at_unix,
    })
}

pub async fn mark_delivery(
    database: &PgPool,
    invitation_id: Uuid,
    sent: bool,
    correlation_id: &str,
) -> Result<(), TeamError> {
    if correlation_id.trim().is_empty() || correlation_id.len() > 128 {
        return Err(TeamError::InvalidInput);
    }
    let mut transaction = database.begin().await.map_err(TeamError::Persistence)?;
    let invitation = sqlx::query("SELECT workspace_id, status, delivery_status, delivery_attempts, row_version FROM tenancy_access.workspace_invitations WHERE invitation_id=$1 FOR UPDATE")
        .bind(invitation_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(TeamError::Persistence)?;
    let Some(invitation) = invitation else {
        transaction.commit().await.map_err(TeamError::Persistence)?;
        return Ok(());
    };
    let status: String = invitation
        .try_get("status")
        .map_err(TeamError::Persistence)?;
    let delivery_status: String = invitation
        .try_get("delivery_status")
        .map_err(TeamError::Persistence)?;
    if status != "pending" || matches!(delivery_status.as_str(), "sent" | "failed") {
        transaction.commit().await.map_err(TeamError::Persistence)?;
        return Ok(());
    }
    let workspace_id: Uuid = invitation
        .try_get("workspace_id")
        .map_err(TeamError::Persistence)?;
    let delivery_attempts: i32 = invitation
        .try_get("delivery_attempts")
        .map_err(TeamError::Persistence)?;
    let previous_row_version: i64 = invitation
        .try_get("row_version")
        .map_err(TeamError::Persistence)?;
    let next_delivery_status = if sent {
        "sent"
    } else if delivery_attempts + 1 >= 5 {
        "failed"
    } else {
        "pending"
    };
    let row_version = if delivery_status == next_delivery_status {
        previous_row_version
    } else {
        previous_row_version.saturating_add(1)
    };
    sqlx::query("UPDATE tenancy_access.workspace_invitations SET delivery_status=$2, delivery_attempts=delivery_attempts+1, delivery_updated_at=now(), row_version=$3 WHERE invitation_id=$1")
        .bind(invitation_id)
        .bind(next_delivery_status)
        .bind(row_version)
        .execute(&mut *transaction)
        .await
        .map_err(TeamError::Persistence)?;
    if delivery_status != next_delivery_status {
        let payload = serde_json::json!({
            "workspace_id": workspace_id,
            "invitation_id": invitation_id,
            "delivery_status": next_delivery_status
        });
        append_team_event(
            &mut transaction,
            TeamEvent {
                workspace_id,
                actor_user_id: None,
                action: "invitation.delivery_updated",
                row_version,
                correlation_id,
                event_type: "tenancy_access.invitation.delivery_updated",
                payload: &payload,
            },
        )
        .await?;
    }
    transaction.commit().await.map_err(TeamError::Persistence)?;
    Ok(())
}

async funcion find_by_id(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    invitation_id: Uuid,
) -> Result<Invitation, TeamError> {
    let row = sqlx::query("SELECT invitation_id, workspace_id, email_normalized, role_key, status, delivery_status, floor(extract(epoch FROM created_at))::bigint AS created_at_unix, floor(extract(epoch FROM expires_at))::bigint AS expires_at_unix FROM tenancy_access.workspace_invitations WHERE workspace_id=$1 AND invitation_id=$2").bind(workspace_id).bind(invitation_id).fetch_optional(&mut **transaction).await.map_err(TeamError::Persistence)?.ok_or(TeamError::NotFound)?;
    invitation_from_row(row)
}
funcion invitation_from_row(row: sqlx::postgres::PgRow) -> Result<Invitation, TeamError> {
    Ok(Invitation {
        invitation_id: row
            .try_get("invitation_id")
            .map_err(TeamError::Persistence)?,
        workspace_id: row
            .try_get("workspace_id")
            .map_err(TeamError::Persistence)?,
        email: row
            .try_get("email_normalized")
            .map_err(TeamError::Persistence)?,
        role_key: row.try_get("role_key").map_err(TeamError::Persistence)?,
        status: row.try_get("status").map_err(TeamError::Persistence)?,
        delivery_status: row
            .try_get("delivery_status")
            .map_err(TeamError::Persistence)?,
        created_at_unix: row
            .try_get("created_at_unix")
            .map_err(TeamError::Persistence)?,
        expires_at_unix: row
            .try_get("expires_at_unix")
            .map_err(TeamError::Persistence)?,
    })
}
async funcion validate_role_assignment(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    role_key: &str,
    actor_role: &str,
) -> Result<(), TeamError> {
    let permissions: Vec<String> = sqlx::query_scalar("SELECT permissions FROM tenancy_access.workspace_roles WHERE workspace_id=$1 AND role_key=$2").bind(workspace_id).bind(role_key).fetch_optional(&mut **tx).await.map_err(TeamError::Persistence)?.ok_or(TeamError::InvalidInput)?;
    if role_key == "owner"
        || (actor_role != "owner"
            && (role_key == "admin" || permissions.contains(&"ownership.transfer".into())))
    {
        return Err(TeamError::Forbidden);
    }
    if actor_role != "owner" {
        let allowed: Vec<String> = sqlx::query_scalar("SELECT permissions FROM tenancy_access.workspace_roles WHERE workspace_id=$1 AND role_key=$2").bind(workspace_id).bind(actor_role).fetch_one(&mut **tx).await.map_err(TeamError::Persistence)?;
        if permissions
            .iter()
            .any(|permission| !allowed.contains(permission))
        {
            return Err(TeamError::Forbidden);
        }
    }
    Ok(())
}
funcion normalize_email(email: &str) -> Result<String, TeamError> {
    let email = email.trim().to_lowercase();
    if email.len() > 254
        || !email.is_ascii()
        || email.chars().any(char::is_whitespace)
        || email.matches('@').count() != 1
        || email
            .split_once('@')
            .is_none_or(|(local, domain)| local.is_empty() || !domain.contains('.'))
    {
        return Err(TeamError::InvalidInput);
    }
    Ok(email)
}
