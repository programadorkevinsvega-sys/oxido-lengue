use async_nats::jetstream::{self, consumer::pull::Config, message::AckKind};
use futures_util::StreamExt;
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;
use xovyra_platform::dependencies::PlatformDependencies;
pub async fn run(dependencies: Arc<PlatformDependencies>, shutdown: watch::Receiver<bool>) {
    let tenancy_dependencies = Arc::clone(&dependencies);
    let notification_dependencies = Arc::clone(&dependencies);
    let asset_dependencies = Arc::clone(&dependencies);
    let jobs_dependencies = Arc::clone(&dependencies);
    let platform_dependencies = Arc::clone(&dependencies);
    let billing_dependencies = Arc::clone(&dependencies);
    tokio::join!(
        consume(
            tenancy_dependencies,
            "TENANCY_ACCESS_EVENTS",
            "xovyra-realtime-tenancy-projection",
            "tenancy_access.>",
            true,
            shutdown.clone(),
        ),
        consume(
            notification_dependencies,
            "NOTIFICATIONS_EVENTS",
            "xovyra-realtime-notifications-projection",
            "notifications.notification.>",
            false,
            shutdown.clone(),
        ),
        consume(
            asset_dependencies,
            "ASSET_STORAGE_EVENTS",
            "xovyra-realtime-assets-projection",
            "asset_storage.>",
            false,
            shutdown.clone(),
        ),
        consume(
            jobs_dependencies,
            "JOBS_EVENTS",
            "xovyra-realtime-jobs-projection",
            "jobs.>",
            false,
            shutdown.clone(),
        ),
        consume(
            platform_dependencies,
            "PLATFORM_CONTROL_EVENTS",
            "xovyra-realtime-platform-control-projection",
            "platform_control.>",
            false,
            shutdown.clone(),
        ),
        consume(
            billing_dependencies,
            "COMMERCIAL_BILLING_EVENTS",
            "xovyra-realtime-commercial-billing-projection",
            "commercial_billing.>",
            false,
            shutdown,
        )
    );
}
async fn consume(
    dependencies: Arc<PlatformDependencies>,
    stream_name: &'static str,
    durable_name: &'static str,
    filter_subject: &'static str,
    team_events: bool,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        if *shutdown.borrow() {
            return;
        }
        match dependencies.jetstream.get_stream(stream_name).await {
            Ok(stream) => {
                let consumer = match stream
                    .get_or_create_consumer(
                        durable_name,
                        Config {
                            durable_name: Some(durable_name.to_owned()),
                            ack_policy: jetstream::consumer::AckPolicy::Explicit,
                            filter_subject: filter_subject.to_owned(),
                            max_deliver: 10,
                            ..Default::default()
                        },
                    )
                    .await
                {
                    Ok(consumer) => consumer,
                    Err(error) => {
                        tracing::error!(
                            stream_name, error_type =% std::any::type_name_of_val(&
                            error),
                            "Realtime projection cannot create its durable consumer; retrying"
                        );
                        if wait_for_reconnect(&mut shutdown).await {
                            return;
                        }
                        continue;
                    }
                };
                let mut messages = match consumer.messages().await {
                    Ok(messages) => messages,
                    Err(error) => {
                        tracing::error!(
                            stream_name, error_type =% std::any::type_name_of_val(&
                            error),
                            "Realtime projection cannot read durable messages; retrying"
                        );
                        if wait_for_reconnect(&mut shutdown).await {
                            return;
                        }
                        continue;
                    }
                };
                loop {
                    tokio::select! {
                        item = messages.next() => match item { Some(Ok(message)) => { let
                        event : Result < Value, _ > = serde_json::from_slice(& message
                        .payload); let trace_id = message.headers.as_ref().and_then(|
                        headers | headers.get_last("traceparent")).and_then(| value |
                        value.as_str().split('-').nth(1)).filter(| value | value.len() ==
                        32 && value.bytes().all(| byte | byte.is_ascii_hexdigit())); let
                        processed = match event { Ok(event) => process_event(&
                        dependencies, & event, team_events, trace_id). await, Err(error)
                        => { tracing::error!(stream_name, error_type =%
                        std::any::type_name_of_val(& error),
                        "Realtime projection event is invalid"); true } }; let
                        acknowledgement = if processed { message.ack(). await } else {
                        message.ack_with(AckKind::Nak(Some(Duration::from_secs(5)))).
                        await }; if let Err(error) = acknowledgement {
                        tracing::warn!(stream_name, error_type =%
                        std::any::type_name_of_val(& error),
                        "Realtime projection acknowledgement failed; durable event will be recovered");
                        break; } } Some(Err(error)) => { tracing::warn!(stream_name,
                        error_type =% std::any::type_name_of_val(& error),
                        "Realtime projection stream failed; rebuilding durable consumer");
                        break; } None => { tracing::warn!(stream_name,
                        "Realtime projection stream closed; rebuilding durable consumer");
                        break; } }, result = shutdown.changed() => if result.is_err() ||
                        * shutdown.borrow() { return; }
                    }
                }
            }
            Err(error) => {
                tracing::error!(
                    stream_name, error_type =% std::any::type_name_of_val(& error),
                    "Realtime projection cannot find its JetStream stream; retrying"
                );
            }
        }
        if wait_for_reconnect(&mut shutdown).await {
            return;
        }
    }
}
async fn wait_for_reconnect(shutdown: &mut watch::Receiver<bool>) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(2)) => false, result = shutdown
        .changed() => result.is_err() || * shutdown.borrow(),
    }
}
async fn process_event(
    dependencies: &PlatformDependencies,
    event: &Value,
    team_event: bool,
    trace_id: Option<&str>,
) -> bool {
    let event_id = event
        .get("event_id")
        .and_then(Value::as_str)
        .unwrap_or("missing");
    let event_type = event
        .get("event_type")
        .and_then(Value::as_str)
        .unwrap_or("missing");
    let correlation_id = event
        .get("correlation_id")
        .and_then(Value::as_str)
        .unwrap_or("missing");
    if team_event
        && let Err(error) = xovyra_notifications::inbox::consume_team_event(
            &dependencies.notifications_database,
            event,
        )
        .await
    {
        match &error {
            xovyra_notifications::inbox::InboxError::InvalidEvent => {
                tracing::error!(
                    failure_class = "terminal_invalid_event", error_code =
                    "team_notification_invalid_event", error_category =
                    "event_validation", operation = "project_team_notification",
                    event_id, event_type, correlation_id, trace_id = ? trace_id,
                    "Team notification projection rejected a permanently invalid event"
                );
                return true;
            }
            xovyra_notifications::inbox::InboxError::Persistence(sqlx::Error::Database(
                database_error,
            )) => {
                let database_code = database_error.code();
                let permanent_failure = database_code
                    .as_deref()
                    .is_some_and(|code| matches!(code.get(..2), Some("22" | "23" | "42")));
                tracing::error!(
                    error_code = "team_notification_database_constraint", error_category
                    = "database_constraint", operation = "project_team_notification",
                    dependency = "postgresql", event_id, event_type, correlation_id,
                    trace_id = ? trace_id, failure_class =
                    "database_constraint_or_statement", database_code = ? database_code,
                    constraint = ? database_error.constraint(),
                    "Team notification projection failed"
                );
                if permanent_failure {
                    return true;
                }
            }
            xovyra_notifications::inbox::InboxError::Persistence(database_error) => {
                tracing::error!(
                    error_code = "team_notification_database_failure", error_category =
                    "database", operation = "project_team_notification", dependency =
                    "postgresql", event_id, event_type, correlation_id, trace_id = ?
                    trace_id, failure_class = "database_connection_or_decode", error_type
                    = % std::any::type_name_of_val(database_error),
                    "Team notification projection failed"
                );
            }
        }
        return false;
    }
    if let Err(error) =
        xovyra_realtime::journal::persist_domain_event(&dependencies.realtime_database, event).await
    {
        match error {
            xovyra_realtime::journal::JournalError::InvalidEvent => {
                tracing::error!(
                    error_code = "realtime_journal_invalid_event", error_category =
                    "event_validation", operation = "persist_domain_event", event_id,
                    event_type, correlation_id, trace_id = ? trace_id,
                    "Realtime journal rejected a permanently invalid event"
                );
                return true;
            }
            xovyra_realtime::journal::JournalError::Persistence(sqlx::Error::Database(error)) => {
                let database_code = error.code();
                tracing::error!(
                    error_code = "realtime_journal_database_constraint", error_category =
                    "database_constraint", operation = "persist_domain_event", dependency
                    = "postgresql", event_id, event_type, correlation_id, trace_id = ?
                    trace_id, database_code = ? database_code, constraint = ? error
                    .constraint(), "Realtime journal projection failed"
                );
                return false;
            }
            xovyra_realtime::journal::JournalError::Persistence(error) => {
                tracing::error!(
                    error_code = "realtime_journal_database_failure", error_category =
                    "database", operation = "persist_domain_event", dependency =
                    "postgresql", event_id, event_type, correlation_id, trace_id = ?
                    trace_id, error_type = % std::any::type_name_of_val(& error),
                    "Realtime journal projection failed"
                );
                return false;
            }
        }
    }
    let event_type = event
        .get("event_type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let audience = realtime_audience(event_type);
    let scope_id = match audience {
        "platform" => Some("00000000-0000-0000-0000-000000000001"),
        "tenant_user" => event
            .pointer("/payload/recipient_user_id")
            .and_then(Value::as_str),
        _ => event
            .pointer("/payload/workspace_id")
            .and_then(Value::as_str),
    };
    let Some(scope_id) = scope_id else {
        return true;
    };
    let event_id = event
        .get("event_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let channel = format!("xovyra:realtime:{audience}:{scope_id}");
    let mut cache = dependencies.cache.clone();
    let result: redis::RedisResult<i64> = redis::cmd("PUBLISH")
        .arg(channel)
        .arg(event_id)
        .query_async(&mut cache)
        .await;
    match result {
        Ok(_) => true,
        Err(error) => {
            tracing::warn!(
                error_type =% std::any::type_name_of_val(& error),
                "Realtime Redis fan-out failed; durable cursor remains available"
            );
            true
        }
    }
}
fn realtime_audience(event_type: &str) -> &'static str {
    if event_type.starts_with("notifications.notification.") {
        "tenant_user"
    } else if event_type.starts_with("platform_control.")
        || event_type.starts_with("commercial_billing.catalog.")
    {
        "platform"
    } else {
        "tenant"
    }
}
#[cfg(test)]
mod tests {
    use super::{process_event, realtime_audience};
    use futures_util::stream;
    use sqlx::Row;
    use std::{error::Error, sync::Arc, time::Duration};
    use tokio::{net::TcpListener, task::JoinHandle};
    use tonic::transport::{Endpoint, Server};
    use uuid::Uuid;
    use xovyra_platform::{
        dependencies::{DependencyConfiguration, PlatformDependencies},
        grpc::{
            RecoverRealtimeEventsRequest, SubscribeRealtimeEventsRequest,
            realtime_gateway_client::RealtimeGatewayClient,
            realtime_gateway_server::RealtimeGatewayServer,
            tenancy_access_server::TenancyAccessServer,
        },
    };
    use xovyra_shared_kernel::BoundedContext;
    const CONTEXTS: [BoundedContext; 0] = [];
    #[test]
    fn billing_events_route_to_the_correct_websocket_audience() {
        assert_eq!(
            realtime_audience("commercial_billing.subscription.plan_change_scheduled"),
            "tenant"
        );
        assert_eq!(
            realtime_audience("commercial_billing.catalog.published"),
            "platform"
        );
    }
    #[tokio::test]
    async fn two_realtime_gateway_instances_share_journal_events_and_redis_fanout()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        xovyra_platform::configuration::load_environment_file()?;
        let database_url = std::env::var("DATABASE_URL")?;
        let identity_database_url = std::env::var("IDENTITY_DATABASE_URL")?;
        let tenancy_database_url = std::env::var("TENANCY_ACCESS_DATABASE_URL")?;
        let commercial_billing_database_url = std::env::var("COMMERCIAL_BILLING_DATABASE_URL")?;
        let notifications_database_url = std::env::var("NOTIFICATIONS_DATABASE_URL")?;
        let realtime_database_url = std::env::var("REALTIME_DATABASE_URL")?;
        let asset_storage_database_url = std::env::var("ASSET_STORAGE_DATABASE_URL")?;
        let jobs_database_url = std::env::var("JOBS_DATABASE_URL")?;
        let redis_url = std::env::var("REDIS_URL")?;
        let nats_url = std::env::var("NATS_URL")?;
        let nats_username = std::env::var("NATS_USERNAME")?;
        let nats_password = std::env::var("NATS_PASSWORD")?;
        let dependencies = Arc::new(
            PlatformDependencies::connect(DependencyConfiguration {
                database_url: &database_url,
                identity_database_url: &identity_database_url,
                platform_control_database_url: &identity_database_url,
                tenancy_access_database_url: &tenancy_database_url,
                commercial_billing_database_url: &commercial_billing_database_url,
                notifications_database_url: &notifications_database_url,
                realtime_database_url: &realtime_database_url,
                asset_storage_database_url: &asset_storage_database_url,
                jobs_database_url: &jobs_database_url,
                redis_url: &redis_url,
                nats_url: &nats_url,
                nats_username: &nats_username,
                nats_password: &nats_password,
                contexts: &CONTEXTS,
            })
            .await?,
        );
        let owner_user_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        let idempotency_key = Uuid::new_v4().to_string();
        let owner_email = format!("{owner_user_id}@integration.xovyra.invalid");
        let workspace = xovyra_tenancy_access::workspaces::create_workspace(
            &dependencies.tenancy_access_database,
            xovyra_tenancy_access::workspaces::WorkspaceCreation {
                user_id: owner_user_id,
                session_id,
                name: "Realtime replica integration",
                workspace_type: "team",
                idempotency_key: &idempotency_key,
                correlation_id: "realtime-replica-integration",
                owner_email: &owner_email,
            },
        )
        .await?;
        let verification = async {
            let event_row = sqlx::query(
                    "SELECT event_id,event_type,event_version,occurred_at::text AS occurred_at,payload FROM tenancy_access.outbox_events WHERE payload->>'workspace_id'=$1 ORDER BY occurred_at DESC LIMIT 1",
                )
                .bind(workspace.workspace_id.to_string())
                .fetch_one(&dependencies.tenancy_access_database)
                .await?;
            let event_id: Uuid = event_row.try_get("event_id")?;
            let event_type: String = event_row.try_get("event_type")?;
            let event_version: i16 = event_row.try_get("event_version")?;
            let occurred_at: String = event_row.try_get("occurred_at")?;
            let payload: serde_json::Value = event_row.try_get("payload")?;
            let event = serde_json::json!(
                { "event_id" : event_id, "event_type" : event_type, "event_version" :
                event_version, "occurred_at" : occurred_at, "correlation_id" : payload
                .get("correlation_id"), "payload" : payload, }
            );
            verify_two_gateway_instances(
                    Arc::clone(&dependencies),
                    owner_user_id,
                    workspace.workspace_id,
                    &event,
                    event_id,
                )
                .await
        }
            .await;
        let cleanup = cleanup_realtime_replica_integration(
            &dependencies,
            owner_user_id,
            workspace.workspace_id,
            &idempotency_key,
        )
        .await;
        dependencies.messaging.flush().await?;
        verification?;
        cleanup?;
        Ok(())
    }
    async fn verify_two_gateway_instances(
        dependencies: Arc<PlatformDependencies>,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        event: &serde_json::Value,
        event_id: Uuid,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let tenancy_listener = TcpListener::bind("127.0.0.1:0").await?;
        let tenancy_endpoint = format!("http://{}", tenancy_listener.local_addr()?);
        let tenancy_incoming = stream::unfold(tenancy_listener, |listener| async move {
            let accepted = listener.accept().await;
            Some((accepted.map(|connection| connection.0), listener))
        });
        let identity_endpoint = tenancy_endpoint.clone();
        let tenancy_dependencies = Arc::clone(&dependencies);
        let (tenancy_shutdown_sender, tenancy_shutdown_receiver) = tokio::sync::oneshot::channel();
        let tenancy_server_task = tokio::spawn(async move {
            Server::builder()
                .add_service(TenancyAccessServer::new(
                    crate::tenancy_service::TenancyAccessService::new(
                        tenancy_dependencies,
                        identity_endpoint,
                    ),
                ))
                .serve_with_incoming_shutdown(tenancy_incoming, async move {
                    if let Err(error) = tenancy_shutdown_receiver.await {
                        tracing::debug!(
                            % error,
                            "tenancy integration server shutdown signal was dropped"
                        );
                    }
                })
                .await
        });
        let (first_endpoint, first_shutdown, first_server_task) =
            start_gateway_instance(Arc::clone(&dependencies), tenancy_endpoint.clone()).await?;
        let (second_endpoint, second_shutdown, second_server_task) =
            start_gateway_instance(Arc::clone(&dependencies), tenancy_endpoint).await?;
        let verification = async {
            let first_channel = Endpoint::from_shared(first_endpoint)?.connect().await?;
            let second_channel = Endpoint::from_shared(second_endpoint)?.connect().await?;
            let mut first_client = RealtimeGatewayClient::new(first_channel);
            let mut second_client = RealtimeGatewayClient::new(second_channel);
            let request = || SubscribeRealtimeEventsRequest {
                audience: "tenant".to_owned(),
                scope_id: workspace_id.to_string(),
                after_cursor: 0,
                correlation_id: Uuid::new_v4().to_string(),
                actor_user_id: actor_user_id.to_string(),
            };
            let mut first_stream = first_client.subscribe_events(request()).await?.into_inner();
            let mut second_stream = second_client
                .subscribe_events(request())
                .await?
                .into_inner();
            if !process_event(&dependencies, event, true, None).await {
                return Err("realtime event projection failed against real services".into());
            }
            if !process_event(&dependencies, event, true, None).await {
                return Err(
                    "replayed realtime event projection failed against real services".into(),
                );
            }
            let first_event = tokio::time::timeout(Duration::from_secs(5), first_stream.message())
                .await??
                .ok_or("first gateway did not deliver the durable event")?;
            let second_event =
                tokio::time::timeout(Duration::from_secs(5), second_stream.message())
                    .await??
                    .ok_or("second gateway did not deliver the durable event")?;
            assert_eq!(first_event.event_id, event_id.to_string());
            assert_eq!(second_event.event_id, event_id.to_string());
            assert_eq!(first_event.stream_cursor, second_event.stream_cursor);
            let repeated_first =
                tokio::time::timeout(Duration::from_millis(750), first_stream.message()).await;
            if repeated_first.is_ok() {
                return Err("duplicate event was delivered by the first gateway".into());
            }
            let repeated_second =
                tokio::time::timeout(Duration::from_millis(750), second_stream.message()).await;
            if repeated_second.is_ok() {
                return Err("duplicate event was delivered by the second gateway".into());
            }
            for (client, endpoint_label) in
                [(&mut first_client, "first"), (&mut second_client, "second")]
            {
                let recovery = client
                    .recover_events(RecoverRealtimeEventsRequest {
                        audience: "tenant".to_owned(),
                        scope_id: workspace_id.to_string(),
                        after_cursor: 0,
                        page_size: 20,
                        correlation_id: Uuid::new_v4().to_string(),
                        actor_user_id: actor_user_id.to_string(),
                    })
                    .await?
                    .into_inner();
                assert_eq!(
                    recovery
                        .events
                        .iter()
                        .filter(|recovered_event| recovered_event.event_id == event_id.to_string())
                        .count(),
                    1,
                    "{endpoint_label} gateway recovery did not return exactly one event"
                );
            }
            Ok::<(), Box<dyn Error + Send + Sync>>(())
        }
        .await;
        let first_shutdown_result = first_shutdown.send(());
        let second_shutdown_result = second_shutdown.send(());
        let tenancy_shutdown_result = tenancy_shutdown_sender.send(());
        if first_shutdown_result.is_err()
            || second_shutdown_result.is_err()
            || tenancy_shutdown_result.is_err()
        {
            return Err("a realtime gateway stopped before receiving shutdown".into());
        }
        first_server_task.await??;
        second_server_task.await??;
        tenancy_server_task.await??;
        verification
    }
    async fn start_gateway_instance(
        dependencies: Arc<PlatformDependencies>,
        tenancy_endpoint: String,
    ) -> Result<
        (
            String,
            tokio::sync::oneshot::Sender<()>,
            JoinHandle<Result<(), tonic::transport::Error>>,
        ),
        Box<dyn Error + Send + Sync>,
    > {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let tenancy_channel = Endpoint::from_shared(tenancy_endpoint)?.connect_lazy();
        let incoming = stream::unfold(listener, |listener| async move {
            let accepted = listener.accept().await;
            Some((accepted.map(|connection| connection.0), listener))
        });
        let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
        let server_task = tokio::spawn(async move {
            Server::builder()
                .add_service(RealtimeGatewayServer::new(
                    crate::realtime_service::RealtimeGatewayService::new(
                        dependencies,
                        tenancy_channel,
                    ),
                ))
                .serve_with_incoming_shutdown(incoming, async move {
                    if let Err(error) = shutdown_receiver.await {
                        tracing::debug!(
                            % error,
                            "gateway integration server shutdown signal was dropped"
                        );
                    }
                })
                .await
        });
        Ok((endpoint, shutdown_sender, server_task))
    }
    async fn cleanup_realtime_replica_integration(
        dependencies: &PlatformDependencies,
        owner_user_id: Uuid,
        workspace_id: Uuid,
        idempotency_key: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        sqlx::query("DELETE FROM realtime.event_journal WHERE audience='tenant' AND scope_id=$1")
            .bind(workspace_id)
            .execute(&dependencies.realtime_database)
            .await?;
        sqlx::query("DELETE FROM realtime.stream_cursors WHERE audience='tenant' AND scope_id=$1")
            .bind(workspace_id)
            .execute(&dependencies.realtime_database)
            .await?;
        sqlx::query("DELETE FROM tenancy_access.outbox_events WHERE payload->>'workspace_id'=$1")
            .bind(workspace_id.to_string())
            .execute(&dependencies.tenancy_access_database)
            .await?;
        sqlx::query("DELETE FROM tenancy_access.workspaces WHERE workspace_id=$1")
            .bind(workspace_id)
            .execute(&dependencies.tenancy_access_database)
            .await?;
        sqlx::query(
                "DELETE FROM tenancy_access.workspace_creation_idempotency WHERE user_id=$1 AND idempotency_key=$2",
            )
            .bind(owner_user_id)
            .bind(idempotency_key)
            .execute(&dependencies.tenancy_access_database)
            .await?;
        Ok(())
    }
}
