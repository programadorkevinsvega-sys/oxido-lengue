use std::error::Error;

use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
async fn inbox_is_idempotent_and_read_state_is_persisted_in_postgres()
-> Result<(), Box<dyn Error + Send + Sync>> {
    if let Err(error) = dotenvy::dotenv() {
        if error.not_found() {
            return Err(
                "backend .env is required for the real notifications integration test".into(),
            );
        }
        return Err(error.into());
    }
    let database_url = std::env::var("NOTIFICATIONS_DATABASE_URL")?;
    let cleanup_url = std::env::var("MIGRATOR_DATABASE_URL")?;
    let database = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await?;
    let cleanup_database = PgPoolOptions::new()
        .max_connections(2)
        .connect(&cleanup_url)
        .await?;
    let event_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    let second_recipient_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let task_id = Uuid::new_v4();
    let event = serde_json::json!({
        "event_id": event_id,
        "event_type": "tenancy_access.task.created",
        "payload": {
            "workspace_id": workspace_id,
            "actor_user_id": Uuid::new_v4(),
            "assignee_user_id": recipient_id,
            "task_id": task_id,
            "correlation_id": Uuid::new_v4(),
        }
    });
    let second_event = serde_json::json!({
        "event_id": Uuid::new_v4(),
        "event_type": "tenancy_access.task.created",
        "payload": {
            "workspace_id": workspace_id,
            "actor_user_id": Uuid::new_v4(),
            "assignee_user_id": second_recipient_id,
            "task_id": Uuid::new_v4(),
            "correlation_id": Uuid::new_v4(),
        }
    });

    let verification = async {
        xovyra_notifications::inbox::consume_team_event(&database, &event).await?;
        xovyra_notifications::inbox::consume_team_event(&database, &event).await?;
        xovyra_notifications::inbox::consume_team_event(&database, &second_event).await?;
        xovyra_notifications::inbox::consume_team_event(&database, &second_event).await?;
        let first_page = xovyra_notifications::inbox::list(&database, recipient_id, 0, 20).await?;
        assert_eq!(first_page.notifications.len(), 1);
        assert_eq!(first_page.unread_count, 1);
        assert_eq!(first_page.notifications[0].stream_cursor, 1);
        let second_page =
            xovyra_notifications::inbox::list(&database, second_recipient_id, 0, 20).await?;
        assert_eq!(second_page.notifications.len(), 1);
        assert_eq!(second_page.unread_count, 1);
        assert_eq!(second_page.notifications[0].stream_cursor, 1);
        let notification_id = first_page.notifications[0].notification_id;
        let marked =
            xovyra_notifications::inbox::mark_read(&database, recipient_id, notification_id)
                .await?
                .ok_or("the recipient's persisted notification disappeared")?;
        assert!(marked.read_at_unix.is_some());
        let replayed =
            xovyra_notifications::inbox::mark_read(&database, recipient_id, notification_id)
                .await?
                .ok_or("mark-read retry was not idempotent")?;
        assert_eq!(replayed.notification_id, notification_id);
        let read_page = xovyra_notifications::inbox::list(&database, recipient_id, 0, 20).await?;
        assert_eq!(read_page.unread_count, 0);
        assert!(read_page.notifications[0].read_at_unix.is_some());
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;

    let cleanup =
        sqlx::query("DELETE FROM notifications.notification_inbox WHERE recipient_user_id=ANY($1)")
            .bind(vec![recipient_id, second_recipient_id])
            .execute(&cleanup_database)
            .await;
    let outbox_cleanup = sqlx::query(
        "DELETE FROM notifications.outbox_events WHERE payload->>'recipient_user_id'=ANY($1)",
    )
    .bind(vec![
        recipient_id.to_string(),
        second_recipient_id.to_string(),
    ])
    .execute(&cleanup_database)
    .await;
    let cursor_cleanup = sqlx::query(
        "DELETE FROM notifications.notification_stream_cursors WHERE recipient_user_id=ANY($1)",
    )
    .bind(vec![recipient_id, second_recipient_id])
    .execute(&cleanup_database)
    .await;
    database.close().await;
    cleanup_database.close().await;
    verification?;
    cleanup?;
    outbox_cleanup?;
    cursor_cleanup?;
    Ok(())
}
