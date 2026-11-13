sintaxis llaves
use std::collections::HashMap;
use std::time::Duration;

use opentelemetry::trace::TraceContextExt;
use tonic::{Request, Response, Status};
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use xovyra_notifications::templates::transactional_email::TransactionalEmail;
use xovyra_platform::dependencies::PlatformDependencies;
use xovyra_platform::grpc::identity_authentication_server::IdentityAuthentication;
use xovyra_platform::grpc::notifications_delivery_client::NotificationsDeliveryClient;
use xovyra_platform::grpc::runtime_readiness_server::RuntimeReadiness;
use xovyra_platform::grpc::{
    AuthenticateSessionRequest, AuthenticatedSession, AuthenticationAudience, EmailActionRequest,
    EmailActionResponse, EmailProviderEventRequest, EmailProviderEventResponse, LoginRequest,
    LoginResponse, LogoutRequest, LogoutResponse, ReadinessRequest, ReadinessResponse,
    RefreshSessionRequest, RegisterRequest, RegistrationResponse, ResetPasswordRequest,
    ResetPasswordResponse, ResolveVerifiedAccountRequest, ResolveVerifiedAccountResponse,
    SendEmailRequest, SetMarketingConsentRequest, SetMarketingConsentResponse, VerifyEmailRequest,
    VerifyEmailResponse,
};

const DEPENDENCY_TIMEOUT: Duration = Duration::from_secs(2);

pub struct ReadinessService {
    dependencies: std::sync::Arc<PlatformDependencies>,
}

impl ReadinessService {
    pub fn new(dependencies: std::sync::Arc<PlatformDependencies>) -> Self {
        Self { dependencies }
    }
}

pub struct IdentityAuthenticationService {
    identity: xovyra_identity::authentication::IdentityService,
    identity_database: sqlx::PgPool,
    notifications_channel: tonic::transport::Channel,
    public_app_origin: String,
}

struct EmailDelivery<'message> {
    recipient: &'message str,
    display_name: &'message str,
    subject: &'message str,
    preheader: &'message str,
    title: &'message str,
    message: &'message str,
    verification_code: Option<&'message str>,
    action: Option<(&'message str, &'message str)>,
    footer_note: Option<&'message str>,
    idempotency_key: &'message str,
    correlation_id: &'message str,
}

impl IdentityAuthenticationService {
    pub fn new(
        dependencies: std::sync::Arc<PlatformDependencies>,
        rate_limit_hmac_secret: String,
        public_app_origin: String,
        notifications_channel: tonic::transport::Channel,
        idle_session_lifetime: Duration,
        absolute_session_lifetime: Duration,
    ) -> Result<Self, xovyra_identity::authentication::AuthenticationError> {
        Ok(Self {
            identity: xovyra_identity::authentication::IdentityService::new(
                dependencies.identity_database.clone(),
                dependencies.cache.clone(),
                rate_limit_hmac_secret,
                idle_session_lifetime,
                absolute_session_lifetime,
            )?,
            identity_database: dependencies.identity_database.clone(),
            notifications_channel,
            public_app_origin,
        })
    }

    async fn deliver_email(&self, delivery: EmailDelivery<'_>) -> Option<String> {
        let (html, text) = TransactionalEmail {
            display_name: delivery.display_name,
            preheader: delivery.preheader,
            title: delivery.title,
            message: delivery.message,
            verification_code: delivery.verification_code,
            action: delivery.action,
            footer_note: delivery.footer_note,
        }
        .render();
        let mut notifications =
            NotificationsDeliveryClient::new(self.notifications_channel.clone());
        let result = tokio::time::timeout(
            Duration::from_secs(12),
            notifications.send_email(SendEmailRequest {
                recipient: delivery.recipient.to_owned(),
                subject: delivery.subject.to_owned(),
                html,
                text,
                idempotency_key: delivery.idempotency_key.to_owned(),
                correlation_id: delivery.correlation_id.to_owned(),
            }),
        )
        .await;
        match result {
            Ok(Ok(response)) => Some(response.into_inner().provider_message_id),
            Ok(Err(status)) => {
                tracing::warn!(correlation_id = %delivery.correlation_id, code = ?status.code(), "Transactional email delivery was not accepted");
                None
            }
            Err(_) => {
                tracing::warn!(correlation_id = %delivery.correlation_id, "Transactional email delivery timed out");
                None
            }
        }
    }
}

