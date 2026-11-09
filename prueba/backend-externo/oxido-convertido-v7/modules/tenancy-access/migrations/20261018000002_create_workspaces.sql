CREATE TABLE workspaces (
    workspace_id UUID PRIMARY KEY,
    name TEXT NOT NULL CHECK (char_length(name) BETWEEN 2 AND 120),
    workspace_type TEXT NOT NULL CHECK (workspace_type IN ('personal', 'team')),
    status TEXT NOT NULL CHECK (status IN ('active', 'suspended', 'closed')),
    created_by_user_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0)
);

CREATE TABLE workspace_memberships (
    workspace_id UUID NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('owner')),
    status TEXT NOT NULL CHECK (status IN ('active', 'suspended', 'revoked')),
    joined_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (workspace_id, user_id)
);

CREATE INDEX workspace_memberships_user_active_idx
    ON workspace_memberships (user_id, joined_at DESC, workspace_id)
    WHERE status = 'active';

CREATE TABLE active_workspace_selections (
    session_id UUID NOT NULL,
    user_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (session_id, user_id),
    FOREIGN KEY (workspace_id, user_id)
        REFERENCES workspace_memberships(workspace_id, user_id) ON DELETE CASCADE
);

CREATE TABLE workspace_creation_idempotency (
    user_id UUID NOT NULL,
    idempotency_key TEXT NOT NULL CHECK (char_length(idempotency_key) BETWEEN 16 AND 128),
    request_digest BYTEA NOT NULL CHECK (octet_length(request_digest) = 32),
    workspace_id UUID REFERENCES workspaces(workspace_id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (user_id, idempotency_key)
);

CREATE INDEX active_workspace_selections_workspace_idx
    ON active_workspace_selections (workspace_id);

GRANT USAGE ON SCHEMA tenancy_access TO xovyra_tenancy_access_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA tenancy_access TO xovyra_tenancy_access_runtime;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA tenancy_access TO xovyra_tenancy_access_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE xovyra_tenancy_access_owner IN SCHEMA tenancy_access
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO xovyra_tenancy_access_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE xovyra_tenancy_access_owner IN SCHEMA tenancy_access
    GRANT USAGE, SELECT ON SEQUENCES TO xovyra_tenancy_access_runtime;
