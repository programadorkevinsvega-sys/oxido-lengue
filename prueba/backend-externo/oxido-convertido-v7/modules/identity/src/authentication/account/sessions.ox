sintaxis llaves
use sqlx::FromRow;
use uuid::Uuid;

use super::super::helpers::hash_secret;
use super::super::outbox::append_event;
use super::super::{AuthenticationError, IdentityService};
use super::cursor::{decode, encode};
use super::{AccountSessionContext, SessionPageRequest, SessionRevocation, require_tenant};

#[derive(FromRow)]
pub struct AccountSession {
    pub session_id: Uuid,
    pub created_at_unix: i64,
    pub last_seen_at_unix: i64,
    pub idle_expires_at_unix: i64,
    pub absolute_expires_at_unix: i64,
    pub current: bool,
}

pub struct AccountSessionPage {
    pub items: Vec<AccountSession>,
    pub next_cursor: Option<String>,
}

pub struct RevokedSession {
    pub revoked: bool,
    pub current_session_revoked: bool,
}

impl IdentityService {
    pub async fn list_account_sessions(
        &self,
        context: AccountSessionContext<'_>,
        page: SessionPageRequest<'_>,
    ) -> Result<AccountSessionPage, AuthenticationError> {
        require_tenant(context.audience)?;
        if !(1..=100).contains(&page.page_size) {
            return Err(AuthenticationError::InvalidCursor);
        }
        let current = self
            .authenticate_session(
                context.session_secret,
                context.csrf_secret,
                context.audience,
            )
            .await?;
        let cursor = decode(page.cursor)?;
        let cursor_timestamp = cursor.map(|cursor| cursor.0);
        let cursor_identifier = cursor.map(|cursor| cursor.1);
        let mut items = sqlx::query_as::<_, AccountSession>(
            "SELECT session_id, floor(extract(epoch FROM created_at))::bigint AS created_at_unix, floor(extract(epoch FROM last_seen_at))::bigint AS last_seen_at_unix, floor(extract(epoch FROM idle_expires_at))::bigint AS idle_expires_at_unix, floor(extract(epoch FROM absolute_expires_at))::bigint AS absolute_expires_at_unix, session_id = $1 AS current FROM identity.sessions WHERE user_id = $2 AND audience = 'tenant' AND revoked_at IS NULL AND idle_expires_at > now() AND absolute_expires_at > now() AND ($3::bigint IS NULL OR (created_at, session_id) < (to_timestamp($3), $4::uuid)) ORDER BY created_at DESC, session_id DESC LIMIT $5",
        )
        .bind(current.session_id)
        .bind(current.user_id)
        .bind(cursor_timestamp)
        .bind(cursor_identifier)
        .bind(page.page_size + 1)
        .fetch_all(&self.database)
        .await
        .map_err(AuthenticationError::Persistence)?;
        let has_more = items.len() as i64 > page.page_size;
        if has_more {
            items.pop();
        }
        let next_cursor = if has_more {
            items
                .last()
                .map(|item| encode(item.created_at_unix, item.session_id))
        } else {
            None
        };
        Ok(AccountSessionPage { items, next_cursor })
    }

