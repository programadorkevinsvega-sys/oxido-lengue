CREATE TABLE workspace_roles (
    workspace_id UUID NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    role_key TEXT NOT NULL CHECK (role_key ~ '^[a-z][a-z0-9-]{1,39}$'),
    display_name TEXT NOT NULL CHECK (char_length(display_name) BETWEEN 2 AND 80),
    permissions TEXT[] NOT NULL,
    system_role BOOLEAN NOT NULL DEFAULT FALSE,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_by_user_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, role_key)
);

INSERT INTO workspace_roles (workspace_id, role_key, display_name, permissions, system_role)
SELECT workspace_id, 'owner', 'Propietario', ARRAY['members.read','members.invite','members.manage','roles.manage','ownership.transfer','tasks.read','tasks.manage','comments.read','comments.create','comments.delete_own'], TRUE FROM workspaces
UNION ALL
SELECT workspace_id, 'admin', 'Administrador', ARRAY['members.read','members.invite','members.manage','roles.manage','tasks.read','tasks.manage','comments.read','comments.create','comments.delete_own'], TRUE FROM workspaces
UNION ALL
SELECT workspace_id, 'editor', 'Editor', ARRAY['members.read','tasks.read','tasks.manage','comments.read','comments.create','comments.delete_own'], TRUE FROM workspaces
UNION ALL
SELECT workspace_id, 'viewer', 'Lector', ARRAY['members.read','tasks.read','comments.read'], TRUE FROM workspaces;

CREATE FUNCTION seed_workspace_team_roles() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO workspace_roles (workspace_id, role_key, display_name, permissions, system_role) VALUES
        (NEW.workspace_id, 'owner', 'Propietario', ARRAY['members.read','members.invite','members.manage','roles.manage','ownership.transfer','tasks.read','tasks.manage','comments.read','comments.create','comments.delete_own'], TRUE),
        (NEW.workspace_id, 'admin', 'Administrador', ARRAY['members.read','members.invite','members.manage','roles.manage','tasks.read','tasks.manage','comments.read','comments.create','comments.delete_own'], TRUE),
        (NEW.workspace_id, 'editor', 'Editor', ARRAY['members.read','tasks.read','tasks.manage','comments.read','comments.create','comments.delete_own'], TRUE),
        (NEW.workspace_id, 'viewer', 'Lector', ARRAY['members.read','tasks.read','comments.read'], TRUE);
    RETURN NEW;
END $$;
CREATE TRIGGER workspace_team_roles_after_insert AFTER INSERT ON workspaces FOR EACH ROW EXECUTE FUNCTION seed_workspace_team_roles();

ALTER TABLE workspace_audit_events
    DROP CONSTRAINT workspace_audit_events_action_check,
    ADD CONSTRAINT workspace_audit_events_action_check CHECK (action IN (
        'workspace.created', 'workspace.updated', 'workspace.deletion_requested', 'workspace.restored', 'workspace.purged',
        'member.invited', 'member.accepted', 'member.role_changed', 'member.suspended', 'member.reactivated', 'member.revoked',
        'role.created', 'role.updated', 'role.deleted', 'ownership.transferred',
        'task.created', 'task.updated', 'task.comment_created'
    ));

ALTER TABLE workspace_memberships
    DROP CONSTRAINT workspace_memberships_role_check,
    ADD COLUMN email_normalized TEXT,
    ADD COLUMN row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    ADD CONSTRAINT workspace_memberships_role_fk
        FOREIGN KEY (workspace_id, role) REFERENCES workspace_roles(workspace_id, role_key);

CREATE UNIQUE INDEX workspace_one_active_owner_idx
    ON workspace_memberships (workspace_id)
    WHERE role = 'owner' AND status = 'active';

CREATE TABLE workspace_invitations (
    invitation_id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    email_normalized TEXT NOT NULL CHECK (email_normalized = lower(btrim(email_normalized)) AND char_length(email_normalized) BETWEEN 3 AND 254),
    role_key TEXT NOT NULL,
    invited_by_user_id UUID NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'accepted', 'revoked', 'expired')),
    delivery_status TEXT NOT NULL CHECK (delivery_status IN ('pending', 'sent', 'failed')),
    created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    accepted_at TIMESTAMPTZ,
    accepted_by_user_id UUID,
    delivery_updated_at TIMESTAMPTZ,
    delivery_attempts INTEGER NOT NULL DEFAULT 0 CHECK (delivery_attempts >= 0),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    idempotency_key TEXT NOT NULL CHECK (char_length(idempotency_key) BETWEEN 16 AND 128),
    request_digest BYTEA NOT NULL CHECK (octet_length(request_digest) = 32),
    FOREIGN KEY (workspace_id, role_key) REFERENCES workspace_roles(workspace_id, role_key),
    UNIQUE (workspace_id, invited_by_user_id, idempotency_key)
);

CREATE UNIQUE INDEX workspace_pending_invitation_email_idx
    ON workspace_invitations (workspace_id, email_normalized)
    WHERE status = 'pending';
CREATE INDEX workspace_invitations_expiry_idx
    ON workspace_invitations (expires_at, invitation_id)
    WHERE status = 'pending';

CREATE TABLE workspace_tasks (
    task_id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK (char_length(title) BETWEEN 1 AND 180),
    description TEXT NOT NULL DEFAULT '' CHECK (char_length(description) <= 10000),
    status TEXT NOT NULL CHECK (status IN ('open', 'in_progress', 'done', 'cancelled')),
    assignee_user_id UUID,
    author_user_id UUID NOT NULL,
    idempotency_key TEXT NOT NULL CHECK (char_length(idempotency_key) BETWEEN 16 AND 128),
    request_digest BYTEA NOT NULL CHECK (octet_length(request_digest) = 32),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    FOREIGN KEY (workspace_id, assignee_user_id)
        REFERENCES workspace_memberships(workspace_id, user_id),
    UNIQUE (workspace_id, author_user_id, idempotency_key)
);
CREATE INDEX workspace_tasks_list_idx
    ON workspace_tasks (workspace_id, created_at DESC, task_id);
CREATE INDEX workspace_tasks_assignee_idx
    ON workspace_tasks (workspace_id, assignee_user_id, status, updated_at DESC);
CREATE UNIQUE INDEX workspace_tasks_workspace_task_idx
    ON workspace_tasks (workspace_id, task_id);

CREATE TABLE workspace_task_comments (
    comment_id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL,
    task_id UUID NOT NULL,
    author_user_id UUID NOT NULL,
    body TEXT NOT NULL CHECK (char_length(body) BETWEEN 1 AND 5000),
    mentioned_user_ids UUID[] NOT NULL DEFAULT '{}',
    idempotency_key TEXT NOT NULL CHECK (char_length(idempotency_key) BETWEEN 16 AND 128),
    created_at TIMESTAMPTZ NOT NULL,
    FOREIGN KEY (workspace_id, task_id)
        REFERENCES workspace_tasks(workspace_id, task_id) ON DELETE CASCADE,
    UNIQUE (workspace_id, author_user_id, idempotency_key)
);
CREATE INDEX workspace_task_comments_list_idx
    ON workspace_task_comments (workspace_id, task_id, created_at, comment_id);

GRANT USAGE ON SCHEMA tenancy_access TO xovyra_tenancy_access_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA tenancy_access TO xovyra_tenancy_access_runtime;
