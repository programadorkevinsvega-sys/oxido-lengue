ALTER TABLE event_journal DROP CONSTRAINT event_journal_audience_check;
ALTER TABLE event_journal ADD CONSTRAINT event_journal_audience_check CHECK (audience IN ('tenant', 'tenant_user', 'platform'));
ALTER TABLE stream_cursors DROP CONSTRAINT stream_cursors_audience_check;
ALTER TABLE stream_cursors ADD CONSTRAINT stream_cursors_audience_check CHECK (audience IN ('tenant', 'tenant_user', 'platform'));
