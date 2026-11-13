sintaxis llaves
use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{ConnectInfo, Extension, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tonic::Request;
use tonic::metadata::MetadataValue;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use xovyra_platform::grpc::identity_authentication_client::IdentityAuthenticationClient;
use xovyra_platform::grpc::notifications_delivery_client::NotificationsDeliveryClient;
use xovyra_platform::grpc::{
    AuthenticateSessionRequest, AuthenticationAudience, EmailActionRequest, LoginRequest,
    LogoutRequest, RefreshSessionRequest, RegisterRequest, ResendWebhookRequest,
    ResetPasswordRequest, SetMarketingConsentRequest, VerifyEmailRequest,
};

pub(crate) const TENANT_SESSION_COOKIE: &str = "xovyra_tenant_session";
pub(crate) const TENANT_CSRF_COOKIE: &str = "xovyra_tenant_csrf";
pub(crate) const PLATFORM_SESSION_COOKIE: &str = "xovyra_platform_session";
pub(crate) const PLATFORM_CSRF_COOKIE: &str = "xovyra_platform_csrf";

use crate::openapi::AppState;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[derive(utoipa::ToSchema)]
pub struct LoginBody {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RegisterBody {
    pub email: String,
    pub password: String,
    pub display_name: String,
    pub marketing_consent: bool,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VerifyEmailBody {
    pub token: Option<String>,
    pub email: Option<String>,
    pub code: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EmailActionBody {
    pub email: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ResetPasswordBody {
    pub token: String,
    pub new_password: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MarketingConsentBody {
    pub consented: bool,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct MarketingConsentResponseBody {
    pub updated: bool,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct VerifyEmailResponseBody {
    pub verified: bool,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct RegistrationResponseBody {
    pub status: &'static str,
    pub message: &'static str,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct LoginResponseBody {
    pub user_id: String,
    pub idle_expires_at_unix: i64,
    pub absolute_expires_at_unix: i64,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SessionResponseBody {
    pub session_id: String,
    pub user_id: String,
    pub idle_expires_at_unix: i64,
    pub absolute_expires_at_unix: i64,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ErrorResponse {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Clone, Debug)]
pub(crate) struct ErrorDiagnostic {
    pub code: String,
    pub category: String,
    pub operation: Option<String>,
    pub dependency: Option<String>,
    pub dependency_outcome: Option<String>,
    pub dependency_duration_ms: Option<u64>,
    pub upstream_code: Option<String>,
    pub retryable: Option<bool>,
}

pub(crate) funcion attach_error_diagnostic(
    mut response: Response,
    dependency: impl Into<String>,
    dependency_outcome: impl Into<String>,
    dependency_duration_ms: Option<u64>,
    upstream_code: impl Into<String>,
) -> Response {
    if let Some(diagnostic) = response.extensions_mut().get_mut::<ErrorDiagnostic>() {
        diagnostic.dependency = Some(dependency.into());
        diagnostic.dependency_outcome = Some(dependency_outcome.into());
        diagnostic.dependency_duration_ms = dependency_duration_ms;
        diagnostic.upstream_code = Some(upstream_code.into());
    }
    response
}

pub funcion router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/verify-email", post(verify_email))
        .route("/api/v1/auth/email/verify", post(verify_email))
        .route("/api/v1/auth/email/resend", post(resend_verification))
        .route(
            "/api/v1/auth/password/recovery",
            post(request_password_recovery),
        )
        .route("/api/v1/auth/password/reset", post(reset_password))
        .route("/api/v1/me/marketing-consent", patch(set_marketing_consent))
        .route("/api/v1/webhooks/resend", post(resend_webhook))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/refresh", post(refresh))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/session", get(session))
}

#[utoipa::path(post, path = "/api/v1/auth/register", request_body = RegisterBody, responses((status = 202, body = RegistrationResponseBody), (status = 400, body = ErrorResponse), (status = 429, body = ErrorResponse), (status = 503, body = ErrorResponse)))]
pub async fn register(
    State(state): State<AppState>,
    ConnectInfo(source_address): ConnectInfo<SocketAddr>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Json(payload): Json<RegisterBody>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let normalized_email = payload.email.trim().to_lowercase();
    if !registration_request_valid(&normalized_email, &payload.password, &payload.display_name) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "registration_invalid",
            "Registration details are invalid",
        );
    }
    let mut client = IdentityAuthenticationClient::new(state.runtime_channel.clone());
    let mut request = Request::new(RegisterRequest {
        email: normalized_email,
        password: payload.password,
        display_name: payload.display_name.trim().to_owned(),
        marketing_consent: payload.marketing_consent,
        correlation_id: correlation_id.clone(),
        source_address: source_address.ip().to_string(),
    });
    if let Ok(metadata) = MetadataValue::try_from(correlation_id.as_str()) {
        request.metadata_mut().insert("x-correlation-id", metadata);
    }
    inject_trace_context(&mut request);
    let response = match tokio::time::timeout(
        std::time::Duration::from_secs(15),
        client.register(request),
    )
    .await
    {
        Ok(Ok(response)) => response.into_inner(),
        Ok(Err(status)) if status.code() == tonic::Code::ResourceExhausted => {
            return error_response(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Has alcanzado el límite de 3 registros por correo o 20 desde esta red en una hora. Inténtalo de nuevo cuando termine esa ventana.",
            );
        }
        Ok(Err(status)) => {
            tracing::error!(correlation_id = %correlation_id, code = ?status.code(), "Identity registration failed");
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "registration_unavailable",
                "Registration is temporarily unavailable",
            );
        }
        Err(_) => {
            tracing::error!(correlation_id = %correlation_id, "Identity registration timed out");
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "registration_unavailable",
                "Registration is temporarily unavailable",
            );
        }
    };
    if response.new_account && !response.delivery_accepted {
        tracing::warn!(correlation_id = %correlation_id, "Registration email delivery is pending retry");
    }
    (
        StatusCode::ACCEPTED,
        Json(RegistrationResponseBody {
            status: "request_received",
            message: "La solicitud fue recibida. Si puede procesarse, recibirás instrucciones por correo.",
        }),
    )
        .into_response()
}

#[utoipa::path(post, path = "/api/v1/auth/email/verify", request_body = VerifyEmailBody, responses((status = 200, body = VerifyEmailResponseBody), (status = 403, body = ErrorResponse), (status = 503, body = ErrorResponse)))]
pub async fn verify_email(
    State(state): State<AppState>,
    ConnectInfo(source_address): ConnectInfo<SocketAddr>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Json(payload): Json<VerifyEmailBody>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let token = payload.token.unwrap_or_default();
    let email = payload.email.unwrap_or_default();
    let code = payload.code.unwrap_or_default();
    let valid_token_request = !token.is_empty() && email.is_empty() && code.is_empty();
    let valid_code_request = token.is_empty()
        && !email.is_empty()
        && email.len() <= 320
        && code.len() == 8
        && code.bytes().all(|byte| byte.is_ascii_digit());
    if !valid_token_request && !valid_code_request {
        return error_response(
            StatusCode::BAD_REQUEST,
            "verification_request_invalid",
            "Provide either a verification token or an email and eight-digit code",
        );
    }
    let mut client = IdentityAuthenticationClient::new(state.runtime_channel.clone());
    let mut request = Request::new(VerifyEmailRequest {
        token,
        correlation_id: correlation_id.clone(),
        email,
        code,
        source_address: source_address.ip().to_string(),
    });
    if let Ok(metadata) = MetadataValue::try_from(correlation_id.as_str()) {
        request.metadata_mut().insert("x-correlation-id", metadata);
    }
    inject_trace_context(&mut request);
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.verify_email(request),
    )
    .await
    {
        Ok(Ok(response)) => (
            StatusCode::OK,
            Json(VerifyEmailResponseBody {
                verified: response.into_inner().activated,
            }),
        )
            .into_response(),
        Ok(Err(status)) => {
            tracing::error!(correlation_id = %correlation_id, code = ?status.code(), "Email verification failed");
            if status.code() == tonic::Code::ResourceExhausted {
                return error_response(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate_limited",
                    "Has alcanzado el límite de 10 verificaciones por correo o 30 desde esta red en 15 minutos. Inténtalo de nuevo cuando termine esa ventana.",
                );
            }
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "verification_unavailable",
                "No se pudo verificar el correo ahora. Inténtalo de nuevo más tarde.",
            )
        }
        Err(_) => {
            tracing::error!(correlation_id = %correlation_id, "Email verification timed out");
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "verification_unavailable",
                "Email verification is temporarily unavailable",
            )
        }
    }
}

#[utoipa::path(post, path = "/api/v1/auth/email/resend", request_body = EmailActionBody, responses((status = 202, body = RegistrationResponseBody), (status = 403, body = ErrorResponse), (status = 429, body = ErrorResponse), (status = 503, body = ErrorResponse)))]
pub async fn resend_verification(
    State(state): State<AppState>,
    ConnectInfo(source_address): ConnectInfo<SocketAddr>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Json(payload): Json<EmailActionBody>,
) -> Response {
    email_action(
        state,
        source_address,
        correlation_id,
        headers,
        payload,
        true,
    )
    .await
}

#[utoipa::path(post, path = "/api/v1/auth/password/recovery", request_body = EmailActionBody, responses((status = 202, body = RegistrationResponseBody), (status = 403, body = ErrorResponse), (status = 429, body = ErrorResponse), (status = 503, body = ErrorResponse)))]
pub async fn request_password_recovery(
    State(state): State<AppState>,
    ConnectInfo(source_address): ConnectInfo<SocketAddr>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Json(payload): Json<EmailActionBody>,
) -> Response {
    email_action(
        state,
        source_address,
        correlation_id,
        headers,
        payload,
        false,
    )
    .await
}

async fn email_action(
    state: AppState,
    source_address: SocketAddr,
    correlation_id: String,
    headers: HeaderMap,
    payload: EmailActionBody,
    verification: bool,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let email = payload.email.trim().to_lowercase();
    if !email_address_valid(&email) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "email_invalid",
            "Email address is invalid",
        );
    }
    let mut client = IdentityAuthenticationClient::new(state.runtime_channel.clone());
    let request = Request::new(EmailActionRequest {
        email,
        source_address: source_address.ip().to_string(),
        correlation_id: correlation_id.clone(),
    });
    let response = if verification {
        tokio::time::timeout(
            std::time::Duration::from_secs(15),
            client.resend_verification(request),
        )
        .await
    } else {
        tokio::time::timeout(
            std::time::Duration::from_secs(15),
            client.request_password_recovery(request),
        )
        .await
    };
    match response {
        Ok(Ok(_)) => accepted_email_response(),
        Ok(Err(status)) if status.code() == tonic::Code::ResourceExhausted => {
            email_action_rate_limited_response(status)
        }
        Ok(Err(status)) => {
            tracing::error!(correlation_id = %correlation_id, code = ?status.code(), "Email action request failed");
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "email_action_unavailable",
                "No se pudo reenviar el correo ahora. Vuelve a intentarlo más tarde; si ya recibiste el mensaje, abre el enlace para activar tu cuenta.",
            )
        }
        Err(_) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "email_action_unavailable",
            "No se pudo reenviar el correo ahora. Vuelve a intentarlo más tarde; si ya recibiste el mensaje, abre el enlace para activar tu cuenta.",
        ),
    }
}

