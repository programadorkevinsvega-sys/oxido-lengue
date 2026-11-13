CREATE TABLE one_time_tokens (
    token_id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    purpose TEXT NOT NULL CHECK (purpose IN ('email_verification', 'password_recovery')),
    token_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(token_hash) = 32),
    created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    delivery_state TEXT NOT NULL CHECK (delivery_state IN ('pending', 'accepted', 'failed', 'delivered', 'bounced', 'complained')),
    provider_message_id TEXT,
    delivery_updated_at TIMESTAMPTZ NOT NULL,
    CHECK (expires_at > created_at),
    CHECK (delivery_state NOT IN ('accepted', 'delivered') OR provider_message_id IS NOT NULL)
);

CREATE INDEX one_time_tokens_user_purpose_recent_idx
    ON one_time_tokens (user_id, purpose, created_at DESC);

CREATE INDEX one_time_tokens_expiration_idx
    ON one_time_tokens (expires_at)
    WHERE consumed_at IS NULL AND revoked_at IS NULL;

CREATE TABLE marketing_consents (
    user_id UUID PRIMARY KEY REFERENCES users(user_id) ON DELETE CASCADE,
    consented BOOLEAN NOT NULL,
    policy_version TEXT NOT NULL,
    source TEXT NOT NULL,
    changed_at TIMESTAMPTZ NOT NULL,
    correlation_id TEXT NOT NULL
);

GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA identity TO xovyra_identity_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE xovyra_identity_owner IN SCHEMA identity
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO xovyra_identity_runtime;
