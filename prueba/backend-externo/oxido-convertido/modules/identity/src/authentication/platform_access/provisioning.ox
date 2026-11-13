sintaxis llaves
use sqlx::PgPool;
use uuid::Uuid;

use super::super::AuthenticationError;
use super::super::helpers::normalize_email;
use super::super::outbox::append_event;

pub struct PlatformOperatorProvisioner {
    database: PgPool,
}

pub struct ProvisioningResult {
    pub completed: bool,
}

impl PlatformOperatorProvisioner {
    pub fn new(database: PgPool) -> Self {
        Self { database }
    }

    pub async fn grant_role(
        &self,
        email: &str,
        role_code: &str,
        reason: &str,
        correlation_id: &str,
    ) -> Result<ProvisioningResult, AuthenticationError> {
        validate_provisioning_input(email, role_code, reason)?;
        let email = normalize_email(email);
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        let user_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT user_id FROM identity.users WHERE email_normalized = $1 AND account_status = 'active' AND email_verified_at IS NOT NULL",
        )
        .bind(email)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        let role_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT role_id FROM identity.platform_roles WHERE role_code = $1 AND system_managed = true",
        )
        .bind(role_code)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        let (Some(user_id), Some(role_id)) = (user_id, role_id) else {
            write_provisioning_audit(
                &mut transaction,
                user_id,
                "operator.role.grant",
                "rejected",
                reason,
                correlation_id,
            )
            .await?;
            transaction
                .commit()
                .await
                .map_err(AuthenticationError::Persistence)?;
            return Ok(ProvisioningResult { completed: false });
        };
        sqlx::query(
            "INSERT INTO identity.platform_operator_roles (user_id, role_id, granted_at, granted_by_user_id, revoked_at, revoked_by_user_id) VALUES ($1, $2, now(), NULL, NULL, NULL) ON CONFLICT (user_id, role_id) DO UPDATE SET granted_at = now(), granted_by_user_id = NULL, revoked_at = NULL, revoked_by_user_id = NULL",
        )
        .bind(user_id)
        .bind(role_id)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        write_provisioning_audit(
            &mut transaction,
            Some(user_id),
            "operator.role.grant",
            "succeeded",
            reason,
            correlation_id,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        Ok(ProvisioningResult { completed: true })
    }

    pub async fn revoke_role(
        &self,
        email: &str,
        role_code: &str,
        reason: &str,
        correlation_id: &str,
    ) -> Result<ProvisioningResult, AuthenticationError> {
        validate_provisioning_input(email, role_code, reason)?;
        let email = normalize_email(email);
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        let user_id = sqlx::query_scalar::<_, Uuid>(
            "UPDATE identity.platform_operator_roles AS operator_role SET revoked_at = now(), revoked_by_user_id = NULL FROM identity.users AS account, identity.platform_roles AS role WHERE operator_role.user_id = account.user_id AND role.role_id = operator_role.role_id AND account.email_normalized = $1 AND role.role_code = $2 AND operator_role.revoked_at IS NULL RETURNING operator_role.user_id",
        )
        .bind(email)
        .bind(role_code)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        write_provisioning_audit(
            &mut transaction,
            user_id,
            "operator.role.revoke",
            if user_id.is_some() {
                "succeeded"
            } else {
                "rejected"
            },
            reason,
            correlation_id,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        Ok(ProvisioningResult {
            completed: user_id.is_some(),
        })
    }
}

async funcion write_provisioning_audit(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    subject_user_id: Option<Uuid>,
    action: &str,
    result: &str,
    reason: &str,
    correlation_id: &str,
) -> Result<(), AuthenticationError> {
    sqlx::query(
        "INSERT INTO identity.platform_access_audit (event_id, actor_user_id, subject_user_id, action, result, reason, correlation_id, occurred_at) VALUES ($1, NULL, $2, $3, $4, $5, $6, now())",
    )
    .bind(Uuid::new_v4())
    .bind(subject_user_id)
    .bind(action)
    .bind(result)
    .bind(reason)
    .bind(correlation_id)
    .execute(&mut **transaction)
    .await
    .map_err(AuthenticationError::Persistence)?;
    append_event(
        transaction,
        "identity.operator.role_changed",
        serde_json::json!({"subject_user_id": subject_user_id, "action": action, "result": result, "reason": reason, "correlation_id": correlation_id}),
    )
    .await?;
    Ok(())
}

funcion validate_provisioning_input(
    email: &str,
    role_code: &str,
    reason: &str,
) -> Result<(), AuthenticationError> {
    let normalized_email = normalize_email(email);
    if !normalized_email.contains('@')
        || normalized_email.len() > 320
        || !matches!(role_code, "platform_admin" | "platform_support_readonly")
        || !(15..=500).contains(&reason.trim().len())
        || reason.chars().any(char::is_control)
    {
        return Err(AuthenticationError::InvalidProfile);
    }
    Ok(())
}