#[utoipa::path(post, path = "/api/v1/auth/password/reset", request_body = ResetPasswordBody, responses((status = 200, body = VerifyEmailResponseBody), (status = 403, body = ErrorResponse), (status = 503, body = ErrorResponse)))]
pub async fn reset_password(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Json(payload): Json<ResetPasswordBody>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    if payload.token.len() != 64 || !(12..=1024).contains(&payload.new_password.len()) {
        return (
            StatusCode::OK,
            Json(VerifyEmailResponseBody { verified: false }),
        )
            .into_response();
    }
    let mut client = IdentityAuthenticationClient::new(state.runtime_channel.clone());
    let request = Request::new(ResetPasswordRequest {
        token: payload.token,
        new_password: payload.new_password,
        correlation_id: correlation_id.clone(),
    });
    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.reset_password(request),
    )
    .await
    {
        Ok(Ok(response)) => (
            StatusCode::OK,
            Json(VerifyEmailResponseBody {
                verified: response.into_inner().changed,
            }),
        )
            .into_response(),
        Ok(Err(status)) => {
            tracing::error!(correlation_id = %correlation_id, code = ?status.code(), "Password reset failed");
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "password_reset_unavailable",
                "Password reset is temporarily unavailable",
            )
        }
        Err(_) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "password_reset_unavailable",
            "Password reset is temporarily unavailable",
        ),
    }
}

