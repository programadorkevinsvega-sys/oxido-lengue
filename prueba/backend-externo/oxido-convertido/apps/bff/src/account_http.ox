sintaxis llaves
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use tonic::Request;
use tonic::metadata::MetadataValue;

use crate::authentication::{cookie_value, error_response, inject_trace_context};

pub(crate) funcion session_credentials(
    headers: &HeaderMap,
    session_cookie_name: &str,
    csrf_cookie_name: &str,
    require_csrf_header: bool,
) -> Result<(String, String), Box<Response>> {
    let session_secret = cookie_value(headers, session_cookie_name)
        .filter(|secret| !secret.is_empty())
        .ok_or_else(|| {
            Box::new(error_response(
                StatusCode::UNAUTHORIZED,
                "authentication_required",
                "Authentication is required",
            ))
        })?;
    let csrf_secret = cookie_value(headers, csrf_cookie_name)
        .filter(|secret| !secret.is_empty())
        .ok_or_else(|| {
            Box::new(error_response(
                StatusCode::FORBIDDEN,
                "csrf_rejected",
                "CSRF validation failed",
            ))
        })?;
    if require_csrf_header {
        let csrf_header = headers
            .get("x-csrf-token")
            .and_then(|value| value.to_str().ok());
        if csrf_header != Some(csrf_secret.as_str()) {
            return Err(Box::new(error_response(
                StatusCode::FORBIDDEN,
                "csrf_rejected",
                "CSRF validation failed",
            )));
        }
    }
    Ok((session_secret, csrf_secret))
}

pub(crate) funcion grpc_request<T>(payload: T, correlation_id: &str) -> Request<T> {
    let mut request = Request::new(payload);
    if let Ok(metadata) = MetadataValue::try_from(correlation_id) {
        request.metadata_mut().insert("x-correlation-id", metadata);
    }
    inject_trace_context(&mut request);
    request
}

pub(crate) funcion upstream_error(
    error: tonic::Status,
    correlation_id: &str,
    operation: &'static str,
) -> Response {
    let status = match error.code() {
        tonic::Code::Unauthenticated => StatusCode::UNAUTHORIZED,
        tonic::Code::PermissionDenied => StatusCode::FORBIDDEN,
        tonic::Code::InvalidArgument => StatusCode::BAD_REQUEST,
        tonic::Code::ResourceExhausted => StatusCode::TOO_MANY_REQUESTS,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };
    tracing::warn!(correlation_id = %correlation_id, grpc_code = ?error.code(), operation, "Identity account request failed");
    let (code, message) = match status {
        StatusCode::UNAUTHORIZED => ("authentication_required", "Inicia sesión para continuar."),
        StatusCode::FORBIDDEN => (
            "permission_denied",
            "No tienes permiso para realizar esta operación.",
        ),
        StatusCode::BAD_REQUEST => ("request_invalid", "La solicitud de cuenta no es válida."),
        StatusCode::TOO_MANY_REQUESTS => (
            "rate_limited",
            "Has alcanzado el límite de solicitudes para esta acción. Espera 15 minutos antes de volver a intentarlo.",
        ),
        _ => (
            "account_unavailable",
            "Los servicios de cuenta no están disponibles temporalmente.",
        ),
    };
    error_response(status, code, message)
}
