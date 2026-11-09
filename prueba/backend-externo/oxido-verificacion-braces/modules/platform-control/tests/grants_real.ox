sintaxis llaves
use std::collections::HashSet;
use std::time::Duration;

use async_nats::jetstream;
use async_nats::jetstream::consumer::pull::Config;
use async_nats::jetstream::stream::Config as StreamConfig;
use futures_util::StreamExt;
use sqlx::{Connection, PgConnection, postgres::PgPoolOptions};
use xovyra_platform::outbox::{JetStreamEventPublisher, OutboxRelay, OutboxWriter};
use xovyra_platform_control::grants::{self, GrantError, NewSupportGrant};

#[tokio::test]
async fn support_grants_audit_and_keyset_pagination_use_postgresql()
-> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("PLATFORM_CONTROL_DATABASE_URL")?;
    let database = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;
    let operator_id = uuid::Uuid::new_v4();
    let workspace_id = uuid::Uuid::new_v4();
    let first_asset_id = uuid::Uuid::new_v4();
    let second_asset_id = uuid::Uuid::new_v4();
    let first_key = uuid::Uuid::new_v4().to_string();
    let second_key = uuid::Uuid::new_v4().to_string();
    let now = sqlx::query_scalar::<_, i64>("SELECT floor(extract(epoch FROM now()))::bigint")
        .fetch_one(&database)
        .await?;

    let first = grants::create(
        &database,
        NewSupportGrant {
            workspace_id,
            asset_id: first_asset_id,
            operator_user_id: operator_id,
            reason: "Customer requested a review of this exact asset.",
            expires_at_unix: now + 600,
            idempotency_key: &first_key,
            correlation_id: "phase-4-5-real-grant-test",
        },
    )
    .await?;
    let repeated = grants::create(
        &database,
        NewSupportGrant {
            workspace_id,
            asset_id: first_asset_id,
            operator_user_id: operator_id,
            reason: "Customer requested a review of this exact asset.",
            expires_at_unix: now + 600,
            idempotency_key: &first_key,
            correlation_id: "phase-4-5-real-grant-test-retry",
        },
    )
    .await?;
    assert_eq!(first.grant_id, repeated.grant_id);

    let conflict = grants::create(
        &database,
        NewSupportGrant {
            workspace_id,
            asset_id: second_asset_id,
            operator_user_id: operator_id,
            reason: "Customer requested a review of this exact asset.",
            expires_at_unix: now + 600,
            idempotency_key: &first_key,
            correlation_id: "phase-4-5-real-grant-test-conflict",
        },
    )
    .await;
    assert!(matches!(conflict, Err(GrantError::IdempotencyConflict)));

    let second = grants::create(
        &database,
        NewSupportGrant {
            workspace_id,
            asset_id: second_asset_id,
            operator_user_id: operator_id,
            reason: "Customer requested a review of this exact asset.",
            expires_at_unix: now + 600,
            idempotency_key: &second_key,
            correlation_id: "phase-4-5-real-grant-test-second",
        },
    )
    .await?;
    assert!(
        grants::authorize_asset_read(
            &database,
            first.grant_id,
            workspace_id,
            first_asset_id,
            operator_id,
            "phase-4-5-real-grant-test-read",
        )
        .await
        .is_ok()
    );
    assert!(matches!(
        grants::authorize_asset_read(
            &database,
            first.grant_id,
            workspace_id,
            second_asset_id,
            operator_id,
            "phase-4-5-real-grant-test-cross-asset",
        )
        .await,
        Err(GrantError::NotFound)
    ));

    let grant_page = grants::list(&database, "", 1).await?;
    assert_eq!(grant_page.grants.len(), 1);
    assert!(!grant_page.next_cursor.is_empty());
    let next_grant_page = grants::list(&database, &grant_page.next_cursor, 10).await?;
    assert_eq!(next_grant_page.grants.len(), 1);
    assert!(next_grant_page.next_cursor.is_empty());

    let revoked = grants::revoke(
        &database,
        first.grant_id,
        operator_id,
        "The approved support review is complete.",
        "phase-4-5-real-grant-test-revoke",
    )
    .await?;
    assert_eq!(revoked.status, "revoked");
    assert!(matches!(
        grants::authorize_asset_read(
            &database,
            first.grant_id,
            workspace_id,
            first_asset_id,
            operator_id,
            "phase-4-5-real-grant-test-revoked",
        )
        .await,
        Err(GrantError::NotFound)
    ));

    let first_audit_page = grants::list_audit(&database, "", 2).await?;
    assert_eq!(first_audit_page.events.len(), 2);
    assert!(!first_audit_page.next_cursor.is_empty());
    let second_audit_page =
        grants::list_audit(&database, &first_audit_page.next_cursor, 200).await?;
    assert!(!second_audit_page.events.is_empty());
    let event_ids = first_audit_page
        .events
        .iter()
        .chain(second_audit_page.events.iter())
        .map(|event| event.event_id)
        .collect::<HashSet<_>>();
    assert_eq!(
        event_ids.len(),
        first_audit_page.events.len() + second_audit_page.events.len()
    );
    assert!(
        first_audit_page
            .events
            .iter()
            .chain(second_audit_page.events.iter())
            .any(|event| event.result == "rejected")
    );

    let mutation =
        sqlx::query("UPDATE platform_control.support_access_audit SET result = 'failed'")
            .execute(&database)
            .await;
    assert!(mutation.is_err());

    database.close().await;
    let _ = second.grant_id;
    Ok(())
}