#[tonic::async_trait]
impl IdentityAuthentication for IdentityAuthenticationService {
    async fn resolve_verified_account(
        &self,
        request: Request<ResolveVerifiedAccountRequest>,
    ) -> Result<Response<ResolveVerifiedAccountResponse>, Status> {
        let input = request.into_inner();
        let email = input.email.trim().to_lowercase();
        if email.is_empty() || email.len() > 320 {
            return Err(Status::invalid_argument("Email address is invalid"));
        }
        let user_id = sqlx::query_scalar::<_, uuid::Uuid>("SELECT user_id FROM identity.users WHERE email_normalized=$1 AND account_status='active' AND email_verified_at IS NOT NULL")
            .bind(email)
            .fetch_optional(&self.identity_database)
            .await
            .map_err(|error| {
                tracing::error!(correlation_id=%input.correlation_id, error_type=%std::any::type_name_of_val(&error), "Verified account lookup failed");
                Status::unavailable("Account lookup is temporarily unavailable")
            })?;
        Ok(Response::new(ResolveVerifiedAccountResponse {
            user_id: user_id.map(|value| value.to_string()),
        }))
    }

    async fn verify_email(
        &self,
        request: Request<VerifyEmailRequest>,
    ) -> Result<Response<VerifyEmailResponse>, Status> {
        let payload = request.into_inner();
        let account = self
            .identity
            .verify_email(
                &payload.token,
                &payload.email,
                &payload.code,
                &payload.source_address,
                &payload.correlation_id,
            )
            .await
            .map_err(authentication_status)?;
        if let Some(account) = account.as_ref() {
            self.deliver_email(EmailDelivery {
                recipient: &account.email,
                display_name: &account.display_name,
                subject: "Tu correo de XOVYRA ya está verificado",
                preheader: "Tu cuenta está activa. Ya puedes iniciar sesión.",
                title: "Tu correo ya está verificado",
                message: "La verificación terminó. Inicia sesión para continuar en XOVYRA.",
                verification_code: None,
                action: Some((
                    &format!("{}/login", self.public_app_origin.trim_end_matches('/')),
                    "Abrir XOVYRA",
                )),
                footer_note: None,
                idempotency_key: &format!("welcome/{}", account.user_id),
                correlation_id: &payload.correlation_id,
            })
            .await;
        }
        Ok(Response::new(VerifyEmailResponse {
            activated: account.is_some(),
        }))
    }

    async fn resend_verification(
        &self,
        request: Request<EmailActionRequest>,
    ) -> Result<Response<EmailActionResponse>, Status> {
        self.deliver_email_action(request.into_inner(), "email_verification")
            .await
    }

    async fn request_password_recovery(
        &self,
        request: Request<EmailActionRequest>,
    ) -> Result<Response<EmailActionResponse>, Status> {
        self.deliver_email_action(request.into_inner(), "password_recovery")
            .await
    }

    async fn reset_password(
        &self,
        request: Request<ResetPasswordRequest>,
    ) -> Result<Response<ResetPasswordResponse>, Status> {
        let payload = request.into_inner();
        let changed = self
            .identity
            .reset_password(
                &payload.token,
                &payload.new_password,
                &payload.correlation_id,
            )
            .await
            .map_err(authentication_status)?;
        Ok(Response::new(ResetPasswordResponse { changed }))
    }

    async fn apply_email_provider_event(
        &self,
        request: Request<EmailProviderEventRequest>,
    ) -> Result<Response<EmailProviderEventResponse>, Status> {
        let payload = request.into_inner();
        let processed = self
            .identity
            .apply_email_provider_event(
                &payload.provider_event_id,
                &payload.event_type,
                &payload.provider_message_id,
                &payload.recipient,
            )
            .await
            .map_err(authentication_status)?;
        Ok(Response::new(EmailProviderEventResponse { processed }))
    }

    async fn set_marketing_consent(
        &self,
        request: Request<SetMarketingConsentRequest>,
    ) -> Result<Response<SetMarketingConsentResponse>, Status> {
        let payload = request.into_inner();
        let updated = self
            .identity
            .set_marketing_consent(&payload.user_id, payload.consented, &payload.correlation_id)
            .await
            .map_err(authentication_status)?;
        Ok(Response::new(SetMarketingConsentResponse { updated }))
    }

