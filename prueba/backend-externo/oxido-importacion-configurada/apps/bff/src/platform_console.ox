sintaxis llaves
#[path = "platform_console/directory.ox"]
pub(crate) mod directory;
#[path = "platform_console/support.ox"]
pub(crate) mod support;

use axum::Router;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use opentelemetry::trace::TraceContextExt;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use xovyra_platform::grpc::{AccountSessionRequest, AuthenticationAudience};

use crate::account_http::session_credentials;
use crate::authentication::{
    PLATFORM_CSRF_COOKIE, PLATFORM_SESSION_COOKIE, attach_error_diagnostic, error_response,
    error_response_with_details, valid_origin,
};
use crate::openapi::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(directory::router())
        .merge(support::router())
}

pub(crate) fn platform_session(
    state: &AppState,
    headers: &HeaderMap,
    correlation_id: &str,
    require_csrf: bool,
) -> Result<AccountSessionRequest, Box<Response>> {
    if !valid_origin(headers, &state.platform_allowed_origins) {
        return Err(Box::new(error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "La solicitud no procede de la consola XOVYRA.",
        )));
    }
    let (session_secret, csrf_secret) = session_credentials(
        headers,
        PLATFORM_SESSION_COOKIE,
        PLATFORM_CSRF_COOKIE,
        require_csrf,
    )?;
    Ok(AccountSessionRequest {
        session_secret,
        csrf_secret,
        audience: AuthenticationAudience::Platform as i32,
        correlation_id: correlation_id.to_owned(),
    })
}

fn grpc_error(
    status: tonic::Status,
    correlation_id: &str,
    route_operation: &'static str,
    dependency_duration_ms: Option<u64>,
) -> Response {
    let (http_status, code, message) = match status.message() {
        "Platform workspace directory is temporarily unavailable" => (
            StatusCode::SERVICE_UNAVAILABLE,
            "workspace_directory_unavailable",
            "No se pudo cargar el directorio de espacios de trabajo porque un servicio requerido no respondió.",
        ),
        "Platform workspace members are temporarily unavailable" => (
            StatusCode::SERVICE_UNAVAILABLE,
            "workspace_members_unavailable",
            "No se pudieron cargar los miembros de este espacio de trabajo. Puedes volver a intentarlo.",
        ),
        _ => match status.code() {
            tonic::Code::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                "authentication_required",
                "Inicia sesión para continuar.",
            ),
            tonic::Code::PermissionDenied => (
                StatusCode::FORBIDDEN,
                "permission_denied",
                "No tienes permiso para esta operación.",
            ),
            tonic::Code::InvalidArgument => (
                StatusCode::BAD_REQUEST,
                "request_invalid",
                "Revisa los datos enviados.",
            ),
            tonic::Code::NotFound => (
                StatusCode::NOT_FOUND,
                "resource_not_found",
                "El recurso solicitado no está disponible.",
            ),
            tonic::Code::AlreadyExists => (
                StatusCode::CONFLICT,
                "request_conflict",
                "La solicitud entra en conflicto con un cambio anterior.",
            ),
            tonic::Code::FailedPrecondition => (
                StatusCode::GONE,
                "operation_unavailable",
                "La autorización expiró o ya no está disponible.",
            ),
            tonic::Code::ResourceExhausted => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Espera antes de volver a intentarlo.",
            ),
            _ => (
                StatusCode::SERVICE_UNAVAILABLE,
                "platform_unavailable",
                "No se pudo completar la operación de la plataforma porque un servicio requerido no respondió.",
            ),
        },
    };
    let error_kind = match (status.code(), status.message()) {
        (tonic::Code::Unavailable, "Platform workspace directory is temporarily unavailable") => {
            "workspace_directory_dependency_failure"
        }
        (tonic::Code::Unavailable, "Platform workspace members are temporarily unavailable") => {
            "workspace_members_dependency_failure"
        }
        (tonic::Code::Unavailable, "Platform authorization is temporarily unavailable") => {
            "platform_identity_dependency_failure"
        }
        (tonic::Code::Unavailable, _) => "grpc_dependency_unavailable",
        (tonic::Code::DeadlineExceeded, _) => "grpc_deadline_exceeded",
        (tonic::Code::Unauthenticated, _) => "authentication_rejected",
        (tonic::Code::PermissionDenied, _) => "authorization_rejected",
        (tonic::Code::InvalidArgument, _) => "request_validation_failed",
        (tonic::Code::NotFound, _) => "resource_not_found",
        (tonic::Code::AlreadyExists, _) => "state_conflict",
        (tonic::Code::FailedPrecondition, _) => "precondition_failed",
        (tonic::Code::ResourceExhausted, _) => "rate_limited",
        _ => "grpc_operation_failed",
    };
    let trace_id = tracing::Span::current()
        .context()
        .span()
        .span_context()
        .trace_id()
        .to_string();
    if http_status.is_server_error() {
        tracing::error!(
            event = "platform_console.dependency.failed",
            error_kind,
            error_code = code,
            error_category = error_kind,
            component = "platform_console_bff",
            operation = route_operation,
            dependency = "module_runtime_grpc",
            grpc_code = ?status.code(),
            dependency_outcome = "failed",
            dependency_duration_ms,
            correlation_id,
            trace_id,
            "Platform console request failed"
        );
    } else {
        tracing::warn!(
            event = "platform_console.dependency.rejected",
            error_kind,
            error_code = code,
            error_category = error_kind,
            component = "platform_console_bff",
            operation = route_operation,
            dependency = "module_runtime_grpc",
            grpc_code = ?status.code(),
            dependency_outcome = "rejected",
            dependency_duration_ms,
            correlation_id,
            trace_id,
            "Platform console request rejected"
        );
    }
    let retryable = match status.code() {
        tonic::Code::Unavailable
        | tonic::Code::DeadlineExceeded
        | tonic::Code::ResourceExhausted => Some(true),
        tonic::Code::InvalidArgument
        | tonic::Code::Unauthenticated
        | tonic::Code::PermissionDenied
        | tonic::Code::NotFound
        | tonic::Code::AlreadyExists
        | tonic::Code::FailedPrecondition => Some(false),
        _ => None,
    };
    let response = error_response_with_details(
        http_status,
        code,
        message,
        Some(serde_json::json!({
            "category": error_kind,
            "operation": route_operation,
            "retryable": retryable,
        })),
    );
    attach_error_diagnostic(
        response,
        "module_runtime_grpc",
        error_kind,
        dependency_duration_ms,
        format!("{:?}", status.code()),
    )
}

