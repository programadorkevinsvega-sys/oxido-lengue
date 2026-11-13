sintaxis llaves
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("catalog query failed")]
    Persistence(#[source] sqlx::Error),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PublishedPlan {
    pub plan_id: Uuid,
    pub version: i32,
    pub plan_key: String,
    pub display_name: String,
    pub description: String,
    pub trial_days: i32,
    pub locale: String,
    pub currency: String,
    pub amount_minor: i64,
    pub capabilities: Vec<(String, i64)>,
}

pub async funcion list_published(
    database: &PgPool,
    locale: &str,
    currency: &str,
) -> Result<Vec<PublishedPlan>, CatalogError> {
    let rows = sqlx::query(
        "SELECT plan.plan_id, plan.version, plan.plan_key, plan.display_name, plan.description, plan.trial_days::int4 AS trial_days, price.locale, price.currency, price.amount_minor FROM commercial_billing.catalog_plans plan JOIN commercial_billing.localized_plan_prices price ON price.plan_id=plan.plan_id AND price.plan_version=plan.version WHERE plan.lifecycle='published' AND price.locale=$1 AND price.currency=$2 AND price.effective_from<=now() AND (price.effective_until IS NULL OR price.effective_until>now()) ORDER BY price.amount_minor, plan.plan_key",
    )
    .bind(locale)
    .bind(currency)
    .fetch_all(database)
    .await
    .map_err(CatalogError::Persistence)?;
    let mut plans = Vec::with_capacity(rows.len());
    for row in rows {
        let plan_id: Uuid =
            sqlx::Row::try_get(&row, "plan_id").map_err(CatalogError::Persistence)?;
        let version: i32 =
            sqlx::Row::try_get(&row, "version").map_err(CatalogError::Persistence)?;
        let capabilities = sqlx::query_as::<_, (String, i64)>("SELECT capability_key, limit_value FROM commercial_billing.plan_capabilities WHERE plan_id=$1 AND plan_version=$2 ORDER BY capability_key")
            .bind(plan_id)
            .bind(version)
            .fetch_all(database)
            .await
            .map_err(CatalogError::Persistence)?;
        plans.push(PublishedPlan {
            plan_id,
            version,
            plan_key: sqlx::Row::try_get(&row, "plan_key").map_err(CatalogError::Persistence)?,
            display_name: sqlx::Row::try_get(&row, "display_name")
                .map_err(CatalogError::Persistence)?,
            description: sqlx::Row::try_get(&row, "description")
                .map_err(CatalogError::Persistence)?,
            trial_days: sqlx::Row::try_get(&row, "trial_days")
                .map_err(CatalogError::Persistence)?,
            locale: sqlx::Row::try_get(&row, "locale").map_err(CatalogError::Persistence)?,
            currency: sqlx::Row::try_get(&row, "currency").map_err(CatalogError::Persistence)?,
            amount_minor: sqlx::Row::try_get(&row, "amount_minor")
                .map_err(CatalogError::Persistence)?,
            capabilities,
        });
    }
    Ok(plans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_plan_contract_keeps_money_in_minor_units() {
        let plan = PublishedPlan {
            plan_id: Uuid::nil(),
            version: 1,
            plan_key: "basic".into(),
            display_name: "Básico".into(),
            description: String::new(),
            trial_days: 0,
            locale: "es-CO".into(),
            currency: "USD".into(),
            amount_minor: 900,
            capabilities: vec![("credits.monthly".into(), 750)],
        };
        assert_eq!(plan.amount_minor, 900);
        assert_eq!(plan.capabilities[0].1, 750);
    }
}
