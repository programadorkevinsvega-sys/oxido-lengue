ALTER TABLE notification_inbox DROP CONSTRAINT notification_inbox_notification_type_check;
ALTER TABLE notification_inbox ADD CONSTRAINT notification_inbox_notification_type_check CHECK (notification_type IN ('task_assigned', 'task_reassigned', 'task_mentioned', 'workspace_invitation_received', 'workspace_invitation_accepted'));
ALTER TABLE notification_inbox DROP CONSTRAINT notification_inbox_resource_type_check;
ALTER TABLE notification_inbox ADD CONSTRAINT notification_inbox_resource_type_check CHECK (resource_type IN ('task', 'workspace_invitation', 'workspace'));
