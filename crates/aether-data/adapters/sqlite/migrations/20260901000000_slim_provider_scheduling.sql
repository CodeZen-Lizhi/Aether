-- 裁剪：移除号池评分体系与供应商侧优先级调度
-- （供应商类型收窄为 custom；调度统一由 routing_groups 调度策略模块承担）

DROP INDEX IF EXISTS idx_provider_api_keys_provider_default_sort;
DROP INDEX IF EXISTS idx_provider_api_keys_provider_active_priority_id;

DROP TABLE IF EXISTS pool_member_scores;

ALTER TABLE providers DROP COLUMN priority;
ALTER TABLE providers DROP COLUMN provider_priority;
ALTER TABLE providers DROP COLUMN keep_priority_on_conversion;

ALTER TABLE provider_api_keys DROP COLUMN internal_priority;
ALTER TABLE provider_api_keys DROP COLUMN global_priority_by_format;
