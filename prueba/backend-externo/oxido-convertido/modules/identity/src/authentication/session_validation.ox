sintaxis llaves
use std::time::SystemTime;

use uuid::Uuid;

use super::account::sessions::insert_security_event;
use super::helpers::{duration_seconds, hash_secret, random_secret, unix_timestamp};
use super::{
    Audience, AuthenticatedSession, AuthenticationError, IdentityService, LoginSession,
    RefreshableSession, SessionRow,
};

impl IdentityService {
    pub async fn authenticate_session(
        &self,
        session_secret: &str,
        csrf_secret: &str,
        audience: Audience,
    ) -> Result<AuthenticatedSession, AuthenticationError> {
        if session_secret.is_empty() || csrf_secret.is_empty() {
            return Err(AuthenticationError::Rejected);
        }
        let now = unix_timestamp(SystemTime::now())?;
        let idle_expiry = now
            .checked_add(duration_seconds(self.idle_session_lifetime)?)
            .ok_or(AuthenticationError::ExpiryOutOfRange)?;
        let session = sqlx::query_as::<_, SessionRow>(
            "UPDATE identity.sessions SET last_seen_at = to_timestamp($4), idle_expires_at = LEAST(to_timestamp($5), absolute_expires_at) WHERE token_hash = $1 AND csrf_hash = $2 AND audience = $3 AND revoked_at IS NULL AND idle_expires_at > to_timestamp($4) AND absolute_expires_at > to_timestamp($4) AND EXISTS (SELECT 1 FROM identity.users WHERE users.user_id = identity.sessions.user_id AND users.account_status = 'active') AND (identity.sessions.audience <> 'platform' OR EXISTS (SELECT 1 FROM identity.platform_operator_roles WHERE platform_operator_roles.user_id = identity.sessions.user_id AND platform_operator_roles.revoked_at IS NULL)) RETURNING session_id, user_id, floor(extract(epoch FROM idle_expires_at))::bigint AS idle_expires_at, floor(extract(epoch FROM absolute_expires_at))::bigint AS absolute_expires_at",
        )
        .bind(hash_secret(session_secret))
        .bind(hash_secret(csrf_secret))
        .bind(audience.as_database_value())
        .bind(now)
        .bind(idle_expiry)
        .fetch_optional(&self.database)
        .await
        .map_err(AuthenticationError::Persistence)?
        .ok_or(AuthenticationError::Rejected)?;
        let (email, email_verified) = sqlx::query_as::<_, (String, bool)>(
            "SELECT email_normalized, email_verified_at IS NOT NULL FROM identity.users WHERE user_id = $1 AND account_status = 'active'",
        )
        .bind(session.user_id)
        .fetch_one(&self.database)
        .await
        .map_err(AuthenticationError::Persistence)?;
        Ok(AuthenticatedSession {
            session_id: session.session_id,
            user_id: session.user_id,
            email,
            email_verified,
            idle_expires_at: session.idle_expires_at,
            absolute_expires_at: session.absolute_expires_at,
        })
    }

