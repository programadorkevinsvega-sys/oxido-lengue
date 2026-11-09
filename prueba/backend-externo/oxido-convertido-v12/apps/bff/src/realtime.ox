sintaxis llaves
use std::time::Duration;

use axum::{
    Json, Router,
    extract::{
        Extension, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};
use tonic::Request;
use xovyra_platform::grpc::{
    AccountSessionRequest, ActiveWorkspaceRequest, AuthenticateSessionRequest,
    AuthenticationAudience, RealtimeEventMessage, RecoverRealtimeEventsRequest,
    SubscribeRealtimeEventsRequest, identity_account_client::IdentityAccountClient,
    identity_authentication_client::IdentityAuthenticationClient,
    realtime_gateway_client::RealtimeGatewayClient, tenancy_access_client::TenancyAccessClient,
};

use crate::{
    authentication::{
        PLATFORM_CSRF_COOKIE, PLATFORM_SESSION_COOKIE, TENANT_CSRF_COOKIE, TENANT_SESSION_COOKIE,
        cookie_value, error_response, valid_origin,
    },
    openapi::AppState,
    workspaces::{authenticate, inject_metadata},
};

#[derive(Deserialize)]
pub(crate) struct RecoveryQuery {
    after_cursor: Option<i64>,
    limit: Option<i32>,
    scope: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct PublicEvent {
    event_id: String,
    stream_cursor: i64,
    event_type: String,
    schema_version: i32,
    audience: String,
    scope_id: String,
    aggregate_id: String,
    aggregate_version: i64,
    occurred_at_unix: i64,
    correlation_id: String,
}

#[derive(Clone)]
struct SessionWatch {
    session_secret: String,
    csrf_secret: String,
    audience: i32,
    user_id: String,
    session_id: String,
    scope_id: String,
}

pub funcion router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/realtime", get(tenant_websocket))
        .route("/api/v1/realtime/events", get(tenant_recovery))
        .route("/platform/realtime", get(platform_websocket))
        .route("/platform/realtime/events", get(platform_recovery))
}

async fn tenant_websocket(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Query(query): Query<RecoveryQuery>,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let after_cursor = match valid_cursor(query.after_cursor, query.limit) {
        Ok(cursor) => cursor,
        Err(response) => return *response,
    };
    let session = match authenticate(&state, &headers, &correlation_id, false).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    if query.scope.as_deref() == Some("user") {
        let watch = SessionWatch {
            session_secret: cookie_value(&headers, TENANT_SESSION_COOKIE).unwrap_or_default(),
            csrf_secret: cookie_value(&headers, TENANT_CSRF_COOKIE).unwrap_or_default(),
            audience: AuthenticationAudience::Tenant as i32,
            scope_id: session.user_id.clone(),
            user_id: session.user_id.clone(),
            session_id: session.session_id,
        };
        return subscribe(
            &state,
            upgrade,
            watch,
            "tenant_user",
            after_cursor,
            correlation_id,
        )
        .await;
    }
    if query.scope.is_some() && query.scope.as_deref() != Some("workspace") {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_realtime_scope",
            "El tipo de sincronización solicitado no es válido.",
        );
    }
    let mut tenancy = TenancyAccessClient::new(state.runtime_channel.clone());
    let mut active_request = Request::new(ActiveWorkspaceRequest {
        user_id: session.user_id.clone(),
        session_id: session.session_id.clone(),
        correlation_id: correlation_id.clone(),
    });
    inject_metadata(&mut active_request, &correlation_id);
    let active = match tenancy.get_active_workspace(active_request).await {
        Ok(response) => response.into_inner().workspace,
        Err(status) => return grpc_error_response(status.code(), &correlation_id),
    };
    let Some(active) = active else {
        return error_response(
            StatusCode::CONFLICT,
            "workspace_not_selected",
            "Selecciona un espacio de trabajo para recibir sus actualizaciones.",
        );
    };
    let watch = SessionWatch {
        session_secret: cookie_value(&headers, TENANT_SESSION_COOKIE).unwrap_or_default(),
        csrf_secret: cookie_value(&headers, TENANT_CSRF_COOKIE).unwrap_or_default(),
        audience: AuthenticationAudience::Tenant as i32,
        user_id: session.user_id,
        session_id: session.session_id,
        scope_id: active.workspace_id.clone(),
    };
    subscribe(
        &state,
        upgrade,
        watch,
        "tenant",
        after_cursor,
        correlation_id,
    )
    .await
}

