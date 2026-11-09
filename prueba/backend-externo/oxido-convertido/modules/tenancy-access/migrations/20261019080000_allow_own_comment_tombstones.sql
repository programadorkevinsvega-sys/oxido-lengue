ALTER TABLE tenancy_access.workspace_task_comments
    ADD COLUMN deleted_at TIMESTAMPTZ;

ALTER TABLE tenancy_access.workspace_audit_events
    DROP CONSTRAINT workspace_audit_events_action_check,
    ADD CONSTRAINT workspace_audit_events_action_check CHECK (action IN (
        'workspace.created', 'workspace.updated', 'workspace.deletion_requested', 'workspace.restored', 'workspace.purged',
        'member.invited', 'member.accepted', 'member.role_changed', 'member.suspended', 'member.reactivated', 'member.revoked',
        'role.created', 'role.updated', 'role.deleted', 'ownership.transferred',
        'task.created', 'task.updated', 'task.comment_created', 'task.comment_deleted'
    ));

GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA tenancy_access TO xovyra_tenancy_access_runtime;
