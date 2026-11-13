CREATE TABLE event_journal (
    stream_cursor BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id UUID NOT NULL UNIQUE,
    event_type TEXT NOT NULL CHECK (char_length(event_type) BETWEEN 1 AND 180),
    audience TEXT NOT NULL CHECK (audience IN ('tenant', 'platform')),
    scope_id UUID NOT NULL,
    aggregate_id UUID NOT NULL,
    aggregate_version BIGINT NOT NULL CHECK (aggregate_version > 0),
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    correlation_id TEXT NOT NULL DEFAULT '' CHECK (char_length(correlation_id) <= 128)
);

CREATE INDEX event_journal_scope_cursor_idx ON event_journal (audience, scope_id, stream_cursor);
CREATE INDEX event_journal_aggregate_version_idx ON event_journal (audience, scope_id, aggregate_id, aggregate_version DESC);

GRANT USAGE ON SCHEMA realtime TO xovyra_realtime_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON event_journal TO xovyra_realtime_runtime;
GRANT USAGE, SELECT ON SEQUENCE event_journal_stream_cursor_seq TO xovyra_realtime_runtime;
ALTER DEFAULT PRIVILEGES IN SCHEMA realtime GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO xovyra_realtime_runtime;
ALTER DEFAULT PRIVILEGES IN SCHEMA realtime GRANT USAGE, SELECT ON SEQUENCES TO xovyra_realtime_runtime;
