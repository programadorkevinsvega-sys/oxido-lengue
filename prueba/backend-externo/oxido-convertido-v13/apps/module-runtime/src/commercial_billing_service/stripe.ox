sintaxis llaves
use sqlx::Row;
use tonic::Status;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;
use xovyra_platform::grpc::*;

use super::CommercialBillingService;
use super::authorization::authorize_workspace;
use xovyra_commercial_billing::subscriptions::WorkspaceSubscription;

pub(super) async fn create_checkout(
    service: &CommercialBillingService,
    input: CreateCheckoutSessionRequest,
) -> Result<CheckoutSessionResponse, Status> {
    let workspace_id = authorize_workspace(
        service.identity_channel.clone(),
        input.session.clone(),
        &input.workspace_id,
        "workspace.billing.manage",
    )
    .await?;
    let api_key = service
        .stripe_secret
        .as_deref()
        .filter(|key| key.starts_with("sk_test_"))
        .ok_or_else(|| Status::failed_precondition("Stripe test checkout is not configured"))?;
    let plan_id = Uuid::parse_str(&input.plan_id)
        .map_err(|_| Status::invalid_argument("Plan identifier is invalid"))?;
    if input.idempotency_key.len() < 16 || input.idempotency_key.len() > 128 {
        return Err(Status::invalid_argument(
            "Checkout idempotency key is invalid",
        ));
    }
    check_billing_mutation_limit(service, workspace_id).await?;
    let price = sqlx::query("SELECT price.price_id, price.stripe_price_id, price.currency, price.amount_minor, plan.trial_days FROM commercial_billing.localized_plan_prices price JOIN commercial_billing.catalog_plans plan ON plan.plan_id=price.plan_id AND plan.version=price.plan_version WHERE price.plan_id=$1 AND price.plan_version=$2 AND price.locale=$3 AND price.currency=$4 AND plan.lifecycle='published' AND price.effective_from<=now() AND (price.effective_until IS NULL OR price.effective_until>now())")
        .bind(plan_id).bind(input.plan_version).bind(&input.locale).bind(input.currency.to_uppercase())
        .fetch_optional(&service.database).await.map_err(database_status)?
        .ok_or_else(|| Status::failed_precondition("The requested plan price is not published"))?;
    let price_id: Uuid = price.try_get("price_id").map_err(database_status)?;
    let stripe_price_id: String = price.try_get("stripe_price_id").map_err(database_status)?;
    let currency: String = price.try_get("currency").map_err(database_status)?;
    let amount_minor: i64 = price.try_get("amount_minor").map_err(database_status)?;
    let trial_days: i32 = price.try_get("trial_days").map_err(database_status)?;
    ensure_stripe_price(service, api_key, &stripe_price_id, &currency, amount_minor).await?;
    let existing: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM commercial_billing.workspace_subscriptions WHERE workspace_id=$1 AND status NOT IN ('canceled','incomplete_expired'))")
        .bind(workspace_id).fetch_one(&service.database).await.map_err(database_status)?;
    if existing {
        return Err(Status::already_exists(
            "Workspace already has an active subscription",
        ));
    }
    let mut form = vec![
        ("mode", "subscription".to_owned()),
        ("line_items[0][price]", stripe_price_id.clone()),
        ("line_items[0][quantity]", "1".to_owned()),
        (
            "success_url",
            format!("{}/suscripcion?checkout=success", service.app_origin),
        ),
        (
            "cancel_url",
            format!("{}/suscripcion?checkout=cancelled", service.app_origin),
        ),
        ("client_reference_id", workspace_id.to_string()),
        ("metadata[workspace_id]", workspace_id.to_string()),
        ("metadata[plan_id]", plan_id.to_string()),
        ("metadata[plan_version]", input.plan_version.to_string()),
        ("metadata[price_id]", price_id.to_string()),
        (
            "subscription_data[metadata][workspace_id]",
            workspace_id.to_string(),
        ),
        ("subscription_data[metadata][plan_id]", plan_id.to_string()),
        (
            "subscription_data[metadata][plan_version]",
            input.plan_version.to_string(),
        ),
    ];
    if trial_days > 0 {
        form.push((
            "subscription_data[trial_period_days]",
            trial_days.to_string(),
        ));
    }
    let response = service
        .http
        .post("https://api.stripe.com/v1/checkout/sessions")
        .basic_auth(api_key, Some(""))
        .header(
            "Idempotency-Key",
            format!("xovyra:{}:{}", workspace_id, input.idempotency_key),
        )
        .form(&form)
        .send()
        .await
        .map_err(|error| provider_error("checkout.create", &error))?;
    let status = response.status();
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|error| provider_error("checkout.decode", &error))?;
    if !status.is_success() {
        tracing::error!(event="billing.stripe.checkout_rejected", component="commercial_billing", operation="create_checkout", dependency="stripe", http_status=status.as_u16(), provider_code=?body.pointer("/error/code").and_then(serde_json::Value::as_str));
        return Err(Status::failed_precondition(
            "Stripe rejected the checkout configuration",
        ));
    }
    let checkout_id = body
        .get("id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Status::unavailable("Stripe checkout response is incomplete"))?;
    let url = body
        .get("url")
        .and_then(serde_json::Value::as_str)
        .filter(|value| value.starts_with("https://checkout.stripe.com/"))
        .ok_or_else(|| Status::unavailable("Stripe checkout response is incomplete"))?;
    let subscription_id = Uuid::new_v4();
    let correlation_id = input
        .session
        .as_ref()
        .map(|session| session.correlation_id.as_str())
        .unwrap_or_default();
    let mut transaction = service.database.begin().await.map_err(database_status)?;
    let inserted = sqlx::query("INSERT INTO commercial_billing.workspace_subscriptions (subscription_id,workspace_id,plan_id,plan_version,price_id,status,stripe_checkout_session_id) VALUES ($1,$2,$3,$4,$5,'incomplete',$6) ON CONFLICT (workspace_id) WHERE status NOT IN ('canceled','incomplete_expired') DO NOTHING")
        .bind(subscription_id).bind(workspace_id).bind(plan_id).bind(input.plan_version).bind(price_id).bind(checkout_id)
        .execute(&mut *transaction).await.map_err(database_status)?;
    if inserted.rows_affected() == 0 {
        let same_checkout: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM commercial_billing.workspace_subscriptions WHERE workspace_id=$1 AND stripe_checkout_session_id=$2)")
            .bind(workspace_id).bind(checkout_id).fetch_one(&mut *transaction).await.map_err(database_status)?;
        transaction.rollback().await.map_err(database_status)?;
        if same_checkout {
            return Ok(CheckoutSessionResponse {
                checkout_url: url.to_owned(),
            });
        }
        return Err(Status::already_exists(
            "Workspace already has a subscription",
        ));
    }
    insert_event(
        &mut transaction,
        "commercial_billing.subscription.checkout_created",
        workspace_id,
        subscription_id,
        1,
        correlation_id,
    )
    .await?;
    transaction.commit().await.map_err(database_status)?;
    Ok(CheckoutSessionResponse {
        checkout_url: url.to_owned(),
    })
}