    pub async fn refresh_session(
        &self,
        session_secret: &str,
        csrf_secret: &str,
        audience: Audience,
        correlation_id: &str,
    ) -> Result<LoginSession, AuthenticationError> {
        if session_secret.is_empty() || csrf_secret.is_empty() {
            return Err(AuthenticationError::Rejected);
        }
        let now = unix_timestamp(SystemTime::now())?;
        let idle_expiry = now
            .checked_add(duration_seconds(self.idle_session_lifetime)?)
            .ok_or(AuthenticationError::ExpiryOutOfRange)?;
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        let session = sqlx::query_as::<_, RefreshableSession>(
            "SELECT session.user_id, floor(extract(epoch FROM session.absolute_expires_at))::bigint AS absolute_expires_at FROM identity.sessions AS session JOIN identity.users AS account ON account.user_id = session.user_id WHERE session.token_hash = $1 AND session.csrf_hash = $2 AND session.audience = $3 AND session.revoked_at IS NULL AND session.idle_expires_at > to_timestamp($4) AND session.absolute_expires_at > to_timestamp($4) AND account.account_status = 'active' AND (session.audience <> 'platform' OR EXISTS (SELECT 1 FROM identity.platform_operator_roles WHERE platform_operator_roles.user_id = session.user_id AND platform_operator_roles.revoked_at IS NULL)) FOR UPDATE OF session",
        )
        .bind(hash_secret(session_secret))
        .bind(hash_secret(csrf_secret))
        .bind(audience.as_database_value())
        .bind(now)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?
        .ok_or(AuthenticationError::Rejected)?;
        let idle_expires_at = idle_expiry.min(session.absolute_expires_at);
        let session_id = Uuid::new_v4();
        let new_session_secret = random_secret()?;
        let new_csrf_secret = random_secret()?;
        sqlx::query(
            "UPDATE identity.sessions SET revoked_at = to_timestamp($2), revocation_reason = 'rotated' WHERE token_hash = $1 AND revoked_at IS NULL",
        )
        .bind(hash_secret(session_secret))
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        sqlx::query(
            "INSERT INTO identity.sessions (session_id, user_id, audience, token_hash, csrf_hash, created_at, last_seen_at, idle_expires_at, absolute_expires_at) VALUES ($1, $2, $3, $4, $5, to_timestamp($6), to_timestamp($6), to_timestamp($7), to_timestamp($8))",
        )
        .bind(session_id)
        .bind(session.user_id)
        .bind(audience.as_database_value())
        .bind(hash_secret(&new_session_secret))
        .bind(hash_secret(&new_csrf_secret))
        .bind(now)
        .bind(idle_expires_at)
        .bind(session.absolute_expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        insert_security_event(
            &mut transaction,
            Some(session.user_id),
            "session.refresh",
            "succeeded",
            correlation_id,
        )
        .await?;
        if audience == Audience::Platform {
            sqlx::query(
                "INSERT INTO identity.platform_access_audit (event_id, actor_user_id, subject_user_id, action, result, reason, correlation_id, occurred_at) VALUES ($1, $2, $2, 'operator.session.refresh', 'succeeded', NULL, $3, to_timestamp($4))",
            )
            .bind(Uuid::new_v4())
            .bind(session.user_id)
            .bind(correlation_id)
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(AuthenticationError::Persistence)?;
        }
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        Ok(LoginSession {
            session_id,
            session_secret: new_session_secret,
            csrf_secret: new_csrf_secret,
            user_id: session.user_id,
            idle_expires_at,
            absolute_expires_at: session.absolute_expires_at,
        })
    }

    pub async fn logout(
        &self,
        session_secret: &str,
        csrf_secret: &str,
        audience: Audience,
        correlation_id: &str,
    ) -> Result<bool, AuthenticationError> {
        if session_secret.is_empty() || csrf_secret.is_empty() {
            return Ok(false);
        }
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        let user_id = sqlx::query_scalar::<_, Uuid>(
            "UPDATE identity.sessions SET revoked_at = now(), revocation_reason = 'logout' WHERE token_hash = $1 AND csrf_hash = $2 AND audience = $3 AND revoked_at IS NULL RETURNING user_id",
        )
        .bind(hash_secret(session_secret))
        .bind(hash_secret(csrf_secret))
        .bind(audience.as_database_value())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        if let Some(user_id) = user_id {
            insert_security_event(
                &mut transaction,
                Some(user_id),
                "session.logout",
                "succeeded",
                correlation_id,
            )
            .await?;
            if audience == Audience::Platform {
                sqlx::query(
                    "INSERT INTO identity.platform_access_audit (event_id, actor_user_id, subject_user_id, action, result, reason, correlation_id, occurred_at) VALUES ($1, $2, $2, 'operator.logout', 'succeeded', NULL, $3, now())",
                )
                .bind(Uuid::new_v4())
                .bind(user_id)
                .bind(correlation_id)
                .execute(&mut *transaction)
                .await
                .map_err(AuthenticationError::Persistence)?;
            }
        }
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        Ok(user_id.is_some())
    }
}
