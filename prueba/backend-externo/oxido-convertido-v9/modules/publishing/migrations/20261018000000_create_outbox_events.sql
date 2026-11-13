CREATE TABLE outbox_events (
    event_id UUID PRIMARY KEY,
    event_type TEXT NOT NULL,
    event_version SMALLINT NOT NULL CHECK (event_version > 0),
    occurred_at TIMESTAMPTZ NOT NULL,
    payload JSONB NOT NULL,
    trace_id TEXT,
    published_at TIMESTAMPTZ,
    delivery_attempts INTEGER NOT NULL DEFAULT 0 CHECK (delivery_attempts >= 0),
    last_delivery_error TEXT
);

CREATE INDEX outbox_events_pending_idx
    ON outbox_events (occurred_at, event_id)
    WHERE published_at IS NULL;
