ALTER TABLE workspace_subscriptions
    DROP CONSTRAINT workspace_subscriptions_workspace_id_key;

CREATE UNIQUE INDEX workspace_subscriptions_one_open_per_workspace_idx
    ON workspace_subscriptions(workspace_id)
    WHERE status NOT IN ('canceled', 'incomplete_expired');

CREATE INDEX workspace_subscriptions_workspace_history_idx
    ON workspace_subscriptions(workspace_id, created_at DESC, subscription_id DESC);
