use sqlx::FromRow;
use uuid::Uuid;

use super::account::AccountSessionContext;
use super::outbox::append_event;
use super::{Audience, AuthenticationError, IdentityService};

mod provisioning;

pub use provisioning::{PlatformOperatorProvisioner, ProvisioningResult};

pub struct PlatformPermissions {
    pub user_id: Uuid,
    pub permissions: Vec<String>,
}

#[derive(FromRow)]
struct PermissionRow {
    permission_code: String,
}

impl IdentityService {
    pub async fn platform_permissions(
        &self,
        context: AccountSessionContext<'_>,
    ) -> Result<PlatformPermissions, AuthenticationError> {
        if context.audience != Audience::Platform {
            return Err(AuthenticationError::Rejected);
        }
        let session = self
            .authenticate_session(
                context.session_secret,
                context.csrf_secret,
                context.audience,
            )
            .await?;
        let permissions: Vec<String> = sqlx::query_as::<_, PermissionRow>(
            "SELECT DISTINCT permission.permission_code FROM identity.platform_operator_roles AS operator_role JOIN identity.platform_role_permissions AS role_permission ON role_permission.role_id = operator_role.role_id JOIN identity.platform_permissions AS permission ON permission.permission_code = role_permission.permission_code WHERE operator_role.user_id = $1 AND operator_role.revoked_at IS NULL ORDER BY permission.permission_code",
        )
        .bind(session.user_id)
        .fetch_all(&self.database)
        .await
        .map_err(AuthenticationError::Persistence)?
        .into_iter()
        .map(|permission| permission.permission_code)
        .collect();
        if !permissions
            .iter()
            .any(|permission| permission == "platform.auth.read")
        {
            self.record_platform_access(
                Some(session.user_id),
                Some(session.user_id),
                "operator.permissions.read",
                "rejected",
                None,
                context.correlation_id,
            )
            .await?;
            return Err(AuthenticationError::PermissionDenied);
        }
        self.record_platform_access(
            Some(session.user_id),
            Some(session.user_id),
            "operator.permissions.read",
            "succeeded",
            None,
            context.correlation_id,
        )
        .await?;
        Ok(PlatformPermissions {
            user_id: session.user_id,
            permissions,
        })
    }

    pub async fn record_platform_access(
        &self,
        actor_user_id: Option<Uuid>,
        subject_user_id: Option<Uuid>,
        action: &str,
        result: &str,
        reason: Option<&str>,
        correlation_id: &str,
    ) -> Result<(), AuthenticationError> {
        let mut transaction = self
            .database
            .begin()
            .await
            .map_err(AuthenticationError::Persistence)?;
        sqlx::query(
            "INSERT INTO identity.platform_access_audit (event_id, actor_user_id, subject_user_id, action, result, reason, correlation_id, occurred_at) VALUES ($1, $2, $3, $4, $5, $6, $7, now())",
        )
        .bind(Uuid::new_v4())
        .bind(actor_user_id)
        .bind(subject_user_id)
        .bind(action)
        .bind(result)
        .bind(reason)
        .bind(correlation_id)
        .execute(&mut *transaction)
        .await
        .map_err(AuthenticationError::Persistence)?;
        append_event(
            &mut transaction,
            "identity.operator.access_recorded",
            serde_json::json!({"actor_user_id": actor_user_id, "subject_user_id": subject_user_id, "action": action, "result": result, "correlation_id": correlation_id}),
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(AuthenticationError::Persistence)?;
        Ok(())
    }
}
