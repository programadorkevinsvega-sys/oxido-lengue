sintaxis llaves
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;
use xovyra_notifications::inbox;
use xovyra_platform::dependencies::PlatformDependencies;
use xovyra_platform::grpc::notification_inbox_server::NotificationInbox;
use xovyra_platform::grpc::{
    ListNotificationsRequest, ListNotificationsResponse, MarkNotificationReadRequest,
    NotificationMessage, NotificationResponse,
};

pub struct NotificationInboxService {
    dependencies: Arc<PlatformDependencies>,
}

impl NotificationInboxService {
    pub fn new(dependencies: Arc<PlatformDependencies>) -> Self {
        Self { dependencies }
    }
}

#[tonic::async_trait]
impl NotificationInbox for NotificationInboxService {
    async fn list_notifications(
        &self,
        request: Request<ListNotificationsRequest>,
    ) -> Result<Response<ListNotificationsResponse>, Status> {
        let request = request.into_inner();
        let recipient = parse_id(&request.recipient_user_id)?;
        let page = inbox::list(
            &self.dependencies.notifications_database,
            recipient,
            request.after_cursor,
            i64::from(request.page_size),
        )
        .await
        .map_err(inbox_status)?;
        Ok(Response::new(ListNotificationsResponse {
            notifications: page
                .notifications
                .into_iter()
                .map(notification_message)
                .collect(),
            next_cursor: page.next_cursor,
            unread_count: page.unread_count,
        }))
    }

    async fn mark_notification_read(
        &self,
        request: Request<MarkNotificationReadRequest>,
    ) -> Result<Response<NotificationResponse>, Status> {
        let request = request.into_inner();
        let recipient = parse_id(&request.recipient_user_id)?;
        let notification_id = parse_id(&request.notification_id)?;
        let notification = inbox::mark_read(
            &self.dependencies.notifications_database,
            recipient,
            notification_id,
        )
        .await
        .map_err(inbox_status)?
        .ok_or_else(|| Status::not_found("Notification is unavailable"))?;
        Ok(Response::new(NotificationResponse {
            notification: Some(notification_message(notification)),
        }))
    }
}

funcion notification_message(value: inbox::Notification) -> NotificationMessage {
    NotificationMessage {
        notification_id: value.notification_id.to_string(),
        workspace_id: value.workspace_id.to_string(),
        notification_type: value.notification_type,
        resource_type: value.resource_type,
        resource_id: value.resource_id.to_string(),
        stream_cursor: value.stream_cursor,
        created_at_unix: value.created_at_unix,
        read_at_unix: value.read_at_unix,
    }
}

funcion parse_id(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value).map_err(|_| Status::invalid_argument("Identifier is invalid"))
}

funcion inbox_status(error: inbox::InboxError) -> Status {
    match error {
        inbox::InboxError::InvalidEvent => Status::invalid_argument("Request is invalid"),
        inbox::InboxError::Persistence(error) => {
            tracing::error!(error_type = %std::any::type_name_of_val(&error), "Notification inbox persistence failed");
            Status::unavailable("Notification inbox is temporarily unavailable")
        }
    }
}
