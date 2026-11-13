sintaxis llaves
use axum::Json;
use axum::extract::Extension;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use opentelemetry::trace::TraceContextExt;
use tonic::Request;
use tonic::metadata::MetadataValue;
use tonic::transport::Channel;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use utoipa::OpenApi;
use xovyra_platform::grpc::ReadinessRequest;
use xovyra_platform::grpc::runtime_readiness_client::RuntimeReadinessClient;

use crate::authentication::{attach_error_diagnostic, error_response_with_details};
use crate::health_response::HealthResponse;

#[derive(Clone)]
pub struct AppState {
    pub runtime_channel: Channel,
    pub tenant_allowed_origins: Vec<String>,
    pub platform_allowed_origins: Vec<String>,
    pub secure_cookies: bool,
}

#[utoipa::path(get, path = "/health/live", responses((status = 200, body = HealthResponse)))]
pub async funcion liveness(Extension(correlation_id): Extension<String>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "live",
        correlation_id,
    })
}

#[utoipa::path(get, path = "/api/v1/system/health", responses((status = 200, body = HealthResponse), (status = 503)))]
pub async funcion readiness(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
) -> Response {
    let started_at = std::time::Instant::now();
    let mut client = RuntimeReadinessClient::new(state.runtime_channel);
    let correlation_metadata = MetadataValue::try_from(correlation_id.as_str()).map_err(|_| {
        readiness_failure(
            &correlation_id,
            ReadinessFailure {
                code: "runtime_readiness_correlation_invalid",
                message: "No se pudo validar la referencia de la solicitud.",
                category: "request_validation",
                retryable: false,
                dependency_outcome: "request_metadata_invalid",
                upstream_code: "invalid_correlation_id",
                duration_ms: started_at.elapsed().as_millis() as u64,
            },
        )
    });
    let Ok(correlation_metadata) = correlation_metadata else {
        return correlation_metadata.unwrap_err();
    };
    let mut request = Request::new(ReadinessRequest {
        correlation_id: correlation_id.clone(),
    });
    request
        .metadata_mut()
        .insert("x-correlation-id", correlation_metadata);
    let current_context = tracing::Span::current().context();
    let trace_context = xovyra_platform::propagation::inject_trace_context(&current_context);
    if let Some(traceparent) = trace_context.get("traceparent") {
        let Ok(metadata) = MetadataValue::try_from(traceparent.as_str()) else {
            return readiness_failure(
                &correlation_id,
                ReadinessFailure {
                    code: "runtime_readiness_trace_context_invalid",
                    message: "No se pudo propagar la referencia de trazabilidad.",
                    category: "trace_context_invalid",
                    retryable: false,
                    dependency_outcome: "request_metadata_invalid",
                    upstream_code: "invalid_traceparent",
                    duration_ms: started_at.elapsed().as_millis() as u64,
                },
            );
        };
        request.metadata_mut().insert("traceparent", metadata);
    }
    if let Some(tracestate) = trace_context.get("tracestate") {
        let Ok(metadata) = MetadataValue::try_from(tracestate.as_str()) else {
            return readiness_failure(
                &correlation_id,
                ReadinessFailure {
                    code: "runtime_readiness_trace_state_invalid",
                    message: "No se pudo propagar la referencia de trazabilidad.",
                    category: "trace_context_invalid",
                    retryable: false,
                    dependency_outcome: "request_metadata_invalid",
                    upstream_code: "invalid_tracestate",
                    duration_ms: started_at.elapsed().as_millis() as u64,
                },
            );
        };
        request.metadata_mut().insert("tracestate", metadata);
    }
    let readiness =
        tokio::time::timeout(std::time::Duration::from_secs(3), client.check(request)).await;
    match readiness {
        Ok(Ok(response)) => {
            let result = response.into_inner();
            let expected_trace_id = tracing::Span::current()
                .context()
                .span()
                .span_context()
                .trace_id()
                .to_string();
            if result.ready && result.trace_id == expected_trace_id {
                Json(HealthResponse {
                    status: "ready",
                    correlation_id,
                })
                .into_response()
            } else {
                let (error_code, error_category, retryable, dependency_outcome, upstream_code) =
                    if !result.ready {
                        tracing::error!(
                            event = "bff.readiness.runtime.not_ready",
                            service = "xovyra-bff",
                            component = "readiness_handler",
                            operation = "check_runtime_readiness",
                            error_code = "runtime_dependencies_not_ready",
                            error_category = "dependency_not_ready",
                            dependency = "module_runtime_readiness",
                            dependency_outcome = "not_ready",
                            dependency_duration_ms = started_at.elapsed().as_millis() as u64,
                            retryable = true,
                            correlation_id,
                            trace_id = current_trace_id(),
                            "Module runtime reported that a critical dependency is not ready"
                        );
                        (
                            "runtime_readiness_not_ready",
                            "dependency_not_ready",
                            true,
                            "not_ready",
                            "runtime_ready_false",
                        )
                    } else {
                        tracing::error!(
                            event = "bff.readiness.trace_context.mismatch",
                            service = "xovyra-bff",
                            component = "readiness_handler",
                            operation = "check_runtime_readiness",
                            error_code = "runtime_readiness_trace_mismatch",
                            error_category = "trace_context_mismatch",
                            dependency = "module_runtime_grpc",
                            dependency_outcome = "trace_mismatch",
                            dependency_duration_ms = started_at.elapsed().as_millis() as u64,
                            retryable = false,
                            correlation_id,
                            trace_id = current_trace_id(),
                            "Module runtime readiness trace context did not match the request"
                        );
                        (
                            "runtime_readiness_trace_mismatch",
                            "trace_context_mismatch",
                            false,
                            "trace_mismatch",
                            "trace_id_mismatch",
                        )
                    };
                readiness_failure(
                    &correlation_id,
                    ReadinessFailure {
                        code: error_code,
                        message: "El servicio todavía no está listo. Inténtalo de nuevo en unos momentos.",
                        category: error_category,
                        retryable,
                        dependency_outcome,
                        upstream_code,
                        duration_ms: started_at.elapsed().as_millis() as u64,
                    },
                )
            }
        }
        Ok(Err(status)) => {
            let (error_code, error_category, retryable) = match status.code() {
                tonic::Code::Unavailable => (
                    "runtime_readiness_dependency_unavailable",
                    "dependency_unavailable",
                    true,
                ),
                tonic::Code::DeadlineExceeded => ("runtime_readiness_timeout", "timeout", true),
                tonic::Code::Unauthenticated => (
                    "runtime_readiness_authentication_failed",
                    "authentication",
                    false,
                ),
                tonic::Code::PermissionDenied => (
                    "runtime_readiness_authorization_failed",
                    "authorization",
                    false,
                ),
                tonic::Code::InvalidArgument | tonic::Code::FailedPrecondition => {
                    ("runtime_readiness_protocol_error", "protocol_error", false)
                }
                _ => ("runtime_readiness_rpc_failed", "dependency_failure", false),
            };
            tracing::error!(
                event = "bff.readiness.grpc.failed",
                service = "xovyra-bff",
                component = "readiness_handler",
                operation = "check_runtime_readiness",
                error_code,
                error_category,
                dependency = "module_runtime_grpc",
                dependency_outcome = "failed",
                grpc_code = ?status.code(),
                dependency_duration_ms = started_at.elapsed().as_millis() as u64,
                retryable,
                correlation_id,
                trace_id = current_trace_id(),
                "Runtime readiness gRPC call failed"
            );
            readiness_failure(
                &correlation_id,
                ReadinessFailure {
                    code: error_code,
                    message: "No se pudo consultar el estado del servicio. Inténtalo de nuevo en unos momentos.",
                    category: error_category,
                    retryable,
                    dependency_outcome: "failed",
                    upstream_code: &format!("grpc_{:?}", status.code()),
                    duration_ms: started_at.elapsed().as_millis() as u64,
                },
            )
        }
        Err(_) => {
            tracing::error!(
                event = "bff.readiness.grpc.timeout",
                service = "xovyra-bff",
                component = "readiness_handler",
                operation = "check_runtime_readiness",
                error_code = "runtime_readiness_timeout",
                error_category = "timeout",
                dependency = "module_runtime_grpc",
                dependency_outcome = "timed_out",
                timeout_ms = 3000_u64,
                dependency_duration_ms = started_at.elapsed().as_millis() as u64,
                retryable = true,
                correlation_id,
                trace_id = current_trace_id(),
                "Runtime readiness gRPC call timed out"
            );
            readiness_failure(
                &correlation_id,
                ReadinessFailure {
                    code: "runtime_readiness_timeout",
                    message: "La consulta del estado del servicio agotó el tiempo. Inténtalo de nuevo.",
                    category: "timeout",
                    retryable: true,
                    dependency_outcome: "timed_out",
                    upstream_code: "deadline_exceeded",
                    duration_ms: started_at.elapsed().as_millis() as u64,
                },
            )
        }
    }
}

