CREATE SCHEMA IF NOT EXISTS platform_control AUTHORIZATION xovyra_platform_control_owner;

CREATE TABLE platform_control.support_grants (
    grant_id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL,
    asset_id UUID NOT NULL,
    issued_by_user_id UUID NOT NULL,
    reason TEXT NOT NULL CHECK (char_length(reason) BETWEEN 12 AND 1000),
    status TEXT NOT NULL CHECK (status IN ('active', 'revoked', 'expired')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    revoked_by_user_id UUID,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    idempotency_key TEXT NOT NULL,
    request_digest BYTEA NOT NULL,
    CHECK (expires_at > created_at AND expires_at <= created_at + interval '1 hour'),
    CHECK ((status = 'revoked') = (revoked_at IS NOT NULL)),
    UNIQUE (issued_by_user_id, idempotency_key)
);

CREATE INDEX support_grants_scope_idx
    ON platform_control.support_grants (workspace_id, asset_id, status, expires_at);
CREATE INDEX support_grants_recent_idx
    ON platform_control.support_grants (created_at DESC, grant_id DESC);

CREATE TABLE platform_control.support_access_audit (
    event_id UUID PRIMARY KEY,
    operator_user_id UUID,
    action TEXT NOT NULL,
    result TEXT NOT NULL CHECK (result IN ('succeeded', 'rejected', 'failed')),
    workspace_id UUID,
    asset_id UUID,
    grant_id UUID,
    reason TEXT,
    correlation_id TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX support_access_audit_recent_idx
    ON platform_control.support_access_audit (occurred_at DESC, event_id DESC);

CREATE FUNCTION platform_control.reject_support_audit_mutation()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'platform support audit is append-only';
END;
$$;
CREATE TRIGGER support_access_audit_immutable
BEFORE UPDATE OR DELETE ON platform_control.support_access_audit
FOR EACH ROW EXECUTE FUNCTION platform_control.reject_support_audit_mutation();

CREATE TABLE platform_control.outbox_events (
    event_id UUID PRIMARY KEY,
    event_type TEXT NOT NULL,
    event_version SMALLINT NOT NULL CHECK (event_version > 0),
    occurred_at TIMESTAMPTZ NOT NULL,
    payload JSONB NOT NULL,
    trace_id TEXT,
    traceparent TEXT,
    tracestate TEXT,
    published_at TIMESTAMPTZ,
    delivery_attempts INTEGER NOT NULL DEFAULT 0 CHECK (delivery_attempts >= 0),
    last_delivery_error TEXT
);
CREATE INDEX platform_control_outbox_pending_idx
    ON platform_control.outbox_events (occurred_at, event_id)
    WHERE published_at IS NULL;

GRANT USAGE ON SCHEMA platform_control TO xovyra_platform_control_runtime;
GRANT SELECT, INSERT, UPDATE ON platform_control.support_grants TO xovyra_platform_control_runtime;
GRANT SELECT, INSERT ON platform_control.support_access_audit TO xovyra_platform_control_runtime;
GRANT SELECT, INSERT, UPDATE ON platform_control.outbox_events TO xovyra_platform_control_runtime;
