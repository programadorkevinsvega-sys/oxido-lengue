sintaxis llaves
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum SubscriptionError {
    #[error("subscription query failed")]
    Persistence(#[source] sqlx::Error),
    #[error("workspace subscription was not found")]
    NotFound,
    #[error("workspace already has a subscription")]
    Conflict,
}

#[derive(Clone, Debug)]
pub struct WorkspaceSubscription {
    pub subscription_id: Uuid,
    pub workspace_id: Uuid,
    pub plan_key: String,
    pub plan_version: i32,
    pub status: String,
    pub current_period_start_unix: i64,
    pub current_period_end_unix: i64,
    pub cancel_at_period_end: bool,
    pub row_version: i64,
    pub scheduled_plan_key: String,
    pub scheduled_plan_version: i32,
}

pub async fn get_for_workspace(
    database: &PgPool,
    workspace_id: Uuid,
) -> Result<WorkspaceSubscription, SubscriptionError> {
    let row = sqlx::query("SELECT subscription.subscription_id, subscription.workspace_id, plan.plan_key, subscription.plan_version, subscription.status, COALESCE(EXTRACT(EPOCH FROM subscription.current_period_start)::BIGINT,0) AS current_period_start_unix, COALESCE(EXTRACT(EPOCH FROM subscription.current_period_end)::BIGINT,0) AS current_period_end_unix, subscription.cancel_at_period_end, subscription.row_version, COALESCE(scheduled.plan_key, '') AS scheduled_plan_key, COALESCE(subscription.scheduled_plan_version, 0) AS scheduled_plan_version FROM commercial_billing.workspace_subscriptions subscription JOIN commercial_billing.catalog_plans plan ON plan.plan_id=subscription.plan_id AND plan.version=subscription.plan_version LEFT JOIN commercial_billing.catalog_plans scheduled ON scheduled.plan_id=subscription.scheduled_plan_id AND scheduled.version=subscription.scheduled_plan_version WHERE subscription.workspace_id=$1")
        .bind(workspace_id)
        .fetch_optional(database)
        .await
        .map_err(SubscriptionError::Persistence)?
        .ok_or(SubscriptionError::NotFound)?;
    use sqlx::Row;
    Ok(WorkspaceSubscription {
        subscription_id: row
            .try_get("subscription_id")
            .map_err(SubscriptionError::Persistence)?,
        workspace_id: row
            .try_get("workspace_id")
            .map_err(SubscriptionError::Persistence)?,
        plan_key: row
            .try_get("plan_key")
            .map_err(SubscriptionError::Persistence)?,
        plan_version: row
            .try_get("plan_version")
            .map_err(SubscriptionError::Persistence)?,
        status: row
            .try_get("status")
            .map_err(SubscriptionError::Persistence)?,
        current_period_start_unix: row
            .try_get("current_period_start_unix")
            .map_err(SubscriptionError::Persistence)?,
        current_period_end_unix: row
            .try_get("current_period_end_unix")
            .map_err(SubscriptionError::Persistence)?,
        cancel_at_period_end: row
            .try_get("cancel_at_period_end")
            .map_err(SubscriptionError::Persistence)?,
        row_version: row
            .try_get("row_version")
            .map_err(SubscriptionError::Persistence)?,
        scheduled_plan_key: row
            .try_get("scheduled_plan_key")
            .map_err(SubscriptionError::Persistence)?,
        scheduled_plan_version: row
            .try_get("scheduled_plan_version")
            .map_err(SubscriptionError::Persistence)?,
    })
}
