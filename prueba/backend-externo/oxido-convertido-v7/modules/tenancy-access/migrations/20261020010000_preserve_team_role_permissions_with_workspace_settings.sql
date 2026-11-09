CREATE OR REPLACE FUNCTION tenancy_access.seed_workspace_team_roles()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    INSERT INTO tenancy_access.workspace_roles (
        workspace_id,
        role_key,
        display_name,
        permissions,
        system_role
    ) VALUES
        (
            NEW.workspace_id,
            'owner',
            'Propietario',
            ARRAY[
                'members.read', 'members.invite', 'members.manage', 'roles.manage',
                'ownership.transfer', 'tasks.read', 'tasks.manage',
                'tasks.update_assigned_status', 'comments.read', 'comments.create',
                'comments.delete_own', 'workspace.settings.manage'
            ],
            TRUE
        ),
        (
            NEW.workspace_id,
            'admin',
            'Administrador',
            ARRAY[
                'members.read', 'members.invite', 'members.manage', 'roles.manage',
                'tasks.read', 'tasks.manage', 'tasks.update_assigned_status',
                'comments.read', 'comments.create', 'comments.delete_own',
                'workspace.settings.manage'
            ],
            TRUE
        ),
        (
            NEW.workspace_id,
            'editor',
            'Editor',
            ARRAY[
                'members.read', 'tasks.read', 'tasks.update_assigned_status',
                'comments.read', 'comments.create', 'comments.delete_own'
            ],
            TRUE
        ),
        (
            NEW.workspace_id,
            'viewer',
            'Lector',
            ARRAY['members.read', 'tasks.read', 'comments.read'],
            TRUE
        );

    RETURN NEW;
END
$$;
