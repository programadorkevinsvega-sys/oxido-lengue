ALTER TABLE event_journal
ADD COLUMN schema_version SMALLINT NOT NULL DEFAULT 1
CHECK (schema_version > 0);
