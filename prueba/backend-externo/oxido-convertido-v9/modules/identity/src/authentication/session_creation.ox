sintaxis llaves
use std::time::SystemTime;

use uuid::Uuid;

use super::account::sessions::insert_security_event;
use super::helpers::{duration_seconds, hash_secret, random_secret, unix_timestamp};
use super::{Audience, AuthenticationError, IdentityService, LoginSession};

impl IdentityService {
    pub(super) async fn create_session(
        &self,
        user_id: Uuid,
        audience: Audience,
        correlation_id: &str,
    ) -> Result<LoginSession, AuthenticationError> {
        let now = unix_timestamp(SystemTime::now())?;
        let idle_expires_at = now
            .checked_add(duration_seconds(self.idle_session_lifetime)?)
            .ok_or(AuthenticationError::ExpiryOutOfRange)?;
        let absolute_expires_at = now
            .checked_add(duration_seconds(self.absolute_session_lifetime)?)
            .ok_or(AuthenticationError::ExpiryOutOfRange)?;
        if idle_expires_at > absolute_expires_at {
            return Err(AuthenticationError::ExpiryOutOfRange);
        }
        let session_id = Uuid::new_v4();
        let session_secret = random_secret()?;
        let csrf_secret = random_secret()?;
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        sqlx::query(
            "INSERT INTO identity.sessions (session_id, user_id, audience, token_hash, csrf_hash, created_at, last_seen_at, idle_expires_at, absolute_expires_at) VALUES ($1, $2, $3, $4, $5, to_timestamp($6), to_timestamp($6), to_timestamp($7), to_timestamp($8))",
        )
        .bind(session_id)
        .bind(user_id)
        .bind(audience.as_database_value())
        .bind(hash_secret(&session_secret))
        .bind(hash_secret(&csrf_secret))
        .bind(now)
        .bind(idle_expires_at)
        .bind(absolute_expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        if audience == Audience::Platform {
            sqlx::query(
                "INSERT INTO identity.platform_access_audit (event_id, actor_user_id, subject_user_id, action, result, reason, correlation_id, occurred_at) VALUES ($1, $2, $2, 'operator.login', 'succeeded', NULL, $3, to_timestamp($4))",
            )
            .bind(Uuid::new_v4())
            .bind(user_id)
            .bind(correlation_id)
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(AuthenticationError::Persistence)?;
        }
        insert_security_event(
            &mut transaction,
            Some(user_id),
            "session.login",
            "succeeded",
            correlation_id,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        Ok(LoginSession {
            session_id,
            session_secret,
            csrf_secret,
            user_id,
            idle_expires_at,
            absolute_expires_at,
        })
    }

    pub(super) async fn record_login_rejection(
        &self,
        user_id: Option<Uuid>,
        correlation_id: &str,
    ) -> Result<(), AuthenticationError> {
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        insert_security_event(
            &mut transaction,
            user_id,
            "session.login",
            "rejected",
            correlation_id,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        Ok(())
    }
}
