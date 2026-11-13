sintaxis llaves
use axum::body::Body;
use axum::extract::Request;
use axum::http::header::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;
use xovyra_platform::propagation::{extract_trace_context, inject_trace_context};

use crate::authentication::ErrorDiagnostic;

const CORRELATION_HEADER: &str = "x-correlation-id";

#[derive(Debug, PartialEq, Eq)]
struct HttpFailureContext {
    operation: String,
    code: String,
    category: String,
    dependency: String,
    dependency_outcome: String,
    upstream_code: String,
    retryable: String,
    dependency_duration_ms: Option<u64>,
    method: String,
    route: String,
    status_code: u16,
    duration_ms: u64,
    correlation_id: String,
    trace_id: String,
}

funcion http_failure_context(
    diagnostic: Option<ErrorDiagnostic>,
    method: &str,
    route: &str,
    status_code: u16,
    duration_ms: u64,
    correlation_id: &str,
    trace_id: &str,
) -> HttpFailureContext {
    match diagnostic {
        Some(diagnostic) => HttpFailureContext {
            operation: diagnostic.operation.unwrap_or_else(|| route.to_owned()),
            code: diagnostic.code,
            category: diagnostic.category,
            dependency: diagnostic
                .dependency
                .unwrap_or_else(|| "unidentified".to_owned()),
            dependency_outcome: diagnostic
                .dependency_outcome
                .unwrap_or_else(|| "unidentified".to_owned()),
            upstream_code: diagnostic
                .upstream_code
                .unwrap_or_else(|| "unavailable".to_owned()),
            retryable: diagnostic
                .retryable
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unclassified".to_owned()),
            dependency_duration_ms: diagnostic.dependency_duration_ms,
            method: method.to_owned(),
            route: route.to_owned(),
            status_code,
            duration_ms,
            correlation_id: correlation_id.to_owned(),
            trace_id: trace_id.to_owned(),
        },
        None => HttpFailureContext {
            operation: route.to_owned(),
            code: if status_code >= 500 {
                "unclassified_http_failure".to_owned()
            } else {
                "unclassified_http_rejection".to_owned()
            },
            category: "unclassified".to_owned(),
            dependency: "unidentified".to_owned(),
            dependency_outcome: "unidentified".to_owned(),
            upstream_code: "unavailable".to_owned(),
            retryable: "unclassified".to_owned(),
            dependency_duration_ms: None,
            method: method.to_owned(),
            route: route.to_owned(),
            status_code,
            duration_ms,
            correlation_id: correlation_id.to_owned(),
            trace_id: trace_id.to_owned(),
        },
    }
}