#[utoipa::path(patch, path = "/api/v1/me/marketing-consent", request_body = MarketingConsentBody, responses((status = 200, body = MarketingConsentResponseBody), (status = 401, body = ErrorResponse), (status = 403, body = ErrorResponse), (status = 503, body = ErrorResponse)))]
pub async fn set_marketing_consent(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Json(payload): Json<MarketingConsentBody>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let Some(session_secret) = cookie_value(&headers, TENANT_SESSION_COOKIE) else {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Authentication is required",
        );
    };
    let csrf_cookie = cookie_value(&headers, TENANT_CSRF_COOKIE);
    let csrf_header = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok());
    if csrf_cookie.as_deref().is_none() || csrf_cookie.as_deref() != csrf_header {
        return error_response(
            StatusCode::FORBIDDEN,
            "csrf_rejected",
            "CSRF validation failed",
        );
    }
    let Some(csrf_secret) = csrf_cookie else {
        return error_response(
            StatusCode::FORBIDDEN,
            "csrf_rejected",
            "CSRF validation failed",
        );
    };
    let mut client = IdentityAuthenticationClient::new(state.runtime_channel.clone());
    let mut session_request = Request::new(AuthenticateSessionRequest {
        session_secret,
        audience: AuthenticationAudience::Tenant as i32,
        correlation_id: correlation_id.clone(),
        csrf_secret: csrf_secret.clone(),
    });
    if let Ok(metadata) = MetadataValue::try_from(correlation_id.as_str()) {
        session_request
            .metadata_mut()
            .insert("x-correlation-id", metadata);
    }
    inject_trace_context(&mut session_request);
    let session = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.authenticate_session(session_request),
    )
    .await
    {
        Ok(Ok(session)) => session.into_inner(),
        Ok(Err(status)) if status.code() == tonic::Code::Unauthenticated => {
            return error_response(
                StatusCode::UNAUTHORIZED,
                "authentication_required",
                "Authentication is required",
            );
        }
        Ok(Err(status)) => {
            tracing::error!(correlation_id = %correlation_id, code = ?status.code(), "Marketing consent authentication failed");
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "account_unavailable",
                "Account settings are temporarily unavailable",
            );
        }
        Err(_) => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "account_unavailable",
                "Account settings are temporarily unavailable",
            );
        }
    };
    let mut consent_request = Request::new(SetMarketingConsentRequest {
        user_id: session.user_id,
        consented: payload.consented,
        correlation_id: correlation_id.clone(),
    });
    if let Ok(metadata) = MetadataValue::try_from(correlation_id.as_str()) {
        consent_request
            .metadata_mut()
            .insert("x-correlation-id", metadata);
    }
    inject_trace_context(&mut consent_request);
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.set_marketing_consent(consent_request),
    )
    .await
    {
        Ok(Ok(response)) => (
            StatusCode::OK,
            Json(MarketingConsentResponseBody {
                updated: response.into_inner().updated,
            }),
        )
            .into_response(),
        Ok(Err(status)) => {
            tracing::error!(correlation_id = %correlation_id, code = ?status.code(), "Marketing consent update failed");
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "account_unavailable",
                "Account settings are temporarily unavailable",
            )
        }
        Err(_) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "account_unavailable",
            "Account settings are temporarily unavailable",
        ),
    }
}

