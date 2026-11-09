use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
async fn published_catalog_reads_versioned_localized_price_and_limits_from_postgres() {
    let database_url = std::env::var("COMMERCIAL_BILLING_DATABASE_URL")
        .expect("COMMERCIAL_BILLING_DATABASE_URL is required for the real database test");
    let database = PgPoolOptions::new()
        .max_connections(3)
        .connect(&database_url)
        .await
        .expect("commercial billing runtime role must connect to PostgreSQL");
    let plan_id = Uuid::new_v4();
    let price_id = Uuid::new_v4();
    let plan_key = format!("integration-{}", Uuid::new_v4());
    let result = async {
        sqlx::query("INSERT INTO commercial_billing.catalog_plans (plan_id,version,plan_key,display_name,description,lifecycle,profitability_evidence,approved_by_operator,approved_at,commercial_gate_reference,tax_gate_reference) VALUES ($1,1,$2,'Test persisted plan','Integration verification','published','approved cost simulation', $3, now(),'test commercial gate','test tax gate')")
            .bind(plan_id).bind(&plan_key).bind(Uuid::new_v4()).execute(&database).await.expect("transient published plan is inserted");
        sqlx::query("INSERT INTO commercial_billing.localized_plan_prices (price_id,plan_id,plan_version,locale,currency,amount_minor,billing_interval,stripe_price_id,effective_from) VALUES ($1,$2,1,'es-CO','USD',1900,'month','price_test_catalog_read',now())")
            .bind(price_id).bind(plan_id).execute(&database).await.expect("transient localized price is inserted");
        sqlx::query("INSERT INTO commercial_billing.plan_capabilities (plan_id,plan_version,capability_key,limit_value) VALUES ($1,1,'credits.monthly',1580)")
            .bind(plan_id).execute(&database).await.expect("transient plan capability is inserted");
        xovyra_commercial_billing::catalog::list_published(&database, "es-CO", "USD").await
    }
    .await;
    let remove_capability =
        sqlx::query("DELETE FROM commercial_billing.plan_capabilities WHERE plan_id=$1")
            .bind(plan_id)
            .execute(&database)
            .await;
    let remove_price =
        sqlx::query("DELETE FROM commercial_billing.localized_plan_prices WHERE price_id=$1")
            .bind(price_id)
            .execute(&database)
            .await;
    let remove_plan = sqlx::query("DELETE FROM commercial_billing.catalog_plans WHERE plan_id=$1")
        .bind(plan_id)
        .execute(&database)
        .await;
    remove_capability.expect("transient test capability is removed");
    remove_price.expect("transient test price is removed");
    remove_plan.expect("transient test catalog version is removed");
    let plans = result.expect("published catalog query succeeds against PostgreSQL");
    let plan = plans
        .iter()
        .find(|plan| plan.plan_id == plan_id && plan.plan_key == plan_key)
        .expect("newly published version is returned by its exact locale and currency");
    assert_eq!(plan.version, 1);
    assert_eq!(plan.amount_minor, 1900);
    assert_eq!(
        plan.capabilities,
        vec![("credits.monthly".to_owned(), 1580)]
    );
    database.close().await;
}
