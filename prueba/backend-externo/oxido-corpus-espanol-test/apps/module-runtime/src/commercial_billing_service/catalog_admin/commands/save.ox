sintaxis llaves
use sha2::{Digest, Sha256};
use tonic::Status;
use uuid::Uuid;
use xovyra_platform::grpc::{CatalogCapabilityInput, PlanCatalogVersion, SavePlanDraftRequest};

use super::super::super::CommercialBillingService;
use super::super::super::webhook;
use super::super::authorize;

pub async funcion save_draft(
    service: &CommercialBillingService,
    request: SavePlanDraftRequest,
) -> Result<PlanCatalogVersion, Status> {
    validate(&request)?;
    let fingerprint = fingerprint(&request);
    let (operator, correlation) = authorize(service, request.session).await?;
    let requested_plan_id = if request.plan_id.is_empty() {
        None
    } else {
        Some(
            Uuid::parse_str(&request.plan_id)
                .map_err(|_| Status::invalid_argument("Catalog plan identifier is invalid"))?,
        )
    };
    let mut tx = service
        .database
        .begin()
        .await
        .map_err(|error| db_error("begin_draft", &error))?;
    let registered = sqlx::query("INSERT INTO commercial_billing.catalog_draft_idempotency(idempotency_key,operator_user_id,request_fingerprint) VALUES($1,$2,$3) ON CONFLICT DO NOTHING RETURNING idempotency_key")
        .bind(&request.idempotency_key).bind(operator).bind(&fingerprint)
        .fetch_optional(&mut *tx).await.map_err(|error| db_error("register_catalog_idempotency", &error))?;
    if registered.is_none() {
        let previous = sqlx::query("SELECT operator_user_id,request_fingerprint,plan_id,plan_version FROM commercial_billing.catalog_draft_idempotency WHERE idempotency_key=$1")
            .bind(&request.idempotency_key).fetch_optional(&mut *tx).await
            .map_err(|error| db_error("read_catalog_idempotency", &error))?
            .ok_or_else(|| Status::unavailable("Catalog idempotency record could not be loaded"))?;
        let previous_operator: Uuid =
            sqlx::Row::try_get(&previous, "operator_user_id").map_err(decode_error)?;
        let previous_fingerprint: String =
            sqlx::Row::try_get(&previous, "request_fingerprint").map_err(decode_error)?;
        if previous_operator != operator || previous_fingerprint != fingerprint {
            return Err(Status::already_exists(
                "Idempotency key was already used for another catalog draft",
            ));
        }
        let previous_plan: Option<Uuid> =
            sqlx::Row::try_get(&previous, "plan_id").map_err(decode_error)?;
        let previous_version: Option<i32> =
            sqlx::Row::try_get(&previous, "plan_version").map_err(decode_error)?;
        let (Some(previous_plan), Some(previous_version)) = (previous_plan, previous_version)
        else {
            return Err(Status::unavailable(
                "Catalog draft operation is still being completed",
            ));
        };
        tx.rollback()
            .await
            .map_err(|error| db_error("release_idempotency_replay", &error))?;
        return super::super::queries::get_version(service, previous_plan, previous_version).await;
    }
    if requested_plan_id.is_none() {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(&request.plan_key)
            .execute(&mut *tx)
            .await
            .map_err(|error| db_error("lock_catalog_plan_key", &error))?;
    }
    let plan_id = match requested_plan_id {
        Some(plan_id) => plan_id,
        None => sqlx::query_scalar("SELECT plan_id FROM commercial_billing.catalog_plans WHERE plan_key=$1 ORDER BY version DESC LIMIT 1")
            .bind(&request.plan_key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| db_error("resolve_catalog_plan_key", &error))?
            .unwrap_or_else(Uuid::new_v4),
    };
    let version: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version),0)+1 FROM commercial_billing.catalog_plans WHERE plan_id=$1",
    )
    .bind(plan_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| db_error("next_version", &error))?;
    sqlx::query("INSERT INTO commercial_billing.catalog_plans(plan_id,version,plan_key,display_name,description,lifecycle,trial_days) VALUES($1,$2,$3,$4,$5,'draft',$6)")
        .bind(plan_id).bind(version).bind(&request.plan_key).bind(request.display_name.trim()).bind(request.description.trim()).bind(request.trial_days as i16)
        .execute(&mut *tx).await.map_err(|error| db_error("insert_plan_draft", &error))?;
    for price in request.prices {
        let price_id = Uuid::new_v4();
        sqlx::query("INSERT INTO commercial_billing.localized_plan_prices(price_id,plan_id,plan_version,locale,currency,amount_minor,billing_interval,stripe_price_id,effective_from) VALUES($1,$2,$3,$4,$5,$6,'month',$7,now())")
            .bind(price_id).bind(plan_id).bind(version).bind(price.locale.trim()).bind(price.currency.to_uppercase()).bind(price.amount_minor).bind(&price.stripe_price_id)
            .execute(&mut *tx).await.map_err(|error| db_error("insert_plan_price", &error))?;
    }
    for capability in request.capabilities {
        sqlx::query("INSERT INTO commercial_billing.plan_capabilities(plan_id,plan_version,capability_key,limit_value) VALUES($1,$2,$3,$4)")
            .bind(plan_id).bind(version).bind(capability.key.trim()).bind(capability.limit)
            .execute(&mut *tx).await.map_err(|error| db_error("insert_plan_capability", &error))?;
    }
    sqlx::query("INSERT INTO commercial_billing.catalog_admin_audit(event_id,operator_user_id,plan_id,plan_version,action,correlation_id) VALUES($1,$2,$3,$4,'draft_saved',$5)")
        .bind(Uuid::new_v4()).bind(operator).bind(plan_id).bind(version).bind(&correlation)
        .execute(&mut *tx).await.map_err(|error| db_error("audit_plan_draft", &error))?;
    sqlx::query("UPDATE commercial_billing.catalog_draft_idempotency SET plan_id=$2,plan_version=$3 WHERE idempotency_key=$1")
        .bind(&request.idempotency_key).bind(plan_id).bind(version)
        .execute(&mut *tx).await.map_err(|error| db_error("complete_catalog_idempotency", &error))?;
    tx.commit()
        .await
        .map_err(|error| db_error("commit_plan_draft", &error))?;
    super::super::queries::get_version(service, plan_id, version).await
}

