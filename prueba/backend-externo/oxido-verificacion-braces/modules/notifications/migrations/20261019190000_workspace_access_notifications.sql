ALTER TABLE notification_inbox DROP CONSTRAINT notification_inbox_notification_type_check;
ALTER TABLE notification_inbox ADD CONSTRAINT notification_inbox_notification_type_check CHECK (notification_type IN (
    'task_assigned',
    'task_reassigned',
    'task_mentioned',
    'workspace_invitation_received',
    'workspace_invitation_accepted',
    'workspace_member_suspended',
    'workspace_member_revoked',
    'workspace_member_reactivated',
    'workspace_member_role_changed',
    'workspace_ownership_received',
    'workspace_deletion_scheduled',
    'workspace_deleted'
));
