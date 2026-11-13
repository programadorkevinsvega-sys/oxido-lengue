CREATE TABLE notification_inbox (
    notification_id UUID PRIMARY KEY,
    source_event_id UUID NOT NULL,
    recipient_user_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    notification_type TEXT NOT NULL CHECK (notification_type IN ('task_assigned', 'task_reassigned', 'task_mentioned')),
    resource_type TEXT NOT NULL CHECK (resource_type = 'task'),
    resource_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    read_at TIMESTAMPTZ,
    UNIQUE (source_event_id, recipient_user_id)
);

CREATE SEQUENCE notification_cursor_seq AS BIGINT;
ALTER TABLE notification_inbox ADD COLUMN stream_cursor BIGINT NOT NULL DEFAULT nextval('notification_cursor_seq');
CREATE UNIQUE INDEX notification_inbox_cursor_idx ON notification_inbox (stream_cursor);
CREATE INDEX notification_inbox_recipient_cursor_idx ON notification_inbox (recipient_user_id, stream_cursor DESC);
CREATE INDEX notification_inbox_unread_idx ON notification_inbox (recipient_user_id, created_at DESC) WHERE read_at IS NULL;

GRANT USAGE ON SCHEMA notifications TO xovyra_notifications_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON notification_inbox TO xovyra_notifications_runtime;
GRANT USAGE, SELECT ON SEQUENCE notification_cursor_seq TO xovyra_notifications_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON outbox_events TO xovyra_notifications_runtime;
ALTER DEFAULT PRIVILEGES IN SCHEMA notifications GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO xovyra_notifications_runtime;
ALTER DEFAULT PRIVILEGES IN SCHEMA notifications GRANT USAGE, SELECT ON SEQUENCES TO xovyra_notifications_runtime;