    pub async fn revoke_account_session(
        &self,
        revocation: SessionRevocation<'_, '_>,
    ) -> Result<RevokedSession, AuthenticationError> {
        require_tenant(revocation.context.audience)?;
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        let current = sqlx::query_as::<_, (Uuid, Uuid)>(
            "SELECT session.session_id, session.user_id FROM identity.sessions AS session JOIN identity.users AS account ON account.user_id = session.user_id WHERE session.token_hash = $1 AND session.csrf_hash = $2 AND session.audience = 'tenant' AND session.revoked_at IS NULL AND session.idle_expires_at > now() AND session.absolute_expires_at > now() AND account.account_status = 'active' FOR UPDATE OF session",
        )
        .bind(hash_secret(revocation.context.session_secret))
        .bind(hash_secret(revocation.context.csrf_secret))
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?
        .ok_or(AuthenticationError::Rejected)?;
        let revoked_session = sqlx::query_scalar::<_, Uuid>(
            "UPDATE identity.sessions SET revoked_at = now(), revocation_reason = 'user_revoked' WHERE session_id = $1 AND user_id = $2 AND audience = 'tenant' AND revoked_at IS NULL RETURNING session_id",
        )
        .bind(revocation.target_session_id)
        .bind(current.1)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        if revoked_session.is_some() {
            insert_security_event(
                &mut transaction,
                Some(current.1),
                "session.revoked",
                "succeeded",
                revocation.correlation_id,
            )
            .await?;
        }
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        Ok(RevokedSession {
            revoked: revoked_session.is_some(),
            current_session_revoked: revoked_session == Some(current.0),
        })
    }

    pub async fn revoke_other_account_sessions(
        &self,
        context: AccountSessionContext<'_>,
        correlation_id: &str,
    ) -> Result<u64, AuthenticationError> {
        require_tenant(context.audience)?;
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        let current = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "SELECT session.session_id, session.user_id, floor(extract(epoch FROM session.absolute_expires_at))::bigint FROM identity.sessions AS session JOIN identity.users AS account ON account.user_id = session.user_id WHERE session.token_hash = $1 AND session.csrf_hash = $2 AND session.audience = 'tenant' AND session.revoked_at IS NULL AND session.idle_expires_at > now() AND session.absolute_expires_at > now() AND account.account_status = 'active' FOR UPDATE OF session",
        )
        .bind(hash_secret(context.session_secret))
        .bind(hash_secret(context.csrf_secret))
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?
        .ok_or(AuthenticationError::Rejected)?;
        let revoked = sqlx::query(
            "UPDATE identity.sessions SET revoked_at = now(), revocation_reason = 'user_revoked_others' WHERE user_id = $1 AND audience = 'tenant' AND session_id <> $2 AND revoked_at IS NULL",
        )
        .bind(current.1)
        .bind(current.0)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?
        .rows_affected();
        insert_security_event(
            &mut transaction,
            Some(current.1),
            "session.others_revoked",
            "succeeded",
            correlation_id,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        Ok(revoked)
    }
}

pub(crate) async funcion insert_security_event(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Option<Uuid>,
    event_type: &str,
    result: &str,
    correlation_id: &str,
) -> Result<(), AuthenticationError> {
    let event_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO identity.security_events (event_id, user_id, event_type, result, occurred_at, correlation_id) VALUES ($1, $2, $3, $4, now(), $5)",
    )
    .bind(event_id)
    .bind(user_id)
    .bind(event_type)
    .bind(result)
    .bind(correlation_id)
    .execute(&mut **transaction)
    .await
    .map_err(AuthenticationError::Persistence)?;
    let event_type_for_outbox = match event_type {
        "session.login" if result == "succeeded" => "identity.session.created",
        "session.login" => "identity.session.login_rejected",
        "session.refresh" => "identity.session.refreshed",
        "session.logout" => "identity.session.revoked",
        "session.revoked" | "session.others_revoked" => "identity.session.revoked",
        "account.registration" => "identity.account.registered",
        "account.email_verified" => "identity.account.email_verified",
        "account.password_change" | "account.password_reset" => "identity.account.password_changed",
        "account.profile_updated" => "identity.account.profile_updated",
        "account.marketing_consent" => "identity.account.marketing_consent_changed",
        "account.verification_resent" | "account.recovery_requested" => {
            "identity.account.email_action_requested"
        }
        _ => "identity.security.event_recorded",
    };
    append_event(
        transaction,
        event_type_for_outbox,
        serde_json::json!({"event_id": event_id, "user_id": user_id, "action": event_type, "result": result, "correlation_id": correlation_id}),
    )
    .await?;
    Ok(())
}
