use super::super::CommercialBillingService;
use super::super::catalog_admin;
use super::super::webhook;
use tonic::Status;
use uuid::Uuid;
use xovyra_platform::grpc::{
    CatalogPrice, ListPlatformCatalogRequest, ListPlatformCatalogResponse, PlanCapability,
    PlanCatalogVersion, ProfitabilityScenario,
};
pub async fn list(
    service: &CommercialBillingService,
    request: ListPlatformCatalogRequest,
) -> Result<ListPlatformCatalogResponse, Status> {
    catalog_admin::authorize(service, request.session).await?;
    let rows = sqlx::query(
            "SELECT plan_id,version,plan_key,display_name,description,lifecycle,trial_days::int4 AS trial_days,COALESCE(profitability_evidence,'') AS profitability_evidence,COALESCE(commercial_gate_reference,'') AS commercial_gate_reference,COALESCE(tax_gate_reference,'') AS tax_gate_reference,row_version,updated_at::text AS updated_at FROM commercial_billing.catalog_plans ORDER BY created_at DESC,plan_id,version DESC LIMIT 100",
        )
        .fetch_all(&service.database)
        .await
        .map_err(|error| {
            webhook::log_database_error("billing.catalog.admin.list", &error);
            Status::unavailable("Platform catalog could not be loaded")
        })?;
    let mut versions = Vec::with_capacity(rows.len());
    for row in rows {
        versions.push(convert_version(service, row).await?);
    }
    Ok(ListPlatformCatalogResponse {
        versions,
        next_cursor: String::new(),
    })
}
pub(super) async fn get_version(
    service: &CommercialBillingService,
    plan_id: Uuid,
    version: i32,
) -> Result<PlanCatalogVersion, Status> {
    let row = sqlx::query(
            "SELECT plan_id,version,plan_key,display_name,description,lifecycle,trial_days::int4 AS trial_days,COALESCE(profitability_evidence,'') AS profitability_evidence,COALESCE(commercial_gate_reference,'') AS commercial_gate_reference,COALESCE(tax_gate_reference,'') AS tax_gate_reference,row_version,updated_at::text AS updated_at FROM commercial_billing.catalog_plans WHERE plan_id=$1 AND version=$2",
        )
        .bind(plan_id)
        .bind(version)
        .fetch_optional(&service.database)
        .await
        .map_err(|error| {
            webhook::log_database_error("billing.catalog.admin.get", &error);
            Status::unavailable("Catalog version could not be loaded")
        })?
        .ok_or_else(|| Status::not_found("Catalog version was not found"))?;
    convert_version(service, row).await
}
async fn convert_version(
    service: &CommercialBillingService,
    row: sqlx::postgres::PgRow,
) -> Result<PlanCatalogVersion, Status> {
    let plan_id: Uuid = sqlx::Row::try_get(&row, "plan_id").map_err(decode_error)?;
    let version: i32 = sqlx::Row::try_get(&row, "version").map_err(decode_error)?;
    let price_rows = sqlx::query(
            "SELECT price_id,locale,currency,amount_minor,stripe_price_id FROM commercial_billing.localized_plan_prices WHERE plan_id=$1 AND plan_version=$2 ORDER BY locale,currency",
        )
        .bind(plan_id)
        .bind(version)
        .fetch_all(&service.database)
        .await
        .map_err(|error| {
            webhook::log_database_error("billing.catalog.admin.prices", &error);
            Status::unavailable("Platform catalog prices could not be loaded")
        })?;
    let prices = price_rows
        .into_iter()
        .map(|price| {
            Ok(CatalogPrice {
                price_id: sqlx::Row::try_get::<Uuid, _>(&price, "price_id")
                    .map_err(decode_error)?
                    .to_string(),
                locale: sqlx::Row::try_get(&price, "locale").map_err(decode_error)?,
                currency: sqlx::Row::try_get::<String, _>(&price, "currency")
                    .map_err(decode_error)?
                    .trim()
                    .to_owned(),
                amount_minor: sqlx::Row::try_get(&price, "amount_minor").map_err(decode_error)?,
                stripe_price_id: sqlx::Row::try_get(&price, "stripe_price_id")
                    .map_err(decode_error)?,
            })
        })
        .collect::<Result<Vec<_>, Status>>()?;
    let capability_rows = sqlx::query(
            "SELECT capability_key,limit_value FROM commercial_billing.plan_capabilities WHERE plan_id=$1 AND plan_version=$2 ORDER BY capability_key",
        )
        .bind(plan_id)
        .bind(version)
        .fetch_all(&service.database)
        .await
        .map_err(|error| {
            webhook::log_database_error("billing.catalog.admin.capabilities", &error);
            Status::unavailable("Platform catalog limits could not be loaded")
        })?;
    let capabilities = capability_rows
        .into_iter()
        .map(|capability| {
            Ok(PlanCapability {
                key: sqlx::Row::try_get(&capability, "capability_key").map_err(decode_error)?,
                limit: sqlx::Row::try_get(&capability, "limit_value").map_err(decode_error)?,
            })
        })
        .collect::<Result<Vec<_>, Status>>()?;
    let simulation_rows = sqlx::query(
            "SELECT locale,currency,provider_expected_minor_per_credit,provider_stress_minor_per_credit,operating_minor_per_credit,payment_fee_basis_points,payment_fixed_minor,target_margin_basis_points,expected_margin_basis_points,stress_margin_basis_points FROM commercial_billing.catalog_profitability_scenarios WHERE plan_id=$1 AND plan_version=$2 ORDER BY locale,currency",
        )
        .bind(plan_id)
        .bind(version)
        .fetch_all(&service.database)
        .await
        .map_err(|error| {
            webhook::log_database_error("billing.catalog.admin.simulations", &error);
            Status::unavailable("Catalog profitability simulations could not be loaded")
        })?;
    let profitability_scenarios = simulation_rows
        .into_iter()
        .map(|scenario| {
            Ok(ProfitabilityScenario {
                locale: sqlx::Row::try_get(&scenario, "locale").map_err(decode_error)?,
                currency: sqlx::Row::try_get::<String, _>(&scenario, "currency")
                    .map_err(decode_error)?
                    .trim()
                    .to_owned(),
                provider_expected_minor_per_credit: sqlx::Row::try_get(
                    &scenario,
                    "provider_expected_minor_per_credit",
                )
                .map_err(decode_error)?,
                provider_stress_minor_per_credit: sqlx::Row::try_get(
                    &scenario,
                    "provider_stress_minor_per_credit",
                )
                .map_err(decode_error)?,
                operating_minor_per_credit: sqlx::Row::try_get(
                    &scenario,
                    "operating_minor_per_credit",
                )
                .map_err(decode_error)?,
                payment_fee_basis_points: sqlx::Row::try_get(&scenario, "payment_fee_basis_points")
                    .map_err(decode_error)?,
                payment_fixed_minor: sqlx::Row::try_get(&scenario, "payment_fixed_minor")
                    .map_err(decode_error)?,
                target_margin_basis_points: sqlx::Row::try_get(
                    &scenario,
                    "target_margin_basis_points",
                )
                .map_err(decode_error)?,
                expected_margin_basis_points: sqlx::Row::try_get(
                    &scenario,
                    "expected_margin_basis_points",
                )
                .map_err(decode_error)?,
                stress_margin_basis_points: sqlx::Row::try_get(
                    &scenario,
                    "stress_margin_basis_points",
                )
                .map_err(decode_error)?,
            })
        })
        .collect::<Result<Vec<_>, Status>>()?;
    Ok(PlanCatalogVersion {
        plan_id: plan_id.to_string(),
        version,
        plan_key: sqlx::Row::try_get(&row, "plan_key").map_err(decode_error)?,
        display_name: sqlx::Row::try_get(&row, "display_name").map_err(decode_error)?,
        description: sqlx::Row::try_get(&row, "description").map_err(decode_error)?,
        lifecycle: sqlx::Row::try_get(&row, "lifecycle").map_err(decode_error)?,
        trial_days: sqlx::Row::try_get(&row, "trial_days").map_err(decode_error)?,
        profitability_evidence: sqlx::Row::try_get(&row, "profitability_evidence")
            .map_err(decode_error)?,
        commercial_gate_reference: sqlx::Row::try_get(&row, "commercial_gate_reference")
            .map_err(decode_error)?,
        tax_gate_reference: sqlx::Row::try_get(&row, "tax_gate_reference").map_err(decode_error)?,
        prices,
        capabilities,
        row_version: sqlx::Row::try_get(&row, "row_version").map_err(decode_error)?,
        updated_at: sqlx::Row::try_get(&row, "updated_at").map_err(decode_error)?,
        profitability_scenarios,
    })
}
fn decode_error(error: sqlx::Error) -> Status {
    tracing::error!(
        event = "billing.catalog.decode_failed", component = "commercial_billing",
        operation = "decode_catalog_version", error_class = "row_decode", error =% error
    );
    Status::internal("Catalog data could not be decoded")
}
