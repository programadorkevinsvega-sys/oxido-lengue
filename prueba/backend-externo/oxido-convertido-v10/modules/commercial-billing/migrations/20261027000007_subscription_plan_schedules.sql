WITH ranked AS (
    SELECT plan_id, version,
           row_number() OVER (PARTITION BY plan_key ORDER BY version DESC, created_at DESC) AS position
    FROM commercial_billing.catalog_plans
    WHERE lifecycle = 'published'
)
UPDATE commercial_billing.catalog_plans plan
SET lifecycle = 'retired', row_version = row_version + 1
FROM ranked
WHERE plan.plan_id = ranked.plan_id
  AND plan.version = ranked.version
  AND ranked.position > 1;

ALTER TABLE commercial_billing.workspace_subscriptions
    ADD COLUMN stripe_subscription_schedule_id TEXT UNIQUE;

CREATE UNIQUE INDEX catalog_plans_single_published_key
    ON commercial_billing.catalog_plans(plan_key)
    WHERE lifecycle = 'published';
