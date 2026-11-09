DROP INDEX notifications.notification_inbox_cursor_idx;
DROP INDEX notifications.notification_inbox_recipient_cursor_idx;

CREATE UNIQUE INDEX notification_inbox_recipient_cursor_idx
    ON notifications.notification_inbox (recipient_user_id, stream_cursor);