pub(super) async fn set_cancel_at_period_end(
    service: &CommercialBillingService,
    input: impl SubscriptionCommand,
    cancel: bool,
) -> Result<WorkspaceSubscriptionResponse, Status> {
    let session = input
        .session()
        .ok_or_else(|| Status::unauthenticated("Tenant session is required"))?;
    let workspace_id = authorize_workspace(
        service.identity_channel.clone(),
        Some(session.clone()),
        input.workspace_id(),
        "workspace.billing.manage",
    )
    .await?;
    let api_key = service
        .stripe_secret
        .as_deref()
        .filter(|key| key.starts_with("sk_test_"))
        .ok_or_else(|| Status::failed_precondition("Stripe test checkout is not configured"))?;
    validate_idempotency_key(input.idempotency_key())?;
    check_billing_mutation_limit(service, workspace_id).await?;
    let row = sqlx::query("SELECT stripe_subscription_id, stripe_subscription_schedule_id, row_version FROM commercial_billing.workspace_subscriptions WHERE workspace_id=$1 AND status IN ('active','trialing','past_due')")
        .bind(workspace_id).fetch_optional(&service.database).await.map_err(database_status)?
        .ok_or_else(|| Status::not_found("Workspace subscription was not found"))?;
    let stripe_id: Option<String> = row
        .try_get("stripe_subscription_id")
        .map_err(database_status)?;
    let row_version: i64 = row.try_get("row_version").map_err(database_status)?;
    let schedule_id: Option<String> = row
        .try_get("stripe_subscription_schedule_id")
        .map_err(database_status)?;
    let stripe_id = stripe_id.ok_or_else(|| {
        Status::failed_precondition("The subscription is awaiting Stripe confirmation")
    })?;
    if cancel && let Some(schedule_id) = schedule_id {
        let release = service
            .http
            .post(format!(
                "https://api.stripe.com/v1/subscription_schedules/{schedule_id}/release"
            ))
            .basic_auth(api_key, Some(""))
            .header(
                "Idempotency-Key",
                format!(
                    "xovyra:billing:schedule:release:{}",
                    input.idempotency_key()
                ),
            )
            .send()
            .await
            .map_err(|error| provider_error("subscription_schedule.release", &error))?;
        if !release.status().is_success() {
            tracing::error!(
                event = "billing.stripe.schedule_release_rejected",
                component = "commercial_billing",
                operation = "cancel_subscription",
                dependency = "stripe",
                http_status = release.status().as_u16()
            );
            return Err(Status::failed_precondition(
                "Stripe could not clear the scheduled plan change",
            ));
        }
    }
    let response = service
        .http
        .post(format!(
            "https://api.stripe.com/v1/subscriptions/{stripe_id}"
        ))
        .basic_auth(api_key, Some(""))
        .header("Idempotency-Key", input.idempotency_key())
        .form(&[(
            "cancel_at_period_end",
            if cancel { "true" } else { "false" },
        )])
        .send()
        .await
        .map_err(|error| provider_error("subscription.update", &error))?;
    if !response.status().is_success() {
        tracing::error!(
            event = "billing.stripe.subscription_update_rejected",
            operation = "set_cancel_at_period_end",
            dependency = "stripe",
            http_status = response.status().as_u16()
        );
        return Err(Status::failed_precondition(
            "Stripe could not update the subscription",
        ));
    }
    let mut transaction = service.database.begin().await.map_err(database_status)?;
    let changed = sqlx::query("UPDATE commercial_billing.workspace_subscriptions SET cancel_at_period_end=$2,scheduled_plan_id=CASE WHEN $2 THEN NULL ELSE scheduled_plan_id END,scheduled_plan_version=CASE WHEN $2 THEN NULL ELSE scheduled_plan_version END,stripe_subscription_schedule_id=CASE WHEN $2 THEN NULL ELSE stripe_subscription_schedule_id END,row_version=row_version+1,updated_at=now() WHERE workspace_id=$1 AND row_version=$3")
        .bind(workspace_id).bind(cancel).bind(row_version).execute(&mut *transaction).await.map_err(database_status)?;
    if changed.rows_affected() != 1 {
        return Err(Status::aborted("Subscription changed concurrently"));
    }
    let aggregate: Uuid = sqlx::query_scalar("SELECT subscription_id FROM commercial_billing.workspace_subscriptions WHERE workspace_id=$1")
        .bind(workspace_id).fetch_one(&mut *transaction).await.map_err(database_status)?;
    insert_event(
        &mut transaction,
        if cancel {
            "commercial_billing.subscription.cancel_scheduled"
        } else {
            "commercial_billing.subscription.reactivated"
        },
        workspace_id,
        aggregate,
        row_version + 1,
        &session.correlation_id,
    )
    .await?;
    transaction.commit().await.map_err(database_status)?;
    let subscription = xovyra_commercial_billing::subscriptions::get_for_workspace(
        &service.database,
        workspace_id,
    )
    .await
    .map_err(|_| Status::unavailable("Workspace subscription could not be reloaded"))?;
    Ok(subscription_response(subscription))
}

