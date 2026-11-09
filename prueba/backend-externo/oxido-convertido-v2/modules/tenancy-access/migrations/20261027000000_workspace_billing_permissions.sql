UPDATE tenancy_access.workspace_roles
SET permissions = ARRAY(SELECT DISTINCT permission FROM unnest(permissions || ARRAY['workspace.billing.read', 'workspace.billing.manage']) permission ORDER BY permission),
    row_version = row_version + 1,
    updated_at = now()
WHERE role_key = 'owner';

UPDATE tenancy_access.workspace_roles
SET permissions = ARRAY(SELECT DISTINCT permission FROM unnest(permissions || ARRAY['workspace.billing.read', 'workspace.billing.manage']) permission ORDER BY permission),
    row_version = row_version + 1,
    updated_at = now()
WHERE role_key = 'admin';

CREATE OR REPLACE FUNCTION tenancy_access.seed_workspace_team_roles()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    INSERT INTO tenancy_access.workspace_roles (workspace_id, role_key, display_name, permissions, system_role) VALUES
        (NEW.workspace_id, 'owner', 'Propietario', ARRAY['members.read','members.invite','members.manage','roles.manage','ownership.transfer','tasks.read','tasks.manage','tasks.update_assigned_status','comments.read','comments.create','comments.delete_own','workspace.settings.manage','assets.read','assets.create','assets.delete','jobs.read','jobs.cancel','jobs.manage','workspace.billing.read','workspace.billing.manage'], TRUE),
        (NEW.workspace_id, 'admin', 'Administrador', ARRAY['members.read','members.invite','members.manage','roles.manage','tasks.read','tasks.manage','tasks.update_assigned_status','comments.read','comments.create','comments.delete_own','workspace.settings.manage','assets.read','assets.create','assets.delete','jobs.read','jobs.cancel','jobs.manage','workspace.billing.read','workspace.billing.manage'], TRUE),
        (NEW.workspace_id, 'editor', 'Editor', ARRAY['members.read','tasks.read','tasks.update_assigned_status','comments.read','comments.create','comments.delete_own','assets.read','assets.create','assets.delete','jobs.read','jobs.cancel'], TRUE),
        (NEW.workspace_id, 'viewer', 'Lector', ARRAY['members.read','tasks.read','comments.read','assets.read','jobs.read'], TRUE);
    RETURN NEW;
END
$$;
