CREATE TABLE users (
    user_id UUID PRIMARY KEY,
    email_normalized TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    account_status TEXT NOT NULL CHECK (account_status IN ('pending_verification', 'active', 'suspended', 'closed')),
    email_verified_at TIMESTAMPTZ,
    display_name TEXT NOT NULL,
    locale TEXT NOT NULL,
    timezone TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    CHECK (account_status <> 'active' OR email_verified_at IS NOT NULL)
);

CREATE TABLE sessions (
    session_id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    audience TEXT NOT NULL CHECK (audience IN ('tenant', 'platform')),
    token_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(token_hash) = 32),
    csrf_hash BYTEA NOT NULL CHECK (octet_length(csrf_hash) = 32),
    created_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL,
    idle_expires_at TIMESTAMPTZ NOT NULL,
    absolute_expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    revocation_reason TEXT,
    user_agent_digest BYTEA,
    ip_digest BYTEA,
    CHECK (idle_expires_at <= absolute_expires_at)
);

CREATE INDEX sessions_user_active_idx
    ON sessions (user_id, audience, last_seen_at DESC)
    WHERE revoked_at IS NULL;

CREATE INDEX sessions_expiration_idx
    ON sessions (idle_expires_at)
    WHERE revoked_at IS NULL;

CREATE TABLE security_events (
    event_id UUID PRIMARY KEY,
    user_id UUID REFERENCES users(user_id) ON DELETE SET NULL,
    event_type TEXT NOT NULL,
    result TEXT NOT NULL CHECK (result IN ('succeeded', 'rejected', 'failed')),
    occurred_at TIMESTAMPTZ NOT NULL,
    correlation_id TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX security_events_user_recent_idx
    ON security_events (user_id, occurred_at DESC);

GRANT USAGE ON SCHEMA identity TO xovyra_identity_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA identity TO xovyra_identity_runtime;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA identity TO xovyra_identity_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE xovyra_identity_owner IN SCHEMA identity
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO xovyra_identity_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE xovyra_identity_owner IN SCHEMA identity
    GRANT USAGE, SELECT ON SEQUENCES TO xovyra_identity_runtime;