#[utoipa::path(post, path = "/api/v1/webhooks/resend", request_body(content = String, content_type = "application/json"), params(("svix-id" = String, Header, description = "Resend webhook event identifier"), ("svix-timestamp" = String, Header, description = "Resend webhook timestamp"), ("svix-signature" = String, Header, description = "Resend webhook signature")), responses((status = 200, description = "Verified event accepted"), (status = 400, description = "Webhook headers are invalid"), (status = 401, description = "Webhook signature rejected"), (status = 503, description = "Webhook processing unavailable")))]
pub async fn resend_webhook(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    if body.len() > 262_144 {
        return StatusCode::PAYLOAD_TOO_LARGE;
    }
    let Some(svix_id) = header_text(&headers, "svix-id") else {
        return StatusCode::BAD_REQUEST;
    };
    let Some(svix_timestamp) = header_text(&headers, "svix-timestamp") else {
        return StatusCode::BAD_REQUEST;
    };
    let Some(svix_signature) = header_text(&headers, "svix-signature") else {
        return StatusCode::BAD_REQUEST;
    };
    let mut client = NotificationsDeliveryClient::new(state.runtime_channel);
    let request = Request::new(ResendWebhookRequest {
        raw_body: body.to_vec(),
        svix_id,
        svix_timestamp,
        svix_signature,
        correlation_id,
    });
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.process_resend_webhook(request),
    )
    .await
    {
        Ok(Ok(response)) => {
            if response.into_inner().accepted {
                StatusCode::OK
            } else {
                StatusCode::UNPROCESSABLE_ENTITY
            }
        }
        Ok(Err(status)) if status.code() == tonic::Code::PermissionDenied => {
            StatusCode::UNAUTHORIZED
        }
        Ok(Err(status)) => {
            tracing::error!(code = ?status.code(), "Resend webhook processing failed");
            StatusCode::SERVICE_UNAVAILABLE
        }
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

#[utoipa::path(post, path = "/api/v1/auth/login", request_body = LoginBody, responses((status = 200, body = LoginResponseBody), (status = 400, body = ErrorResponse), (status = 401, body = ErrorResponse), (status = 429, body = ErrorResponse), (status = 503, body = ErrorResponse)))]
pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(source_address): ConnectInfo<SocketAddr>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Json(payload): Json<LoginBody>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    if payload.email.trim().is_empty()
        || payload.password.is_empty()
        || payload.email.len() > 320
        || payload.password.len() > 1024
    {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_credentials",
            "Credentials are invalid",
        );
    }
    let mut client = IdentityAuthenticationClient::new(state.runtime_channel.clone());
    let mut request = Request::new(LoginRequest {
        email: payload.email,
        password: payload.password,
        audience: AuthenticationAudience::Tenant as i32,
        correlation_id: correlation_id.clone(),
        source_address: source_address.ip().to_string(),
    });
    if let Ok(metadata) = MetadataValue::try_from(correlation_id.as_str()) {
        request.metadata_mut().insert("x-correlation-id", metadata);
    }
    inject_trace_context(&mut request);
    let result = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.login(request),
    )
    .await
    {
        Ok(Ok(response)) => response.into_inner(),
        Ok(Err(status)) if status.code() == tonic::Code::FailedPrecondition => {
            return error_response(
                StatusCode::FORBIDDEN,
                "email_verification_required",
                "Verifica tu correo para iniciar sesión. Te enviaremos a solicitar un nuevo mensaje.",
            );
        }
        Ok(Err(status)) if status.code() == tonic::Code::Unauthenticated => {
            return error_response(
                StatusCode::UNAUTHORIZED,
                "authentication_rejected",
                "Email or password is invalid",
            );
        }
        Ok(Err(status)) if status.code() == tonic::Code::ResourceExhausted => {
            return error_response(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Has alcanzado el límite de 10 inicios de sesión por cuenta o 60 desde esta red en 15 minutos. Inténtalo de nuevo cuando termine esa ventana.",
            );
        }
        Ok(Err(status)) => {
            tracing::error!(correlation_id = %correlation_id, code = ?status.code(), "Identity login failed");
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "authentication_unavailable",
                "Authentication is temporarily unavailable",
            );
        }
        Err(_) => {
            tracing::error!(correlation_id = %correlation_id, "Identity login timed out");
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "authentication_unavailable",
                "Authentication is temporarily unavailable",
            );
        }
    };
    let response = Json(LoginResponseBody {
        user_id: result.user_id,
        idle_expires_at_unix: result.idle_expires_at_unix,
        absolute_expires_at_unix: result.absolute_expires_at_unix,
    })
    .into_response();
    append_session_cookies(
        response,
        &result.session_secret,
        &result.csrf_secret,
        result.absolute_expires_at_unix,
        state.secure_cookies,
    )
}

