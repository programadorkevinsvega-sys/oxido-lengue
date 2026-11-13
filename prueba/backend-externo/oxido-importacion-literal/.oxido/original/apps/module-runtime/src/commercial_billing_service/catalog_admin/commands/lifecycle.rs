use tonic::Status;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;
use xovyra_platform::grpc::{ApprovePlanRequest, PlanCatalogVersion, PublishPlanRequest};

use super::super::super::CommercialBillingService;
use super::super::super::webhook;
use super::super::authorize;

pub async fn approve(
    service: &CommercialBillingService,
    request: ApprovePlanRequest,
) -> Result<PlanCatalogVersion, Status> {
    let (operator, correlation) = authorize(service, request.session).await?;
    if request.profitability_evidence.trim().is_empty()
        || request.commercial_gate_reference.trim().is_empty()
        || request.tax_gate_reference.trim().is_empty()
    {
        return Err(Status::invalid_argument(
            "All catalog approval gates are required",
        ));
    }
    let plan_id = parse_id(&request.plan_id)?;
    let mut tx = service
        .database
        .begin()
        .await
        .map_err(|error| database_error("begin_approval", &error))?;
    super::simulation::persist(&mut tx, plan_id, request.version, &request.scenarios).await?;
    let version = sqlx::query("UPDATE commercial_billing.catalog_plans SET lifecycle='approved',profitability_evidence=$4,commercial_gate_reference=$5,tax_gate_reference=$6,approved_by_operator=$3,approved_at=now(),row_version=row_version+1,updated_at=now() WHERE plan_id=$1 AND version=$2 AND lifecycle='draft' AND row_version=$7 AND EXISTS(SELECT 1 FROM commercial_billing.localized_plan_prices WHERE plan_id=$1 AND plan_version=$2) AND EXISTS(SELECT 1 FROM commercial_billing.plan_capabilities WHERE plan_id=$1 AND plan_version=$2) RETURNING row_version")
        .bind(plan_id).bind(request.version).bind(operator).bind(request.profitability_evidence.trim()).bind(request.commercial_gate_reference.trim()).bind(request.tax_gate_reference.trim()).bind(request.expected_row_version)
        .fetch_optional(&mut *tx).await.map_err(|error| database_error("approve_catalog_plan", &error))?;
    if version.is_none() {
        return Err(Status::failed_precondition(
            "Catalog version is incomplete, changed, or no longer a draft",
        ));
    }
    record_action(
        &mut tx,
        operator,
        plan_id,
        request.version,
        "approved",
        &correlation,
    )
    .await?;
    tx.commit()
        .await
        .map_err(|error| database_error("commit_catalog_approval", &error))?;
    super::super::queries::get_version(service, plan_id, request.version).await
}

