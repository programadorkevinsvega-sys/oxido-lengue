sintaxis llaves
use axum::{
    Json, Router,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};
use tonic::Request;
use utoipa::ToSchema;
use xovyra_platform::grpc::notification_inbox_client::NotificationInboxClient;
use xovyra_platform::grpc::{
    ListNotificationsRequest, MarkNotificationReadRequest, NotificationMessage,
};

use crate::{
    authentication::{error_response, valid_origin},
    openapi::AppState,
    workspaces::{authenticate, inject_metadata},
};

#[derive(Deserialize)]
pub(crate) struct NotificationQuery {
    after_cursor: Option<i64>,
    limit: Option<i32>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct NotificationBody {
    notification_id: String,
    workspace_id: String,
    notification_type: String,
    resource_type: String,
    resource_id: String,
    stream_cursor: i64,
    created_at_unix: i64,
    read_at_unix: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct NotificationListBody {
    notifications: Vec<NotificationBody>,
    next_cursor: i64,
    unread_count: i64,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/notifications", get(list))
        .route(
            "/api/v1/notifications/{notification_id}/read",
            axum::routing::post(mark_read),
        )
}

#[utoipa::path(get, path = "/api/v1/notifications", params(("after_cursor" = Option<i64>, Query), ("limit" = Option<i32>, Query)), responses((status = 200, body = NotificationListBody), (status = 401), (status = 503)))]
pub(crate) async fn list(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Query(query): Query<NotificationQuery>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let session = match authenticate(&state, &headers, &correlation_id, false).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let after_cursor = query.after_cursor.unwrap_or(0);
    let limit = query.limit.unwrap_or(50);
    if after_cursor < 0 || !(1..=100).contains(&limit) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_pagination",
            "El cursor o el límite de notificaciones no es válido.",
        );
    }
    let mut client = NotificationInboxClient::new(state.runtime_channel);
    let mut request = Request::new(ListNotificationsRequest {
        recipient_user_id: session.user_id,
        after_cursor,
        page_size: limit,
        correlation_id: correlation_id.clone(),
    });
    inject_metadata(&mut request, &correlation_id);
    match client.list_notifications(request).await {
        Ok(response) => {
            let response = response.into_inner();
            Json(NotificationListBody {
                notifications: response
                    .notifications
                    .into_iter()
                    .map(notification_body)
                    .collect(),
                next_cursor: response.next_cursor,
                unread_count: response.unread_count,
            })
            .into_response()
        }
        Err(status) => {
            tracing::error!(correlation_id = %correlation_id, code = ?status.code(), "Notification inbox request failed");
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "notifications_unavailable",
                "No se pudieron cargar tus notificaciones ahora.",
            )
        }
    }
}

#[utoipa::path(post, path = "/api/v1/notifications/{notification_id}/read", params(("notification_id" = String, Path)), responses((status = 200, body = NotificationBody), (status = 401), (status = 403), (status = 404), (status = 503)))]
pub(crate) async fn mark_read(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Path(notification_id): Path<String>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let session = match authenticate(&state, &headers, &correlation_id, true).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    let mut client = NotificationInboxClient::new(state.runtime_channel);
    let mut request = Request::new(MarkNotificationReadRequest {
        recipient_user_id: session.user_id,
        notification_id,
        correlation_id: correlation_id.clone(),
    });
    inject_metadata(&mut request, &correlation_id);
    match client.mark_notification_read(request).await {
        Ok(response) => response
            .into_inner()
            .notification
            .map(notification_body)
            .map(|body| Json(body).into_response())
            .unwrap_or_else(|| {
                error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "notifications_response_invalid",
                    "El servidor devolvió una notificación incompatible.",
                )
            }),
        Err(status) if status.code() == tonic::Code::NotFound => error_response(
            StatusCode::NOT_FOUND,
            "notification_unavailable",
            "La notificación ya no está disponible.",
        ),
        Err(status) => {
            tracing::error!(correlation_id = %correlation_id, code = ?status.code(), "Notification update failed");
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "notifications_unavailable",
                "No se pudo actualizar la notificación ahora.",
            )
        }
    }
}

fn notification_body(value: NotificationMessage) -> NotificationBody {
    NotificationBody {
        notification_id: value.notification_id,
        workspace_id: value.workspace_id,
        notification_type: value.notification_type,
        resource_type: value.resource_type,
        resource_id: value.resource_id,
        stream_cursor: value.stream_cursor,
        created_at_unix: value.created_at_unix,
        read_at_unix: value.read_at_unix,
    }
}
