CREATE TABLE provider_key_health_pending_facts (
    id TEXT PRIMARY KEY NOT NULL,
    observed_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    payload TEXT NOT NULL
);
CREATE INDEX provider_key_health_pending_facts_order_idx
    ON provider_key_health_pending_facts (observed_at, id);