pub(super) async fn schedule_plan_change(
    service: &CommercialBillingService,
    input: ChangeSubscriptionPlanRequest,
) -> Result<WorkspaceSubscriptionResponse, Status> {
    let session = input
        .session
        .as_ref()
        .ok_or_else(|| Status::unauthenticated("Tenant session is required"))?;
    let workspace_id = authorize_workspace(
        service.identity_channel.clone(),
        Some(session.clone()),
        &input.workspace_id,
        "workspace.billing.manage",
    )
    .await?;
    let api_key = service
        .stripe_secret
        .as_deref()
        .filter(|key| key.starts_with("sk_test_"))
        .ok_or_else(|| Status::failed_precondition("Stripe test billing is not configured"))?;
    validate_idempotency_key(&input.idempotency_key)?;
    check_billing_mutation_limit(service, workspace_id).await?;
    let target_plan_id = Uuid::parse_str(&input.plan_id)
        .map_err(|_| Status::invalid_argument("Plan identifier is invalid"))?;
    let target_price = sqlx::query("SELECT price.price_id, price.stripe_price_id, price.currency, price.amount_minor, plan.plan_key FROM commercial_billing.localized_plan_prices price JOIN commercial_billing.catalog_plans plan ON plan.plan_id=price.plan_id AND plan.version=price.plan_version WHERE price.plan_id=$1 AND price.plan_version=$2 AND price.locale=$3 AND price.currency=$4 AND plan.lifecycle='published' AND price.effective_from<=now() AND (price.effective_until IS NULL OR price.effective_until>now())")
        .bind(target_plan_id).bind(input.plan_version).bind(&input.locale).bind(input.currency.to_uppercase())
        .fetch_optional(&service.database).await.map_err(database_status)?
        .ok_or_else(|| Status::failed_precondition("The requested plan price is not published"))?;
    let target_stripe_price: String = target_price
        .try_get("stripe_price_id")
        .map_err(database_status)?;
    let target_currency: String = target_price.try_get("currency").map_err(database_status)?;
    let target_amount: i64 = target_price
        .try_get("amount_minor")
        .map_err(database_status)?;
    let target_plan_key: String = target_price.try_get("plan_key").map_err(database_status)?;
    ensure_stripe_price(
        service,
        api_key,
        &target_stripe_price,
        &target_currency,
        target_amount,
    )
    .await?;

    let current = sqlx::query("SELECT subscription_id, stripe_subscription_id, stripe_subscription_schedule_id, price.stripe_price_id AS current_stripe_price_id, price.currency AS current_currency, EXTRACT(EPOCH FROM subscription.current_period_start)::BIGINT AS current_period_start_unix, EXTRACT(EPOCH FROM subscription.current_period_end)::BIGINT AS current_period_end_unix, subscription.row_version, subscription.plan_id, subscription.plan_version FROM commercial_billing.workspace_subscriptions subscription JOIN commercial_billing.localized_plan_prices price ON price.price_id=subscription.price_id WHERE subscription.workspace_id=$1 AND subscription.status IN ('active','trialing')")
        .bind(workspace_id).fetch_optional(&service.database).await.map_err(database_status)?
        .ok_or_else(|| Status::not_found("An active workspace subscription was not found"))?;
    let current_plan_id: Uuid = current.try_get("plan_id").map_err(database_status)?;
    let current_plan_version: i32 = current.try_get("plan_version").map_err(database_status)?;
    if current_plan_id == target_plan_id && current_plan_version == input.plan_version {
        return Err(Status::failed_precondition(
            "The requested plan is already active",
        ));
    }
    let stripe_subscription_id: Option<String> = current
        .try_get("stripe_subscription_id")
        .map_err(database_status)?;
    let stripe_subscription_id = stripe_subscription_id.ok_or_else(|| {
        Status::failed_precondition("The subscription is awaiting Stripe confirmation")
    })?;
    let existing_schedule: Option<String> = current
        .try_get("stripe_subscription_schedule_id")
        .map_err(database_status)?;
    let current_stripe_price: String = current
        .try_get("current_stripe_price_id")
        .map_err(database_status)?;
    let current_currency: String = current
        .try_get("current_currency")
        .map_err(database_status)?;
    if !current_currency.eq_ignore_ascii_case(&target_currency) {
        return Err(Status::failed_precondition(
            "A plan change must keep the subscription currency",
        ));
    }
    let period_start: Option<i64> = current
        .try_get("current_period_start_unix")
        .map_err(database_status)?;
    let period_end: Option<i64> = current
        .try_get("current_period_end_unix")
        .map_err(database_status)?;
    let row_version: i64 = current.try_get("row_version").map_err(database_status)?;
    let period_start = period_start.ok_or_else(|| {
        Status::failed_precondition("The current billing period is not confirmed yet")
    })?;
    let period_end = period_end.ok_or_else(|| {
        Status::failed_precondition("The current billing period is not confirmed yet")
    })?;
    if period_end <= period_start {
        return Err(Status::failed_precondition(
            "The current billing period is invalid",
        ));
    }

    let schedule_id = if let Some(schedule) = existing_schedule {
        schedule
    } else {
        let response = service
            .http
            .post("https://api.stripe.com/v1/subscription_schedules")
            .basic_auth(api_key, Some(""))
            .header(
                "Idempotency-Key",
                format!("xovyra:billing:schedule:create:{}", input.idempotency_key),
            )
            .form(&[("from_subscription", stripe_subscription_id.as_str())])
            .send()
            .await
            .map_err(|error| provider_error("subscription_schedule.create", &error))?;
        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|error| provider_error("subscription_schedule.create.decode", &error))?;
        if !status.is_success() {
            log_stripe_rejection("subscription_schedule.create", status.as_u16(), &body);
            return Err(Status::failed_precondition(
                "Stripe could not prepare the plan change",
            ));
        }
        body.get("id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| id.starts_with("sub_sched_"))
            .map(str::to_owned)
            .ok_or_else(|| {
                Status::unavailable("Stripe returned an incomplete plan change response")
            })?
    };
    let schedule_form = schedule_update_form(
        &current_stripe_price,
        &target_stripe_price,
        period_start,
        period_end,
    );
    let update = service
        .http
        .post(format!(
            "https://api.stripe.com/v1/subscription_schedules/{schedule_id}"
        ))
        .basic_auth(api_key, Some(""))
        .header(
            "Idempotency-Key",
            format!("xovyra:billing:schedule:update:{}", input.idempotency_key),
        )
        .form(&schedule_form)
        .send()
        .await
        .map_err(|error| provider_error("subscription_schedule.update", &error))?;
    let update_status = update.status();
    let update_body: serde_json::Value = update
        .json()
        .await
        .map_err(|error| provider_error("subscription_schedule.update.decode", &error))?;
    if !update_status.is_success() {
        log_stripe_rejection(
            "subscription_schedule.update",
            update_status.as_u16(),
            &update_body,
        );
        return Err(Status::failed_precondition(
            "Stripe rejected the scheduled plan change",
        ));
    }

    let mut transaction = service.database.begin().await.map_err(database_status)?;
    let changed = sqlx::query("UPDATE commercial_billing.workspace_subscriptions SET scheduled_plan_id=$2,scheduled_plan_version=$3,stripe_subscription_schedule_id=$4,row_version=row_version+1,updated_at=now() WHERE workspace_id=$1 AND row_version=$5")
        .bind(workspace_id).bind(target_plan_id).bind(input.plan_version).bind(&schedule_id).bind(row_version)
        .execute(&mut *transaction).await.map_err(database_status)?;
    if changed.rows_affected() != 1 {
        transaction.rollback().await.map_err(database_status)?;
        tracing::error!(event="billing.subscription.plan_change_conflict", component="commercial_billing", operation="schedule_plan_change", dependency="postgresql", workspace_id=%workspace_id, correlation_id=%session.correlation_id);
        return Err(Status::aborted(
            "Subscription changed concurrently; refresh and retry",
        ));
    }
    insert_event(
        &mut transaction,
        "commercial_billing.subscription.plan_change_scheduled",
        workspace_id,
        current
            .try_get("subscription_id")
            .map_err(database_status)?,
        row_version + 1,
        &session.correlation_id,
    )
    .await?;
    transaction.commit().await.map_err(database_status)?;
    let result = xovyra_commercial_billing::subscriptions::get_for_workspace(&service.database, workspace_id).await.map_err(|error| {
        tracing::error!(event="billing.subscription.reload_failed", component="commercial_billing", operation="schedule_plan_change.reload", dependency="postgresql", correlation_id=%session.correlation_id, error_category="persistence");
        match error { xovyra_commercial_billing::subscriptions::SubscriptionError::NotFound => Status::not_found("Workspace subscription was not found"), _ => Status::unavailable("Subscription was updated but could not be reloaded") }
    })?;
    let mut response = subscription_response(result);
    response.scheduled_plan_key = target_plan_key;
    response.scheduled_plan_version = input.plan_version;
    Ok(response)
}