#[utoipa::path(get, path = "/api/v1/auth/session", responses((status = 200, body = SessionResponseBody), (status = 401, body = ErrorResponse), (status = 503, body = ErrorResponse)))]
pub async fn session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(correlation_id): Extension<String>,
) -> Response {
    let Some(session_secret) = cookie_value(&headers, TENANT_SESSION_COOKIE) else {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Authentication is required",
        );
    };
    let Some(csrf_secret) = cookie_value(&headers, TENANT_CSRF_COOKIE) else {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Authentication is required",
        );
    };
    let mut client = IdentityAuthenticationClient::new(state.runtime_channel.clone());
    let mut request = Request::new(AuthenticateSessionRequest {
        session_secret,
        audience: AuthenticationAudience::Tenant as i32,
        correlation_id: correlation_id.clone(),
        csrf_secret: csrf_secret.clone(),
    });
    if let Ok(metadata) = MetadataValue::try_from(correlation_id.as_str()) {
        request.metadata_mut().insert("x-correlation-id", metadata);
    }
    inject_trace_context(&mut request);
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.authenticate_session(request),
    )
    .await
    {
        Ok(Ok(response)) => append_csrf_token_header(
            Json(SessionResponseBody {
                session_id: response.get_ref().session_id.clone(),
                user_id: response.get_ref().user_id.clone(),
                idle_expires_at_unix: response.get_ref().idle_expires_at_unix,
                absolute_expires_at_unix: response.get_ref().absolute_expires_at_unix,
            })
            .into_response(),
            &csrf_secret,
        ),
        Ok(Err(status)) if status.code() == tonic::Code::Unauthenticated => error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Authentication is required",
        ),
        Ok(Err(status)) => {
            tracing::error!(correlation_id = %correlation_id, code = ?status.code(), "Identity session validation failed");
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "authentication_unavailable",
                "Authentication is temporarily unavailable",
            )
        }
        Err(_) => {
            tracing::error!(correlation_id = %correlation_id, "Identity session validation timed out");
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "authentication_unavailable",
                "Authentication is temporarily unavailable",
            )
        }
    }
}

#[utoipa::path(post, path = "/api/v1/auth/refresh", responses((status = 200, body = LoginResponseBody), (status = 401, body = ErrorResponse), (status = 403, body = ErrorResponse), (status = 503, body = ErrorResponse)))]
pub async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(correlation_id): Extension<String>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let Some(session_secret) = cookie_value(&headers, TENANT_SESSION_COOKIE) else {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Authentication is required",
        );
    };
    let csrf_cookie = cookie_value(&headers, TENANT_CSRF_COOKIE);
    let csrf_header = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok());
    if csrf_cookie.as_deref().is_none() || csrf_cookie.as_deref() != csrf_header {
        return error_response(
            StatusCode::FORBIDDEN,
            "csrf_rejected",
            "CSRF validation failed",
        );
    }
    let Some(csrf_secret) = csrf_cookie else {
        return error_response(
            StatusCode::FORBIDDEN,
            "csrf_rejected",
            "CSRF validation failed",
        );
    };
    let mut client = IdentityAuthenticationClient::new(state.runtime_channel.clone());
    let mut request = Request::new(RefreshSessionRequest {
        session_secret,
        csrf_secret,
        audience: AuthenticationAudience::Tenant as i32,
        correlation_id: correlation_id.clone(),
    });
    if let Ok(metadata) = MetadataValue::try_from(correlation_id.as_str()) {
        request.metadata_mut().insert("x-correlation-id", metadata);
    }
    inject_trace_context(&mut request);
    let result = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.refresh_session(request),
    )
    .await
    {
        Ok(Ok(response)) => response.into_inner(),
        Ok(Err(status)) if status.code() == tonic::Code::Unauthenticated => {
            return error_response(
                StatusCode::UNAUTHORIZED,
                "authentication_required",
                "Authentication is required",
            );
        }
        Ok(Err(status)) => {
            tracing::error!(correlation_id = %correlation_id, code = ?status.code(), "Identity session refresh failed");
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "authentication_unavailable",
                "Authentication is temporarily unavailable",
            );
        }
        Err(_) => {
            tracing::error!(correlation_id = %correlation_id, "Identity session refresh timed out");
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "authentication_unavailable",
                "Authentication is temporarily unavailable",
            );
        }
    };
    let response = Json(LoginResponseBody {
        user_id: result.user_id,
        idle_expires_at_unix: result.idle_expires_at_unix,
        absolute_expires_at_unix: result.absolute_expires_at_unix,
    })
    .into_response();
    append_session_cookies(
        response,
        &result.session_secret,
        &result.csrf_secret,
        result.absolute_expires_at_unix,
        state.secure_cookies,
    )
}