    async fn register(
        &self,
        request: Request<RegisterRequest>,
    ) -> Result<Response<RegistrationResponse>, Status> {
        let payload = request.into_inner();
        let email = xovyra_identity::authentication::normalize_email(&payload.email);
        if !xovyra_identity::authentication::registration_input_valid(
            &email,
            &payload.password,
            &payload.display_name,
        ) {
            return Err(Status::invalid_argument("Registration details are invalid"));
        }
        let registration = self
            .identity
            .register(
                &email,
                &payload.password,
                &payload.display_name,
                payload.marketing_consent,
                &payload.source_address,
                &payload.correlation_id,
            )
            .await
            .map_err(authentication_status)?;
        let xovyra_identity::authentication::RegistrationResult::Created(pending) = registration
        else {
            return Ok(Response::new(RegistrationResponse {
                delivery_accepted: false,
                new_account: false,
            }));
        };
        let account_created_delivery = self
            .deliver_email(EmailDelivery {
                recipient: &pending.email,
                display_name: &pending.display_name,
                subject: "Tu cuenta de XOVYRA ya está creada",
                preheader: "Solo falta verificar el correo para activar el acceso.",
                title: "Cuenta creada",
                message: "Ya registramos tu cuenta. Para activar el acceso, confirma tu correo con el mensaje de verificación que enviamos por separado.",
                verification_code: None,
                action: None,
                footer_note: Some("Si no creaste esta cuenta, puedes ignorar este correo."),
                idempotency_key: &format!("account-created/{}", pending.user_id),
                correlation_id: &payload.correlation_id,
            })
            .await;
        if account_created_delivery.is_none() {
            tracing::warn!(correlation_id = %payload.correlation_id, "Account creation notice was not accepted by email provider");
        }
        let verification_url = format!(
            "{}/verify-email#token={}",
            self.public_app_origin.trim_end_matches('/'),
            pending.token
        );
        let delivery = self
            .deliver_email(EmailDelivery {
                recipient: &pending.email,
                display_name: &pending.display_name,
                subject: "Verifica tu correo de XOVYRA",
                preheader: "Confirma tu dirección de correo para activar tu cuenta.",
                title: "Confirma tu correo",
                message: "Confirma tu dirección de correo para activar tu cuenta.",
                verification_code: pending.verification_code.as_deref(),
                action: Some((&verification_url, "Verificar correo")),
                footer_note: Some("Si no creaste esta cuenta, puedes ignorar este mensaje."),
                idempotency_key: &format!("email-verification/{}", pending.token_id),
                correlation_id: &payload.correlation_id,
            })
            .await;
        self.identity
            .set_email_delivery(
                pending.token_id,
                delivery.is_some(),
                delivery.as_deref(),
                &payload.correlation_id,
            )
            .await
            .map_err(authentication_status)?;
        Ok(Response::new(RegistrationResponse {
            delivery_accepted: delivery.is_some(),
            new_account: true,
        }))
    }

    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let payload = request.into_inner();
        let correlation_id = payload.correlation_id;
        let audience = authentication_audience(payload.audience)?;
        let session = self
            .identity
            .login(
                &payload.email,
                &payload.password,
                audience,
                &payload.source_address,
                &correlation_id,
            )
            .await
            .map_err(authentication_status)?;
        Ok(Response::new(LoginResponse {
            session_id: session.session_id.to_string(),
            session_secret: session.session_secret,
            csrf_secret: session.csrf_secret,
            user_id: session.user_id.to_string(),
            idle_expires_at_unix: session.idle_expires_at,
            absolute_expires_at_unix: session.absolute_expires_at,
        }))
    }

    async fn authenticate_session(
        &self,
        request: Request<AuthenticateSessionRequest>,
    ) -> Result<Response<AuthenticatedSession>, Status> {
        let payload = request.into_inner();
        let audience = authentication_audience(payload.audience)?;
        let session = self
            .identity
            .authenticate_session(&payload.session_secret, &payload.csrf_secret, audience)
            .await
            .map_err(authentication_status)?;
        Ok(Response::new(AuthenticatedSession {
            session_id: session.session_id.to_string(),
            user_id: session.user_id.to_string(),
            idle_expires_at_unix: session.idle_expires_at,
            absolute_expires_at_unix: session.absolute_expires_at,
            email: session.email,
            email_verified: session.email_verified,
        }))
    }

    async fn logout(
        &self,
        request: Request<LogoutRequest>,
    ) -> Result<Response<LogoutResponse>, Status> {
        let payload = request.into_inner();
        let audience = authentication_audience(payload.audience)?;
        let revoked = self
            .identity
            .logout(
                &payload.session_secret,
                &payload.csrf_secret,
                audience,
                &payload.correlation_id,
            )
            .await
            .map_err(authentication_status)?;
        Ok(Response::new(LogoutResponse { revoked }))
    }

    async fn refresh_session(
        &self,
        request: Request<RefreshSessionRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let payload = request.into_inner();
        let audience = authentication_audience(payload.audience)?;
        let session = self
            .identity
            .refresh_session(
                &payload.session_secret,
                &payload.csrf_secret,
                audience,
                &payload.correlation_id,
            )
            .await
            .map_err(authentication_status)?;
        Ok(Response::new(LoginResponse {
            session_id: session.session_id.to_string(),
            session_secret: session.session_secret,
            csrf_secret: session.csrf_secret,
            user_id: session.user_id.to_string(),
            idle_expires_at_unix: session.idle_expires_at,
            absolute_expires_at_unix: session.absolute_expires_at,
        }))
    }
}

