sintaxis llaves
use std::sync::Arc;

use sqlx::PgPool;
use tonic::{Request, Response, Status};
use xovyra_platform::dependencies::PlatformDependencies;
use xovyra_platform::grpc::commercial_billing_server::CommercialBilling;
use xovyra_platform::grpc::*;

mod authorization;
mod catalog_admin;
mod catalog_cache;
mod stripe;
mod webhook;

pub struct CommercialBillingService {
    database: PgPool,
    cache: redis::aio::ConnectionManager,
    identity_channel: tonic::transport::Channel,
    stripe_secret: Option<String>,
    stripe_webhook_secret: Option<String>,
    app_origin: String,
    http: reqwest::Client,
}

impl CommercialBillingService {
    pub fn new(
        dependencies: Arc<PlatformDependencies>,
        identity_channel: tonic::transport::Channel,
        stripe_secret: Option<String>,
        stripe_webhook_secret: Option<String>,
        app_origin: String,
    ) -> Self {
        Self {
            database: dependencies.commercial_billing_database.clone(),
            cache: dependencies.cache.clone(),
            identity_channel,
            stripe_secret,
            stripe_webhook_secret,
            app_origin,
            http: reqwest::Client::new(),
        }
    }
}

#[tonic::async_trait]
impl CommercialBilling for CommercialBillingService {
    async fn list_platform_catalog(
        &self,
        request: Request<ListPlatformCatalogRequest>,
    ) -> Result<Response<ListPlatformCatalogResponse>, Status> {
        catalog_admin::list(self, request.into_inner())
            .await
            .map(Response::new)
    }

    async fn save_plan_draft(
        &self,
        request: Request<SavePlanDraftRequest>,
    ) -> Result<Response<PlanCatalogVersion>, Status> {
        catalog_admin::save_draft(self, request.into_inner())
            .await
            .map(Response::new)
    }

    async fn approve_plan(
        &self,
        request: Request<ApprovePlanRequest>,
    ) -> Result<Response<PlanCatalogVersion>, Status> {
        catalog_admin::approve(self, request.into_inner())
            .await
            .map(Response::new)
    }

    async fn publish_plan(
        &self,
        request: Request<PublishPlanRequest>,
    ) -> Result<Response<PlanCatalogVersion>, Status> {
        catalog_admin::publish(self, request.into_inner())
            .await
            .map(Response::new)
    }

    async fn list_published_plans(
        &self,
        request: Request<ListPublishedPlansRequest>,
    ) -> Result<Response<ListPublishedPlansResponse>, Status> {
        let input = request.into_inner();
        let plans = catalog_cache::list_published(
            &self.database,
            &mut self.cache.clone(),
            &input.locale,
            &input.currency,
        )
        .await
        .map_err(catalog_status)?;
        Ok(Response::new(ListPublishedPlansResponse {
            plans: plans
                .into_iter()
                .map(|plan| PublishedPlan {
                    plan_id: plan.plan_id.to_string(),
                    version: plan.version,
                    plan_key: plan.plan_key,
                    display_name: plan.display_name,
                    description: plan.description,
                    trial_days: plan.trial_days,
                    currency: plan.currency,
                    amount_minor: plan.amount_minor,
                    locale: plan.locale,
                    capabilities: plan
                        .capabilities
                        .into_iter()
                        .map(|(key, limit)| PlanCapability { key, limit })
                        .collect(),
                })
                .collect(),
        }))
    }

    async fn get_workspace_subscription(
        &self,
        request: Request<GetWorkspaceSubscriptionRequest>,
    ) -> Result<Response<WorkspaceSubscriptionResponse>, Status> {
        let input = request.into_inner();
        let workspace_id = authorization::authorize_workspace(
            self.identity_channel.clone(),
            input.session,
            &input.workspace_id,
            "workspace.billing.read",
        )
        .await?;
        let subscription = xovyra_commercial_billing::subscriptions::get_for_workspace(
            &self.database,
            workspace_id,
        )
        .await
        .map_err(subscription_status)?;
        Ok(Response::new(stripe::subscription_response(subscription)))
    }

    async fn create_checkout_session(
        &self,
        request: Request<CreateCheckoutSessionRequest>,
    ) -> Result<Response<CheckoutSessionResponse>, Status> {
        stripe::create_checkout(self, request.into_inner())
            .await
            .map(Response::new)
    }

    async fn cancel_subscription(
        &self,
        request: Request<CancelSubscriptionRequest>,
    ) -> Result<Response<WorkspaceSubscriptionResponse>, Status> {
        stripe::set_cancel_at_period_end(self, request.into_inner(), true)
            .await
            .map(Response::new)
    }

    async fn reactivate_subscription(
        &self,
        request: Request<ReactivateSubscriptionRequest>,
    ) -> Result<Response<WorkspaceSubscriptionResponse>, Status> {
        stripe::set_cancel_at_period_end(self, request.into_inner(), false)
            .await
            .map(Response::new)
    }

    async fn change_subscription_plan(
        &self,
        request: Request<ChangeSubscriptionPlanRequest>,
    ) -> Result<Response<WorkspaceSubscriptionResponse>, Status> {
        stripe::schedule_plan_change(self, request.into_inner())
            .await
            .map(Response::new)
    }

    async fn apply_stripe_webhook(
        &self,
        request: Request<ApplyStripeWebhookRequest>,
    ) -> Result<Response<ApplyStripeWebhookResponse>, Status> {
        webhook::apply(self, request.into_inner())
            .await
            .map(Response::new)
    }
}

fn catalog_status(error: xovyra_commercial_billing::catalog::CatalogError) -> Status {
    match error {
        xovyra_commercial_billing::catalog::CatalogError::Persistence(source) => {
            webhook::log_database_error("billing.catalog.list", &source);
            Status::unavailable("Billing catalog could not be loaded")
        }
    }
}

fn subscription_status(
    error: xovyra_commercial_billing::subscriptions::SubscriptionError,
) -> Status {
    match error {
        xovyra_commercial_billing::subscriptions::SubscriptionError::NotFound => {
            Status::not_found("Workspace subscription was not found")
        }
        xovyra_commercial_billing::subscriptions::SubscriptionError::Conflict => {
            Status::already_exists("Workspace already has a subscription")
        }
        xovyra_commercial_billing::subscriptions::SubscriptionError::Persistence(source) => {
            webhook::log_database_error("billing.subscription.read", &source);
            Status::unavailable("Workspace subscription could not be loaded")
        }
    }
}