fn schedule_update_form(
    current_price: &str,
    next_price: &str,
    period_start: i64,
    period_end: i64,
) -> Vec<(&'static str, String)> {
    vec![
        ("end_behavior", "release".to_owned()),
        ("proration_behavior", "none".to_owned()),
        ("phases[0][start_date]", period_start.to_string()),
        ("phases[0][end_date]", period_end.to_string()),
        ("phases[0][items][0][price]", current_price.to_owned()),
        ("phases[0][items][0][quantity]", "1".to_owned()),
        ("phases[1][start_date]", period_end.to_string()),
        ("phases[1][items][0][price]", next_price.to_owned()),
        ("phases[1][items][0][quantity]", "1".to_owned()),
    ]
}

funcion validate_idempotency_key(key: &str) -> Result<(), Status> {
    if (16..=128).contains(&key.len())
        && key
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || b"_-.:".contains(&value))
    {
        Ok(())
    } else {
        Err(Status::invalid_argument(
            "Subscription idempotency key is invalid",
        ))
    }
}

async fn check_billing_mutation_limit(
    service: &CommercialBillingService,
    workspace_id: Uuid,
) -> Result<(), Status> {
    let hour = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Status::internal("System clock is invalid"))?
        .as_secs()
        / 3600;
    let key = format!("xovyra:billing:mutations:v1:{workspace_id}:{hour}");
    let mut cache = service.cache.clone();
    let count: Result<i64, redis::RedisError> = redis::cmd("EVAL")
        .arg("local n=redis.call('INCR',KEYS[1]); if n==1 then redis.call('EXPIRE',KEYS[1],ARGV[1]); end; return n")
        .arg(1)
        .arg(key)
        .arg(3600)
        .query_async(&mut cache)
        .await;
    match count {
        Ok(value) if value <= 12 => Ok(()),
        Ok(_) => Err(Status::resource_exhausted(
            "Billing changes exceeded the current usage limit",
        )),
        Err(error) => {
            tracing::error!(event="billing.abuse_protection.unavailable", component="commercial_billing", operation="check_billing_mutation_limit", dependency="redis", workspace_id=%workspace_id, error_category="cache_command_failed", error=%error);
            Err(Status::unavailable(
                "Billing is temporarily unavailable while usage limits are checked",
            ))
        }
    }
}