impl IdentityAuthenticationService {
    async fn deliver_email_action(
        &self,
        payload: EmailActionRequest,
        purpose: &str,
    ) -> Result<Response<EmailActionResponse>, Status> {
        let action = match purpose {
            "email_verification" => {
                self.identity
                    .resend_verification(
                        &payload.email,
                        &payload.source_address,
                        &payload.correlation_id,
                    )
                    .await
            }
            "password_recovery" => {
                self.identity
                    .request_password_recovery(
                        &payload.email,
                        &payload.source_address,
                        &payload.correlation_id,
                    )
                    .await
            }
            _ => return Err(Status::invalid_argument("Email action is not supported")),
        }
        .map_err(authentication_status)?;
        if let Some(pending) = action {
            let (subject, preheader, title, message, path, label, footer_note) = if purpose
                == "email_verification"
            {
                (
                    "Verifica tu correo de XOVYRA",
                    "Confirma tu dirección de correo para activar tu cuenta.",
                    "Confirma tu correo",
                    "Confirma tu dirección de correo para activar tu cuenta.",
                    "/verify-email",
                    "Verificar correo",
                    Some("Si no creaste esta cuenta, puedes ignorar este mensaje."),
                )
            } else {
                (
                    "Restablece tu contraseña de XOVYRA",
                    "Usa el enlace seguro para cambiar la contraseña de tu cuenta.",
                    "Restablece tu contraseña",
                    "Solicitaste cambiar la contraseña de tu cuenta.",
                    "/reset-password",
                    "Restablecer contraseña",
                    Some(
                        "Si no solicitaste este cambio, ignora este correo. Tu contraseña actual no cambiará.",
                    ),
                )
            };
            let url = format!(
                "{}{path}#token={}",
                self.public_app_origin.trim_end_matches('/'),
                pending.token
            );
            let message_id = self
                .deliver_email(EmailDelivery {
                    recipient: &pending.email,
                    display_name: &pending.display_name,
                    subject,
                    preheader,
                    title,
                    message,
                    verification_code: pending.verification_code.as_deref(),
                    action: Some((&url, label)),
                    footer_note,
                    idempotency_key: &format!("{purpose}/{}", pending.token_id),
                    correlation_id: &payload.correlation_id,
                })
                .await;
            self.identity
                .set_email_delivery(
                    pending.token_id,
                    message_id.is_some(),
                    message_id.as_deref(),
                    &payload.correlation_id,
                )
                .await
                .map_err(authentication_status)?;
        }
        Ok(Response::new(EmailActionResponse { accepted: true }))
    }
}

funcion authentication_audience(
    audience: i32,
) -> Result<xovyra_identity::authentication::Audience, Status> {
    match AuthenticationAudience::try_from(audience) {
        Ok(AuthenticationAudience::Tenant) => Ok(xovyra_identity::authentication::Audience::Tenant),
        Ok(AuthenticationAudience::Platform) => {
            Ok(xovyra_identity::authentication::Audience::Platform)
        }
        Ok(AuthenticationAudience::Unspecified) | Err(_) => Err(Status::invalid_argument(
            "Authentication audience is required",
        )),
    }
}

