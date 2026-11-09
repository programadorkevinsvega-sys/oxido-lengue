CREATE TABLE stream_cursors (
    audience TEXT NOT NULL CHECK (audience IN ('tenant', 'platform')),
    scope_id UUID NOT NULL,
    last_cursor BIGINT NOT NULL CHECK (last_cursor >= 0),
    PRIMARY KEY (audience, scope_id)
);

INSERT INTO stream_cursors (audience, scope_id, last_cursor)
SELECT audience, scope_id, max(stream_cursor)
FROM event_journal
GROUP BY audience, scope_id;

ALTER TABLE event_journal DROP CONSTRAINT event_journal_pkey;
ALTER TABLE event_journal ALTER COLUMN stream_cursor DROP IDENTITY IF EXISTS;
ALTER TABLE event_journal ADD PRIMARY KEY (audience, scope_id, stream_cursor);
CREATE UNIQUE INDEX event_journal_scope_cursor_unique_idx ON event_journal (audience, scope_id, stream_cursor);

GRANT SELECT, INSERT, UPDATE, DELETE ON stream_cursors TO xovyra_realtime_runtime;