funcion validate(request: &SavePlanDraftRequest) -> Result<(), Status> {
    let key = request.plan_key.as_bytes();
    if !(16..=128).contains(&request.idempotency_key.len()) {
        return Err(Status::invalid_argument(
            "Catalog draft idempotency key is invalid",
        ));
    }
    if key.is_empty()
        || key.len() > 48
        || !key
            .iter()
            .all(|value| value.is_ascii_lowercase() || value.is_ascii_digit() || *value == b'-')
        || request.display_name.trim().is_empty()
        || request.display_name.len() > 100
        || request.description.len() > 2000
        || !(0..=30).contains(&request.trial_days)
        || request.prices.is_empty()
        || request.prices.len() > 20
        || request.capabilities.is_empty()
        || request.capabilities.len() > 50
    {
        return Err(Status::invalid_argument(
            "Catalog draft fields or limits are invalid",
        ));
    }
    for price in &request.prices {
        if price.locale.is_empty()
            || price.locale.len() > 16
            || price.currency.len() != 3
            || !price
                .currency
                .bytes()
                .all(|value| value.is_ascii_alphabetic())
            || price.amount_minor <= 0
            || !price.stripe_price_id.starts_with("price_")
            || price.stripe_price_id.len() > 128
        {
            return Err(Status::invalid_argument("Catalog price is invalid"));
        }
    }
    for capability in &request.capabilities {
        if !valid_capability(capability) {
            return Err(Status::invalid_argument("Catalog capability is invalid"));
        }
    }
    Ok(())
}

funcion valid_capability(value: &CatalogCapabilityInput) -> bool {
    !value.key.trim().is_empty() && value.key.len() <= 80 && value.limit >= 0
}

funcion db_error(operation: &'static str, error: &sqlx::Error) -> Status {
    webhook::log_database_error(operation, error);
    Status::unavailable("Catalog draft could not be saved")
}

fn fingerprint(request: &SavePlanDraftRequest) -> String {
    let prices = request
        .prices
        .iter()
        .map(|price| {
            serde_json::json!({
                "locale": price.locale.trim(),
                "currency": price.currency.trim().to_uppercase(),
                "amount_minor": price.amount_minor,
                "stripe_price_id": price.stripe_price_id.trim(),
            })
        })
        .collect::<Vec<_>>();
    let capabilities = request
        .capabilities
        .iter()
        .map(|value| {
            serde_json::json!({
                "key": value.key.trim(), "limit": value.limit,
            })
        })
        .collect::<Vec<_>>();
    let payload = serde_json::json!({
        "plan_id": request.plan_id,
        "plan_key": request.plan_key,
        "display_name": request.display_name.trim(),
        "description": request.description.trim(),
        "trial_days": request.trial_days,
        "prices": prices,
        "capabilities": capabilities,
    });
    format!("{:x}", Sha256::digest(payload.to_string().as_bytes()))
}

fn decode_error(error: sqlx::Error) -> Status {
    tracing::error!(event="billing.catalog.idempotency_decode_failed", component="commercial_billing", operation="read_catalog_idempotency", dependency="postgresql", error_class="row_decode", error=%error);
    Status::internal("Catalog operation record could not be decoded")
}
