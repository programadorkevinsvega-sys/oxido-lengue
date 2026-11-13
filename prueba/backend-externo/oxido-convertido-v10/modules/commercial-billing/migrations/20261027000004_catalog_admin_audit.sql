CREATE TABLE catalog_admin_audit (
    event_id UUID PRIMARY KEY,
    operator_user_id UUID NOT NULL,
    plan_id UUID NOT NULL,
    plan_version INTEGER NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('draft_saved', 'approved', 'published')),
    correlation_id TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX catalog_admin_audit_plan_idx
    ON catalog_admin_audit(plan_id, plan_version, occurred_at DESC);

GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA commercial_billing
    TO xovyra_commercial_billing_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE xovyra_commercial_billing_owner IN SCHEMA commercial_billing
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO xovyra_commercial_billing_runtime;