fn log_stripe_rejection(operation: &'static str, http_status: u16, body: &serde_json::Value) {
    tracing::error!(event="billing.stripe.operation_rejected", component="commercial_billing", operation, dependency="stripe", http_status, provider_code=?body.pointer("/error/code").and_then(serde_json::Value::as_str), provider_type=?body.pointer("/error/type").and_then(serde_json::Value::as_str));
}

pub(super) funcion subscription_response(
    subscription: WorkspaceSubscription,
) -> WorkspaceSubscriptionResponse {
    WorkspaceSubscriptionResponse {
        subscription_id: subscription.subscription_id.to_string(),
        workspace_id: subscription.workspace_id.to_string(),
        plan_key: subscription.plan_key,
        plan_version: subscription.plan_version,
        status: subscription.status,
        current_period_start_unix: subscription.current_period_start_unix,
        current_period_end_unix: subscription.current_period_end_unix,
        cancel_at_period_end: subscription.cancel_at_period_end,
        row_version: subscription.row_version,
        scheduled_plan_key: subscription.scheduled_plan_key,
        scheduled_plan_version: subscription.scheduled_plan_version,
    }
}

pub(super) fn database_status(error: sqlx::Error) -> Status {
    let code = error
        .as_database_error()
        .and_then(|value| value.code())
        .map(|value| value.into_owned());
    let constraint = error
        .as_database_error()
        .and_then(|value| value.constraint())
        .map(str::to_owned);
    tracing::error!(event="billing.persistence.failed", component="commercial_billing", operation="database_command", dependency="postgresql", sqlstate=?code, constraint=?constraint);
    Status::unavailable("Billing data could not be updated")
}

