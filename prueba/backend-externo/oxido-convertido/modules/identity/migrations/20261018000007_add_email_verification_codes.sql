ALTER TABLE identity.one_time_tokens
    ADD COLUMN verification_code_hash BYTEA,
    ADD COLUMN verification_code_expires_at TIMESTAMPTZ,
    ADD COLUMN verification_code_attempts INTEGER NOT NULL DEFAULT 0,
    ADD CONSTRAINT one_time_tokens_verification_code_hash_check
        CHECK (verification_code_hash IS NULL OR octet_length(verification_code_hash) = 32),
    ADD CONSTRAINT one_time_tokens_verification_code_attempts_check
        CHECK (verification_code_attempts BETWEEN 0 AND 5),
    ADD CONSTRAINT one_time_tokens_verification_code_fields_check
        CHECK ((verification_code_hash IS NULL) = (verification_code_expires_at IS NULL));

CREATE INDEX one_time_tokens_email_verification_code_expiration_idx
    ON identity.one_time_tokens (user_id, verification_code_expires_at DESC)
    WHERE purpose = 'email_verification'
      AND verification_code_hash IS NOT NULL
      AND consumed_at IS NULL
      AND revoked_at IS NULL;
