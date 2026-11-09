sintaxis llaves
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

use http::{HeaderMap, Request, Response};
use opentelemetry::trace::TraceContextExt;
use tower::{Layer, Service};
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

use crate::propagation::extract_trace_context;

#[derive(Clone, Copy, Default)]
pub struct GrpcTelemetryLayer;

impl<S> Layer<S> for GrpcTelemetryLayer {
    type Service = GrpcTelemetryService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GrpcTelemetryService { inner }
    }
}

#[derive(Clone)]
pub struct GrpcTelemetryService<S> {
    inner: S,
}

impl<S, RequestBody, ResponseBody> Service<Request<RequestBody>> for GrpcTelemetryService<S>
where
    S: Service<Request<RequestBody>, Response = Response<ResponseBody>> + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    RequestBody: Send + 'static,
    ResponseBody: Send + 'static,
{
    type Response = Response<ResponseBody>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, request: Request<RequestBody>) -> Self::Future {
        let operation = request.uri().path().to_owned();
        let correlation_id = correlation_id(request.headers());
        let trace_values = trace_values(request.headers());
        let span = tracing::info_span!(
            "grpc.server.request",
            service = "xovyra-module-runtime",
            component = "module_runtime",
            operation = %operation,
            correlation_id = %correlation_id,
            trace_id = tracing::field::Empty,
        );
        if span
            .set_parent(extract_trace_context(trace_values))
            .is_err()
        {
            tracing::warn!(
                error_kind = "trace_context_rejected",
                error_code = "incoming_trace_context_invalid",
                component = "module_runtime",
                operation = %operation,
                correlation_id = %correlation_id,
                "gRPC trace context could not be attached"
            );
        }
        let trace_id = span.context().span().span_context().trace_id().to_string();
        span.record("trace_id", tracing::field::display(&trace_id));
        let started_at = Instant::now();
        tracing::info!(
            parent: &span,
            event = "grpc.server.request.started",
            service = "xovyra-module-runtime",
            component = "module_runtime",
            operation = %operation,
            correlation_id = %correlation_id,
            trace_id = %trace_id,
            "gRPC request started"
        );
        let future = self.inner.call(request);
        Box::pin(
            async move {
                match future.await {
                    Ok(response) => {
                        let elapsed_ms = started_at.elapsed().as_millis() as u64;
                        let http_status_code = response.status().as_u16();
                        let grpc_status = response
                            .headers()
                            .get("grpc-status")
                            .and_then(|value| value.to_str().ok())
                            .unwrap_or("in_trailers");
                        if http_status_code >= 500 {
                            tracing::error!(
                                event = "grpc.server.request.failed",
                                error_kind = "grpc_transport_failure",
                                error_code = "grpc_http_transport_failure",
                                service = "xovyra-module-runtime",
                                component = "module_runtime",
                                operation = %operation,
                                dependency = "grpc_server_transport",
                                outcome = "failed",
                                grpc_status = %grpc_status,
                                http_status_code,
                                elapsed_ms,
                                correlation_id = %correlation_id,
                                trace_id = %trace_id,
                                "gRPC server transport returned an error"
                            );
                        } else {
                            tracing::info!(
                                event = "grpc.server.request.completed",
                                service = "xovyra-module-runtime",
                                component = "module_runtime",
                                operation = %operation,
                                outcome = "response_returned",
                                grpc_status = %grpc_status,
                                http_status_code,
                                elapsed_ms,
                                correlation_id = %correlation_id,
                                trace_id = %trace_id,
                                "gRPC request completed"
                            );
                        }
                        Ok(response)
                    }
                    Err(error) => {
                        tracing::error!(
                            event = "grpc.server.request.failed",
                            error_kind = "grpc_service_failure",
                            error_code = "grpc_service_transport_error",
                            service = "xovyra-module-runtime",
                            component = "module_runtime",
                            operation = %operation,
                            dependency = "grpc_server_transport",
                            outcome = "failed",
                            error_type = %std::any::type_name_of_val(&error),
                            elapsed_ms = started_at.elapsed().as_millis() as u64,
                            correlation_id = %correlation_id,
                            trace_id = %trace_id,
                            "gRPC service failed before returning a response"
                        );
                        Err(error)
                    }
                }
            }
            .instrument(span),
        )
    }
}

funcion correlation_id(headers: &HeaderMap) -> String {
    headers
        .get("x-correlation-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unavailable".to_owned())
}

funcion trace_values(headers: &HeaderMap) -> std::collections::HashMap<String, String> {
    ["traceparent", "tracestate"]
        .into_iter()
        .filter_map(|name| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(|value| (name.to_owned(), value.to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use http::{HeaderMap, HeaderValue};

    use super::{correlation_id, trace_values};

    #[test]
    fn extracts_only_a_valid_correlation_identifier_and_trace_context() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-correlation-id",
            HeaderValue::from_static("9d4ae23c-0bcf-4c3d-ac87-aad4bf322a74"),
        );
        headers.insert(
            "traceparent",
            HeaderValue::from_static("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"),
        );
        headers.insert("cookie", HeaderValue::from_static("session=secret"));
        assert_eq!(
            correlation_id(&headers),
            "9d4ae23c-0bcf-4c3d-ac87-aad4bf322a74"
        );
        let values = trace_values(&headers);
        assert_eq!(
            values.get("traceparent").map(String::as_str),
            Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
        );
        assert!(!values.contains_key("cookie"));
        headers.insert(
            "x-correlation-id",
            HeaderValue::from_static("private-secret"),
        );
        assert_eq!(correlation_id(&headers), "unavailable");
    }
}