fn provider_error(operation: &'static str, error: &reqwest::Error) -> Status {
    tracing::error!(
        event = "billing.stripe.request_failed",
        component = "commercial_billing",
        operation,
        dependency = "stripe",
        timeout = error.is_timeout(),
        connect = error.is_connect()
    );
    Status::unavailable("Stripe is temporarily unavailable")
}

pub(super) async fn ensure_stripe_price(
    service: &CommercialBillingService,
    api_key: &str,
    price_id: &str,
    currency: &str,
    amount: i64,
) -> Result<(), Status> {
    let response = service
        .http
        .get(format!("https://api.stripe.com/v1/prices/{price_id}"))
        .basic_auth(api_key, Some(""))
        .send()
        .await
        .map_err(|error| provider_error("price.verify", &error))?;
    let status = response.status();
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|error| provider_error("price.decode", &error))?;
    let valid = status.is_success()
        && body.get("active").and_then(serde_json::Value::as_bool) == Some(true)
        && body
            .pointer("/recurring/interval")
            .and_then(serde_json::Value::as_str)
            == Some("month")
        && body.get("currency").and_then(serde_json::Value::as_str)
            == Some(currency.to_lowercase().as_str())
        && body.get("unit_amount").and_then(serde_json::Value::as_i64) == Some(amount);
    if !valid {
        return Err(Status::failed_precondition(
            "The published price does not match the Stripe monthly price",
        ));
    }
    Ok(())
}