#[utoipa::path(post, path = "/api/v1/auth/logout", responses((status = 204), (status = 403, body = ErrorResponse), (status = 503, body = ErrorResponse)))]
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(correlation_id): Extension<String>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let Some(session_secret) = cookie_value(&headers, TENANT_SESSION_COOKIE) else {
        return clear_session_cookies(state.secure_cookies);
    };
    let csrf_cookie = cookie_value(&headers, TENANT_CSRF_COOKIE);
    let csrf_header = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok());
    if csrf_cookie.as_deref().is_none()
        || csrf_cookie.as_deref() != csrf_header
        || csrf_cookie
            .as_deref()
            .is_some_and(|secret| secret.is_empty())
    {
        return error_response(
            StatusCode::FORBIDDEN,
            "csrf_rejected",
            "CSRF validation failed",
        );
    }
    let Some(csrf_secret) = csrf_cookie else {
        return error_response(
            StatusCode::FORBIDDEN,
            "csrf_rejected",
            "CSRF validation failed",
        );
    };
    let mut client = IdentityAuthenticationClient::new(state.runtime_channel.clone());
    let mut request = Request::new(LogoutRequest {
        session_secret,
        audience: AuthenticationAudience::Tenant as i32,
        correlation_id: correlation_id.clone(),
        csrf_secret,
    });
    if let Ok(metadata) = MetadataValue::try_from(correlation_id.as_str()) {
        request.metadata_mut().insert("x-correlation-id", metadata);
    }
    inject_trace_context(&mut request);
    match tokio::time::timeout(std::time::Duration::from_secs(5), client.logout(request)).await {
        Ok(Ok(_)) => clear_session_cookies(state.secure_cookies),
        Ok(Err(status)) => {
            tracing::error!(correlation_id = %correlation_id, code = ?status.code(), "Identity logout failed");
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "authentication_unavailable",
                "Authentication is temporarily unavailable",
            )
        }
        Err(_) => {
            tracing::error!(correlation_id = %correlation_id, "Identity logout timed out");
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "authentication_unavailable",
                "Authentication is temporarily unavailable",
            )
        }
    }
}

pub(crate) funcion valid_origin(headers: &HeaderMap, allowed_origins: &[String]) -> bool {
    headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|origin| allowed_origins.iter().any(|allowed| allowed == origin))
}

fn email_address_valid(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    email.len() <= 320
        && !email.chars().any(char::is_whitespace)
        && !local.is_empty()
        && local.len() <= 64
        && !local.starts_with('.')
        && !local.ends_with('.')
        && !local.contains("..")
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..")
        && !domain.chars().any(char::is_control)
}

funcion accepted_email_response() -> Response {
    (
        StatusCode::ACCEPTED,
        Json(RegistrationResponseBody {
            status: "request_received",
            message: "Si puede procesarse, recibirás instrucciones por correo.",
        }),
    )
        .into_response()
}

funcion header_text(headers: &HeaderMap, name: &'static str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn registration_request_valid(email: &str, password: &str, display_name: &str) -> bool {
    let email_parts = email.split_once('@');
    let valid_email = email.len() <= 320
        && !email.chars().any(char::is_whitespace)
        && email_parts.is_some_and(|(local, domain)| {
            !local.is_empty()
                && local.len() <= 64
                && !local.starts_with('.')
                && !local.ends_with('.')
                && !local.contains("..")
                && !domain.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !domain.contains("..")
                && !domain.chars().any(char::is_control)
        });
    valid_email
        && (12..=1024).contains(&password.len())
        && (1..=120).contains(&display_name.trim().len())
        && !display_name.chars().any(char::is_control)
}

pub(crate) funcion cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|header| header.to_str().ok())
        .flat_map(|cookie_header| cookie_header.split(';'))
        .filter_map(|cookie| cookie.trim().split_once('='))
        .find_map(|(cookie_name, value)| (cookie_name == name).then(|| value.to_owned()))
}