async fn platform_websocket(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Query(query): Query<RecoveryQuery>,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !valid_origin(&headers, &state.platform_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let after_cursor = match valid_cursor(query.after_cursor, query.limit) {
        Ok(cursor) => cursor,
        Err(response) => return *response,
    };
    let watch = match authenticate_platform(&state, &headers, &correlation_id).await {
        Ok(watch) => watch,
        Err(response) => return *response,
    };
    subscribe(
        &state,
        upgrade,
        watch,
        "platform",
        after_cursor,
        correlation_id,
    )
    .await
}

#[utoipa::path(get, path = "/api/v1/realtime/events", params(("after_cursor" = Option<i64>, Query), ("limit" = Option<i32>, Query)), responses((status = 200), (status = 401), (status = 403), (status = 503)))]
pub(crate) async fn tenant_recovery(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Query(query): Query<RecoveryQuery>,
) -> Response {
    if !valid_origin(&headers, &state.tenant_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let after_cursor = match valid_cursor(query.after_cursor, query.limit) {
        Ok(cursor) => cursor,
        Err(response) => return *response,
    };
    let session = match authenticate(&state, &headers, &correlation_id, false).await {
        Ok(session) => session,
        Err(response) => return *response,
    };
    if query.scope.as_deref() == Some("user") {
        return recover(
            &state,
            "tenant_user",
            &session.user_id,
            after_cursor,
            query.limit.unwrap_or(100),
            &session.user_id,
            &correlation_id,
        )
        .await;
    }
    if query.scope.is_some() && query.scope.as_deref() != Some("workspace") {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_realtime_scope",
            "El tipo de sincronización solicitado no es válido.",
        );
    }
    let scope_id = match active_workspace_scope(
        &state,
        &session.user_id,
        &session.session_id,
        &correlation_id,
    )
    .await
    {
        Ok(scope_id) => scope_id,
        Err(response) => return *response,
    };
    recover(
        &state,
        "tenant",
        &scope_id,
        after_cursor,
        query.limit.unwrap_or(100),
        &session.user_id,
        &correlation_id,
    )
    .await
}

#[utoipa::path(get, path = "/platform/realtime/events", params(("after_cursor" = Option<i64>, Query), ("limit" = Option<i32>, Query)), responses((status = 200), (status = 401), (status = 403), (status = 503)))]
pub(crate) async fn platform_recovery(
    State(state): State<AppState>,
    Extension(correlation_id): Extension<String>,
    headers: HeaderMap,
    Query(query): Query<RecoveryQuery>,
) -> Response {
    if !valid_origin(&headers, &state.platform_allowed_origins) {
        return error_response(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "Request origin is not allowed",
        );
    }
    let after_cursor = match valid_cursor(query.after_cursor, query.limit) {
        Ok(cursor) => cursor,
        Err(response) => return *response,
    };
    let watch = match authenticate_platform(&state, &headers, &correlation_id).await {
        Ok(watch) => watch,
        Err(response) => return *response,
    };
    recover(
        &state,
        "platform",
        &watch.scope_id,
        after_cursor,
        query.limit.unwrap_or(100),
        &watch.user_id,
        &correlation_id,
    )
    .await
}

async funcion active_workspace_scope(
    state: &AppState,
    user_id: &str,
    session_id: &str,
    correlation_id: &str,
) -> Result<String, Box<Response>> {
    let mut client = TenancyAccessClient::new(state.runtime_channel.clone());
    let mut request = Request::new(ActiveWorkspaceRequest {
        user_id: user_id.to_owned(),
        session_id: session_id.to_owned(),
        correlation_id: correlation_id.to_owned(),
    });
    inject_metadata(&mut request, correlation_id);
    match client.get_active_workspace(request).await {
        Ok(response) => response
            .into_inner()
            .workspace
            .map(|workspace| workspace.workspace_id)
            .ok_or_else(|| {
                Box::new(error_response(
                    StatusCode::CONFLICT,
                    "workspace_not_selected",
                    "Selecciona un espacio de trabajo para sincronizar sus cambios.",
                ))
            }),
        Err(status) => Err(Box::new(grpc_error_response(status.code(), correlation_id))),
    }
}

async fn authenticate_platform(
    state: &AppState,
    headers: &HeaderMap,
    correlation_id: &str,
) -> Result<SessionWatch, Box<Response>> {
    let Some(session_secret) = cookie_value(headers, PLATFORM_SESSION_COOKIE) else {
        return Err(Box::new(error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Inicia sesión en la consola de plataforma.",
        )));
    };
    let Some(csrf_secret) = cookie_value(headers, PLATFORM_CSRF_COOKIE) else {
        return Err(Box::new(error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "La sesión de plataforma no está disponible.",
        )));
    };
    let mut client = IdentityAuthenticationClient::new(state.runtime_channel.clone());
    let mut request = Request::new(AuthenticateSessionRequest {
        session_secret: session_secret.clone(),
        audience: AuthenticationAudience::Platform as i32,
        correlation_id: correlation_id.to_owned(),
        csrf_secret: csrf_secret.clone(),
    });
    inject_metadata(&mut request, correlation_id);
    match client.authenticate_session(request).await {
        Ok(response) => {
            let session = response.into_inner();
            let mut permissions_client = IdentityAccountClient::new(state.runtime_channel.clone());
            let mut permissions_request = Request::new(AccountSessionRequest {
                session_secret: session_secret.clone(),
                csrf_secret: csrf_secret.clone(),
                audience: AuthenticationAudience::Platform as i32,
                correlation_id: correlation_id.to_owned(),
            });
            inject_metadata(&mut permissions_request, correlation_id);
            let permissions_result = tokio::time::timeout(
                Duration::from_secs(5),
                permissions_client.get_platform_permissions(permissions_request),
            )
            .await;
            match permissions_result {
                Ok(Ok(response)) => {
                    if !response
                        .into_inner()
                        .permissions
                        .iter()
                        .any(|permission| permission == "platform.auth.read")
                    {
                        return Err(Box::new(error_response(
                            StatusCode::FORBIDDEN,
                            "access_denied",
                            "No tienes permiso para consultar las actualizaciones de plataforma.",
                        )));
                    }
                }
                Ok(Err(status)) => {
                    return Err(Box::new(grpc_error_response(status.code(), correlation_id)));
                }
                Err(_) => {
                    return Err(Box::new(error_response(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "realtime_unavailable",
                        "No se pudieron validar tus permisos ahora.",
                    )));
                }
            }
            Ok(SessionWatch {
                session_secret,
                csrf_secret,
                audience: AuthenticationAudience::Platform as i32,
                scope_id: "00000000-0000-0000-0000-000000000001".to_owned(),
                user_id: session.user_id,
                session_id: session.session_id,
            })
        }
        Err(status) => Err(Box::new(grpc_error_response(status.code(), correlation_id))),
    }
}

