use super::CommercialBillingService;
use super::stripe::{database_status, insert_event};
use hmac::{Hmac, Mac};
use sqlx::Row;
use tonic::Status;
use uuid::Uuid;
use xovyra_platform::grpc::{ApplyStripeWebhookRequest, ApplyStripeWebhookResponse};
type HmacSha256 = Hmac<sha2::Sha256>;
pub(super) async fn apply(
    service: &CommercialBillingService,
    input: ApplyStripeWebhookRequest,
) -> Result<ApplyStripeWebhookResponse, Status> {
    let secret = service
        .stripe_webhook_secret
        .as_deref()
        .filter(|secret| secret.starts_with("whsec_"))
        .ok_or_else(|| {
            Status::failed_precondition("Stripe webhook verification is not configured")
        })?;
    verify_signature(&input.signature, &input.payload, secret)?;
    let event: serde_json::Value = serde_json::from_slice(&input.payload)
        .map_err(|_| Status::invalid_argument("Stripe event payload is invalid"))?;
    let event_id = event
        .get("id")
        .and_then(serde_json::Value::as_str)
        .filter(|value| value.starts_with("evt_"))
        .ok_or_else(|| Status::invalid_argument("Stripe event identifier is invalid"))?;
    let event_type = event
        .get("type")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Status::invalid_argument("Stripe event type is invalid"))?;
    let object = event
        .pointer("/data/object")
        .ok_or_else(|| Status::invalid_argument("Stripe event object is missing"))?;
    let event_created = event
        .get("created")
        .and_then(serde_json::Value::as_i64)
        .filter(|created| *created > 0)
        .ok_or_else(|| Status::invalid_argument("Stripe event timestamp is invalid"))?;
    let mut transaction = service.database.begin().await.map_err(database_status)?;
    let inserted = sqlx::query(
            "INSERT INTO commercial_billing.stripe_webhook_inbox (event_id,event_type,outcome) VALUES ($1,$2,'received') ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(event_id)
        .bind(event_type)
        .execute(&mut *transaction)
        .await
        .map_err(database_status)?;
    if inserted.rows_affected() == 0 {
        transaction.rollback().await.map_err(database_status)?;
        return Ok(ApplyStripeWebhookResponse {
            accepted: true,
            duplicate: true,
        });
    }
    apply_event(
        &mut transaction,
        event_type,
        object,
        event_created,
        &input.correlation_id,
    )
    .await?;
    sqlx::query(
            "UPDATE commercial_billing.stripe_webhook_inbox SET outcome='processed',processed_at=now() WHERE event_id=$1",
        )
        .bind(event_id)
        .execute(&mut *transaction)
        .await
        .map_err(database_status)?;
    transaction.commit().await.map_err(database_status)?;
    Ok(ApplyStripeWebhookResponse {
        accepted: true,
        duplicate: false,
    })
}
fn verify_signature(header: &str, payload: &[u8], secret: &str) -> Result<(), Status> {
    let mut timestamp = None;
    let mut signatures = Vec::new();
    for field in header.split(',') {
        let (key, value) = field.split_once('=').unwrap_or(("", ""));
        match key {
            "t" => timestamp = value.parse::<i64>().ok(),
            "v1" => signatures.push(value),
            _ => {}
        }
    }
    let timestamp =
        timestamp.ok_or_else(|| Status::unauthenticated("Stripe signature is invalid"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Status::internal("System clock is invalid"))?
        .as_secs() as i64;
    if (now - timestamp).abs() > 300 || signatures.is_empty() {
        return Err(Status::unauthenticated(
            "Stripe signature has expired or is invalid",
        ));
    }
    let valid = signatures.iter().any(|signature| {
        decode_hex(signature).is_some_and(|provided| {
            HmacSha256::new_from_slice(secret.as_bytes()).is_ok_and(|mut verifier| {
                verifier.update(timestamp.to_string().as_bytes());
                verifier.update(b".");
                verifier.update(payload);
                verifier.verify_slice(&provided).is_ok()
            })
        })
    });
    valid
        .then_some(())
        .ok_or_else(|| Status::unauthenticated("Stripe signature is invalid"))
}
fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if value.len() != 64 {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}
async fn apply_event(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_type: &str,
    object: &serde_json::Value,
    event_created: i64,
    correlation_id: &str,
) -> Result<(), Status> {
    let stripe_subscription_id = match event_type {
        "checkout.session.completed" => object
            .get("subscription")
            .and_then(serde_json::Value::as_str),
        "customer.subscription.created"
        | "customer.subscription.updated"
        | "customer.subscription.deleted"
        | "invoice.paid"
        | "invoice.payment_failed" => object
            .get("id")
            .and_then(serde_json::Value::as_str)
            .filter(|_| event_type.starts_with("customer.subscription."))
            .or_else(|| {
                object
                    .get("subscription")
                    .and_then(serde_json::Value::as_str)
            }),
        _ => None,
    };
    let Some(stripe_subscription_id) = stripe_subscription_id else {
        return Ok(());
    };
    let status = match event_type {
        "checkout.session.completed" => "incomplete",
        "customer.subscription.deleted" => "canceled",
        "invoice.paid" => "active",
        "invoice.payment_failed" => "past_due",
        _ => object
            .get("status")
            .and_then(serde_json::Value::as_str)
            .filter(|value| {
                matches!(
                    *value,
                    "trialing"
                        | "active"
                        | "past_due"
                        | "canceled"
                        | "incomplete"
                        | "incomplete_expired"
                )
            })
            .unwrap_or("incomplete"),
    };
    let stripe_checkout_id = object
        .get("id")
        .and_then(serde_json::Value::as_str)
        .filter(|value| value.starts_with("cs_"));
    let workspace_id = object
        .pointer("/metadata/workspace_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    let stripe_price_id = object
        .pointer("/items/data/0/price/id")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            object
                .pointer("/items/data/0/price")
                .and_then(serde_json::Value::as_str)
        });
    let row = sqlx::query(
            "WITH matched_price AS (SELECT plan_id,plan_version,price_id FROM commercial_billing.localized_plan_prices WHERE stripe_price_id=$10 AND effective_from<=now() AND (effective_until IS NULL OR effective_until>now()) ORDER BY effective_from DESC LIMIT 1) UPDATE commercial_billing.workspace_subscriptions subscription SET stripe_subscription_id=$1,status=$2,current_period_start=CASE WHEN $3>0 THEN to_timestamp($3) ELSE current_period_start END,current_period_end=CASE WHEN $4>0 THEN to_timestamp($4) ELSE current_period_end END,cancel_at_period_end=COALESCE($5,cancel_at_period_end),stripe_customer_id=COALESCE($6,stripe_customer_id),plan_id=COALESCE((SELECT plan_id FROM matched_price),plan_id),plan_version=COALESCE((SELECT plan_version FROM matched_price),plan_version),price_id=COALESCE((SELECT price_id FROM matched_price),price_id),scheduled_plan_id=CASE WHEN subscription.scheduled_plan_id=(SELECT plan_id FROM matched_price) AND subscription.scheduled_plan_version=(SELECT plan_version FROM matched_price) THEN NULL ELSE subscription.scheduled_plan_id END,scheduled_plan_version=CASE WHEN subscription.scheduled_plan_id=(SELECT plan_id FROM matched_price) AND subscription.scheduled_plan_version=(SELECT plan_version FROM matched_price) THEN NULL ELSE subscription.scheduled_plan_version END,stripe_subscription_schedule_id=CASE WHEN subscription.scheduled_plan_id=(SELECT plan_id FROM matched_price) AND subscription.scheduled_plan_version=(SELECT plan_version FROM matched_price) THEN NULL ELSE subscription.stripe_subscription_schedule_id END,provider_event_created_at=$8,row_version=row_version+1,updated_at=now() WHERE (stripe_subscription_id=$1 OR stripe_checkout_session_id=$7 OR workspace_id=$9) AND provider_event_created_at <= $8 RETURNING workspace_id,subscription_id,row_version",
        )
        .bind(stripe_subscription_id)
        .bind(status)
        .bind(
            object
                .get("current_period_start")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0),
        )
        .bind(
            object
                .get("current_period_end")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0),
        )
        .bind(object.get("cancel_at_period_end").and_then(serde_json::Value::as_bool))
        .bind(object.get("customer").and_then(serde_json::Value::as_str))
        .bind(stripe_checkout_id)
        .bind(event_created)
        .bind(workspace_id)
        .bind(stripe_price_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(database_status)?;
    if let Some(row) = row {
        let workspace_id: Uuid = row.try_get("workspace_id").map_err(database_status)?;
        let aggregate: Uuid = row.try_get("subscription_id").map_err(database_status)?;
        let version: i64 = row.try_get("row_version").map_err(database_status)?;
        insert_event(
            transaction,
            "commercial_billing.subscription.provider_state_changed",
            workspace_id,
            aggregate,
            version,
            correlation_id,
        )
        .await?;
    }
    Ok(())
}
pub(super) fn log_database_error(operation: &'static str, error: &sqlx::Error) {
    let sqlstate = error
        .as_database_error()
        .and_then(|value| value.code())
        .map(|value| value.into_owned());
    let constraint = error
        .as_database_error()
        .and_then(|value| value.constraint())
        .map(str::to_owned);
    let table = error
        .as_database_error()
        .and_then(|value| value.table())
        .map(str::to_owned);
    tracing::error!(
        event = "billing.persistence.failed", component = "commercial_billing",
        operation, dependency = "postgresql", sqlstate =? sqlstate, constraint =?
        constraint, table =? table
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;
    use tonic::transport::Endpoint;
    #[test]
    fn stripe_signature_accepts_current_payload_and_rejects_tampering_and_replay() {
        let secret = "whsec_signature_unit_key";
        let payload = br#"{"id":"evt_test","type":"customer.subscription.updated"}"#;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_secs();
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .expect("HMAC accepts the configured key bytes");
        mac.update(timestamp.to_string().as_bytes());
        mac.update(b".");
        mac.update(payload);
        let digest = mac.finalize().into_bytes();
        let signature = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let header = format!("t={timestamp},v1={signature}");
        assert!(verify_signature(&header, payload, secret).is_ok());
        assert!(verify_signature(&header, b"tampered payload", secret).is_err());
        assert!(verify_signature("t=1,v1=00", payload, secret).is_err());
    }
    #[tokio::test]
    async fn signed_subscription_webhook_updates_once_and_appends_one_outbox_event()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        xovyra_platform::configuration::load_environment_file()?;
        let database_url = std::env::var("COMMERCIAL_BILLING_DATABASE_URL")?;
        let redis_url = std::env::var("REDIS_URL")?;
        let database = PgPoolOptions::new()
            .max_connections(3)
            .connect(&database_url)
            .await?;
        let redis = redis::Client::open(redis_url)?;
        let cache = redis.get_connection_manager().await?;
        let secret = "whsec_xovyra_automated_integration";
        let service = CommercialBillingService {
            database: database.clone(),
            cache,
            identity_channel: Endpoint::from_static("http://127.0.0.1:1").connect_lazy(),
            stripe_secret: None,
            stripe_webhook_secret: Some(secret.to_owned()),
            app_origin: "https://app-xovyra.jnmtortas.com".to_owned(),
            http: reqwest::Client::new(),
        };
        let plan_id = Uuid::new_v4();
        let price_id = Uuid::new_v4();
        let subscription_id = Uuid::new_v4();
        let workspace_id = Uuid::new_v4();
        let event_id = format!("evt_{}", Uuid::new_v4().simple());
        let plan_key = format!("webhook-integration-{}", Uuid::new_v4().simple());
        let stripe_price_id = format!("price_{}", Uuid::new_v4().simple());
        let stripe_subscription_id = format!("sub_{}", Uuid::new_v4().simple());
        let event_created = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;
        let period_start = event_created - 60;
        let period_end = event_created + 30 * 24 * 60 * 60;
        let setup = async {
            sqlx::query(
                    "INSERT INTO commercial_billing.catalog_plans (plan_id,version,plan_key,display_name,description,lifecycle,profitability_evidence,approved_by_operator,approved_at,commercial_gate_reference,tax_gate_reference) VALUES ($1,1,$2,'Webhook test plan','Integration','published','test evidence',$3,now(),'test commercial gate','test tax gate')",
                )
                .bind(plan_id)
                .bind(&plan_key)
                .bind(Uuid::new_v4())
                .execute(&database)
                .await?;
            sqlx::query(
                    "INSERT INTO commercial_billing.localized_plan_prices (price_id,plan_id,plan_version,locale,currency,amount_minor,billing_interval,stripe_price_id,effective_from) VALUES ($1,$2,1,'es-CO','USD',1900,'month',$3,now())",
                )
                .bind(price_id)
                .bind(plan_id)
                .bind(&stripe_price_id)
                .execute(&database)
                .await?;
            sqlx::query(
                    "INSERT INTO commercial_billing.workspace_subscriptions (subscription_id,workspace_id,plan_id,plan_version,price_id,status,stripe_subscription_id) VALUES ($1,$2,$3,1,$4,'incomplete',$5)",
                )
                .bind(subscription_id)
                .bind(workspace_id)
                .bind(plan_id)
                .bind(price_id)
                .bind(&stripe_subscription_id)
                .execute(&database)
                .await?;
            Ok::<(), sqlx::Error>(())
        }
            .await;
        setup?;
        let payload = serde_json::json!(
            { "id" : event_id, "type" : "customer.subscription.updated", "created" :
            event_created, "data" : { "object" : { "id" : stripe_subscription_id,
            "status" : "active", "metadata" : { "workspace_id" : workspace_id },
            "current_period_start" : period_start, "current_period_end" : period_end,
            "cancel_at_period_end" : false, "customer" : "cus_test_integration", "items"
            : { "data" : [{ "price" : { "id" : stripe_price_id } }] } } } }
        );
        let payload = serde_json::to_vec(&payload)?;
        let signature_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())?;
        mac.update(signature_time.to_string().as_bytes());
        mac.update(b".");
        mac.update(&payload);
        let signature = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let signature = format!("t={signature_time},v1={signature}");
        let request = || xovyra_platform::grpc::ApplyStripeWebhookRequest {
            signature: signature.clone(),
            payload: payload.clone(),
            correlation_id: "billing-webhook-integration".to_owned(),
        };
        let verification = async {
            let first = apply(&service, request()).await?;
            let replay = apply(&service, request()).await?;
            assert!(first.accepted);
            assert!(! first.duplicate);
            assert!(replay.accepted);
            assert!(replay.duplicate);
            let row = sqlx::query(
                    "SELECT status,row_version,provider_event_created_at,current_period_start,current_period_end FROM commercial_billing.workspace_subscriptions WHERE workspace_id=$1",
                )
                .bind(workspace_id)
                .fetch_one(&database)
                .await?;
            let status: String = row.try_get("status")?;
            let row_version: i64 = row.try_get("row_version")?;
            let provider_event_created_at: i64 = row
                .try_get("provider_event_created_at")?;
            assert_eq!(status, "active");
            assert_eq!(row_version, 2);
            assert_eq!(provider_event_created_at, event_created);
            let outbox_count: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM commercial_billing.outbox_events WHERE event_type='commercial_billing.subscription.provider_state_changed' AND payload->>'workspace_id'=$1",
                )
                .bind(workspace_id.to_string())
                .fetch_one(&database)
                .await?;
            assert_eq!(outbox_count, 1);
            let inbox_outcome: String = sqlx::query_scalar(
                    "SELECT outcome FROM commercial_billing.stripe_webhook_inbox WHERE event_id=$1",
                )
                .bind(&event_id)
                .fetch_one(&database)
                .await?;
            assert_eq!(inbox_outcome, "processed");
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        }
            .await;
        let cleanup = async {
            sqlx::query(
                "DELETE FROM commercial_billing.outbox_events WHERE payload->>'workspace_id'=$1",
            )
            .bind(workspace_id.to_string())
            .execute(&database)
            .await?;
            sqlx::query("DELETE FROM commercial_billing.stripe_webhook_inbox WHERE event_id=$1")
                .bind(&event_id)
                .execute(&database)
                .await?;
            sqlx::query(
                "DELETE FROM commercial_billing.workspace_subscriptions WHERE workspace_id=$1",
            )
            .bind(workspace_id)
            .execute(&database)
            .await?;
            sqlx::query("DELETE FROM commercial_billing.localized_plan_prices WHERE price_id=$1")
                .bind(price_id)
                .execute(&database)
                .await?;
            sqlx::query("DELETE FROM commercial_billing.catalog_plans WHERE plan_id=$1")
                .bind(plan_id)
                .execute(&database)
                .await?;
            Ok::<(), sqlx::Error>(())
        }
        .await;
        database.close().await;
        verification?;
        cleanup?;
        Ok(())
    }
}
