use async_nats::jetstream::{self, consumer::pull::Config, message::AckKind};
use futures_util::StreamExt;
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;
use tonic::Request;
use xovyra_platform::{
    dependencies::PlatformDependencies,
    grpc::{
        CompleteWorkspaceInvitationDeliveryRequest, GetWorkspaceInvitationDeliveryRequest,
        SendEmailRequest, notifications_delivery_client::NotificationsDeliveryClient,
        tenancy_teams_client::TenancyTeamsClient,
    },
};

pub async fn run(
    dependencies: Arc<PlatformDependencies>,
    runtime_endpoint: String,
    public_app_origin: String,
    mut shutdown: watch::Receiver<bool>,
) {
    let stream = loop {
        match dependencies
            .jetstream
            .get_stream("TENANCY_ACCESS_EVENTS")
            .await
        {
            Ok(stream) => break stream,
            Err(error) => {
                tracing::error!(error_type=%std::any::type_name_of_val(&error),"Invitation worker cannot find tenancy event stream");
                tokio::select! {_=tokio::time::sleep(Duration::from_secs(3))=>{},_=shutdown.changed()=>return}
            }
        }
    };
    let consumer = loop {
        match stream
            .get_or_create_consumer(
                "xovyra-workspace-invitation-mailer",
                Config {
                    durable_name: Some("xovyra-workspace-invitation-mailer".to_owned()),
                    ack_policy: jetstream::consumer::AckPolicy::Explicit,
                    filter_subject: "tenancy_access.invitation.requested.v1".to_owned(),
                    max_deliver: 5,
                    ..Default::default()
                },
            )
            .await
        {
            Ok(consumer) => break consumer,
            Err(error) => {
                tracing::error!(error_type=%std::any::type_name_of_val(&error),"Invitation worker cannot create durable consumer");
                tokio::select! {_=tokio::time::sleep(Duration::from_secs(3))=>{},_=shutdown.changed()=>return}
            }
        }
    };
    let mut messages = match consumer.messages().await {
        Ok(messages) => messages,
        Err(error) => {
            tracing::error!(error_type=%std::any::type_name_of_val(&error),"Invitation worker cannot receive durable messages");
            return;
        }
    };
    loop {
        tokio::select! {item=messages.next()=>match item{Some(Ok(message))=>{if process(&runtime_endpoint,&public_app_origin,&message.payload).await{if let Err(error)=message.ack().await{tracing::warn!(error_type=%std::any::type_name_of_val(&error),"Invitation event acknowledgement failed")}}else if let Err(error)=message.ack_with(AckKind::Nak(Some(Duration::from_secs(10)))).await{tracing::warn!(error_type=%std::any::type_name_of_val(&error),"Invitation event retry request failed")}},Some(Err(error))=>tracing::warn!(error_type=%std::any::type_name_of_val(&error),"Invitation event delivery stream failed"),None=>{tokio::select!{_=tokio::time::sleep(Duration::from_secs(2))=>{},_=shutdown.changed()=>return}}},_=shutdown.changed()=>return}
    }
}

