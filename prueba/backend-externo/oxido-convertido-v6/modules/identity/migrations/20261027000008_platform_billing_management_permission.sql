INSERT INTO platform_permissions (permission_code, description, catalog_version)
VALUES ('platform.billing.manage', 'Manage versioned commercial plans and approval gates', 1)
ON CONFLICT (permission_code) DO NOTHING;

INSERT INTO platform_role_permissions (role_id, permission_code)
SELECT role.role_id, 'platform.billing.manage'
FROM platform_roles AS role
WHERE role.role_code = 'platform_admin'
ON CONFLICT DO NOTHING;
