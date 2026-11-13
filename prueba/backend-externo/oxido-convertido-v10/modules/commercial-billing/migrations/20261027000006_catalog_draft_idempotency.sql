CREATE TABLE catalog_draft_idempotency (
    idempotency_key TEXT PRIMARY KEY CHECK (length(idempotency_key) BETWEEN 16 AND 128),
    operator_user_id UUID NOT NULL,
    request_fingerprint TEXT NOT NULL,
    plan_id UUID,
    plan_version INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((plan_id IS NULL) = (plan_version IS NULL))
);