#[tokio::test]
async fn platform_outbox_is_confirmed_by_jetstream_and_projected_to_realtime()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    xovyra_platform::configuration::load_environment_file()?;
    let platform_database_url = std::env::var("PLATFORM_CONTROL_DATABASE_URL")?;
    let realtime_database_url = std::env::var("REALTIME_DATABASE_URL")?;
    let migrator_database_url = std::env::var("MIGRATOR_DATABASE_URL")?;
    let nats_url = std::env::var("NATS_URL")?;
    let nats_username = std::env::var("NATS_USERNAME")?;
    let nats_password = std::env::var("NATS_PASSWORD")?;
    let messaging =
        async_nats::ConnectOptions::with_user_and_password(nats_username, nats_password)
            .connect(nats_url)
            .await?;
    let jetstream = jetstream::new(messaging.clone());
    let stream = jetstream
        .get_or_create_stream(StreamConfig {
            name: "PLATFORM_CONTROL_EVENTS".to_owned(),
            subjects: vec!["platform_control.>".to_owned()],
            ..Default::default()
        })
        .await?;
    let event_id = uuid::Uuid::new_v4();
    let event_id_text = event_id.to_string();
    let event_type = format!(
        "platform_control.support_test_{}.created",
        event_id.simple()
    );
    let subject = format!("{event_type}.v1");
    let workspace_id = uuid::Uuid::new_v4();
    let payload = serde_json::json!({
        "correlation_id": event_id_text,
        "operator_user_id": uuid::Uuid::new_v4(),
        "platform_scope_id": "00000000-0000-0000-0000-000000000001",
        "workspace_id": workspace_id,
        "asset_id": uuid::Uuid::new_v4(),
        "grant_id": uuid::Uuid::new_v4(),
        "aggregate_id": event_id,
        "aggregate_version": 1,
        "action": "support_grant.created"
    });
    let mut platform_connection = PgConnection::connect(&platform_database_url).await?;
    let realtime_database = PgPoolOptions::new()
        .max_connections(2)
        .connect(&realtime_database_url)
        .await?;
    let verification = async {
        let scope_id = uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001")?;
        let baseline_cursor = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(stream_cursor), 0) FROM realtime.event_journal WHERE audience='platform' AND scope_id=$1",
        )
        .bind(scope_id)
        .fetch_one(&realtime_database)
        .await?;
        OutboxWriter::new(xovyra_platform_control::module_public_api::bounded_context())?
            .append(
                &mut platform_connection,
                &event_id_text,
                &event_type,
                1,
                &payload,
                &opentelemetry::Context::new(),
            )
            .await?;
        let relay = OutboxRelay::new(xovyra_platform_control::module_public_api::bounded_context())?;
        let publisher = JetStreamEventPublisher::new(jetstream.clone());
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let published = sqlx::query_scalar::<_, bool>(
                "SELECT published_at IS NOT NULL FROM platform_control.outbox_events WHERE event_id=$1",
            )
            .bind(event_id)
            .fetch_one(&mut platform_connection)
            .await?;
            if published {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                return Err("platform Outbox event was not confirmed by JetStream".into());
            }
            relay
                .relay_one(&mut platform_connection, &publisher)
                .await?;
        }
        let consumer = stream
            .create_consumer(Config {
                ack_policy: jetstream::consumer::AckPolicy::Explicit,
                filter_subject: subject.clone(),
                ..Default::default()
            })
            .await?;
        let mut messages = consumer.messages().await?;
        let message = tokio::time::timeout(Duration::from_secs(5), messages.next())
            .await?
            .ok_or("JetStream did not deliver the platform Outbox event")??;
        let event: serde_json::Value = serde_json::from_slice(&message.payload)?;
        let delivered_id = event
            .get("event_id")
            .and_then(serde_json::Value::as_str)
            .ok_or("platform event has no event_id")?;
        if delivered_id != event_id_text {
            return Err("JetStream delivered a different platform event".into());
        }
        xovyra_realtime::journal::persist_domain_event(&realtime_database, &event).await?;
        let page = xovyra_realtime::journal::list_after(
            &realtime_database,
            "platform",
            scope_id,
            baseline_cursor,
            20,
        )
        .await?;
        if !page.events.iter().any(|entry| entry.event_id == event_id) {
            return Err("platform event was not projected to the realtime journal".into());
        }
        message.ack().await?;
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    }
    .await;
    let event_cleanup = sqlx::query("DELETE FROM realtime.event_journal WHERE event_id=$1")
        .bind(event_id)
        .execute(&realtime_database)
        .await;
    let mut migrator_connection = PgConnection::connect(&migrator_database_url).await?;
    let outbox_cleanup =
        sqlx::query("DELETE FROM platform_control.outbox_events WHERE event_id=$1")
            .bind(event_id)
            .execute(&mut migrator_connection)
            .await;
    let message_cleanup = stream.purge().filter(subject).await;
    platform_connection.close().await?;
    realtime_database.close().await;
    migrator_connection.close().await?;
    verification?;
    event_cleanup?;
    outbox_cleanup?;
    message_cleanup?;
    messaging.flush().await?;
    Ok(())
}
