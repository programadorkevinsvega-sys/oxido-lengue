ALTER TABLE identity.one_time_tokens
    DROP CONSTRAINT one_time_tokens_delivery_state_check,
    ADD CONSTRAINT one_time_tokens_delivery_state_check
        CHECK (delivery_state IN ('pending', 'accepted', 'failed', 'delivered', 'bounced', 'complained', 'suppressed'));

CREATE TABLE identity.email_provider_events (
    provider_event_id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    provider_message_id TEXT,
    received_at TIMESTAMPTZ NOT NULL,
    processed_at TIMESTAMPTZ NOT NULL
);

GRANT SELECT, INSERT, UPDATE, DELETE ON identity.email_provider_events TO xovyra_identity_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE xovyra_identity_owner IN SCHEMA identity
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO xovyra_identity_runtime;