#[cfg(test)]
mod diagnostic_tests {
    use axum::body::to_bytes;
    use axum::http::StatusCode;

    use super::grpc_error;
    use crate::authentication::ErrorDiagnostic;

    #[tokio::test]
    async fn unavailable_identity_error_is_safely_mapped_and_keeps_diagnostics()
    -> Result<(), Box<dyn std::error::Error>> {
        let response = grpc_error(
            tonic::Status::unavailable("Platform authorization is temporarily unavailable"),
            "9d4ae23c-0bcf-4c3d-ac87-aad4bf322a74",
            "list_platform_workspaces",
            Some(17),
        );
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let diagnostic = response
            .extensions()
            .get::<ErrorDiagnostic>()
            .expect("mapped response retains internal diagnostic context");
        assert_eq!(diagnostic.code, "platform_unavailable");
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
            Some("platform_identity_dependency_failure")
        );
        assert_eq!(diagnostic.dependency_duration_ms, Some(17));
        assert_eq!(diagnostic.upstream_code.as_deref(), Some("Unavailable"));
        assert_eq!(diagnostic.retryable, Some(true));
        let body = to_bytes(response.into_body(), 4096)
            .await
            .expect("error envelope is readable");
        let body: serde_json::Value = serde_json::from_slice(&body)?;
        assert_eq!(body["code"], "platform_unavailable");
        assert_eq!(body["details"]["operation"], "list_platform_workspaces");
        assert_eq!(body["details"]["retryable"], true);
        assert!(!body.to_string().contains("temporarily unavailable"));
        Ok(())
    }
}
