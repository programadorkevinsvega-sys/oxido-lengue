CREATE TABLE catalog_plans (
    plan_id UUID NOT NULL,
    version INTEGER NOT NULL CHECK (version > 0),
    plan_key TEXT NOT NULL,
    display_name TEXT NOT NULL,
    description TEXT NOT NULL,
    lifecycle TEXT NOT NULL CHECK (lifecycle IN ('draft', 'approved', 'published', 'retired')),
    trial_days SMALLINT NOT NULL DEFAULT 0 CHECK (trial_days BETWEEN 0 AND 30),
    profitability_evidence TEXT,
    approved_by_operator UUID,
    approved_at TIMESTAMPTZ,
    row_version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (plan_id, version),
    UNIQUE (plan_key, version),
    CHECK ((lifecycle NOT IN ('approved', 'published')) OR
           (profitability_evidence IS NOT NULL AND approved_by_operator IS NOT NULL AND approved_at IS NOT NULL))
);

CREATE TABLE localized_plan_prices (
    price_id UUID PRIMARY KEY,
    plan_id UUID NOT NULL,
    plan_version INTEGER NOT NULL,
    locale TEXT NOT NULL,
    currency CHAR(3) NOT NULL,
    amount_minor BIGINT NOT NULL CHECK (amount_minor > 0),
    billing_interval TEXT NOT NULL CHECK (billing_interval = 'month'),
    stripe_price_id TEXT NOT NULL,
    effective_from TIMESTAMPTZ NOT NULL,
    effective_until TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (plan_id, plan_version) REFERENCES catalog_plans(plan_id, version),
    UNIQUE (plan_id, plan_version, locale, currency),
    CHECK (effective_until IS NULL OR effective_until > effective_from)
);

CREATE TABLE plan_capabilities (
    plan_id UUID NOT NULL,
    plan_version INTEGER NOT NULL,
    capability_key TEXT NOT NULL,
    limit_value BIGINT NOT NULL CHECK (limit_value >= 0),
    PRIMARY KEY (plan_id, plan_version, capability_key),
    FOREIGN KEY (plan_id, plan_version) REFERENCES catalog_plans(plan_id, version)
);

CREATE TABLE workspace_subscriptions (
    subscription_id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL,
    plan_id UUID NOT NULL,
    plan_version INTEGER NOT NULL,
    price_id UUID NOT NULL REFERENCES localized_plan_prices(price_id),
    status TEXT NOT NULL CHECK (status IN ('incomplete', 'trialing', 'active', 'past_due', 'canceled', 'incomplete_expired')),
    stripe_customer_id TEXT,
    stripe_subscription_id TEXT UNIQUE,
    stripe_checkout_session_id TEXT UNIQUE,
    current_period_start TIMESTAMPTZ,
    current_period_end TIMESTAMPTZ,
    cancel_at_period_end BOOLEAN NOT NULL DEFAULT FALSE,
    provider_event_created_at BIGINT NOT NULL DEFAULT 0 CHECK (provider_event_created_at >= 0),
    scheduled_plan_id UUID,
    scheduled_plan_version INTEGER,
    row_version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id),
    FOREIGN KEY (plan_id, plan_version) REFERENCES catalog_plans(plan_id, version),
    CHECK ((scheduled_plan_id IS NULL) = (scheduled_plan_version IS NULL))
);

CREATE TABLE stripe_webhook_inbox (
    event_id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at TIMESTAMPTZ,
    outcome TEXT NOT NULL CHECK (outcome IN ('received', 'processed', 'ignored', 'failed')),
    failure_code TEXT
);

CREATE INDEX workspace_subscriptions_status_idx ON workspace_subscriptions(status, updated_at);
CREATE INDEX catalog_plans_published_idx ON catalog_plans(plan_key, version DESC) WHERE lifecycle = 'published';

GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA commercial_billing TO xovyra_commercial_billing_owner;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA commercial_billing TO xovyra_commercial_billing_owner;
GRANT USAGE ON SCHEMA commercial_billing TO xovyra_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA commercial_billing TO xovyra_runtime;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA commercial_billing TO xovyra_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE xovyra_commercial_billing_owner IN SCHEMA commercial_billing
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO xovyra_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE xovyra_commercial_billing_owner IN SCHEMA commercial_billing
    GRANT USAGE, SELECT ON SEQUENCES TO xovyra_runtime;
