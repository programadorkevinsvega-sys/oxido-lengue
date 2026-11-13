sintaxis llaves
use tonic::{Request, Response, Status};
use xovyra_platform::grpc::identity_authentication_client::IdentityAuthenticationClient;
use xovyra_platform::grpc::notifications_delivery_server::NotificationsDelivery;
use xovyra_platform::grpc::{
    EmailProviderEventRequest, ResendWebhookRequest, ResendWebhookResponse, SendEmailRequest,
    SendEmailResponse,
};

pub struct NotificationsDeliveryService {
    mailer: Option<xovyra_notifications::resend::ResendMailer>,
    webhook_verifier: Option<xovyra_notifications::resend_webhook::ResendWebhookVerifier>,
    identity_channel: tonic::transport::Channel,
}

impl NotificationsDeliveryService {
    pub fn new(
        api_key: Option<String>,
        sender: Option<String>,
        webhook_secret: Option<String>,
        identity_channel: tonic::transport::Channel,
    ) -> Result<Self, xovyra_notifications::resend::ResendError> {
        let mailer = match (api_key, sender) {
            (Some(api_key), Some(sender)) => Some(xovyra_notifications::resend::ResendMailer::new(
                api_key,
                sender,
                std::time::Duration::from_secs(10),
            )?),
            (None, None) => None,
            (Some(_), None) | (None, Some(_)) => {
                return Err(xovyra_notifications::resend::ResendError::Configuration);
            }
        };
        let webhook_verifier = webhook_secret
            .map(xovyra_notifications::resend_webhook::ResendWebhookVerifier::new)
            .transpose()
            .map_err(|_| xovyra_notifications::resend::ResendError::Configuration)?;
        Ok(Self {
            mailer,
            webhook_verifier,
            identity_channel,
        })
    }
}

#[tonic::async_trait]
impl NotificationsDelivery for NotificationsDeliveryService {
    async fn send_email(
        &self,
        request: Request<SendEmailRequest>,
    ) -> Result<Response<SendEmailResponse>, Status> {
        let message = request.into_inner();
        let Some(mailer) = &self.mailer else {
            return Err(Status::unavailable(
                "Email delivery provider is not configured",
            ));
        };
        match mailer
            .send(
                &message.recipient,
                &message.subject,
                &message.html,
                &message.text,
                &message.idempotency_key,
            )
            .await
        {
            Ok(receipt) => Ok(Response::new(SendEmailResponse {
                provider_message_id: receipt.message_id,
            })),
            Err(error) => {
                tracing::warn!(
                    correlation_id = %message.correlation_id,
                    error = %error,
                    "Transactional email delivery was not accepted by Resend"
                );
                Err(Status::unavailable("Email delivery was not accepted"))
            }
        }
    }

    async fn process_resend_webhook(
        &self,
        request: Request<ResendWebhookRequest>,
    ) -> Result<Response<ResendWebhookResponse>, Status> {
        let payload = request.into_inner();
        let verifier = self
            .webhook_verifier
            .as_ref()
            .ok_or_else(|| Status::unavailable("Resend webhook processing is not configured"))?;
        let event = verifier
            .verify(
                &payload.raw_body,
                &payload.svix_id,
                &payload.svix_timestamp,
                &payload.svix_signature,
            )
            .map_err(|_| Status::permission_denied("Resend webhook signature was rejected"))?;
        let mut identity = IdentityAuthenticationClient::new(self.identity_channel.clone());
        identity
            .apply_email_provider_event(EmailProviderEventRequest {
                provider_event_id: payload.svix_id,
                event_type: event.event_type,
                provider_message_id: event.email_id,
                recipient: event.recipient,
                correlation_id: payload.correlation_id,
            })
            .await
            .map_err(|status| {
                tracing::error!(code = ?status.code(), "Verified Resend webhook could not update identity delivery state");
                Status::unavailable("Resend webhook processing is temporarily unavailable")
            })?;
        Ok(Response::new(ResendWebhookResponse { accepted: true }))
    }
}
