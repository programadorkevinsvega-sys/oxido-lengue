sintaxis llaves
use std::{pin::Pin, sync::Arc, time::Duration};

use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use uuid::Uuid;
use xovyra_platform::dependencies::PlatformDependencies;
use xovyra_platform::grpc::realtime_gateway_server::RealtimeGateway;
use xovyra_platform::grpc::{
    GetWorkspaceRequest, RealtimeEventMessage, RecoverRealtimeEventsRequest,
    RecoverRealtimeEventsResponse, SubscribeRealtimeEventsRequest,
    tenancy_access_client::TenancyAccessClient,
};

type EventStream =
    Pin<Box<dyn tokio_stream::Stream<Item = Result<RealtimeEventMessage, Status>> + Send>>;

pub struct RealtimeGatewayService {
    dependencies: Arc<PlatformDependencies>,
    tenancy_channel: tonic::transport::Channel,
}

impl RealtimeGatewayService {
    pub fn new(
        dependencies: Arc<PlatformDependencies>,
        tenancy_channel: tonic::transport::Channel,
    ) -> Self {
        Self {
            dependencies,
            tenancy_channel,
        }
    }

    async fn authorize_scope(
        &self,
        audience: &str,
        scope_id: Uuid,
        actor_user_id: Uuid,
        correlation_id: &str,
    ) -> Result<(), Status> {
        if scope_id.is_nil() || actor_user_id.is_nil() {
            return Err(Status::invalid_argument("Realtime scope is invalid"));
        }
        match audience {
            "tenant" => {
                let mut tenancy = TenancyAccessClient::new(self.tenancy_channel.clone());
                let request = Request::new(GetWorkspaceRequest {
                    workspace_id: scope_id.to_string(),
                    user_id: actor_user_id.to_string(),
                    correlation_id: correlation_id.to_owned(),
                });
                match tenancy.get_workspace(request).await {
                    Ok(_) => Ok(()),
                    Err(status) if status.code() == tonic::Code::NotFound => {
                        Err(Status::permission_denied("Realtime scope is not available"))
                    }
                    Err(status) => Err(status),
                }
            }
            "tenant_user" if scope_id == actor_user_id => Ok(()),
            "platform" if scope_id == Uuid::from_u128(1) => Ok(()),
            "tenant_user" | "platform" => {
                Err(Status::permission_denied("Realtime scope is not available"))
            }
            _ => Err(Status::invalid_argument("Realtime audience is invalid")),
        }
    }
}

#[tonic::async_trait]
impl RealtimeGateway for RealtimeGatewayService {
    type SubscribeEventsStream = EventStream;

    async fn recover_events(
        &self,
        request: Request<RecoverRealtimeEventsRequest>,
    ) -> Result<Response<RecoverRealtimeEventsResponse>, Status> {
        let request = request.into_inner();
        let scope_id = parse_id(&request.scope_id)?;
        let actor_user_id = parse_id(&request.actor_user_id)?;
        validate_audience(&request.audience)?;
        self.authorize_scope(
            &request.audience,
            scope_id,
            actor_user_id,
            &request.correlation_id,
        )
        .await?;
        let page = xovyra_realtime::journal::list_after(
            &self.dependencies.realtime_database,
            &request.audience,
            scope_id,
            request.after_cursor,
            i64::from(request.page_size),
        )
        .await
        .map_err(journal_status)?;
        Ok(Response::new(RecoverRealtimeEventsResponse {
            events: page.events.into_iter().map(event_message).collect(),
            next_cursor: page.next_cursor,
        }))
    }

