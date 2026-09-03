-- Key 优先级属于 Key 配置本体，调度策略不再提供重复入口。
-- 20260901000000 曾移除该列；恢复后将已有 Key 初始化为中性优先级。
ALTER TABLE provider_api_keys
    ADD COLUMN internal_priority INTEGER NOT NULL DEFAULT 50;

CREATE INDEX IF NOT EXISTS idx_provider_api_keys_provider_default_sort
    ON provider_api_keys (provider_id, internal_priority, name, id);

CREATE INDEX IF NOT EXISTS idx_provider_api_keys_provider_active_priority_id
    ON provider_api_keys (provider_id, is_active, internal_priority, id);