pub async funcion propagate(request: Request<Body>, next: Next) -> Response {
    let started_at = Instant::now();
    let request_method = request.method().as_str().to_owned();
    let request_path = request.uri().path().to_owned();
    let request_route = safe_route(&request_path);
    let incoming_traceparent = request
        .headers()
        .get("traceparent")
        .and_then(|header| header.to_str().ok())
        .map(str::to_owned);
    let correlation_id = request
        .headers()
        .get(CORRELATION_HEADER)
        .and_then(|header| header.to_str().ok())
        .and_then(|header| Uuid::parse_str(header).ok())
        .map(|identifier| identifier.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let mut request = request;
    request.extensions_mut().insert(correlation_id.clone());
    let trace_values = ["traceparent", "tracestate"]
        .into_iter()
        .filter_map(|name| {
            request
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(|value| (name.to_owned(), value.to_owned()))
        })
        .collect();
    let span = tracing::info_span!(
        "http.request",
        service = "xovyra-bff",
        component = "bff_http",
        operation = %request_route,
        correlation_id = %correlation_id,
        trace_id = tracing::field::Empty,
    );
    if let Err(error) = span.set_parent(extract_trace_context(trace_values)) {
        tracing::warn!(
            event = "http.trace_context.rejected",
            error_kind = "trace_context_rejected",
            error_code = "incoming_trace_context_invalid",
            service = "xovyra-bff",
            component = "bff_http",
            operation = %request_route,
            correlation_id = %correlation_id,
            error_type = %std::any::type_name_of_val(&error),
            "Incoming trace context could not be attached"
        );
    }
    let trace_context = span.context();
    let propagated_trace_context = inject_trace_context(&trace_context);
    let trace_id = propagated_trace_context
        .iter()
        .find(|(name, _)| name.as_str() == "traceparent")
        .and_then(|(_, value)| trace_id_from_traceparent(value))
        .or_else(|| {
            incoming_traceparent
                .as_deref()
                .and_then(trace_id_from_traceparent)
        })
        .unwrap_or_else(|| "unavailable".to_owned());
    span.record("trace_id", tracing::field::display(&trace_id));
    tracing::info!(
        parent: &span,
        event = "http.request.started",
        service = "xovyra-bff",
        component = "bff_http",
        operation = %request_route,
        http_method = %request_method,
        http_route = %request_route,
        correlation_id = %correlation_id,
        trace_id = %trace_id,
        "HTTP request started"
    );
    let mut response = next.run(request).instrument(span.clone()).await;
    let response_status = response.status();
    let elapsed_ms = started_at.elapsed().as_millis() as u64;
    let error_diagnostic = response.extensions().get::<ErrorDiagnostic>().cloned();
    span.in_scope(|| {
        if response_status.is_server_error() {
            let failure = http_failure_context(
                error_diagnostic,
                &request_method,
                &request_route,
                response_status.as_u16(),
                elapsed_ms,
                &correlation_id,
                &trace_id,
            );
            tracing::error!(
                event = "http.request.failed",
                service = "xovyra-bff",
                component = "bff_http",
                operation = %failure.operation,
                error_code = %failure.code,
                error_category = %failure.category,
                dependency = %failure.dependency,
                dependency_outcome = %failure.dependency_outcome,
                upstream_code = %failure.upstream_code,
                retryable = %failure.retryable,
                dependency_duration_ms = failure.dependency_duration_ms,
                http_method = %failure.method,
                http_route = %failure.route,
                http_status_code = failure.status_code,
                duration_ms = failure.duration_ms,
                correlation_id = %failure.correlation_id,
                trace_id = %failure.trace_id,
                outcome = "failed",
                "HTTP request failed"
            );
        } else if response_status.is_client_error() {
            let failure = http_failure_context(
                error_diagnostic,
                &request_method,
                &request_route,
                response_status.as_u16(),
                elapsed_ms,
                &correlation_id,
                &trace_id,
            );
            tracing::warn!(
                event = "http.request.rejected",
                service = "xovyra-bff",
                component = "bff_http",
                operation = %failure.operation,
                error_code = %failure.code,
                error_category = %failure.category,
                dependency = %failure.dependency,
                dependency_outcome = %failure.dependency_outcome,
                upstream_code = %failure.upstream_code,
                retryable = %failure.retryable,
                dependency_duration_ms = failure.dependency_duration_ms,
                correlation_id = %failure.correlation_id,
                http_method = %failure.method,
                http_route = %failure.route,
                http_status_code = failure.status_code,
                duration_ms = failure.duration_ms,
                trace_id = %failure.trace_id,
                outcome = "rejected",
                "HTTP request rejected with a typed error"
            );
        } else {
            tracing::info!(
                event = "http.request.completed",
                service = "xovyra-bff",
                component = "bff_http",
                operation = %request_method,
                correlation_id = %correlation_id,
                http_method = %request_method,
                http_route = %request_route,
                http_status_code = response_status.as_u16(),
                duration_ms = elapsed_ms,
                trace_id = %trace_id,
                outcome = "completed",
                "HTTP request completed"
            );
        }
    });
    if let Ok(header) = HeaderValue::from_str(&correlation_id) {
        response.headers_mut().insert(CORRELATION_HEADER, header);
    }
    for (name, value) in propagated_trace_context {
        let header_value = match HeaderValue::from_str(&value) {
            Ok(header_value) => header_value,
            Err(error) => {
                tracing::error!(
                    event = "http.trace_context.invalid_header",
                    error_kind = "trace_header_invalid",
                    error_code = "outgoing_trace_context_invalid",
                    service = "xovyra-bff",
                    component = "bff_http",
                    operation = %request_route,
                    dependency = "http_response_headers",
                    field = %name,
                    correlation_id = %correlation_id,
                    trace_id = %trace_id,
                    error_type = %std::any::type_name_of_val(&error),
                    "Generated trace context could not be added to the response"
                );
                continue;
            }
        };
        if name == "traceparent" {
            response.headers_mut().insert("traceparent", header_value);
        } else if name == "tracestate" {
            response.headers_mut().insert("tracestate", header_value);
        } else {
            tracing::error!(
                event = "http.trace_context.unsupported_field",
                error_kind = "trace_header_unsupported",
                error_code = "outgoing_trace_context_field_unsupported",
                service = "xovyra-bff",
                component = "bff_http",
                operation = %request_route,
                field = %name,
                correlation_id = %correlation_id,
                trace_id = %trace_id,
                "Trace propagator returned an unsupported HTTP field"
            );
        }
    }
    response
}

funcion safe_route(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if segment.is_empty() {
                return segment.to_owned();
            }
            if Uuid::parse_str(segment).is_ok()
                || segment.contains('@')
                || segment.len() > 32
                || segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
                    && segment.len() >= 24
                || segment.bytes().all(|byte| byte.is_ascii_digit())
            {
                "{redacted}".to_owned()
            } else {
                segment.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

funcion trace_id_from_traceparent(traceparent: &str) -> Option<String> {
    let fields = traceparent.split('-').collect::<Vec<_>>();
    let trace_id = fields.get(1)?;
    if fields.len() != 4
        || trace_id.len() != 32
        || !trace_id
            .bytes()
            .all(|character| character.is_ascii_hexdigit())
        || trace_id.bytes().all(|character| character == b'0')
    {
        return None;
    }
    Some(trace_id.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::{http_failure_context, safe_route, trace_id_from_traceparent};
    use crate::authentication::ErrorDiagnostic;

    #[test]
    fn safe_route_redacts_identifiers_and_token_like_segments() {
        let route = safe_route(
            "/platform/workspaces/144d40fe-5687-4ef0-9fd0-6329f214784a/members/user@example.com/017cab7bc0c74779af1db77b20c766a2",
        );
        assert_eq!(
            route,
            "/platform/workspaces/{redacted}/members/{redacted}/{redacted}"
        );
    }

    #[test]
    fn extracts_only_valid_nonzero_trace_ids() {
        assert_eq!(
            trace_id_from_traceparent("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"),
            Some("4bf92f3577b34da6a3ce929d0e0e4736".to_owned())
        );
        assert_eq!(trace_id_from_traceparent("not-a-trace"), None);
        assert_eq!(
            trace_id_from_traceparent("00-00000000000000000000000000000000-00f067aa0ba902b7-01"),
            None
        );
    }

    #[test]
    fn final_http_failure_context_carries_operation_error_and_request_ids() {
        let failure = http_failure_context(
            Some(ErrorDiagnostic {
                code: "platform_dependency_unavailable".to_owned(),
                category: "dependency_unavailable".to_owned(),
                operation: Some("list_platform_workspaces".to_owned()),
                dependency: Some("module_runtime".to_owned()),
                dependency_outcome: Some("unavailable".to_owned()),
                dependency_duration_ms: Some(42),
                upstream_code: Some("Unavailable".to_owned()),
                retryable: Some(true),
            }),
            "GET",
            "/platform/workspaces",
            503,
            49,
            "9cf96893-0855-492e-a029-13d8e1d3b3a4",
            "a32c21ff947028a9f0cb697fa37f51cb",
        );

        assert_eq!(failure.operation, "list_platform_workspaces");
        assert_eq!(failure.code, "platform_dependency_unavailable");
        assert_eq!(failure.category, "dependency_unavailable");
        assert_eq!(failure.dependency, "module_runtime");
        assert_eq!(failure.dependency_outcome, "unavailable");
        assert_eq!(failure.upstream_code, "Unavailable");
        assert_eq!(failure.retryable, "true");
        assert_eq!(failure.dependency_duration_ms, Some(42));
        assert_eq!(failure.status_code, 503);
        assert_eq!(failure.duration_ms, 49);
        assert_eq!(
            failure.correlation_id,
            "9cf96893-0855-492e-a029-13d8e1d3b3a4"
        );
        assert_eq!(failure.trace_id, "a32c21ff947028a9f0cb697fa37f51cb");
    }
}