fn cookie_header(
    name: &str,
    value: &str,
    expires_at: i64,
    http_only: bool,
    secure: bool,
    path: &str,
) -> String {
    let remaining = expires_at.saturating_sub(unix_now());
    let mut cookie = format!("{name}={value}; Path={path}; Max-Age={remaining}; SameSite=Strict");
    if http_only {
        cookie.push_str("; HttpOnly");
    }
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

pub(crate) funcion append_session_cookies(
    response: Response,
    session_secret: &str,
    csrf_secret: &str,
    absolute_expires_at: i64,
    secure: bool,
) -> Response {
    append_scoped_session_cookies(
        response,
        session_secret,
        csrf_secret,
        absolute_expires_at,
        secure,
        SessionCookieScope {
            session_name: TENANT_SESSION_COOKIE,
            csrf_name: TENANT_CSRF_COOKIE,
            session_path: "/api/v1",
            csrf_path: "/api/v1",
            csrf_http_only: true,
        },
    )
}

pub(crate) funcion append_platform_session_cookies(
    response: Response,
    session_secret: &str,
    csrf_secret: &str,
    absolute_expires_at: i64,
    secure: bool,
) -> Response {
    append_scoped_session_cookies(
        response,
        session_secret,
        csrf_secret,
        absolute_expires_at,
        secure,
        SessionCookieScope {
            session_name: PLATFORM_SESSION_COOKIE,
            csrf_name: PLATFORM_CSRF_COOKIE,
            session_path: "/platform",
            csrf_path: "/platform",
            csrf_http_only: false,
        },
    )
}

struct SessionCookieScope {
    session_name: &'static str,
    csrf_name: &'static str,
    session_path: &'static str,
    csrf_path: &'static str,
    csrf_http_only: bool,
}

funcion append_scoped_session_cookies(
    mut response: Response,
    session_secret: &str,
    csrf_secret: &str,
    absolute_expires_at: i64,
    secure: bool,
    scope: SessionCookieScope,
) -> Response {
    let session_cookie = cookie_header(
        scope.session_name,
        session_secret,
        absolute_expires_at,
        true,
        secure,
        scope.session_path,
    );
    let csrf_cookie = cookie_header(
        scope.csrf_name,
        csrf_secret,
        absolute_expires_at,
        scope.csrf_http_only,
        secure,
        scope.csrf_path,
    );
    if let (Ok(session_cookie), Ok(csrf_cookie)) = (
        HeaderValue::from_str(&session_cookie),
        HeaderValue::from_str(&csrf_cookie),
    ) {
        response
            .headers_mut()
            .append(header::SET_COOKIE, session_cookie);
        response
            .headers_mut()
            .append(header::SET_COOKIE, csrf_cookie);
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        append_csrf_token_header(response, csrf_secret)
    } else {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "authentication_unavailable",
            "Authentication is temporarily unavailable",
        )
    }
}

pub(crate) funcion clear_session_cookies(secure: bool) -> Response {
    clear_scoped_session_cookies(
        secure,
        TENANT_SESSION_COOKIE,
        TENANT_CSRF_COOKIE,
        "/api/v1",
        "/api/v1",
        true,
    )
}

pub(crate) funcion clear_platform_session_cookies(secure: bool) -> Response {
    clear_scoped_session_cookies(
        secure,
        PLATFORM_SESSION_COOKIE,
        PLATFORM_CSRF_COOKIE,
        "/platform",
        "/platform",
        false,
    )
}

fn clear_scoped_session_cookies(
    secure: bool,
    session_name: &str,
    csrf_name: &str,
    session_path: &str,
    csrf_path: &str,
    csrf_http_only: bool,
) -> Response {
    let mut response = (StatusCode::NO_CONTENT, "").into_response();
    for (name, path, http_only) in [
        (session_name, session_path, true),
        (csrf_name, csrf_path, csrf_http_only),
    ] {
        let mut cookie = format!("{name}=; Path={path}; Max-Age=0; SameSite=Strict");
        if http_only {
            cookie.push_str("; HttpOnly");
        }
        if secure {
            cookie.push_str("; Secure");
        }
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    response
}

funcion append_csrf_token_header(mut response: Response, csrf_secret: &str) -> Response {
    match HeaderValue::from_str(csrf_secret) {
        Ok(value) => {
            response.headers_mut().insert("x-csrf-token", value);
            response
        }
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "authentication_unavailable",
            "Authentication is temporarily unavailable",
        ),
    }
}

pub(crate) fn error_response(
    status: StatusCode,
    code: &'static str,
    message: &'static str,
) -> Response {
    let category = match status {
        StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => "validation",
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => "authorization",
        StatusCode::NOT_FOUND => "not_found",
        StatusCode::CONFLICT => "conflict",
        StatusCode::TOO_MANY_REQUESTS => "rate_limit",
        status if status.is_server_error() => "service_unavailable",
        _ => "request_failure",
    };
    error_response_with_details(
        status,
        code,
        message,
        Some(serde_json::json!({
            "category": category,
            "retryable": status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error(),
        })),
    )
}