fn authentication_status(error: xovyra_identity::authentication::AuthenticationError) -> Status {
    match error {
        xovyra_identity::authentication::AuthenticationError::EmailActionRateLimited {
            retry_after_seconds,
            account_limit_exceeded,
            limit,
        } => {
            let mut metadata = tonic::metadata::MetadataMap::new();
            if let Ok(value) =
                tonic::metadata::MetadataValue::try_from(retry_after_seconds.to_string())
            {
                metadata.insert("retry-after", value);
            }
            if let Ok(value) = tonic::metadata::MetadataValue::try_from(limit.to_string()) {
                metadata.insert("xovyra-rate-limit", value);
            }
            metadata.insert(
                "xovyra-rate-limit-scope",
                tonic::metadata::MetadataValue::from_static(if account_limit_exceeded {
                    "email"
                } else {
                    "source"
                }),
            );
            Status::with_metadata(
                tonic::Code::ResourceExhausted,
                "Email action rate limit exceeded",
                metadata,
            )
        }
        xovyra_identity::authentication::AuthenticationError::EmailVerificationRequired => {
            Status::failed_precondition("Email verification is required")
        }
        xovyra_identity::authentication::AuthenticationError::Rejected
        | xovyra_identity::authentication::AuthenticationError::InvalidSessionSecret => {
            Status::unauthenticated("Authentication was rejected")
        }
        xovyra_identity::authentication::AuthenticationError::RateLimited => {
            Status::resource_exhausted("Authentication rate limit exceeded")
        }
        xovyra_identity::authentication::AuthenticationError::Persistence(database_error) => {
            let database_code = database_error
                .as_database_error()
                .and_then(|database_error| database_error.code());
            let constraint = database_error
                .as_database_error()
                .and_then(|database_error| database_error.constraint());
            tracing::error!(
                database_code = ?database_code,
                constraint = ?constraint,
                "Identity database operation failed"
            );
            Status::internal("Identity operation failed")
        }
        error => {
            tracing::error!(error = %error, "Identity persistence or cryptographic operation failed");
            Status::internal("Identity operation failed")
        }
    }
}

#[tonic::async_trait]
impl RuntimeReadiness for ReadinessService {
    async fn check(
        &self,
        request: Request<ReadinessRequest>,
    ) -> Result<Response<ReadinessResponse>, Status> {
        let trace_values = ["traceparent", "tracestate"]
            .into_iter()
            .filter_map(|name| {
                request
                    .metadata()
                    .get(name)
                    .and_then(|value| value.to_str().ok())
                    .map(|value| (name.to_owned(), value.to_owned()))
            })
            .collect::<HashMap<String, String>>();
        let correlation_id = request.into_inner().correlation_id;
        let span = tracing::info_span!("grpc.runtime.readiness", correlation_id = %correlation_id);
        if let Err(error) = span.set_parent(xovyra_platform::propagation::extract_trace_context(
            trace_values,
        )) {
            tracing::warn!(error = %error, "Incoming trace context could not be attached to the gRPC span");
        }
        self.check_dependencies(correlation_id)
            .instrument(span)
            .await
    }
}

impl ReadinessService {
    async fn check_dependencies(
        &self,
        correlation_id: String,
    ) -> Result<Response<ReadinessResponse>, Status> {
        let database_check = tokio::time::timeout(
            DEPENDENCY_TIMEOUT,
            sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&self.dependencies.database),
        )
        .await;
        let identity_database_check = tokio::time::timeout(
            DEPENDENCY_TIMEOUT,
            sqlx::query_scalar::<_, i32>("SELECT 1")
                .fetch_one(&self.dependencies.identity_database),
        )
        .await;
        let tenancy_database_check = tokio::time::timeout(
            DEPENDENCY_TIMEOUT,
            sqlx::query_scalar::<_, i32>("SELECT 1")
                .fetch_one(&self.dependencies.tenancy_access_database),
        )
        .await;
        let messaging_check =
            tokio::time::timeout(DEPENDENCY_TIMEOUT, self.dependencies.messaging.flush()).await;
        let cache_check = tokio::time::timeout(
            DEPENDENCY_TIMEOUT,
            redis::cmd("PING").query_async::<String>(&mut self.dependencies.cache.clone()),
        )
        .await;
        let ready = matches!(database_check, Ok(Ok(1)))
            && matches!(identity_database_check, Ok(Ok(1)))
            && matches!(tenancy_database_check, Ok(Ok(1)))
            && matches!(cache_check, Ok(Ok(ref response)) if response == "PONG")
            && matches!(messaging_check, Ok(Ok(())));
        if !ready {
            return Err(Status::unavailable(
                "A critical runtime dependency is unavailable",
            ));
        }
        let trace_id = tracing::Span::current()
            .context()
            .span()
            .span_context()
            .trace_id()
            .to_string();
        Ok(Response::new(ReadinessResponse {
            ready,
            correlation_id,
            trace_id,
        }))
    }
}
