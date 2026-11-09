CREATE TABLE jobs (
    job_id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL,
    owner_user_id UUID NOT NULL,
    idempotency_key TEXT NOT NULL CHECK (char_length(idempotency_key) BETWEEN 16 AND 128),
    job_type TEXT NOT NULL CHECK (job_type IN ('asset.delete')),
    resource_id UUID NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled')),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    lease_owner TEXT,
    lease_expires_at TIMESTAMPTZ,
    failure_code TEXT,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, idempotency_key)
);

CREATE INDEX jobs_workspace_owner_created_idx ON jobs (workspace_id, owner_user_id, created_at DESC);
CREATE INDEX jobs_queue_claim_idx ON jobs (next_attempt_at, created_at) WHERE status IN ('queued', 'running');

GRANT SELECT, INSERT, UPDATE, DELETE ON jobs TO xovyra_jobs_runtime;
ALTER DEFAULT PRIVILEGES IN SCHEMA jobs
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO xovyra_jobs_runtime;