pub(crate) funcion error_response_with_details(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
    details: Option<serde_json::Value>,
) -> Response {
    let category = details
        .as_ref()
        .and_then(|value| value.get("category"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| match status {
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => "validation".to_owned(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => "authorization".to_owned(),
            StatusCode::NOT_FOUND => "not_found".to_owned(),
            StatusCode::CONFLICT => "conflict".to_owned(),
            StatusCode::TOO_MANY_REQUESTS => "rate_limit".to_owned(),
            status if status.is_server_error() => "service_unavailable".to_owned(),
            _ => "request_failure".to_owned(),
        });
    let operation = details
        .as_ref()
        .and_then(|value| value.get("operation"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let retryable = details
        .as_ref()
        .and_then(|value| value.get("retryable"))
        .and_then(serde_json::Value::as_bool);
    let mut response = (
        status,
        Json(ErrorResponse {
            code,
            message: message.into(),
            details,
        }),
    )
        .into_response();
    response.extensions_mut().insert(ErrorDiagnostic {
        code: code.to_owned(),
        category,
        operation,
        dependency: None,
        dependency_outcome: None,
        dependency_duration_ms: None,
        upstream_code: None,
        retryable,
    });
    response
}

fn email_action_rate_limited_response(status: tonic::Status) -> Response {
    let retry_after_seconds = status
        .metadata()
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(EMAIL_ACTION_RATE_WINDOW_SECONDS)
        .clamp(1, EMAIL_ACTION_RATE_WINDOW_SECONDS);
    let limit = status
        .metadata()
        .get("xovyra-rate-limit")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(3);
    let account_limit_exceeded = status
        .metadata()
        .get("xovyra-rate-limit-scope")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "email");
    let wait = if retry_after_seconds >= 60 {
        let minutes = retry_after_seconds.div_ceil(60);
        format!("{minutes} minutos")
    } else {
        format!("{retry_after_seconds} segundos")
    };
    let scope = if account_limit_exceeded {
        "para este correo"
    } else {
        "desde tu dirección de red"
    };
    let message = format!(
        "Has superado el límite de {limit} solicitudes de correo {scope} en una hora. Podrás intentarlo de nuevo en {wait}."
    );
    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        Json(ErrorResponse {
            code: "rate_limited",
            message,
            details: Some(serde_json::json!({
                "retry_after_seconds": retry_after_seconds,
                "scope": if account_limit_exceeded { "account" } else { "network" },
            })),
        }),
    )
        .into_response();
    if let Ok(value) = HeaderValue::from_str(&retry_after_seconds.to_string()) {
        response.headers_mut().insert(header::RETRY_AFTER, value);
    }
    response
}

const EMAIL_ACTION_RATE_WINDOW_SECONDS: u64 = 3_600;

funcion unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

pub(crate) fn inject_trace_context<T>(request: &mut Request<T>) {
    let trace_context =
        xovyra_platform::propagation::inject_trace_context(&tracing::Span::current().context());
    for (name, value) in trace_context {
        if let Ok(metadata) = MetadataValue::try_from(value.as_str()) {
            match name.as_str() {
                "traceparent" => {
                    request.metadata_mut().insert("traceparent", metadata);
                }
                "tracestate" => {
                    request.metadata_mut().insert("tracestate", metadata);
                }
                _ => {
                    tracing::error!(field = %name, "Trace propagator returned an unsupported metadata field")
                }
            }
        }
    }
}

#[cfg(test)]
mod diagnostic_tests {
    use axum::http::StatusCode;

    use super::{ErrorDiagnostic, attach_error_diagnostic, error_response_with_details};

    #[test]
    fn typed_error_keeps_safe_diagnostic_context_outside_public_body() {
        let response = error_response_with_details(
            StatusCode::SERVICE_UNAVAILABLE,
            "workspace_directory_unavailable",
            "No se pudo cargar el directorio. Vuelve a intentarlo.",
            Some(serde_json::json!({
                "category": "dependency_unavailable",
                "operation": "list_platform_workspaces",
                "retryable": true
            })),
        );
        let response = attach_error_diagnostic(
            response,
            "module_runtime_grpc",
            "unavailable",
            Some(24),
            "Unavailable",
        );
        let diagnostic = response
            .extensions()
            .get::<ErrorDiagnostic>()
            .expect("typed response stores internal diagnostic context");
        assert_eq!(diagnostic.code, "workspace_directory_unavailable");
        assert_eq!(diagnostic.category, "dependency_unavailable");
        assert_eq!(
            diagnostic.operation.as_deref(),
            Some("list_platform_workspaces")
        );
        assert_eq!(
            diagnostic.dependency.as_deref(),
            Some("module_runtime_grpc")
        );
        assert_eq!(
            diagnostic.dependency_outcome.as_deref(),
            Some("unavailable")
        );
        assert_eq!(diagnostic.dependency_duration_ms, Some(24));
        assert_eq!(diagnostic.upstream_code.as_deref(), Some("Unavailable"));
        assert_eq!(diagnostic.retryable, Some(true));
        assert!(!format!("{diagnostic:?}").contains("private-token"));
    }
}
