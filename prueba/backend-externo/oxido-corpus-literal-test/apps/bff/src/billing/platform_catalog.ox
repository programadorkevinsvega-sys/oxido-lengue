sintaxis llaves
use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tonic::Request;
use utoipa::ToSchema;
use xovyra_platform::grpc::{
    ApprovePlanRequest, CatalogCapabilityInput, CatalogPriceInput, ListPlatformCatalogRequest,
    PlanCatalogVersion, ProfitabilityScenario, ProfitabilityScenarioInput, PublishPlanRequest,
    SavePlanDraftRequest, commercial_billing_client::CommercialBillingClient,
};

use crate::{
    authentication::error_response, openapi::AppState, platform_console::platform_session,
    workspaces::inject_metadata,
};

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DraftBody {
    plan_id: Option<String>,
    plan_key: String,
    display_name: String,
    description: String,
    trial_days: i32,
    prices: Vec<PriceBody>,
    capabilities: Vec<CapabilityBody>,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PriceBody {
    locale: String,
    currency: String,
    amount_minor: i64,
    stripe_price_id: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CapabilityBody {
    key: String,
    limit: i64,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ApprovalBody {
    profitability_evidence: String,
    commercial_gate_reference: String,
    tax_gate_reference: String,
    expected_row_version: i64,
    scenarios: Vec<ProfitabilityBody>,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProfitabilityBody {
    locale: String,
    currency: String,
    provider_expected_minor_per_credit: i64,
    provider_stress_minor_per_credit: i64,
    operating_minor_per_credit: i64,
    payment_fee_basis_points: i32,
    payment_fixed_minor: i64,
    target_margin_basis_points: i32,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PublishBody {
    expected_row_version: i64,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CatalogVersionBody {
    plan_id: String,
    version: i32,
    plan_key: String,
    display_name: String,
    description: String,
    lifecycle: String,
    trial_days: i32,
    profitability_evidence: String,
    commercial_gate_reference: String,
    tax_gate_reference: String,
    prices: Vec<PriceResponse>,
    capabilities: Vec<CapabilityResponse>,
    row_version: i64,
    updated_at: String,
    profitability_scenarios: Vec<ProfitabilityResponse>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct PriceResponse {
    price_id: String,
    locale: String,
    currency: String,
    amount_minor: i64,
    stripe_price_id: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CapabilityResponse {
    key: String,
    limit: i64,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ProfitabilityResponse {
    locale: String,
    currency: String,
    provider_expected_minor_per_credit: i64,
    provider_stress_minor_per_credit: i64,
    operating_minor_per_credit: i64,
    payment_fee_basis_points: i32,
    payment_fixed_minor: i64,
    target_margin_basis_points: i32,
    expected_margin_basis_points: i32,
    stress_margin_basis_points: i32,
}

impl From<ProfitabilityScenario> for ProfitabilityResponse {
    fn from(value: ProfitabilityScenario) -> Self {
        Self {
            locale: value.locale,
            currency: value.currency,
            provider_expected_minor_per_credit: value.provider_expected_minor_per_credit,
            provider_stress_minor_per_credit: value.provider_stress_minor_per_credit,
            operating_minor_per_credit: value.operating_minor_per_credit,
            payment_fee_basis_points: value.payment_fee_basis_points,
            payment_fixed_minor: value.payment_fixed_minor,
            target_margin_basis_points: value.target_margin_basis_points,
            expected_margin_basis_points: value.expected_margin_basis_points,
            stress_margin_basis_points: value.stress_margin_basis_points,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CatalogResponse {
    versions: Vec<CatalogVersionBody>,
    next_cursor: String,
}

impl From<PlanCatalogVersion> for CatalogVersionBody {
    fn from(value: PlanCatalogVersion) -> Self {
        Self {
            plan_id: value.plan_id,
            version: value.version,
            plan_key: value.plan_key,
            display_name: value.display_name,
            description: value.description,
            lifecycle: value.lifecycle,
            trial_days: value.trial_days,
            profitability_evidence: value.profitability_evidence,
            commercial_gate_reference: value.commercial_gate_reference,
            tax_gate_reference: value.tax_gate_reference,
            prices: value
                .prices
                .into_iter()
                .map(|price| PriceResponse {
                    price_id: price.price_id,
                    locale: price.locale,
                    currency: price.currency,
                    amount_minor: price.amount_minor,
                    stripe_price_id: price.stripe_price_id,
                })
                .collect(),
            capabilities: value
                .capabilities
                .into_iter()
                .map(|capability| CapabilityResponse {
                    key: capability.key,
                    limit: capability.limit,
                })
                .collect(),
            row_version: value.row_version,
            updated_at: value.updated_at,
            profitability_scenarios: value
                .profitability_scenarios
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/platform/billing/catalog", get(list))
        .route("/platform/billing/catalog/drafts", post(save_draft))
        .route(
            "/platform/billing/catalog/{plan_id}/{version}/approve",
            post(approve),
        )
        .route(
            "/platform/billing/catalog/{plan_id}/{version}/publish",
            post(publish),
        )
}

#[utoipa::path(get, path="/platform/billing/catalog", responses((status=200, description="Versiones del catálogo comercial", body=CatalogResponse),(status=401),(status=403),(status=503)))]
pub(crate) async fn list(
    State(state): State<AppState>,
    Extension(correlation): Extension<String>,
    headers: HeaderMap,
) -> Response {
    let session = match platform_session(&state, &headers, &correlation, false) {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut client = CommercialBillingClient::new(state.runtime_channel);
    let mut request = Request::new(ListPlatformCatalogRequest {
        session: Some(session),
        cursor: String::new(),
        page_size: 100,
    });
    inject_metadata(&mut request, &correlation);
    match client.list_platform_catalog(request).await {
        Ok(response) => Json(CatalogResponse {
            versions: response
                .into_inner()
                .versions
                .into_iter()
                .map(Into::into)
                .collect(),
            next_cursor: String::new(),
        })
        .into_response(),
        Err(status) => grpc_error(status, &correlation, "list_platform_catalog"),
    }
}

#[utoipa::path(post, path="/platform/billing/catalog/drafts", request_body=DraftBody, responses((status=200, description="Borrador guardado", body=CatalogVersionBody),(status=400),(status=401),(status=403),(status=503)), params(("Idempotency-Key" = String, Header, description="Clave idempotente de 16 a 128 caracteres")))]
pub(crate) async fn save_draft(
    State(state): State<AppState>,
    Extension(correlation): Extension<String>,
    headers: HeaderMap,
    Json(body): Json<DraftBody>,
) -> Response {
    let Some(idempotency_key) = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| (16..=128).contains(&value.len()))
    else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "idempotency_key_required",
            "La solicitud requiere una clave de operación válida.",
        );
    };
    let session = match platform_session(&state, &headers, &correlation, true) {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut client = CommercialBillingClient::new(state.runtime_channel);
    let mut request = Request::new(SavePlanDraftRequest {
        session: Some(session),
        plan_id: body.plan_id.unwrap_or_default(),
        plan_key: body.plan_key,
        display_name: body.display_name,
        description: body.description,
        trial_days: body.trial_days,
        prices: body
            .prices
            .into_iter()
            .map(|price| CatalogPriceInput {
                locale: price.locale,
                currency: price.currency,
                amount_minor: price.amount_minor,
                stripe_price_id: price.stripe_price_id,
            })
            .collect(),
        capabilities: body
            .capabilities
            .into_iter()
            .map(|value| CatalogCapabilityInput {
                key: value.key,
                limit: value.limit,
            })
            .collect(),
        idempotency_key: idempotency_key.to_owned(),
    });
    inject_metadata(&mut request, &correlation);
    match client.save_plan_draft(request).await {
        Ok(response) => Json(CatalogVersionBody::from(response.into_inner())).into_response(),
        Err(status) => grpc_error(status, &correlation, "save_catalog_draft"),
    }
}

#[utoipa::path(post, path="/platform/billing/catalog/{plan_id}/{version}/approve", request_body=ApprovalBody, responses((status=200, description="Versión aprobada", body=CatalogVersionBody),(status=400),(status=401),(status=403),(status=404),(status=409),(status=503)), params(("plan_id" = String, Path),("version" = i32, Path)))]
pub(crate) async fn approve(
    State(state): State<AppState>,
    Extension(correlation): Extension<String>,
    headers: HeaderMap,
    Path((plan_id, version)): Path<(String, i32)>,
    Json(body): Json<ApprovalBody>,
) -> Response {
    let session = match platform_session(&state, &headers, &correlation, true) {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut client = CommercialBillingClient::new(state.runtime_channel);
    let mut request = Request::new(ApprovePlanRequest {
        session: Some(session),
        plan_id,
        version,
        profitability_evidence: body.profitability_evidence,
        commercial_gate_reference: body.commercial_gate_reference,
        tax_gate_reference: body.tax_gate_reference,
        expected_row_version: body.expected_row_version,
        scenarios: body
            .scenarios
            .into_iter()
            .map(|value| ProfitabilityScenarioInput {
                locale: value.locale,
                currency: value.currency,
                provider_expected_minor_per_credit: value.provider_expected_minor_per_credit,
                provider_stress_minor_per_credit: value.provider_stress_minor_per_credit,
                operating_minor_per_credit: value.operating_minor_per_credit,
                payment_fee_basis_points: value.payment_fee_basis_points,
                payment_fixed_minor: value.payment_fixed_minor,
                target_margin_basis_points: value.target_margin_basis_points,
            })
            .collect(),
    });
    inject_metadata(&mut request, &correlation);
    match client.approve_plan(request).await {
        Ok(response) => Json(CatalogVersionBody::from(response.into_inner())).into_response(),
        Err(status) => grpc_error(status, &correlation, "approve_catalog_plan"),
    }
}

#[utoipa::path(post, path="/platform/billing/catalog/{plan_id}/{version}/publish", request_body=PublishBody, responses((status=200, description="Versión publicada", body=CatalogVersionBody),(status=400),(status=401),(status=403),(status=404),(status=409),(status=503)), params(("plan_id" = String, Path),("version" = i32, Path)))]
pub(crate) async fn publish(
    State(state): State<AppState>,
    Extension(correlation): Extension<String>,
    headers: HeaderMap,
    Path((plan_id, version)): Path<(String, i32)>,
    Json(body): Json<PublishBody>,
) -> Response {
    let session = match platform_session(&state, &headers, &correlation, true) {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut client = CommercialBillingClient::new(state.runtime_channel);
    let mut request = Request::new(PublishPlanRequest {
        session: Some(session),
        plan_id,
        version,
        expected_row_version: body.expected_row_version,
    });
    inject_metadata(&mut request, &correlation);
    match client.publish_plan(request).await {
        Ok(response) => Json(CatalogVersionBody::from(response.into_inner())).into_response(),
        Err(status) => grpc_error(status, &correlation, "publish_catalog_plan"),
    }
}

fn grpc_error(status: tonic::Status, correlation: &str, operation: &'static str) -> Response {
    let (http, code, message) = match status.code() {
        tonic::Code::Unauthenticated => (
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Inicia sesión en la consola de plataforma.",
        ),
        tonic::Code::PermissionDenied => (
            StatusCode::FORBIDDEN,
            "platform_billing_permission_required",
            "Tu cuenta no tiene permiso para administrar el catálogo comercial.",
        ),
        tonic::Code::InvalidArgument => (
            StatusCode::BAD_REQUEST,
            "catalog_request_invalid",
            "Revisa los campos del catálogo y vuelve a intentarlo.",
        ),
        tonic::Code::NotFound => (
            StatusCode::NOT_FOUND,
            "catalog_version_not_found",
            "No se encontró esa versión del catálogo.",
        ),
        tonic::Code::FailedPrecondition => (
            StatusCode::CONFLICT,
            "catalog_transition_rejected",
            "La versión cambió o todavía no cumple los requisitos para esta operación.",
        ),
        tonic::Code::Unavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            "catalog_service_unavailable",
            "El catálogo comercial no está disponible ahora. Puedes volver a intentarlo.",
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "catalog_operation_failed",
            "No se pudo completar la operación del catálogo.",
        ),
    };
    tracing::error!(event="billing.admin.http_operation_failed", component="bff", operation, dependency="module_runtime_grpc", correlation_id=%correlation, grpc_code=?status.code(), http_status=http.as_u16(), error_class="grpc_operation_failed");
    error_response(http, code, message)
}
