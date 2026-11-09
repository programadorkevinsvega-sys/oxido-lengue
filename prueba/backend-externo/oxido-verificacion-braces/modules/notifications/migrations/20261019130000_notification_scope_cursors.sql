CREATE TABLE notification_stream_cursors (
    recipient_user_id UUID PRIMARY KEY,
    last_cursor BIGINT NOT NULL CHECK (last_cursor >= 0)
);

INSERT INTO notification_stream_cursors (recipient_user_id, last_cursor)
SELECT recipient_user_id, max(stream_cursor)
FROM notification_inbox
GROUP BY recipient_user_id;

ALTER TABLE notification_inbox ALTER COLUMN stream_cursor DROP DEFAULT;
DROP SEQUENCE notification_cursor_seq;

GRANT SELECT, INSERT, UPDATE, DELETE ON notification_stream_cursors TO xovyra_notifications_runtime;