async funcion subscribe(
    state: &AppState,
    upgrade: WebSocketUpgrade,
    watch: SessionWatch,
    audience: &'static str,
    after_cursor: i64,
    correlation_id: String,
) -> Response {
    let mut client = RealtimeGatewayClient::new(state.runtime_channel.clone());
    let mut request = Request::new(SubscribeRealtimeEventsRequest {
        audience: audience.to_owned(),
        scope_id: watch.scope_id.clone(),
        after_cursor,
        correlation_id: correlation_id.clone(),
        actor_user_id: watch.user_id.clone(),
    });
    inject_metadata(&mut request, &correlation_id);
    match tokio::time::timeout(Duration::from_secs(5), client.subscribe_events(request)).await {
        Ok(Ok(response)) => {
            let stream = response.into_inner();
            let state = state.clone();
            upgrade
                .max_frame_size(16 * 1024)
                .max_message_size(16 * 1024)
                .on_upgrade(move |socket| {
                    bridge(socket, stream, state, watch, audience, correlation_id)
                })
                .into_response()
        }
        Ok(Err(status)) => grpc_error_response(status.code(), &correlation_id),
        Err(_) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "realtime_unavailable",
            "La sincronización en tiempo real no está disponible ahora.",
        ),
    }
}

async fn bridge(
    mut socket: WebSocket,
    mut stream: tonic::Streaming<RealtimeEventMessage>,
    state: AppState,
    watch: SessionWatch,
    audience: &'static str,
    correlation_id: String,
) {
    let mut heartbeat = tokio::time::interval(Duration::from_secs(15));
    loop {
        tokio::select! {
            event = stream.message() => match event {
                Ok(Some(event)) => {
                    let public_event = public_event(event);
                    let frame = serde_json::json!({ "type": "event", "event": public_event });
                    if socket.send(Message::Text(frame.to_string().into())).await.is_err() { return; }
                }
                Ok(None) => {
                    let _ = socket.send(Message::Text("{\"type\":\"resync_required\"}".into())).await;
                    return;
                }
                Err(status) => {
                    tracing::warn!(correlation_id = %correlation_id, code = ?status.code(), "Realtime gRPC stream closed");
                    let _ = socket.send(Message::Text("{\"type\":\"resync_required\"}".into())).await;
                    return;
                }
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return,
                Some(Ok(Message::Ping(payload))) => if socket.send(Message::Pong(payload)).await.is_err() { return; },
                Some(Ok(Message::Text(text))) if text.len() > 1024 => return,
                Some(Ok(Message::Text(_))) | Some(Ok(Message::Binary(_))) | Some(Ok(Message::Pong(_))) => {}
            },
            _ = heartbeat.tick() => {
                if !session_is_current(&state, &watch, audience, &correlation_id).await {
                    let _ = socket.send(Message::Close(None)).await;
                    return;
                }
                if socket.send(Message::Ping(Vec::new().into())).await.is_err() { return; }
            }
        }
    }
}