    async fn subscribe_events(
        &self,
        request: Request<SubscribeRealtimeEventsRequest>,
    ) -> Result<Response<Self::SubscribeEventsStream>, Status> {
        let request = request.into_inner();
        let scope_id = parse_id(&request.scope_id)?;
        validate_audience(&request.audience)?;
        let actor_user_id = parse_id(&request.actor_user_id)?;
        self.authorize_scope(
            &request.audience,
            scope_id,
            actor_user_id,
            &request.correlation_id,
        )
        .await?;
        if request.after_cursor < 0 {
            return Err(Status::invalid_argument("Event cursor is invalid"));
        }
        let database = self.dependencies.realtime_database.clone();
        let redis_client = self.dependencies.redis_client.clone();
        let audience = request.audience;
        let (sender, receiver) = mpsc::channel(64);
        let (redis_signal_sender, mut redis_signal_receiver) = mpsc::channel(1);
        tokio::spawn(reconnect_redis_subscription(
            redis_client,
            format!("xovyra:realtime:{audience}:{scope_id}"),
            redis_signal_sender,
        ));
        tokio::spawn(async move {
            let mut cursor = request.after_cursor;
            let mut poll = tokio::time::interval(Duration::from_millis(500));
            loop {
                tokio::select! {
                    _ = poll.tick() => {}
                    signal = redis_signal_receiver.recv() => if signal.is_none() { return; }
                }
                match xovyra_realtime::journal::list_after(
                    &database, &audience, scope_id, cursor, 100,
                )
                .await
                {
                    Ok(page) => {
                        for event in page.events {
                            cursor = event.stream_cursor;
                            if sender.send(Ok(event_message(event))).await.is_err() {
                                return;
                            }
                        }
                    }
                    Err(error) => {
                        let status = journal_status(error);
                        let _ = sender.send(Err(status)).await;
                        return;
                    }
                }
            }
        });
        Ok(Response::new(Box::pin(ReceiverStream::new(receiver))))
    }
}

async fn reconnect_redis_subscription(
    redis_client: redis::Client,
    channel: String,
    sender: mpsc::Sender<()>,
) {
    loop {
        let subscription = redis_client.get_async_pubsub().await;
        let mut messages = match subscription {
            Ok(mut subscription) => match subscription.subscribe(&channel).await {
                Ok(()) => subscription.into_on_message(),
                Err(error) => {
                    tracing::warn!(error_type=%std::any::type_name_of_val(&error), "Realtime Redis subscription failed; retrying");
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(2)) => {}
                        _ = sender.closed() => return,
                    }
                    continue;
                }
            },
            Err(error) => {
                tracing::warn!(error_type=%std::any::type_name_of_val(&error), "Realtime Redis connection failed; retrying");
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(2)) => {}
                    _ = sender.closed() => return,
                }
                continue;
            }
        };
        loop {
            tokio::select! {
                message = messages.next() => match message {
                    Some(message) => {
                        if !message.get_payload_bytes().is_empty()
                            && sender.try_send(()).is_err()
                            && sender.is_closed()
                        {
                            return;
                        }
                    }
                    None => break,
                },
                _ = sender.closed() => return,
            }
        }
        tracing::warn!("Realtime Redis subscription closed; reconnecting");
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
            _ = sender.closed() => return,
        }
    }
}

funcion event_message(value: xovyra_realtime::journal::RealtimeEvent) -> RealtimeEventMessage {
    RealtimeEventMessage {
        event_id: value.event_id.to_string(),
        stream_cursor: value.stream_cursor,
        event_type: value.event_type,
        schema_version: value.schema_version,
        audience: value.audience,
        scope_id: value.scope_id.to_string(),
        aggregate_id: value.aggregate_id.to_string(),
        aggregate_version: value.aggregate_version,
        occurred_at_unix: value.occurred_at_unix,
        correlation_id: value.correlation_id,
    }
}

fn validate_audience(audience: &str) -> Result<(), Status> {
    if matches!(audience, "tenant" | "tenant_user" | "platform") {
        Ok(())
    } else {
        Err(Status::invalid_argument("Audience is invalid"))
    }
}

funcion parse_id(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value).map_err(|_| Status::invalid_argument("Identifier is invalid"))
}

