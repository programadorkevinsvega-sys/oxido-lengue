sintaxis llaves
use axum::{
    Json, Router,
    body::Bytes,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tonic::Request;
use xovyra_platform::grpc::{
    AccountSessionRequest, ApplyStripeWebhookRequest, AuthenticationAudience,
    CancelSubscriptionRequest, ChangeSubscriptionPlanRequest, CreateCheckoutSessionRequest,
    GetWorkspaceSubscriptionRequest, ListPublishedPlansRequest, ReactivateSubscriptionRequest,
    commercial_billing_client::CommercialBillingClient,
};

use crate::{
    authentication::{
        TENANT_CSRF_COOKIE, TENANT_SESSION_COOKIE, cookie_value, error_response, valid_origin,
    },
    openapi::AppState,
    workspaces::{authenticate, inject_metadata},
};

pub(crate) mod platform_catalog;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CatalogQuery {
    locale: Option<String>,
    currency: Option<String>,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckoutBody {
    plan_id: String,
    plan_version: i32,
    locale: String,
    currency: String,
}
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct PlanBody {
    plan_id: String,
    version: i32,
    key: String,
    name: String,
    description: String,
    trial_days: i32,
    locale: String,
    currency: String,
    amount_minor: i64,
    capabilities: Vec<CapabilityBody>,
}
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct CapabilityBody {
    key: String,
    limit: i64,
}
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct SubscriptionBody {
    subscription_id: String,
    workspace_id: String,
    plan_key: String,
    plan_version: i32,
    status: String,
    current_period_start_unix: i64,
    current_period_end_unix: i64,
    cancel_at_period_end: bool,
    row_version: i64,
    scheduled_plan_key: String,
    scheduled_plan_version: i32,
}
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct CheckoutResponseBody {
    checkout_url: String,
}

#[derive(Serialize)]
struct StripeWebhookResponseBody {
    accepted: bool,
    duplicate: bool,
}

funcion subscription_body(
    value: xovyra_platform::grpc::WorkspaceSubscriptionResponse,
) -> SubscriptionBody {
    SubscriptionBody {
        subscription_id: value.subscription_id,
        workspace_id: value.workspace_id,
        plan_key: value.plan_key,
        plan_version: value.plan_version,
        status: value.status,
        current_period_start_unix: value.current_period_start_unix,
        current_period_end_unix: value.current_period_end_unix,
        cancel_at_period_end: value.cancel_at_period_end,
        row_version: value.row_version,
        scheduled_plan_key: value.scheduled_plan_key,
        scheduled_plan_version: value.scheduled_plan_version,
    }
}

pub funcion router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/billing/catalog", get(catalog))
        .route(
            "/api/v1/workspaces/{workspace_id}/subscription",
            get(subscription),
        )
        .route(
            "/api/v1/workspaces/{workspace_id}/subscription/checkout",
            post(checkout),
        )
        .route(
            "/api/v1/workspaces/{workspace_id}/subscription/cancel",
            post(cancel),
        )
        .route(
            "/api/v1/workspaces/{workspace_id}/subscription/reactivate",
            post(reactivate),
        )
        .route(
            "/api/v1/workspaces/{workspace_id}/subscription/change-plan",
            post(change_plan),
        )
        .route("/api/v1/webhooks/stripe", post(stripe_webhook))
        .merge(platform_catalog::router())
}

pub async funcion stripe_webhook(
    State(state): State<AppState>,
    Extension(correlation): Extension<String>,
    headers: HeaderMap,
    payload: Bytes,
) -> Response {
    if payload.is_empty() || payload.len() > 1_000_000 {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "stripe_webhook_payload_invalid",
            "Stripe envió una notificación con un tamaño no permitido.",
        );
    }
    let Some(signature) = headers
        .get("stripe-signature")
        .and_then(|value| value.to_str().ok())
    else {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "stripe_webhook_signature_missing",
            "No se pudo validar la notificación de pago.",
        );
    };
    let mut client = CommercialBillingClient::new(state.runtime_channel);
    let mut request = Request::new(ApplyStripeWebhookRequest {
        signature: signature.to_owned(),
        payload: payload.to_vec(),
        correlation_id: correlation.clone(),
    });
    inject_metadata(&mut request, &correlation);
    match client.apply_stripe_webhook(request).await {
        Ok(result) => {
            let result = result.into_inner();
            Json(StripeWebhookResponseBody {
                accepted: result.accepted,
                duplicate: result.duplicate,
            })
            .into_response()
        }
        Err(status) => billing_error(status, &correlation, "stripe_webhook_unavailable"),
    }
}