funcion current_trace_id() -> String {
    tracing::Span::current()
        .context()
        .span()
        .span_context()
        .trace_id()
        .to_string()
}

struct ReadinessFailure<'a> {
    code: &'static str,
    message: &'static str,
    category: &'static str,
    retryable: bool,
    dependency_outcome: &'static str,
    upstream_code: &'a str,
    duration_ms: u64,
}

funcion readiness_failure(correlation_id: &str, failure: ReadinessFailure<'_>) -> Response {
    let response = error_response_with_details(
        StatusCode::SERVICE_UNAVAILABLE,
        failure.code,
        failure.message,
        Some(serde_json::json!({
            "category": failure.category,
            "operation": "check_runtime_readiness",
            "retryable": failure.retryable,
            "request_id": correlation_id,
        })),
    );
    attach_error_diagnostic(
        response,
        "module_runtime_grpc",
        failure.dependency_outcome,
        Some(failure.duration_ms),
        failure.upstream_code,
    )
}

#[cfg(test)]
mod readiness_tests {
    use axum::body::to_bytes;

    use super::{ReadinessFailure, readiness_failure};
    use crate::authentication::ErrorDiagnostic;

    #[tokio::test]
    async fn readiness_failure_keeps_safe_diagnostic_for_final_http_log() {
        let correlation_id = "f73dd4df-4779-4a22-85e9-a6bc624dd29e";
        let response = readiness_failure(
            correlation_id,
            ReadinessFailure {
                code: "runtime_readiness_dependency_unavailable",
                message: "No se pudo consultar el estado del servicio.",
                category: "dependency_unavailable",
                retryable: true,
                dependency_outcome: "failed",
                upstream_code: "grpc_Unavailable",
                duration_ms: 17,
            },
        );
        let diagnostic = response
            .extensions()
            .get::<ErrorDiagnostic>()
            .expect("the final HTTP middleware needs the operation diagnostic");
        assert_eq!(diagnostic.code, "runtime_readiness_dependency_unavailable");
        assert_eq!(diagnostic.category, "dependency_unavailable");
        assert_eq!(
            diagnostic.operation.as_deref(),
            Some("check_runtime_readiness")
        );
        assert_eq!(
            diagnostic.dependency.as_deref(),
            Some("module_runtime_grpc")
        );
        assert_eq!(diagnostic.dependency_outcome.as_deref(), Some("failed"));
        assert_eq!(diagnostic.dependency_duration_ms, Some(17));
        assert_eq!(
            diagnostic.upstream_code.as_deref(),
            Some("grpc_Unavailable")
        );
        assert_eq!(diagnostic.retryable, Some(true));

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body is readable");
        let body: serde_json::Value = serde_json::from_slice(&body).expect("safe JSON error");
        assert_eq!(body["code"], "runtime_readiness_dependency_unavailable");
        assert_eq!(body["details"]["request_id"], correlation_id);
        assert!(!body.to_string().contains("password"));
        assert!(!body.to_string().contains("cookie"));
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        liveness,
        readiness,
        crate::authentication::register,
        crate::authentication::verify_email,
        crate::authentication::resend_verification,
        crate::authentication::request_password_recovery,
        crate::authentication::reset_password,
        crate::authentication::set_marketing_consent,
        crate::authentication::resend_webhook,
        crate::authentication::login,
        crate::authentication::refresh,
        crate::authentication::session,
        crate::authentication::logout,
        crate::account_profile::get_profile,
        crate::account_profile::update_profile,
        crate::account_sessions::list::list_sessions,
        crate::account_sessions::revoke::revoke_session,
        crate::account_sessions::revoke::revoke_others,
        crate::account_password::change_password,
        crate::account_security_events::list_security_events,
        crate::platform_auth::login::login,
        crate::platform_auth::session::current,
        crate::platform_auth::session::refresh,
        crate::platform_auth::session::logout,
        crate::platform_auth::permissions::get_permissions,
        crate::platform_console::directory::list_workspaces,
        crate::platform_console::directory::list_members,
        crate::platform_console::directory::list_assets,
        crate::platform_console::support::list_grants,
        crate::platform_console::support::create_grant,
        crate::platform_console::support::revoke_grant,
        crate::platform_console::support::download_asset,
        crate::platform_console::support::list_audit,
        crate::workspaces::list,
        crate::workspaces::create,
        crate::workspaces::get_active,
        crate::workspaces::set_active
        ,crate::workspaces::get_detail,
        crate::workspaces::update,
        crate::workspaces::delete,
        crate::workspaces::restore
        ,crate::workspace_team::list_members,
        crate::workspace_team::create_invitation,
        crate::workspace_team::list_invitations,
        crate::workspace_team::revoke_invitation,
        crate::workspace_team::accept_invitation,
        crate::workspace_team::list_roles,
        crate::workspace_team::create_role,
        crate::workspace_team::update_role,
        crate::workspace_team::delete_role,
        crate::workspace_team::update_member,
        crate::workspace_team::revoke_member,
        crate::workspace_team::list_tasks,
        crate::workspace_team::create_task,
        crate::workspace_team::update_task,
        crate::workspace_team::list_comments,
        crate::workspace_team::create_comment,
        crate::workspace_team::delete_comment,
        crate::notification_inbox::list,
        crate::notification_inbox::mark_read,
        crate::realtime::tenant_recovery,
        crate::realtime::platform_recovery,
        crate::workspace_team::transfer_ownership
        ,crate::assets::search,
        crate::assets::create_upload,
        crate::assets::complete_upload,
        crate::assets::create_download,
        crate::assets::delete_asset,
        crate::jobs::list,
        crate::jobs::cancel,
        crate::billing::catalog,
        crate::billing::subscription,
        crate::billing::checkout,
        crate::billing::cancel,
        crate::billing::reactivate,
        crate::billing::change_plan,
        crate::billing::platform_catalog::list,
        crate::billing::platform_catalog::save_draft,
        crate::billing::platform_catalog::approve,
        crate::billing::platform_catalog::publish
    ),
    components(schemas(
        HealthResponse,
        crate::authentication::RegisterBody,
        crate::authentication::RegistrationResponseBody,
        crate::authentication::VerifyEmailBody,
        crate::authentication::VerifyEmailResponseBody,
        crate::authentication::EmailActionBody,
        crate::authentication::ResetPasswordBody,
        crate::authentication::MarketingConsentBody,
        crate::authentication::MarketingConsentResponseBody,
        crate::authentication::LoginBody,
        crate::authentication::LoginResponseBody,
        crate::authentication::SessionResponseBody,
        crate::authentication::ErrorResponse,
        crate::account_profile::UpdateProfileBody,
        crate::account_profile::ProfileResponseBody,
        crate::account_sessions::SessionSummaryBody,
        crate::account_sessions::SessionListBody,
        crate::account_sessions::SessionRevocationBody,
        crate::account_sessions::RevokeOthersBody,
        crate::account_password::ChangePasswordBody,
        crate::account_password::PasswordChangedBody,
        crate::account_security_events::SecurityEventBody,
        crate::account_security_events::SecurityEventListBody,
        crate::platform_auth::PlatformLoginBody,
        crate::platform_auth::PlatformLoginResponseBody,
        crate::platform_auth::PlatformSessionResponseBody,
        crate::platform_auth::PlatformRefreshResponseBody,
        crate::platform_auth::PlatformPermissionsBody,
        crate::platform_console::support::CreateGrantBody,
        crate::platform_console::support::RevokeGrantBody,
        crate::workspaces::CreateWorkspaceBody,
        crate::workspaces::SetActiveWorkspaceBody,
        crate::workspaces::UpdateWorkspaceBody,
        crate::workspaces::DeleteWorkspaceBody,
        crate::workspaces::WorkspaceDeletionBody,
        crate::workspaces::WorkspaceBody,
        crate::workspaces::WorkspaceListBody,
        crate::workspaces::ActiveWorkspaceBody,
        crate::workspace_team::RoleBody,
        crate::workspace_team::InviteBody,
        crate::workspace_team::MemberBody,
        crate::workspace_team::TaskBody,
        crate::workspace_team::TaskUpdateBody,
        crate::workspace_team::CommentBody,
        crate::workspace_team::AcceptBody,
        crate::workspace_team::TransferBody,
        crate::assets::UploadBody,
        crate::billing::CatalogQuery,
        crate::billing::CheckoutBody,
        crate::billing::PlanBody,
        crate::billing::CapabilityBody,
        crate::billing::SubscriptionBody,
        crate::billing::CheckoutResponseBody,
        crate::billing::platform_catalog::DraftBody,
        crate::billing::platform_catalog::PriceBody,
        crate::billing::platform_catalog::CapabilityBody,
        crate::billing::platform_catalog::ApprovalBody,
        crate::billing::platform_catalog::ProfitabilityBody,
        crate::billing::platform_catalog::PublishBody,
        crate::billing::platform_catalog::CatalogResponse,
        crate::billing::platform_catalog::CatalogVersionBody,
        crate::billing::platform_catalog::PriceResponse,
        crate::billing::platform_catalog::CapabilityResponse,
        crate::billing::platform_catalog::ProfitabilityResponse
    )),
    tags((name = "system", description = "Runtime status contracts"))
)]
pub struct ApiDocument;

#[cfg(test)]
mod billing_catalog_openapi_tests {
    use super::ApiDocument;
    use utoipa::OpenApi;

    #[test]
    fn platform_catalog_routes_are_in_the_published_openapi_document() {
        let paths = ApiDocument::openapi().paths.paths;
        for path in [
            "/platform/billing/catalog",
            "/platform/billing/catalog/drafts",
            "/platform/billing/catalog/{plan_id}/{version}/approve",
            "/platform/billing/catalog/{plan_id}/{version}/publish",
        ] {
            assert!(paths.contains_key(path), "OpenAPI is missing {path}");
        }
    }
}
