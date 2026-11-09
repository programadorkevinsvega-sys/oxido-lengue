INSERT INTO identity.platform_permissions (permission_code, description, catalog_version) VALUES
    ('platform.support.grants.manage', 'Create and revoke scoped temporary support grants', 1),
    ('platform.support.content.read', 'Read tenant content only through an active scoped grant', 1)
ON CONFLICT (permission_code) DO NOTHING;

INSERT INTO identity.platform_role_permissions (role_id, permission_code)
SELECT role.role_id, permission.permission_code
FROM identity.platform_roles AS role
JOIN identity.platform_permissions AS permission ON permission.permission_code IN (
    'platform.support.grants.manage',
    'platform.support.content.read'
)
WHERE role.role_code = 'platform_admin'
ON CONFLICT DO NOTHING;
