use std::time::SystemTime;
use uuid::Uuid;

use super::helpers::unix_timestamp;
use super::outbox::append_event;
use super::{AuthenticationError, IdentityService};

impl IdentityService {
    pub async fn apply_email_provider_event(
        &self,
        event_id: &str,
        event_type: &str,
        message_id: &str,
        recipient: &str,
    ) -> Result<bool, AuthenticationError> {
        let Some(delivery_state) = delivery_state(event_type) else {
            return Ok(false);
        };
        if event_id.is_empty()
            || event_id.len() > 255
            || message_id.is_empty()
            || message_id.len() > 255
            || recipient.is_empty()
            || recipient.len() > 320
        {
            return Ok(false);
        }
        let now = unix_timestamp(SystemTime::now())?;
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        let inserted = sqlx::query_scalar::<_, String>(
            "INSERT INTO identity.email_provider_events (provider_event_id, event_type, provider_message_id, received_at, processed_at) VALUES ($1, $2, $3, to_timestamp($4), to_timestamp($4)) ON CONFLICT (provider_event_id) DO NOTHING RETURNING provider_event_id",
        )
        .bind(event_id)
        .bind(event_type)
        .bind(message_id)
        .bind(now)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?
        .is_some();
        if !inserted {
            transaction
                .commit()
                .await
                .map_err(AuthenticationError::Persistence)?;
            return Ok(false);
        }
        let normalized_recipient = super::normalize_email(recipient);
        let delivered_token = sqlx::query_as::<_, (Uuid, Uuid)>(
            "UPDATE identity.one_time_tokens AS token SET delivery_state = $2, delivery_updated_at = to_timestamp($3) FROM identity.users AS account WHERE token.user_id = account.user_id AND token.provider_message_id = $1 AND account.email_normalized = $4 RETURNING token.token_id, token.user_id",
        )
        .bind(message_id)
        .bind(delivery_state)
        .bind(now)
        .bind(&normalized_recipient)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        if let Some((token_id, user_id)) = delivered_token {
            append_event(
                &mut transaction,
                "identity.email.delivery_updated",
                serde_json::json!({"token_id": token_id, "user_id": user_id, "delivery_state": delivery_state, "provider_event_id": event_id}),
            )
            .await?;
        }
        if matches!(delivery_state, "bounced" | "complained" | "suppressed") {
            let suppressed_users = sqlx::query_scalar::<_, Uuid>(
                "UPDATE identity.marketing_consents AS consent SET consented = false, source = 'resend_suppression', changed_at = to_timestamp($2), correlation_id = $3 FROM identity.users AS account WHERE consent.user_id = account.user_id AND account.email_normalized = $1 AND consent.consented = true RETURNING consent.user_id",
            )
                .bind(normalized_recipient)
                .bind(now)
                .bind(event_id)
                .fetch_all(&mut *transaction)
                .await
                .map_err(AuthenticationError::Persistence)?;
            for user_id in suppressed_users {
                append_event(
                    &mut transaction,
                    "identity.account.marketing_consent_changed",
                    serde_json::json!({"user_id": user_id, "consented": false, "reason": "provider_suppression", "correlation_id": event_id}),
                )
                .await?;
            }
        }
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        Ok(true)
    }
}

fn delivery_state(event_type: &str) -> Option<&'static str> {
    match event_type {
        "email.delivered" => Some("delivered"),
        "email.bounced" => Some("bounced"),
        "email.complained" => Some("complained"),
        "email.failed" => Some("failed"),
        "email.suppressed" => Some("suppressed"),
        _ => None,
    }
}
