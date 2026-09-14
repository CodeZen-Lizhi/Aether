CREATE TABLE provider_key_health_settlements (
    id TEXT NOT NULL PRIMARY KEY,
    key_id TEXT NOT NULL,
    api_format TEXT NOT NULL,
    policy_version INTEGER NOT NULL,
    attempt_started_at INTEGER NOT NULL,
    settled_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);

CREATE INDEX provider_key_health_settlements_expiry_idx
    ON provider_key_health_settlements (expires_at);