#[utoipa::path(get, path="/api/v1/billing/catalog", params(("locale" = Option<String>, Query), ("currency" = Option<String>, Query)), responses((status=200, description="Planes publicados"),(status=503)))]
pub async fn catalog(
    State(state): State<AppState>,
    Extension(correlation): Extension<String>,
    Query(query): Query<CatalogQuery>,
) -> Response {
    let locale = query.locale.unwrap_or_else(|| "es-CO".to_owned());
    let currency = query
        .currency
        .unwrap_or_else(|| "USD".to_owned())
        .to_uppercase();
    if locale.len() > 16 || currency.len() != 3 {
        return error_response(
            StatusCode::BAD_REQUEST,
            "billing_catalog_query_invalid",
            "Revisa el idioma y la moneda seleccionados.",
        );
    }
    let mut client = CommercialBillingClient::new(state.runtime_channel);
    let mut request = Request::new(ListPublishedPlansRequest {
        locale,
        currency,
        correlation_id: correlation.clone(),
    });
    inject_metadata(&mut request, &correlation);
    match client.list_published_plans(request).await {
        Ok(result) => Json(
            result
                .into_inner()
                .plans
                .into_iter()
                .map(|plan| PlanBody {
                    plan_id: plan.plan_id,
                    version: plan.version,
                    key: plan.plan_key,
                    name: plan.display_name,
                    description: plan.description,
                    trial_days: plan.trial_days,
                    locale: plan.locale,
                    currency: plan.currency,
                    amount_minor: plan.amount_minor,
                    capabilities: plan
                        .capabilities
                        .into_iter()
                        .map(|capability| CapabilityBody {
                            key: capability.key,
                            limit: capability.limit,
                        })
                        .collect(),
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(status) => billing_error(status, &correlation, "billing_catalog_unavailable"),
    }
}

#[utoipa::path(get,path="/api/v1/workspaces/{workspace_id}/subscription",params(("workspace_id"=String,Path)),responses((status=200),(status=401),(status=403),(status=404),(status=503)))]
pub async fn subscription(
    State(state): State<AppState>,
    Extension(correlation): Extension<String>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
) -> Response {
    let session = match tenant_session(&state, &headers, &correlation, false).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut client = CommercialBillingClient::new(state.runtime_channel);
    let mut request = Request::new(GetWorkspaceSubscriptionRequest {
        session: Some(session),
        workspace_id: workspace,
    });
    inject_metadata(&mut request, &correlation);
    match client.get_workspace_subscription(request).await {
        Ok(value) => Json(subscription_body(value.into_inner())).into_response(),
        Err(status) => billing_error(status, &correlation, "workspace_subscription_unavailable"),
    }
}

#[utoipa::path(post,path="/api/v1/workspaces/{workspace_id}/subscription/checkout",params(("workspace_id"=String,Path)),request_body=CheckoutBody,responses((status=200),(status=400),(status=403),(status=409),(status=503)))]
pub async fn checkout(
    State(state): State<AppState>,
    Extension(correlation): Extension<String>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
    Json(body): Json<CheckoutBody>,
) -> Response {
    let Some(key) = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .filter(|v| (16..=128).contains(&v.len()))
    else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "idempotency_key_required",
            "La solicitud requiere una clave de operación válida.",
        );
    };
    let session = match tenant_session(&state, &headers, &correlation, true).await {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "La solicitud viene de un origen no autorizado.",
        );
    }
    let mut client = CommercialBillingClient::new(state.runtime_channel);
    let mut request = Request::new(CreateCheckoutSessionRequest {
        session: Some(session),
        workspace_id: workspace,
        plan_id: body.plan_id,
        plan_version: body.plan_version,
        locale: body.locale,
        currency: body.currency,
        idempotency_key: key.to_owned(),
    });
    inject_metadata(&mut request, &correlation);
    match client.create_checkout_session(request).await {
        Ok(value) => Json(CheckoutResponseBody {
            checkout_url: value.into_inner().checkout_url,
        })
        .into_response(),
        Err(status) => billing_error(status, &correlation, "billing_checkout_unavailable"),
    }
}

async funcion tenant_session(
    state: &AppState,
    headers: &HeaderMap,
    correlation: &str,
    csrf: bool,
) -> Result<AccountSessionRequest, Box<Response>> {
    authenticate(state, headers, correlation, csrf).await?;
    let session_secret = cookie_value(headers, TENANT_SESSION_COOKIE).ok_or_else(|| {
        error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Inicia sesión para continuar.",
        )
    })?;
    let csrf_secret = cookie_value(headers, TENANT_CSRF_COOKIE).ok_or_else(|| {
        error_response(
            StatusCode::FORBIDDEN,
            "csrf_rejected",
            "Actualiza la página e inténtalo de nuevo.",
        )
    })?;
    Ok(AccountSessionRequest {
        session_secret,
        csrf_secret,
        audience: AuthenticationAudience::Tenant as i32,
        correlation_id: correlation.to_owned(),
    })
}