pub async fn publish(
    service: &CommercialBillingService,
    request: PublishPlanRequest,
) -> Result<PlanCatalogVersion, Status> {
    let (operator, correlation) = authorize(service, request.session).await?;
    let plan_id = parse_id(&request.plan_id)?;
    let stripe_key = service
        .stripe_secret
        .as_deref()
        .filter(|key| key.starts_with("sk_test_"))
        .ok_or_else(|| {
            Status::failed_precondition("Stripe test credentials are required to publish prices")
        })?;
    let prices = sqlx::query("SELECT stripe_price_id,currency,amount_minor FROM commercial_billing.localized_plan_prices WHERE plan_id=$1 AND plan_version=$2 AND effective_from<=now() AND (effective_until IS NULL OR effective_until>now())")
        .bind(plan_id).bind(request.version).fetch_all(&service.database).await
        .map_err(|error| database_error("validate_published_prices", &error))?;
    if prices.is_empty() {
        return Err(Status::failed_precondition(
            "A plan needs at least one active localized price",
        ));
    }
    for price in prices {
        let stripe_price_id: String = sqlx::Row::try_get(&price, "stripe_price_id").map_err(|error| {
            tracing::error!(event="billing.catalog.row_decode_failed", component="commercial_billing", operation="validate_published_prices", dependency="postgresql", error_class="row_decode", correlation_id=%correlation, error=%error);
            Status::internal("Catalog price data could not be read")
        })?;
        let currency: String = sqlx::Row::try_get(&price, "currency").map_err(|error| {
            tracing::error!(event="billing.catalog.row_decode_failed", component="commercial_billing", operation="validate_published_prices", dependency="postgresql", error_class="row_decode", correlation_id=%correlation, error=%error);
            Status::internal("Catalog price data could not be read")
        })?;
        let amount: i64 = sqlx::Row::try_get(&price, "amount_minor").map_err(|error| {
            tracing::error!(event="billing.catalog.row_decode_failed", component="commercial_billing", operation="validate_published_prices", dependency="postgresql", error_class="row_decode", correlation_id=%correlation, error=%error);
            Status::internal("Catalog price data could not be read")
        })?;
        super::super::super::stripe::ensure_stripe_price(
            service,
            stripe_key,
            &stripe_price_id,
            &currency,
            amount,
        )
        .await?;
    }
    let mut tx = service
        .database
        .begin()
        .await
        .map_err(|error| database_error("begin_publication", &error))?;
    let plan_key: String = sqlx::query_scalar(
        "SELECT plan_key FROM commercial_billing.catalog_plans WHERE plan_id=$1 AND version=$2",
    )
    .bind(plan_id)
    .bind(request.version)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| database_error("read_publication_plan_key", &error))?
    .ok_or_else(|| Status::not_found("Catalog version was not found"))?;
    sqlx::query("UPDATE commercial_billing.catalog_plans SET lifecycle='retired',row_version=row_version+1,updated_at=now() WHERE plan_key=$1 AND lifecycle='published' AND (plan_id,version)<>($2,$3)")
        .bind(&plan_key).bind(plan_id).bind(request.version).execute(&mut *tx).await
        .map_err(|error| database_error("retire_previous_catalog_version", &error))?;
    let row = sqlx::query("UPDATE commercial_billing.catalog_plans SET lifecycle='published',row_version=row_version+1,updated_at=now() WHERE plan_id=$1 AND version=$2 AND lifecycle='approved' AND row_version=$3 AND profitability_evidence IS NOT NULL AND commercial_gate_reference IS NOT NULL AND tax_gate_reference IS NOT NULL AND EXISTS(SELECT 1 FROM commercial_billing.localized_plan_prices WHERE plan_id=$1 AND plan_version=$2 AND effective_from<=now() AND (effective_until IS NULL OR effective_until>now())) RETURNING row_version")
        .bind(plan_id).bind(request.version).bind(request.expected_row_version).fetch_optional(&mut *tx).await
        .map_err(|error| database_error("publish_catalog_plan", &error))?;
    let Some(row) = row else {
        return Err(Status::failed_precondition(
            "Catalog version is not approved, complete, or current",
        ));
    };
    let row_version: i64 = sqlx::Row::try_get(&row, "row_version").map_err(|error| {
        tracing::error!(event="billing.catalog.row_decode_failed", component="commercial_billing", operation="publish_catalog_plan", dependency="postgresql", error_class="row_decode", error=%error);
        Status::internal("Catalog publication result could not be decoded")
    })?;
    let event_id = Uuid::new_v4();
    let payload = serde_json::json!({"event_id":event_id,"event_type":"commercial_billing.catalog.published","aggregate_id":plan_id,"aggregate_version":row_version,"plan_version":request.version,"audience":"platform","platform_scope_id":"00000000-0000-0000-0000-000000000001","correlation_id":correlation});
    let writer = xovyra_platform::outbox::OutboxWriter::new(
        xovyra_commercial_billing::module_public_api::bounded_context(),
    )
    .map_err(|_| Status::internal("Catalog publication event could not be recorded"))?;
    writer
        .append(
            &mut tx,
            &event_id.to_string(),
            "commercial_billing.catalog.published",
            1,
            &payload,
            &tracing::Span::current().context(),
        )
        .await
        .map_err(|error| {
            match error {
                xovyra_platform::outbox::OutboxWriteError::Persistence(source) => {
                    database_error("write_catalog_outbox", &source)
                }
                xovyra_platform::outbox::OutboxWriteError::InvalidEvent => {
                    tracing::error!(event="billing.catalog.outbox_invalid", component="commercial_billing", operation="publish_catalog_plan", dependency="outbox", correlation_id=%correlation);
                    Status::internal("Catalog publication event could not be recorded")
                }
                xovyra_platform::outbox::OutboxWriteError::PayloadSerialization(_) => {
                    tracing::error!(event="billing.catalog.outbox_serialization_failed", component="commercial_billing", operation="publish_catalog_plan", dependency="outbox", correlation_id=%correlation);
                    Status::internal("Catalog publication event could not be recorded")
                }
            }
        })?;
    record_action(
        &mut tx,
        operator,
        plan_id,
        request.version,
        "published",
        &correlation,
    )
    .await?;
    tx.commit()
        .await
        .map_err(|error| database_error("commit_catalog_publication", &error))?;
    super::super::queries::get_version(service, plan_id, request.version).await
}

async fn record_action(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    operator: Uuid,
    plan_id: Uuid,
    version: i32,
    action: &str,
    correlation: &str,
) -> Result<(), Status> {
    sqlx::query("INSERT INTO commercial_billing.catalog_admin_audit(event_id,operator_user_id,plan_id,plan_version,action,correlation_id) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(Uuid::new_v4()).bind(operator).bind(plan_id).bind(version).bind(action).bind(correlation)
        .execute(&mut **tx).await.map_err(|error| database_error("write_catalog_audit", &error))?;
    Ok(())
}

fn parse_id(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value)
        .map_err(|_| Status::invalid_argument("Catalog plan identifier is invalid"))
}

fn database_error(operation: &'static str, error: &sqlx::Error) -> Status {
    webhook::log_database_error(operation, error);
    Status::unavailable("Catalog lifecycle operation could not be completed")
}