pub(super) trait SubscriptionCommand {
    fn session(&self) -> Option<&AccountSessionRequest>;
    fn workspace_id(&self) -> &str;
    fn idempotency_key(&self) -> &str;
}
macro_rules! subscription_command {
    ($type:ty) => {
        impl SubscriptionCommand for $type {
            fn session(&self) -> Option<&AccountSessionRequest> {
                self.session.as_ref()
            }
            fn workspace_id(&self) -> &str {
                &self.workspace_id
            }
            fn idempotency_key(&self) -> &str {
                &self.idempotency_key
            }
        }
    };
}
subscription_command!(CancelSubscriptionRequest);
subscription_command!(ReactivateSubscriptionRequest);

pub(super) async fn insert_event(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_type: &str,
    workspace_id: Uuid,
    aggregate: Uuid,
    version: i64,
    correlation_id: &str,
) -> Result<(), Status> {
    let event_id = Uuid::new_v4();
    let payload = serde_json::json!({"event_id":event_id,"event_type":event_type,"aggregate_id":aggregate,"aggregate_version":version,"workspace_id":workspace_id,"audience":"tenant","correlation_id":correlation_id});
    let writer = xovyra_platform::outbox::OutboxWriter::new(
        xovyra_commercial_billing::module_public_api::bounded_context(),
    )
    .map_err(|_| {
        tracing::error!(
            event = "billing.outbox.configuration_invalid",
            component = "commercial_billing",
            operation = "append_subscription_event",
            dependency = "outbox",
            correlation_id
        );
        Status::internal("Billing event could not be recorded")
    })?;
    writer
        .append(
            transaction,
            &event_id.to_string(),
            event_type,
            1,
            &payload,
            &tracing::Span::current().context(),
        )
        .await
        .map_err(|error| {
            match error {
                xovyra_platform::outbox::OutboxWriteError::Persistence(source) => {
                    let sqlstate = source.as_database_error().and_then(|value| value.code()).map(|value| value.into_owned());
                    let constraint = source.as_database_error().and_then(|value| value.constraint()).map(str::to_owned);
                    tracing::error!(event="billing.outbox.append_failed", component="commercial_billing", operation="append_subscription_event", dependency="postgresql", correlation_id, error_category="persistence", sqlstate=?sqlstate, constraint=?constraint);
                }
                xovyra_platform::outbox::OutboxWriteError::InvalidEvent => tracing::error!(event="billing.outbox.append_failed", component="commercial_billing", operation="append_subscription_event", dependency="outbox", correlation_id, error_category="invalid_event"),
                xovyra_platform::outbox::OutboxWriteError::PayloadSerialization(_) => tracing::error!(event="billing.outbox.append_failed", component="commercial_billing", operation="append_subscription_event", dependency="outbox", correlation_id, error_category="serialization"),
            }
            Status::unavailable("Billing event could not be recorded")
        })?;
    Ok(())
}

#[cfg(test)]
mod plan_change_tests {
    use super::schedule_update_form;

    #[test]
    fn plan_change_starts_at_next_period_without_proration() {
        let form = schedule_update_form("price_current", "price_next", 100, 200);
        let value = |key: &str| {
            form.iter()
                .find(|(candidate, _)| *candidate == key)
                .map(|(_, value)| value.as_str())
                .expect("schedule form has the requested field")
        };

        assert_eq!(value("proration_behavior"), "none");
        assert_eq!(value("phases[0][end_date]"), "200");
        assert_eq!(value("phases[1][start_date]"), "200");
        assert_eq!(value("phases[0][items][0][price]"), "price_current");
        assert_eq!(value("phases[1][items][0][price]"), "price_next");
    }
}