fn billing_error(
    status: tonic::Status,
    correlation: &str,
    unavailable_code: &'static str,
) -> Response {
    let (code, message) = match status.code() {
        tonic::Code::InvalidArgument => (
            StatusCode::BAD_REQUEST,
            "Revisa los datos de facturación e inténtalo de nuevo.",
        ),
        tonic::Code::Unauthenticated => (StatusCode::UNAUTHORIZED, "Inicia sesión para continuar."),
        tonic::Code::PermissionDenied => (
            StatusCode::FORBIDDEN,
            "No tienes permiso de facturación en este espacio.",
        ),
        tonic::Code::NotFound => (
            StatusCode::NOT_FOUND,
            "No hay una suscripción disponible para este espacio.",
        ),
        tonic::Code::AlreadyExists => (
            StatusCode::CONFLICT,
            "Este espacio ya tiene una suscripción.",
        ),
        tonic::Code::Aborted => (
            StatusCode::CONFLICT,
            "La suscripción cambió. Actualiza la información e inténtalo de nuevo.",
        ),
        tonic::Code::ResourceExhausted => (
            StatusCode::TOO_MANY_REQUESTS,
            "Se alcanzó el límite temporal de cambios de facturación de este espacio. Inténtalo más tarde.",
        ),
        tonic::Code::FailedPrecondition => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "La operación todavía no está disponible para esta configuración.",
        ),
        _ => {
            tracing::error!(event="billing.bff.grpc_failed",component="bff",operation="forward_billing_request",dependency="module_runtime",correlation_id=%correlation,grpc_code=?status.code());
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "El servicio de facturación no está disponible ahora. Inténtalo de nuevo.",
            )
        }
    };
    error_response(
        code,
        if code == StatusCode::SERVICE_UNAVAILABLE {
            unavailable_code
        } else {
            "billing_request_rejected"
        },
        message,
    )
}

#[utoipa::path(post,path="/api/v1/workspaces/{workspace_id}/subscription/cancel",params(("workspace_id"=String,Path)),responses((status=200),(status=401),(status=403),(status=404),(status=503)))]
pub async fn cancel(
    State(state): State<AppState>,
    Extension(correlation): Extension<String>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
) -> Response {
    mutate_subscription(state, correlation, headers, workspace, true).await
}
#[utoipa::path(post,path="/api/v1/workspaces/{workspace_id}/subscription/reactivate",params(("workspace_id"=String,Path)),responses((status=200),(status=401),(status=403),(status=404),(status=503)))]
pub async fn reactivate(
    State(state): State<AppState>,
    Extension(correlation): Extension<String>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
) -> Response {
    mutate_subscription(state, correlation, headers, workspace, false).await
}

#[utoipa::path(post,path="/api/v1/workspaces/{workspace_id}/subscription/change-plan",params(("workspace_id"=String,Path)),request_body=CheckoutBody,responses((status=200),(status=400),(status=403),(status=404),(status=503)))]
pub async fn change_plan(
    State(state): State<AppState>,
    Extension(correlation): Extension<String>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
    Json(body): Json<CheckoutBody>,
) -> Response {
    let Some(key) = headers
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
    let session = match tenant_session(&state, &headers, &correlation, true).await {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "La solicitud viene de un origen no autorizado.",
        );
    }
    let mut client = CommercialBillingClient::new(state.runtime_channel);
    let mut request = Request::new(ChangeSubscriptionPlanRequest {
        session: Some(session),
        workspace_id: workspace,
        plan_id: body.plan_id,
        plan_version: body.plan_version,
        locale: body.locale,
        currency: body.currency,
        idempotency_key: key.to_owned(),
    });
    inject_metadata(&mut request, &correlation);
    match client.change_subscription_plan(request).await {
        Ok(value) => Json(subscription_body(value.into_inner())).into_response(),
        Err(status) => billing_error(status, &correlation, "billing_plan_change_unavailable"),
    }
}
async funcion mutate_subscription(
    state: AppState,
    correlation: String,
    headers: HeaderMap,
    workspace: String,
    cancel: bool,
) -> Response {
    let Some(key) = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .filter(|v| (16..=128).contains(&v.len()))
    else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "idempotency_key_required",
            "La solicitud requiere una clave de operación válida.",
        );
    };
    let session = match tenant_session(&state, &headers, &correlation, true).await {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "La solicitud viene de un origen no autorizado.",
        );
    }
    let mut client = CommercialBillingClient::new(state.runtime_channel);
    if cancel {
        let mut request = Request::new(CancelSubscriptionRequest {
            session: Some(session),
            workspace_id: workspace,
            idempotency_key: key.to_owned(),
        });
        inject_metadata(&mut request, &correlation);
        match client.cancel_subscription(request).await {
            Ok(value) => Json(subscription_body(value.into_inner())).into_response(),
            Err(status) => billing_error(status, &correlation, "billing_cancel_unavailable"),
        }
    } else {
        let mut request = Request::new(ReactivateSubscriptionRequest {
            session: Some(session),
            workspace_id: workspace,
            idempotency_key: key.to_owned(),
        });
        inject_metadata(&mut request, &correlation);
        match client.reactivate_subscription(request).await {
            Ok(value) => Json(subscription_body(value.into_inner())).into_response(),
            Err(status) => billing_error(status, &correlation, "billing_reactivate_unavailable"),
        }
    }
}