async fn process(endpoint: &str, origin: &str, payload: &[u8]) -> bool {
    let event: Value = match serde_json::from_slice(payload) {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(error_type=%std::any::type_name_of_val(&error),"Invitation event payload is invalid");
            return true;
        }
    };
    let Some(event_id) = event.get("event_id").and_then(Value::as_str) else {
        tracing::error!("Invitation event has no event identifier");
        return true;
    };
    let correlation_id = event
        .get("correlation_id")
        .and_then(Value::as_str)
        .or_else(|| {
            event
                .pointer("/payload/correlation_id")
                .and_then(Value::as_str)
        })
        .filter(|value| !value.trim().is_empty() && value.len() <= 128)
        .unwrap_or(event_id)
        .to_owned();
    let Some(id) = event
        .pointer("/payload/invitation_id")
        .and_then(Value::as_str)
    else {
        tracing::error!("Invitation event has no invitation identifier");
        return true;
    };
    let channel = match tonic::transport::Endpoint::from_shared(endpoint.to_owned()) {
        Ok(endpoint) => endpoint.connect_lazy(),
        Err(_) => return false,
    };
    let delivery = match TenancyTeamsClient::new(channel.clone())
        .get_workspace_invitation_delivery(Request::new(GetWorkspaceInvitationDeliveryRequest {
            invitation_id: id.to_owned(),
            correlation_id: correlation_id.clone(),
        }))
        .await
    {
        Ok(result) => result.into_inner(),
        Err(status) if status.code() == tonic::Code::NotFound => return true,
        Err(status) if status.code() == tonic::Code::PermissionDenied => return true,
        Err(_) => return false,
    };
    let url = format!(
        "{}/invitation/accept#token={}",
        origin.trim_end_matches('/'),
        delivery.token
    );
    let subject = format!("Invitación a {} en XOVYRA", delivery.workspace_name);
    let safe_name = escape_html(&delivery.workspace_name);
    let safe_url = escape_html(&url);
    let html = format!(
        "<!doctype html><html lang=\"es\"><body style=\"margin:0;background:#f4f5f8;font-family:Arial,sans-serif;color:#172033\"><main style=\"max-width:560px;margin:32px auto;background:#fff;border-radius:16px;padding:32px\"><p style=\"font-weight:700;letter-spacing:.12em;color:#6d4aff\">XOVYRA</p><h1 style=\"font-size:24px\">Te invitaron a {safe_name}</h1><p>Una persona del equipo te invitó a colaborar en este espacio de trabajo.</p><p style=\"margin:28px 0\"><a href=\"{safe_url}\" style=\"background:#6d4aff;color:#fff;padding:13px 20px;border-radius:9px;text-decoration:none;font-weight:700\">Aceptar invitación</a></p><p>El enlace caduca en siete días y solo puede aceptarse con la cuenta invitada.</p><p style=\"color:#687084;font-size:13px\">Si no esperabas esta invitación, puedes ignorar este mensaje.</p></main></body></html>"
    );
    let text = format!(
        "Te invitaron a colaborar en {workspace}. Acepta la invitación aquí: {url}. El enlace caduca en siete días y solo funciona con la cuenta invitada.",
        workspace = delivery.workspace_name,
        url = url
    );
    let send = NotificationsDeliveryClient::new(channel.clone())
        .send_email(Request::new(SendEmailRequest {
            recipient: delivery.email,
            subject,
            html,
            text,
            idempotency_key: format!("workspace-invitation-{}", delivery.invitation_id),
            correlation_id: correlation_id.clone(),
        }))
        .await;
    let sent = send.is_ok();
    let marked = TenancyTeamsClient::new(channel)
        .complete_workspace_invitation_delivery(Request::new(
            CompleteWorkspaceInvitationDeliveryRequest {
                invitation_id: delivery.invitation_id,
                sent,
                correlation_id,
            },
        ))
        .await;
    if marked.is_err() {
        return false;
    }
    sent
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use std::{error::Error, sync::Arc, time::Duration};

    use async_nats::jetstream::{self, consumer::pull::Config, message::AckKind};
    use futures_util::StreamExt;
    use futures_util::stream;
    use sqlx::Row;
    use tokio::net::TcpListener;
    use tonic::transport::Server;
    use uuid::Uuid;
    use xovyra_platform::{
        dependencies::{DependencyConfiguration, PlatformDependencies},
        grpc::tenancy_teams_server::TenancyTeamsServer,
    };
    use xovyra_shared_kernel::BoundedContext;
    use xovyra_tenancy_access::teams::{
        invitations::{self, InvitationCreation},
        models::Invitation,
        token::InvitationTokenSigner,
    };

    use super::process;
    use crate::tenancy_teams_service::TenancyTeamsService;

    #[cfg(feature = "real-resend-tests")]
    use crate::notifications_service::NotificationsDeliveryService;
    #[cfg(feature = "real-resend-tests")]
    use xovyra_platform::grpc::notifications_delivery_server::NotificationsDeliveryServer;

    const CONTEXTS: [BoundedContext; 1] = [BoundedContext {
        name: "tenancy_access",
        schema: "tenancy_access",
    }];

    #[tokio::test]
    async fn invitation_worker_acknowledges_revocation_and_retries_unavailable_mailer_to_terminal_state()
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
        let workspace_key = Uuid::new_v4().to_string();
        let owner_email = format!("{owner_user_id}@integration.xovyra.invalid");
        let workspace = xovyra_tenancy_access::workspaces::create_workspace(
            &dependencies.tenancy_access_database,
            xovyra_tenancy_access::workspaces::WorkspaceCreation {
                user_id: owner_user_id,
                session_id,
                name: "Invitation worker lifecycle integration",
                workspace_type: "team",
                idempotency_key: &workspace_key,
                correlation_id: "invitation-worker-integration",
                owner_email: &owner_email,
            },
        )
        .await?;
        let signer = InvitationTokenSigner::new(Uuid::new_v4().to_string())?;
        let revoked_invitation = create_invitation(
            &dependencies,
            &signer,
            workspace.workspace_id,
            owner_user_id,
            &format!("revoked-{}@integration.xovyra.invalid", Uuid::new_v4()),
            "invitation-worker-revoked",
        )
        .await?;
        invitations::revoke(
            &dependencies.tenancy_access_database,
            workspace.workspace_id,
            owner_user_id,
            revoked_invitation.invitation_id,
            "invitation-worker-revoked-before-send",
        )
        .await?;
        let retry_invitation = create_invitation(
            &dependencies,
            &signer,
            workspace.workspace_id,
            owner_user_id,
            &format!("retry-{}@integration.xovyra.invalid", Uuid::new_v4()),
            "invitation-worker-provider-retry",
        )
        .await?;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let incoming = stream::unfold(listener, |listener| async move {
            let accepted = listener.accept().await;
            Some((accepted.map(|connection| connection.0), listener))
        });
        let service = TenancyTeamsService::new(
            Arc::clone(&dependencies),
            Uuid::new_v4().to_string(),
            tonic::transport::Endpoint::from_static("http://127.0.0.1:1").connect_lazy(),
        )
        .map_err(std::io::Error::other)?;
        let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            Server::builder()
                .add_service(TenancyTeamsServer::new(service))
                .serve_with_incoming_shutdown(incoming, async move {
                    if let Err(error) = shutdown_receiver.await {
                        tracing::debug!(%error, "Invitation integration server shutdown signal was dropped");
                    }
                })
                .await
        });

        let verification: Result<(), Box<dyn Error + Send + Sync>> = async {
            let stream = dependencies
                .jetstream
                .get_stream("TENANCY_ACCESS_EVENTS")
                .await?;
            let consumer_name = format!("xovyra-invitation-test-{}", Uuid::new_v4().simple());
            let consumer = stream
                .create_consumer(Config {
                    durable_name: Some(consumer_name.clone()),
                    ack_policy: jetstream::consumer::AckPolicy::Explicit,
                    deliver_policy: jetstream::consumer::DeliverPolicy::New,
                    filter_subject: "tenancy_access.invitation.requested.v1".to_owned(),
                    max_deliver: 10,
                    ..Default::default()
                })
                .await?;
            let mut messages = consumer.messages().await?;
            let revoked_event = invitation_event(
                &dependencies.tenancy_access_database,
                revoked_invitation.invitation_id,
            )
            .await?;
            let revoked_payload = serde_json::to_vec(&revoked_event)?;
            dependencies
                .jetstream
                .publish(
                    "tenancy_access.invitation.requested.v1".to_owned(),
                    revoked_payload.clone().into(),
                )
                .await?
                .await?;
            let revoked_message = tokio::time::timeout(Duration::from_secs(5), messages.next())
                .await?
                .ok_or("JetStream did not deliver the revoked invitation")??;
            if !process(&endpoint, "https://app-xovyra.jnmtortas.com", &revoked_message.payload).await {
                return Err(Box::<dyn Error + Send + Sync>::from(
                    std::io::Error::other(
                        "revoked invitation event was not terminally acknowledged",
                    ),
                ));
            }
            revoked_message.ack().await?;

            let retry_event = invitation_event(
                &dependencies.tenancy_access_database,
                retry_invitation.invitation_id,
            )
            .await?;
            let retry_payload = serde_json::to_vec(&retry_event)?;
            dependencies
                .jetstream
                .publish(
                    "tenancy_access.invitation.requested.v1".to_owned(),
                    retry_payload.into(),
                )
                .await?
                .await?;
            for attempt in 1..=6 {
                let retry_message = tokio::time::timeout(Duration::from_secs(5), messages.next())
                    .await?
                    .ok_or("JetStream did not redeliver the invitation event")??;
                let terminal = process(
                    &endpoint,
                    "https://app-xovyra.jnmtortas.com",
                    &retry_message.payload,
                )
                .await;
                if terminal {
                    if attempt != 6 {
                        return Err(Box::<dyn Error + Send + Sync>::from(
                            std::io::Error::other(format!(
                                "invitation worker acknowledged retry {attempt} before terminal delivery failure"
                            )),
                        ));
                    }
                    retry_message.ack().await?;
                    break;
                }
                if attempt == 6 {
                    return Err(Box::<dyn Error + Send + Sync>::from(
                        std::io::Error::other(
                            "invitation worker did not acknowledge the terminal delivery failure",
                        ),
                    ));
                }
                retry_message
                    .ack_with(AckKind::Nak(Some(Duration::from_millis(20))))
                    .await?;
            }
            let status: String = sqlx::query_scalar("SELECT delivery_status FROM tenancy_access.workspace_invitations WHERE invitation_id=$1")
                .bind(retry_invitation.invitation_id)
                .fetch_one(&dependencies.tenancy_access_database)
                .await?;
            if status != "failed" {
                return Err(Box::<dyn Error + Send + Sync>::from(
                    std::io::Error::other(
                        "invitation delivery did not reach its terminal failed state",
                    ),
                ));
            }
            drop(messages);
            stream.delete_consumer(&consumer_name).await?;
            Ok::<(), Box<dyn Error + Send + Sync>>(())
        }
        .await;

        if shutdown_sender.send(()).is_err() {
            tracing::debug!("Invitation integration server stopped before shutdown signal");
        }
        let server_result = server.await?;
        let cleanup = cleanup_invitation_workspace(
            &dependencies,
            workspace.workspace_id,
            owner_user_id,
            &workspace_key,
        )
        .await;
        verification?;
        server_result?;
        cleanup?;
        Ok(())
    }

    #[cfg(feature = "real-resend-tests")]
    #[tokio::test]
    async fn invitation_worker_delivers_email_through_real_resend_and_persists_success()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        xovyra_platform::configuration::load_environment_file()?;
        let recipient = std::env::var("RESEND_TEST_RECIPIENT")?;
        if recipient.trim().is_empty() || !recipient.contains('@') {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "RESEND_TEST_RECIPIENT must be an explicitly configured email address",
            )
            .into());
        }
        let resend_api_key = std::env::var("RESEND_API_KEY")?;
        let resend_from_email = std::env::var("RESEND_FROM_EMAIL")?;
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
        let invitation_signing_key = std::env::var("TENANCY_INVITATION_TOKEN_SECRET")?;
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
        let workspace_key = Uuid::new_v4().to_string();
        let owner_email = format!("{owner_user_id}@integration.xovyra.invalid");
        let workspace = xovyra_tenancy_access::workspaces::create_workspace(
            &dependencies.tenancy_access_database,
            xovyra_tenancy_access::workspaces::WorkspaceCreation {
                user_id: owner_user_id,
                session_id,
                name: "Resend invitation delivery integration",
                workspace_type: "team",
                idempotency_key: &workspace_key,
                correlation_id: "resend-invitation-integration",
                owner_email: &owner_email,
            },
        )
        .await?;
        let signer = InvitationTokenSigner::new(invitation_signing_key.clone())?;
        let invitation = create_invitation(
            &dependencies,
            &signer,
            workspace.workspace_id,
            owner_user_id,
            &recipient,
            "resend-invitation-delivery-integration",
        )
        .await?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let incoming = stream::unfold(listener, |listener| async move {
            let accepted = listener.accept().await;
            Some((accepted.map(|connection| connection.0), listener))
        });
        let identity_channel =
            tonic::transport::Endpoint::from_static("http://127.0.0.1:1").connect_lazy();
        let team_service = TenancyTeamsService::new(
            Arc::clone(&dependencies),
            invitation_signing_key,
            identity_channel.clone(),
        )
        .map_err(std::io::Error::other)?;
        let notification_service = NotificationsDeliveryService::new(
            Some(resend_api_key),
            Some(resend_from_email),
            None,
            identity_channel,
        )?;
        let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            Server::builder()
                .add_service(TenancyTeamsServer::new(team_service))
                .add_service(NotificationsDeliveryServer::new(notification_service))
                .serve_with_incoming_shutdown(incoming, async move {
                    if let Err(error) = shutdown_receiver.await {
                        tracing::debug!(%error, "Resend integration server shutdown signal was dropped");
                    }
                })
                .await
        });

        let verification: Result<(), Box<dyn Error + Send + Sync>> = async {
            let stream = dependencies
                .jetstream
                .get_stream("TENANCY_ACCESS_EVENTS")
                .await?;
            let consumer_name = format!("xovyra-resend-test-{}", Uuid::new_v4().simple());
            let consumer = stream
                .create_consumer(Config {
                    durable_name: Some(consumer_name.clone()),
                    ack_policy: jetstream::consumer::AckPolicy::Explicit,
                    deliver_policy: jetstream::consumer::DeliverPolicy::New,
                    filter_subject: "tenancy_access.invitation.requested.v1".to_owned(),
                    max_deliver: 3,
                    ..Default::default()
                })
                .await?;
            let mut messages = consumer.messages().await?;
            let event = invitation_event(
                &dependencies.tenancy_access_database,
                invitation.invitation_id,
            )
            .await?;
            dependencies
                .jetstream
                .publish(
                    "tenancy_access.invitation.requested.v1".to_owned(),
                    serde_json::to_vec(&event)?.into(),
                )
                .await?
                .await?;
            let message = tokio::time::timeout(Duration::from_secs(10), messages.next())
                .await?
                .ok_or("JetStream did not deliver the Resend invitation event")??;
            if !process(&endpoint, "https://app-xovyra.jnmtortas.com", &message.payload).await {
                return Err(std::io::Error::other(
                    "invitation worker did not complete successful Resend delivery",
                )
                .into());
            }
            message.ack().await?;
            let (delivery_status, delivery_attempts): (String, i32) = sqlx::query_as(
                "SELECT delivery_status, delivery_attempts FROM tenancy_access.workspace_invitations WHERE invitation_id=$1",
            )
            .bind(invitation.invitation_id)
            .fetch_one(&dependencies.tenancy_access_database)
            .await?;
            if delivery_status != "sent" || delivery_attempts != 1 {
                return Err(std::io::Error::other(
                    "Resend success was not persisted exactly once for the invitation",
                )
                .into());
            }
            drop(messages);
            stream.delete_consumer(&consumer_name).await?;
            Ok(())
        }
        .await;

        if shutdown_sender.send(()).is_err() {
            tracing::debug!("Resend integration server stopped before shutdown signal");
        }
        let server_result = server.await?;
        let cleanup = cleanup_invitation_workspace(
            &dependencies,
            workspace.workspace_id,
            owner_user_id,
            &workspace_key,
        )
        .await;
        verification?;
        server_result?;
        cleanup?;
        Ok(())
    }

    async fn create_invitation(
        dependencies: &PlatformDependencies,
        signer: &InvitationTokenSigner,
        workspace_id: Uuid,
        actor: Uuid,
        email: &str,
        correlation_id: &str,
    ) -> Result<Invitation, xovyra_tenancy_access::teams::models::TeamError> {
        let mut cache = dependencies.cache.clone();
        let idempotency_key = Uuid::new_v4().to_string();
        invitations::create(
            &dependencies.tenancy_access_database,
            &mut cache,
            signer,
            InvitationCreation {
                workspace_id,
                actor,
                email,
                invited_user_id: None,
                role_key: "editor",
                idempotency_key: &idempotency_key,
                correlation_id,
            },
        )
        .await
    }

    async fn invitation_event(
        database: &sqlx::PgPool,
        invitation_id: Uuid,
    ) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>> {
        let row = sqlx::query("SELECT event_id,event_type,event_version,occurred_at::text AS occurred_at,payload FROM tenancy_access.outbox_events WHERE payload->>'invitation_id'=$1 AND event_type='tenancy_access.invitation.requested' ORDER BY occurred_at DESC LIMIT 1")
            .bind(invitation_id.to_string())
            .fetch_one(database)
            .await?;
        let payload: serde_json::Value = row.try_get("payload")?;
        Ok(serde_json::json!({
            "event_id": row.try_get::<Uuid, _>("event_id")?,
            "event_type": row.try_get::<String, _>("event_type")?,
            "event_version": row.try_get::<i16, _>("event_version")?,
            "occurred_at": row.try_get::<String, _>("occurred_at")?,
            "correlation_id": payload.get("correlation_id"),
            "payload": payload,
        }))
    }

    async fn cleanup_invitation_workspace(
        dependencies: &PlatformDependencies,
        workspace_id: Uuid,
        owner_user_id: Uuid,
        idempotency_key: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM tenancy_access.outbox_events WHERE payload->>'workspace_id'=$1")
            .bind(workspace_id.to_string())
            .execute(&dependencies.tenancy_access_database)
            .await?;
        sqlx::query("DELETE FROM tenancy_access.workspace_audit_events WHERE workspace_id=$1")
            .bind(workspace_id)
            .execute(&dependencies.tenancy_access_database)
            .await?;
        sqlx::query("DELETE FROM tenancy_access.workspace_creation_idempotency WHERE user_id=$1 AND idempotency_key=$2")
            .bind(owner_user_id)
            .bind(idempotency_key)
            .execute(&dependencies.tenancy_access_database)
            .await?;
        sqlx::query("DELETE FROM tenancy_access.workspaces WHERE workspace_id=$1")
            .bind(workspace_id)
            .execute(&dependencies.tenancy_access_database)
            .await?;
        Ok(())
    }
}
