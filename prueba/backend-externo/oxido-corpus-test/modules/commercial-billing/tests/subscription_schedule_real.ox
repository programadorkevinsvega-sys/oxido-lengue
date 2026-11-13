sintaxis llaves
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
async fn workspace_subscription_retains_a_scheduled_plan_and_period_in_postgres() {
    let database_url = std::env::var("COMMERCIAL_BILLING_DATABASE_URL")
        .expect("COMMERCIAL_BILLING_DATABASE_URL is required for this real database test");
    let database = PgPoolOptions::new()
        .max_connections(3)
        .connect(&database_url)
        .await
        .expect("commercial billing runtime role connects to PostgreSQL");
    let current_plan_id = Uuid::new_v4();
    let scheduled_plan_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let current_price_id = Uuid::new_v4();
    let scheduled_price_id = Uuid::new_v4();
    let plan_key = format!("subscription-integration-{}", Uuid::new_v4());
    let scheduled_key = format!("scheduled-test-{}", Uuid::new_v4());

    let result = async {
        for (plan_id, version, key, name) in [
            (current_plan_id, 1_i32, plan_key.as_str(), "Current test plan"),
            (scheduled_plan_id, 1_i32, scheduled_key.as_str(), "Scheduled test plan"),
        ] {
            sqlx::query("INSERT INTO commercial_billing.catalog_plans (plan_id,version,plan_key,display_name,description,lifecycle,profitability_evidence,approved_by_operator,approved_at,commercial_gate_reference,tax_gate_reference) VALUES ($1,$2,$3,$4,'real database integration','published','verified cost scenarios',$5,now(),'commercial ref','tax ref')")
                .bind(plan_id).bind(version).bind(key).bind(name).bind(Uuid::new_v4())
                .execute(&database).await.expect("test plan version inserts");
        }
        for (price_id, plan_id, stripe_price_id, amount) in [
            (current_price_id, current_plan_id, "price_test_current", 900_i64),
            (scheduled_price_id, scheduled_plan_id, "price_test_scheduled", 1900_i64),
        ] {
            sqlx::query("INSERT INTO commercial_billing.localized_plan_prices (price_id,plan_id,plan_version,locale,currency,amount_minor,billing_interval,stripe_price_id,effective_from) VALUES ($1,$2,1,'es-CO','USD',$3,'month',$4,now())")
                .bind(price_id).bind(plan_id).bind(amount).bind(format!("{stripe_price_id}_{}", Uuid::new_v4()))
                .execute(&database).await.expect("test localized price inserts");
        }
        sqlx::query("INSERT INTO commercial_billing.workspace_subscriptions (subscription_id,workspace_id,plan_id,plan_version,price_id,status,current_period_start,current_period_end,stripe_subscription_id,scheduled_plan_id,scheduled_plan_version,stripe_subscription_schedule_id) VALUES ($1,$2,$3,1,$4,'active',now(),now()+interval '30 days',$5,$6,1,$7)")
            .bind(Uuid::new_v4()).bind(workspace_id).bind(current_plan_id).bind(current_price_id)
            .bind(format!("sub_test_{}", Uuid::new_v4())).bind(scheduled_plan_id)
            .bind(format!("sub_sched_test_{}", Uuid::new_v4()))
            .execute(&database).await.expect("workspace subscription with a future plan inserts");

        let subscription = xovyra_commercial_billing::subscriptions::get_for_workspace(&database, workspace_id)
            .await.expect("subscription query returns current and scheduled plans");
        assert_eq!(subscription.plan_key, plan_key);
        assert_eq!(subscription.scheduled_plan_key, scheduled_key);
        assert_eq!(subscription.scheduled_plan_version, 1);
        assert!(subscription.current_period_end_unix > subscription.current_period_start_unix);
        Ok::<(), sqlx::Error>(())
    }
    .await;

    sqlx::query("DELETE FROM commercial_billing.workspace_subscriptions WHERE workspace_id=$1")
        .bind(workspace_id)
        .execute(&database)
        .await
        .expect("test subscription is removed");
    sqlx::query("DELETE FROM commercial_billing.localized_plan_prices WHERE price_id=ANY($1)")
        .bind(vec![current_price_id, scheduled_price_id])
        .execute(&database)
        .await
        .expect("test prices are removed");
    sqlx::query("DELETE FROM commercial_billing.catalog_plans WHERE plan_id=ANY($1)")
        .bind(vec![current_plan_id, scheduled_plan_id])
        .execute(&database)
        .await
        .expect("test plans are removed");
    result.expect("subscription storage assertions pass against PostgreSQL");
    database.close().await;
}
