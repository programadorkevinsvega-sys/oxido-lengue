ALTER TABLE tenancy_access.workspaces
    DROP CONSTRAINT workspaces_status_check,
    ADD CONSTRAINT workspaces_status_check
        CHECK (status IN ('active', 'suspended', 'deletion_pending', 'closed')),
    ADD COLUMN locale TEXT NOT NULL DEFAULT 'es-CO'
        CHECK (char_length(locale) BETWEEN 2 AND 35),
    ADD COLUMN timezone TEXT NOT NULL DEFAULT 'America/Bogota'
        CHECK (char_length(timezone) BETWEEN 1 AND 64),
    ADD COLUMN deletion_effective_at TIMESTAMPTZ,
    ADD CONSTRAINT workspace_deletion_effective_at_check
        CHECK ((status = 'deletion_pending') = (deletion_effective_at IS NOT NULL));

CREATE TABLE tenancy_access.workspace_deletion_requests (
    request_id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL,
    requester_user_id UUID NOT NULL,
    idempotency_key TEXT NOT NULL CHECK (char_length(idempotency_key) BETWEEN 16 AND 128),
    request_digest BYTEA NOT NULL CHECK (octet_length(request_digest) = 32),
    workspace_name TEXT NOT NULL CHECK (char_length(workspace_name) BETWEEN 2 AND 120),
    requested_at TIMESTAMPTZ NOT NULL,
    effective_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'cancelled', 'completed')),
    row_version BIGINT NOT NULL CHECK (row_version > 0),
    completed_at TIMESTAMPTZ,
    UNIQUE (requester_user_id, idempotency_key),
    CHECK (effective_at > requested_at),
    CHECK ((status = 'completed') = (completed_at IS NOT NULL))
);

CREATE INDEX workspace_deletion_requests_due_idx
    ON tenancy_access.workspace_deletion_requests (effective_at, request_id)
    WHERE status = 'pending';

CREATE TABLE tenancy_access.workspace_audit_events (
    event_id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL,
    actor_user_id UUID,
    action TEXT NOT NULL CHECK (action IN (
        'workspace.created',
        'workspace.updated',
        'workspace.deletion_requested',
        'workspace.restored',
        'workspace.purged'
    )),
    row_version BIGINT NOT NULL CHECK (row_version > 0),
    correlation_id TEXT NOT NULL CHECK (char_length(correlation_id) BETWEEN 1 AND 128),
    occurred_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX workspace_audit_events_workspace_idx
    ON tenancy_access.workspace_audit_events (workspace_id, occurred_at DESC, event_id);

CREATE TABLE tenancy_access.workspace_purge_holds (
    workspace_id UUID NOT NULL,
    source_context TEXT NOT NULL CHECK (char_length(source_context) BETWEEN 1 AND 64),
    reference_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (workspace_id, source_context, reference_id)
);

CREATE INDEX workspace_purge_holds_workspace_idx
    ON tenancy_access.workspace_purge_holds (workspace_id);

GRANT SELECT, INSERT, UPDATE, DELETE ON tenancy_access.workspace_deletion_requests TO xovyra_tenancy_access_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON tenancy_access.workspace_audit_events TO xovyra_tenancy_access_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON tenancy_access.workspace_purge_holds TO xovyra_tenancy_access_runtime;
