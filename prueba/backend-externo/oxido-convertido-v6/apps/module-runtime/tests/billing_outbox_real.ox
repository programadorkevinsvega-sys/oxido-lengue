sintaxis llaves
use std::{error::Error, time::Duration};

use async_nats::jetstream::{self, consumer::pull::Config};
use futures_util::StreamExt;
use opentelemetry::Context;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;
use xovyra_platform::outbox::{JetStreamEventPublisher, OutboxRelay, OutboxWriter};

#[tokio::test]
async funcion billing_outbox_reaches_jetstream_and_workspace_realtime_journal()
-> Result<(), Box<dyn Error + Send + Sync>> {
    xovyra_platform::configuration::load_environment_file()?;
    let database_url = std::env::var("COMMERCIAL_BILLING_DATABASE_URL")?;
    let realtime_database_url = std::env::var("REALTIME_DATABASE_URL")?;
    let nats_url = std::env::var("NATS_URL")?;
    let nats_username = std::env::var("NATS_USERNAME")?;
    let nats_password = std::env::var("NATS_PASSWORD")?;
    let messaging =
        async_nats::ConnectOptions::with_user_and_password(nats_username, nats_password)
            .connect(nats_url)
            .await?;
    let jetstream = jetstream::new(messaging.clone());
    let billing_database = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;
    let realtime_database = PgPoolOptions::new()
        .max_connections(2)
        .connect(&realtime_database_url)
        .await?;

    let event_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let subscription_id = Uuid::new_v4();
    let event_type = format!(
        "commercial_billing.subscription.integration_{}",
        Uuid::new_v4()
    );
    let subject = format!("{event_type}.v1");
    let correlation_id = format!("billing-outbox-{}", Uuid::new_v4());
    let payload = serde_json::json!({
        "event_id": event_id,
        "event_type": event_type,
        "aggregate_id": subscription_id,
        "aggregate_version": 1,
        "workspace_id": workspace_id,
        "audience": "tenant",
        "correlation_id": correlation_id,
    });
    let stream = jetstream.get_stream("COMMERCIAL_BILLING_EVENTS").await?;
    let consumer = stream
        .create_consumer(Config {
            ack_policy: jetstream::consumer::AckPolicy::Explicit,
            deliver_policy: jetstream::consumer::DeliverPolicy::New,
            filter_subject: subject.clone(),
            ..Default::default()
        })
        .await?;

    let verification = async {
        let writer =
            OutboxWriter::new(xovyra_commercial_billing::module_public_api::bounded_context())?;
        let mut transaction = billing_database.begin().await?;
        writer
            .append(
                &mut transaction,
                &event_id.to_string(),
                &event_type,
                1,
                &payload,
                &Context::new(),
            )
            .await?;
        transaction.commit().await?;

        let relay =
            OutboxRelay::new(xovyra_commercial_billing::module_public_api::bounded_context())?;
        let publisher = JetStreamEventPublisher::new(jetstream.clone());
        let mut connection = billing_database.acquire().await?;
        assert!(
            relay
                .relay_event(&mut connection, &publisher, &event_id.to_string())
                .await?
        );
        assert!(
            !relay
                .relay_event(&mut connection, &publisher, &event_id.to_string())
                .await?
        );

        let mut messages = consumer.messages().await?;
        let message = tokio::time::timeout(Duration::from_secs(5), messages.next())
            .await?
            .ok_or("JetStream did not deliver the billing Outbox event")??;
        let envelope: serde_json::Value = serde_json::from_slice(&message.payload)?;
        assert_eq!(envelope["event_id"], event_id.to_string());
        assert_eq!(envelope["event_type"], event_type);
        assert_eq!(
            envelope["payload"]["workspace_id"],
            workspace_id.to_string()
        );
        message.ack().await?;

        let replay_consumer = stream
            .create_consumer(Config {
                ack_policy: jetstream::consumer::AckPolicy::Explicit,
                deliver_policy: jetstream::consumer::DeliverPolicy::All,
                filter_subject: subject.clone(),
                ..Default::default()
            })
            .await?;
        let mut replay_messages = replay_consumer.messages().await?;
        let replay = tokio::time::timeout(Duration::from_secs(5), replay_messages.next())
            .await?
            .ok_or("JetStream did not replay the persisted billing event")??;
        assert_eq!(replay.payload.as_ref(), message.payload.as_ref());
        replay.ack().await?;

        xovyra_realtime::journal::persist_domain_event(&realtime_database, &envelope).await?;
        xovyra_realtime::journal::persist_domain_event(&realtime_database, &envelope).await?;
        let page =
            xovyra_realtime::journal::list_after(&realtime_database, "tenant", workspace_id, 0, 20)
                .await?;
        let projected = page
            .events
            .iter()
            .find(|event| event.event_id == event_id)
            .ok_or("billing event is missing from the workspace realtime journal")?;
        assert_eq!(
            page.events
                .iter()
                .filter(|event| event.event_id == event_id)
                .count(),
            1
        );
        assert_eq!(projected.audience, "tenant");
        assert_eq!(projected.scope_id, workspace_id);
        assert_eq!(projected.aggregate_id, subscription_id);
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;

    let journal_cleanup = sqlx::query("DELETE FROM realtime.event_journal WHERE event_id=$1")
        .bind(event_id)
        .execute(&realtime_database)
        .await;
    let cursor_cleanup =
        sqlx::query("DELETE FROM realtime.stream_cursors WHERE audience='tenant' AND scope_id=$1")
            .bind(workspace_id)
            .execute(&realtime_database)
            .await;
    let outbox_cleanup =
        sqlx::query("DELETE FROM commercial_billing.outbox_events WHERE event_id=$1")
            .bind(event_id)
            .execute(&billing_database)
            .await;
    let stream_cleanup = async {
        let stored = stream.get_last_raw_message_by_subject(&subject).await?;
        stream.delete_message(stored.sequence).await?;
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;
    billing_database.close().await;
    realtime_database.close().await;
    messaging.flush().await?;
    verification?;
    journal_cleanup?;
    cursor_cleanup?;
    outbox_cleanup?;
    stream_cleanup?;
    Ok(())
}
