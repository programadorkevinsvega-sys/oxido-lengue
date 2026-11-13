use crate::commercial_billing_service::webhook;
use tonic::Status;
use uuid::Uuid;
use xovyra_platform::grpc::ProfitabilityScenarioInput;
pub(super) async fn persist(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    plan_id: Uuid,
    version: i32,
    scenarios: &[ProfitabilityScenarioInput],
) -> Result<(), Status> {
    let credits: i64 = sqlx::query_scalar(
            "SELECT limit_value FROM commercial_billing.plan_capabilities WHERE plan_id=$1 AND plan_version=$2 AND capability_key='credits.monthly'",
        )
        .bind(plan_id)
        .bind(version)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|error| db_error("read_credit_limit", &error))?
        .ok_or_else(|| Status::failed_precondition(
            "Plans must define monthly credits",
        ))?;
    let prices = sqlx::query(
            "SELECT locale,currency,amount_minor FROM commercial_billing.localized_plan_prices WHERE plan_id=$1 AND plan_version=$2",
        )
        .bind(plan_id)
        .bind(version)
        .fetch_all(&mut **transaction)
        .await
        .map_err(|error| db_error("read_localized_prices", &error))?;
    if prices.is_empty() || scenarios.len() != prices.len() {
        return Err(Status::failed_precondition(
            "Provide one profitability simulation for every localized price",
        ));
    }
    for price in prices {
        let locale: String = sqlx::Row::try_get(&price, "locale").map_err(decode_error)?;
        let currency = sqlx::Row::try_get::<String, _>(&price, "currency")
            .map_err(decode_error)?
            .trim()
            .to_owned();
        let amount: i64 = sqlx::Row::try_get(&price, "amount_minor").map_err(decode_error)?;
        let scenario = scenarios
            .iter()
            .find(|value| value.locale == locale && value.currency.eq_ignore_ascii_case(&currency))
            .ok_or_else(|| {
                Status::failed_precondition(
                    "A localized price is missing its profitability simulation",
                )
            })?;
        let (expected_margin, stress_margin) = evaluate(amount, credits, scenario)?;
        sqlx::query(
                "INSERT INTO commercial_billing.catalog_profitability_scenarios(plan_id,plan_version,locale,currency,provider_expected_minor_per_credit,provider_stress_minor_per_credit,operating_minor_per_credit,payment_fee_basis_points,payment_fixed_minor,target_margin_basis_points,expected_margin_basis_points,stress_margin_basis_points) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
            )
            .bind(plan_id)
            .bind(version)
            .bind(locale)
            .bind(currency)
            .bind(scenario.provider_expected_minor_per_credit)
            .bind(scenario.provider_stress_minor_per_credit)
            .bind(scenario.operating_minor_per_credit)
            .bind(scenario.payment_fee_basis_points)
            .bind(scenario.payment_fixed_minor)
            .bind(scenario.target_margin_basis_points)
            .bind(expected_margin)
            .bind(stress_margin)
            .execute(&mut **transaction)
            .await
            .map_err(|error| db_error("persist_profitability_simulation", &error))?;
    }
    Ok(())
}
fn evaluate(
    monthly_price: i64,
    monthly_credits: i64,
    scenario: &ProfitabilityScenarioInput,
) -> Result<(i32, i32), Status> {
    if monthly_price <= 0
        || monthly_credits <= 0
        || scenario.provider_expected_minor_per_credit < 0
        || scenario.provider_stress_minor_per_credit < scenario.provider_expected_minor_per_credit
        || scenario.operating_minor_per_credit < 0
        || !(0..=5000).contains(&scenario.payment_fee_basis_points)
        || scenario.payment_fixed_minor < 0
        || !(1..=9000).contains(&scenario.target_margin_basis_points)
    {
        return Err(Status::invalid_argument(
            "Profitability simulation inputs are outside allowed limits",
        ));
    }
    let revenue = i128::from(monthly_price);
    let fee = (revenue * i128::from(scenario.payment_fee_basis_points) + 9_999) / 10_000
        + i128::from(scenario.payment_fixed_minor);
    let credits = i128::from(monthly_credits);
    let margin = |provider: i64| {
        let cost =
            credits * (i128::from(provider) + i128::from(scenario.operating_minor_per_credit));
        ((revenue - fee - cost) * 10_000 / revenue) as i32
    };
    let expected = margin(scenario.provider_expected_minor_per_credit);
    let stress = margin(scenario.provider_stress_minor_per_credit);
    if expected < scenario.target_margin_basis_points
        || stress < scenario.target_margin_basis_points
    {
        return Err(Status::failed_precondition(
            "Expected or high-usage profitability is below the approved margin target",
        ));
    }
    Ok((expected, stress))
}
fn decode_error(error: sqlx::Error) -> Status {
    tracing::error!(
        event = "billing.catalog.row_decode_failed", component = "commercial_billing",
        operation = "simulate_catalog_profitability", dependency = "postgresql",
        error_class = "row_decode", error =% error
    );
    Status::internal("Profitability simulation data could not be decoded")
}
fn db_error(operation: &'static str, error: &sqlx::Error) -> Status {
    webhook::log_database_error(operation, error);
    Status::unavailable("Profitability simulation could not be evaluated")
}
#[cfg(test)]
mod tests {
    use super::evaluate;
    use xovyra_platform::grpc::ProfitabilityScenarioInput;
    #[test]
    fn profitability_requires_expected_and_stress_scenarios_to_meet_margin() {
        let scenario = ProfitabilityScenarioInput {
            locale: "es-CO".into(),
            currency: "USD".into(),
            provider_expected_minor_per_credit: 1,
            provider_stress_minor_per_credit: 2,
            operating_minor_per_credit: 1,
            payment_fee_basis_points: 300,
            payment_fixed_minor: 30,
            target_margin_basis_points: 3000,
        };
        assert_eq!(
            evaluate(1900, 100, &scenario).expect("profitable test tariff"),
            (8489, 7963)
        );
        let loss = ProfitabilityScenarioInput {
            provider_stress_minor_per_credit: 100,
            ..scenario
        };
        assert!(evaluate(1900, 100, &loss).is_err());
    }
}
