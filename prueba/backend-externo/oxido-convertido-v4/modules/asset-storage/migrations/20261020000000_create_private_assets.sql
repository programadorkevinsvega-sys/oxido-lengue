CREATE TABLE assets (
    asset_id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL,
    owner_user_id UUID NOT NULL,
    object_key TEXT NOT NULL UNIQUE,
    upload_object_key TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 255),
    content_type TEXT NOT NULL CHECK (char_length(content_type) BETWEEN 1 AND 160),
    declared_size_bytes BIGINT NOT NULL CHECK (declared_size_bytes BETWEEN 1 AND 2147483648),
    stored_size_bytes BIGINT,
    etag TEXT,
    lifecycle_state TEXT NOT NULL CHECK (lifecycle_state IN ('upload_pending', 'active', 'delete_pending', 'deleted', 'failed')),
    search_document TSVECTOR GENERATED ALWAYS AS (to_tsvector('simple', display_name)) STORED,
    idempotency_key TEXT NOT NULL CHECK (char_length(idempotency_key) BETWEEN 16 AND 128),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    upload_expires_at TIMESTAMPTZ NOT NULL DEFAULT (now() + interval '15 minutes'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (workspace_id, owner_user_id, idempotency_key)
);

CREATE INDEX assets_workspace_owner_state_idx
    ON assets (workspace_id, owner_user_id, lifecycle_state, created_at DESC);
CREATE INDEX assets_search_document_idx ON assets USING GIN (search_document);

GRANT SELECT, INSERT, UPDATE, DELETE ON assets TO xovyra_asset_storage_runtime;
ALTER DEFAULT PRIVILEGES IN SCHEMA asset_storage
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO xovyra_asset_storage_runtime;