async fn session_is_current(
    state: &AppState,
    watch: &SessionWatch,
    audience: &'static str,
    correlation_id: &str,
) -> bool {
    let mut identity = IdentityAuthenticationClient::new(state.runtime_channel.clone());
    let mut request = Request::new(AuthenticateSessionRequest {
        session_secret: watch.session_secret.clone(),
        audience: watch.audience,
        correlation_id: correlation_id.to_owned(),
        csrf_secret: watch.csrf_secret.clone(),
    });
    inject_metadata(&mut request, correlation_id);
    let Ok(response) = identity.authenticate_session(request).await else {
        return false;
    };
    let session = response.into_inner();
    if session.user_id != watch.user_id || session.session_id != watch.session_id {
        return false;
    }
    if audience == "platform" {
        return true;
    }
    if audience == "tenant_user" {
        return true;
    }
    let mut tenancy = TenancyAccessClient::new(state.runtime_channel.clone());
    let mut request = Request::new(ActiveWorkspaceRequest {
        user_id: watch.user_id.clone(),
        session_id: watch.session_id.clone(),
        correlation_id: correlation_id.to_owned(),
    });
    inject_metadata(&mut request, correlation_id);
    match tenancy.get_active_workspace(request).await {
        Ok(response) => response
            .into_inner()
            .workspace
            .is_some_and(|workspace| workspace.workspace_id == watch.scope_id),
        Err(_) => false,
    }
}

async fn recover(
    state: &AppState,
    audience: &str,
    scope_id: &str,
    after_cursor: i64,
    page_size: i32,
    actor_user_id: &str,
    correlation_id: &str,
) -> Response {
    let mut client = RealtimeGatewayClient::new(state.runtime_channel.clone());
    let mut request = Request::new(RecoverRealtimeEventsRequest {
        audience: audience.to_owned(),
        scope_id: scope_id.to_owned(),
        after_cursor,
        page_size,
        correlation_id: correlation_id.to_owned(),
        actor_user_id: actor_user_id.to_owned(),
    });
    inject_metadata(&mut request, correlation_id);
    match client.recover_events(request).await {
        Ok(response) => {
            let response = response.into_inner();
            Json(serde_json::json!({
                "events": response.events.into_iter().map(public_event).collect::<Vec<_>>(),
                "next_cursor": response.next_cursor,
            }))
            .into_response()
        }
        Err(status) => grpc_error_response(status.code(), correlation_id),
    }
}

fn valid_cursor(cursor: Option<i64>, limit: Option<i32>) -> Result<i64, Box<Response>> {
    let cursor = cursor.unwrap_or(0);
    let limit = limit.unwrap_or(100);
    if cursor < 0 || !(1..=500).contains(&limit) {
        Err(Box::new(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_cursor",
            "El cursor de sincronización no es válido.",
        )))
    } else {
        Ok(cursor)
    }
}

funcion public_event(event: RealtimeEventMessage) -> PublicEvent {
    PublicEvent {
        event_id: event.event_id,
        stream_cursor: event.stream_cursor,
        event_type: event.event_type,
        schema_version: event.schema_version,
        audience: event.audience,
        scope_id: event.scope_id,
        aggregate_id: event.aggregate_id,
        aggregate_version: event.aggregate_version,
        occurred_at_unix: event.occurred_at_unix,
        correlation_id: event.correlation_id,
    }
}

fn grpc_error_response(code: tonic::Code, correlation_id: &str) -> Response {
    match code {
        tonic::Code::Unauthenticated => error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Inicia sesión para continuar.",
        ),
        tonic::Code::PermissionDenied => error_response(
            StatusCode::FORBIDDEN,
            "access_denied",
            "No tienes acceso a estas actualizaciones.",
        ),
        tonic::Code::NotFound => error_response(
            StatusCode::NOT_FOUND,
            "resource_unavailable",
            "El recurso ya no está disponible.",
        ),
        _ => {
            tracing::error!(correlation_id, code = ?code, "Realtime BFF request failed");
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "realtime_unavailable",
                "No se pudieron sincronizar los cambios ahora.",
            )
        }
    }
}
