CREATE UNIQUE INDEX one_time_tokens_provider_message_id_unique_idx
    ON identity.one_time_tokens (provider_message_id)
    WHERE provider_message_id IS NOT NULL;
