CREATE TABLE platform_permissions (
    permission_code TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    catalog_version INTEGER NOT NULL CHECK (catalog_version > 0)
);

CREATE TABLE platform_roles (
    role_id UUID PRIMARY KEY,
    role_code TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL,
    system_managed BOOLEAN NOT NULL DEFAULT true
);

CREATE TABLE platform_role_permissions (
    role_id UUID NOT NULL REFERENCES platform_roles(role_id) ON DELETE CASCADE,
    permission_code TEXT NOT NULL REFERENCES platform_permissions(permission_code) ON DELETE RESTRICT,
    PRIMARY KEY (role_id, permission_code)
);

CREATE TABLE platform_operator_roles (
    user_id UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES platform_roles(role_id) ON DELETE RESTRICT,
    granted_at TIMESTAMPTZ NOT NULL,
    granted_by_user_id UUID REFERENCES users(user_id) ON DELETE SET NULL,
    revoked_at TIMESTAMPTZ,
    revoked_by_user_id UUID REFERENCES users(user_id) ON DELETE SET NULL,
    PRIMARY KEY (user_id, role_id),
    CHECK (revoked_at IS NOT NULL OR revoked_by_user_id IS NULL)
);

CREATE INDEX platform_operator_roles_active_idx
    ON platform_operator_roles (user_id, role_id)
    WHERE revoked_at IS NULL;

CREATE TABLE platform_access_audit (
    event_id UUID PRIMARY KEY,
    actor_user_id UUID REFERENCES users(user_id) ON DELETE SET NULL,
    subject_user_id UUID REFERENCES users(user_id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    result TEXT NOT NULL CHECK (result IN ('succeeded', 'rejected', 'failed')),
    reason TEXT,
    correlation_id TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX platform_access_audit_recent_idx
    ON platform_access_audit (occurred_at DESC, event_id DESC);

INSERT INTO platform_permissions (permission_code, description, catalog_version) VALUES
    ('platform.auth.read', 'Read the authenticated operator identity and permissions', 1),
    ('platform.operators.manage', 'Provision and revoke platform operator roles', 1),
    ('platform.organizations.read', 'Read global organization status', 1),
    ('platform.billing.read', 'Read global billing status', 1),
    ('platform.usage.read', 'Read global usage and storage attribution', 1),
    ('platform.integrations.read', 'Read global integration status', 1),
    ('platform.incidents.manage', 'Manage platform incidents', 1),
    ('platform.audit.read', 'Read platform access audit events', 1);

INSERT INTO platform_roles (role_id, role_code, description) VALUES
    ('870e3af7-3c04-48f6-bcba-343c82040701', 'platform_admin', 'Full platform administration'),
    ('870e3af7-3c04-48f6-bcba-343c82040702', 'platform_support_readonly', 'Read-only platform support');

INSERT INTO platform_role_permissions (role_id, permission_code)
SELECT '870e3af7-3c04-48f6-bcba-343c82040701', permission_code
FROM platform_permissions;

INSERT INTO platform_role_permissions (role_id, permission_code) VALUES
    ('870e3af7-3c04-48f6-bcba-343c82040702', 'platform.auth.read'),
    ('870e3af7-3c04-48f6-bcba-343c82040702', 'platform.organizations.read'),
    ('870e3af7-3c04-48f6-bcba-343c82040702', 'platform.billing.read'),
    ('870e3af7-3c04-48f6-bcba-343c82040702', 'platform.usage.read'),
    ('870e3af7-3c04-48f6-bcba-343c82040702', 'platform.integrations.read'),
    ('870e3af7-3c04-48f6-bcba-343c82040702', 'platform.audit.read');

GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA identity TO xovyra_identity_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE xovyra_identity_owner IN SCHEMA identity
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO xovyra_identity_runtime;