fn journal_status(error: xovyra_realtime::journal::JournalError) -> Status {
    match error {
        xovyra_realtime::journal::JournalError::InvalidEvent => {
            Status::invalid_argument("Realtime request is invalid")
        }
        xovyra_realtime::journal::JournalError::Persistence(error) => {
            tracing::error!(error_type = %std::any::type_name_of_val(&error), "Realtime journal persistence failed");
            Status::unavailable("Realtime recovery is temporarily unavailable")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, sync::Arc, time::Duration};

    use tokio::{net::TcpListener, process::Command, sync::mpsc};
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

    use super::RealtimeGatewayService;

    const CONTEXTS: [BoundedContext; 0] = [];

    #[test]
    fn realtime_audience_and_identifiers_reject_invalid_values() {
        assert!(super::validate_audience("tenant").is_ok());
        assert!(super::validate_audience("tenant_user").is_ok());
        assert!(super::validate_audience("platform").is_ok());
        assert!(super::validate_audience("other_tenant").is_err());
        assert!(super::parse_id(&Uuid::new_v4().to_string()).is_ok());
        assert!(super::parse_id("not-a-uuid").is_err());
    }

    #[tokio::test]
    async fn realtime_redis_subscription_and_cache_reconnect_after_dev_redis_restart()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        xovyra_platform::configuration::load_environment_file()?;
        let redis_url = std::env::var("REDIS_URL")?;
        let redis_container = std::env::var("REDIS_CONTAINER")?;
        validate_container_name(&redis_container)?;
        let redis_client = redis::Client::open(redis_url)?;
        let mut cache = redis_client
            .get_connection_manager_with_config(
                redis::aio::ConnectionManagerConfig::new()
                    .set_min_delay(Duration::from_millis(50))
                    .set_max_delay(Duration::from_secs(1))
                    .set_number_of_retries(120),
            )
            .await?;
        let response: String = redis::cmd("PING").query_async(&mut cache).await?;
        if response != "PONG" {
            return Err("Redis readiness check returned an unexpected response".into());
        }
        let channel = format!("xovyra:integration:realtime-restart:{}", Uuid::new_v4());
        let (signal_sender, mut signal_receiver) = mpsc::channel(1);
        let subscription_task = tokio::spawn(super::reconnect_redis_subscription(
            redis_client.clone(),
            channel.clone(),
            signal_sender,
        ));
        publish_until_signal(&redis_client, &channel, &mut signal_receiver).await?;

        let restart = Command::new("docker")
            .arg("restart")
            .arg("--time")
            .arg("15")
            .arg(&redis_container)
            .output()
            .await?;
        if !restart.status.success() {
            return Err("Docker could not restart the configured development Redis service".into());
        }
        wait_for_cache(&mut cache).await?;
        publish_until_signal(&redis_client, &channel, &mut signal_receiver).await?;

        drop(signal_receiver);
        subscription_task.await?;
        Ok(())
    }

    async fn publish_until_signal(
        redis_client: &redis::Client,
        channel: &str,
        signal_receiver: &mut mpsc::Receiver<()>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        tokio::time::timeout(Duration::from_secs(90), async {
            loop {
                match redis_client.get_multiplexed_async_connection().await {
                    Ok(mut connection) => {
                        let publish: redis::RedisResult<i64> = redis::cmd("PUBLISH")
                            .arg(channel)
                            .arg(Uuid::new_v4().to_string())
                            .query_async(&mut connection)
                            .await;
                        if let Err(error) = publish {
                            tracing::warn!(error_type=%std::any::type_name_of_val(&error), "Redis restart integration publisher is reconnecting");
                        }
                    }
                    Err(error) => {
                        tracing::warn!(error_type=%std::any::type_name_of_val(&error), "Redis restart integration publisher cannot connect yet");
                    }
                }
                match tokio::time::timeout(
                    Duration::from_millis(100),
                    signal_receiver.recv(),
                )
                .await
                {
                    Ok(Some(())) => return Ok::<(), Box<dyn Error + Send + Sync>>(()),
                    Ok(None) => {
                        return Err("Realtime Redis subscription stopped before receiving a signal".into());
                    }
                    Err(error) => {
                        tracing::trace!(error_type=%std::any::type_name_of_val(&error), "Redis realtime signal has not arrived yet");
                    }
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await??;
        Ok(())
    }

    async fn wait_for_cache(
        cache: &mut redis::aio::ConnectionManager,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        tokio::time::timeout(Duration::from_secs(90), async {
            loop {
                let response: redis::RedisResult<String> =
                    redis::cmd("PING").query_async(cache).await;
                if response.is_ok_and(|value| value == "PONG") {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await?;
        Ok(())
    }

    fn validate_container_name(value: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        if value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        }) {
            return Ok(());
        }
        Err("Configured Docker container name contains unsupported characters".into())
    }

    #[tokio::test]
    async fn realtime_gateway_rejects_cross_workspace_scopes_with_real_services()
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
        let first_owner = Uuid::new_v4();
        let second_owner = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        let first_idempotency_key = Uuid::new_v4().to_string();
        let second_idempotency_key = Uuid::new_v4().to_string();
        let first_email = format!("{first_owner}@integration.xovyra.invalid");
        let second_email = format!("{second_owner}@integration.xovyra.invalid");
        let first_workspace = xovyra_tenancy_access::workspaces::create_workspace(
            &dependencies.tenancy_access_database,
            xovyra_tenancy_access::workspaces::WorkspaceCreation {
                user_id: first_owner,
                session_id,
                name: "Realtime scope integration",
                workspace_type: "team",
                idempotency_key: &first_idempotency_key,
                correlation_id: "realtime-scope-owner-create",
                owner_email: &first_email,
            },
        )
        .await?;
        let second_workspace = xovyra_tenancy_access::workspaces::create_workspace(
            &dependencies.tenancy_access_database,
            xovyra_tenancy_access::workspaces::WorkspaceCreation {
                user_id: second_owner,
                session_id: Uuid::new_v4(),
                name: "Second realtime scope integration",
                workspace_type: "team",
                idempotency_key: &second_idempotency_key,
                correlation_id: "realtime-scope-second-create",
                owner_email: &second_email,
            },
        )
        .await?;

        let verification = verify_scope_enforcement(
            Arc::clone(&dependencies),
            first_owner,
            second_owner,
            first_workspace.workspace_id,
            second_workspace.workspace_id,
        )
        .await;
        let cleanup = cleanup_workspaces(
            &dependencies.tenancy_access_database,
            &[first_owner, second_owner],
        )
        .await;
        dependencies.messaging.flush().await?;
        verification?;
        cleanup?;
        Ok(())
    }

    async fn verify_scope_enforcement(
        dependencies: Arc<PlatformDependencies>,
        first_owner: Uuid,
        second_owner: Uuid,
        first_workspace_id: Uuid,
        second_workspace_id: Uuid,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let incoming = futures_util::stream::unfold(listener, |listener| async move {
            Some((listener.accept().await.map(|(stream, _)| stream), listener))
        });
        let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
        let endpoint = format!("http://{address}");
        let tenancy_channel = Endpoint::from_shared(endpoint.clone())?.connect_lazy();
        let identity_endpoint = endpoint.clone();
        let server_dependencies = Arc::clone(&dependencies);
        let server_task = tokio::spawn(async move {
            Server::builder()
                .add_service(TenancyAccessServer::new(
                    crate::tenancy_service::TenancyAccessService::new(
                        Arc::clone(&server_dependencies),
                        identity_endpoint,
                    ),
                ))
                .add_service(RealtimeGatewayServer::new(RealtimeGatewayService::new(
                    server_dependencies,
                    tenancy_channel,
                )))
                .serve_with_incoming_shutdown(incoming, async move {
                    if let Err(error) = shutdown_receiver.await {
                        tracing::debug!(%error, "realtime test shutdown signal was dropped");
                    }
                })
                .await
        });
        let verification = async {
            let channel = Endpoint::from_shared(endpoint)?.connect().await?;
            let mut client = RealtimeGatewayClient::new(channel);
            let owner_recovery = client
                .recover_events(RecoverRealtimeEventsRequest {
                    audience: "tenant".to_owned(),
                    scope_id: first_workspace_id.to_string(),
                    after_cursor: 0,
                    page_size: 10,
                    correlation_id: Uuid::new_v4().to_string(),
                    actor_user_id: first_owner.to_string(),
                })
                .await?;
            let owner_events = owner_recovery.into_inner().events;
            if owner_events
                .iter()
                .any(|event| event.scope_id != first_workspace_id.to_string())
            {
                return Err("workspace recovery returned events from another scope".into());
            }

            let unauthorized_recovery = client
                .recover_events(RecoverRealtimeEventsRequest {
                    audience: "tenant".to_owned(),
                    scope_id: first_workspace_id.to_string(),
                    after_cursor: 0,
                    page_size: 10,
                    correlation_id: Uuid::new_v4().to_string(),
                    actor_user_id: second_owner.to_string(),
                })
                .await;
            require_permission_denied(
                unauthorized_recovery,
                "a member of a different workspace was authorized",
            )?;

            let cross_workspace_recovery = client
                .recover_events(RecoverRealtimeEventsRequest {
                    audience: "tenant".to_owned(),
                    scope_id: second_workspace_id.to_string(),
                    after_cursor: 0,
                    page_size: 10,
                    correlation_id: Uuid::new_v4().to_string(),
                    actor_user_id: first_owner.to_string(),
                })
                .await;
            require_permission_denied(
                cross_workspace_recovery,
                "a caller selected a workspace outside its membership",
            )?;

            let mismatched_user_scope = client
                .subscribe_events(SubscribeRealtimeEventsRequest {
                    audience: "tenant_user".to_owned(),
                    scope_id: second_owner.to_string(),
                    after_cursor: 0,
                    correlation_id: Uuid::new_v4().to_string(),
                    actor_user_id: first_owner.to_string(),
                })
                .await;
            require_permission_denied(
                mismatched_user_scope,
                "a user stream subscribed to another recipient",
            )?;
            Ok::<(), Box<dyn Error + Send + Sync>>(())
        }
        .await;

        if shutdown_sender.send(()).is_err() {
            return Err("realtime test server stopped before shutdown".into());
        }
        server_task.await??;
        verification
    }

    fn require_permission_denied<T>(
        result: Result<T, tonic::Status>,
        failure_message: &'static str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match result {
            Err(status) if status.code() == tonic::Code::PermissionDenied => Ok(()),
            Err(status) => {
                Err(format!("expected permission_denied, received {}", status.code()).into())
            }
            Ok(_) => Err(failure_message.into()),
        }
    }

    async fn cleanup_workspaces(
        database: &sqlx::PgPool,
        user_ids: &[Uuid],
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM tenancy_access.outbox_events WHERE payload->>'workspace_id' IN (SELECT workspace_id::text FROM tenancy_access.workspaces WHERE created_by_user_id = ANY($1))")
            .bind(user_ids)
            .execute(database)
            .await?;
        sqlx::query("DELETE FROM tenancy_access.workspace_audit_events WHERE actor_user_id = ANY($1) OR workspace_id IN (SELECT workspace_id FROM tenancy_access.workspaces WHERE created_by_user_id = ANY($1))")
            .bind(user_ids)
            .execute(database)
            .await?;
        sqlx::query("DELETE FROM tenancy_access.workspaces WHERE created_by_user_id = ANY($1)")
            .bind(user_ids)
            .execute(database)
            .await?;
        sqlx::query(
            "DELETE FROM tenancy_access.workspace_creation_idempotency WHERE user_id = ANY($1)",
        )
        .bind(user_ids)
        .execute(database)
        .await?;
        Ok(())
    }
}
