sintaxis llaves
use axum::extract::State;
use axum::http::header;
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::openapi::AppState;

const ALLOWED_METHODS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
const ALLOWED_HEADERS: [&str; 6] = [
    "content-type",
    "x-csrf-token",
    "x-correlation-id",
    "traceparent",
    "tracestate",
    "idempotency-key",
];

pub async fn enforce(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let is_webhook = path == "/api/v1/webhooks/resend";
    if is_webhook && origin.is_some() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let origins = if path == "/platform" || path.starts_with("/platform/") {
        &state.platform_allowed_origins
    } else if path == "/api/v1" || path.starts_with("/api/v1/") {
        &state.tenant_allowed_origins
    } else {
        return next.run(request).await;
    };
    let Some(origin) = origin else {
        return next.run(request).await;
    };
    if !origins.iter().any(|allowed| allowed == &origin) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if request.method() == Method::OPTIONS
        && request
            .headers()
            .contains_key(header::ACCESS_CONTROL_REQUEST_METHOD)
    {
        return preflight_response(request, &origin);
    }
    let mut response = next.run(request).await;
    append_origin_headers(response.headers_mut(), &origin);
    response
}

fn preflight_response(request: axum::extract::Request, origin: &str) -> Response {
    let Some(method) = request
        .headers()
        .get(header::ACCESS_CONTROL_REQUEST_METHOD)
        .and_then(|value| value.to_str().ok())
    else {
        return StatusCode::FORBIDDEN.into_response();
    };
    if !ALLOWED_METHODS.contains(&method) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if let Some(headers) = request
        .headers()
        .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
    {
        let Ok(headers) = headers.to_str() else {
            return StatusCode::FORBIDDEN.into_response();
        };
        if headers.split(',').any(|requested| {
            let requested = requested.trim().to_ascii_lowercase();
            requested.is_empty() || !ALLOWED_HEADERS.contains(&requested.as_str())
        }) {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    append_origin_headers(response.headers_mut(), origin);
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET,POST,PUT,PATCH,DELETE,HEAD,OPTIONS"),
    );
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static(
            "content-type,x-csrf-token,x-correlation-id,traceparent,tracestate,idempotency-key",
        ),
    );
    response.headers_mut().insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("600"),
    );
    append_vary(response.headers_mut(), "Access-Control-Request-Method");
    append_vary(response.headers_mut(), "Access-Control-Request-Headers");
    response
}

fn append_origin_headers(headers: &mut axum::http::HeaderMap, origin: &str) {
    if let Ok(origin) = HeaderValue::from_str(origin) {
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
            HeaderValue::from_static("true"),
        );
        headers.insert(
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            HeaderValue::from_static("x-correlation-id,x-csrf-token,traceparent,tracestate"),
        );
        append_vary(headers, "Origin");
    }
}

fn append_vary(headers: &mut axum::http::HeaderMap, value: &'static str) {
    let current_values = headers
        .get_all(header::VARY)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .collect::<Vec<_>>();
    let mut vary_values = current_values.join(", ");
    if !vary_values
        .split(',')
        .any(|existing| existing.trim().eq_ignore_ascii_case(value))
    {
        if !vary_values.is_empty() {
            vary_values.push_str(", ");
        }
        vary_values.push_str(value);
    }
    if let Ok(vary) = HeaderValue::from_str(&vary_values) {
        headers.insert(header::VARY, vary);
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, header};

    use super::preflight_response;

    #[test]
    fn workspace_create_preflight_allows_idempotency_and_csrf_headers() {
        let request = Request::builder()
            .method("OPTIONS")
            .uri("/api/v1/workspaces")
            .header("Access-Control-Request-Method", "POST")
            .header(
                "Access-Control-Request-Headers",
                "content-type,x-correlation-id,idempotency-key,x-csrf-token",
            )
            .body(Body::empty())
            .expect("valid preflight request");

        let response = preflight_response(request, "https://app-xovyra.jnmtortas.com");

        assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .and_then(|value| value.to_str().ok()),
            Some("https://app-xovyra.jnmtortas.com")
        );
        assert!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.contains("idempotency-key"))
        );
    }

    #[test]
    fn workspace_selection_preflight_allows_put() {
        let request = Request::builder()
            .method("OPTIONS")
            .uri("/api/v1/workspaces/active")
            .header("Access-Control-Request-Method", "PUT")
            .header(
                "Access-Control-Request-Headers",
                "content-type,x-csrf-token",
            )
            .body(Body::empty())
            .expect("valid preflight request");

        let response = preflight_response(request, "https://app-xovyra.jnmtortas.com");

        assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
        assert!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_METHODS)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.contains("PUT"))
        );
    }

    #[test]
    fn preflight_rejects_headers_outside_the_contract() {
        let request = Request::builder()
            .method("OPTIONS")
            .uri("/api/v1/workspaces")
            .header("Access-Control-Request-Method", "POST")
            .header("Access-Control-Request-Headers", "x-unrecognized-header")
            .body(Body::empty())
            .expect("valid preflight request");

        let response = preflight_response(request, "https://app-xovyra.jnmtortas.com");

        assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
    }
}
