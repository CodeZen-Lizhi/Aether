-- 修复已删除供应商/渠道密钥遗留的优先级；不改变白名单、规则条件和 stop_processing。
-- 优先级只对存在的实体有意义；历史版本保留供审计，当前配置在写入和实体删除时归一化。
CREATE VIEW routing_priority_repairs AS
WITH RECURSIVE
stale_paths AS (
    SELECT g.id, r.key AS rule_index, a.key AS action_index,
           r.fullkey || '.actions[' || a.key || ']' AS path
    FROM routing_groups g, json_each(g.config_json, '$.rules') r,
         json_each(r.value, '$.actions') a
    WHERE (json_extract(a.value, '$.type') = 'set_provider_priority'
           AND json_type(a.value, '$.provider_id') = 'text'
           AND NOT EXISTS (SELECT 1 FROM providers p WHERE p.id = json_extract(a.value, '$.provider_id')))
       OR (json_extract(a.value, '$.type') = 'set_key_priority'
           AND json_type(a.value, '$.key_id') = 'text'
           AND NOT EXISTS (SELECT 1 FROM provider_api_keys k WHERE k.id = json_extract(a.value, '$.key_id')))
    UNION ALL
    SELECT g.id, -1, -1, m.fullkey || '.provider_priority_overrides.' || json_quote(o.key)
    FROM routing_groups g, json_each(g.config_json, '$.model_policies') m,
         json_each(m.value, '$.provider_priority_overrides') o
    WHERE json_type(m.value, '$.provider_priority_overrides') = 'object'
      AND NOT EXISTS (SELECT 1 FROM providers p WHERE p.id = o.key)
    UNION ALL
    SELECT g.id, -1, -1, m.fullkey || '.key_priority_overrides.' || json_quote(o.key)
    FROM routing_groups g, json_each(g.config_json, '$.model_policies') m,
         json_each(m.value, '$.key_priority_overrides') o
    WHERE json_type(m.value, '$.key_priority_overrides') = 'object'
      AND NOT EXISTS (SELECT 1 FROM provider_api_keys k WHERE k.id = o.key)
),
ordered_paths AS (
    -- 数组从尾到头移除，避免下标移动造成误删；对象字段删除不影响数组位置。
    SELECT id, path, row_number() OVER (
        PARTITION BY id ORDER BY rule_index DESC, action_index DESC, path
    ) AS step FROM stale_paths
),
repaired(id, step, config_json) AS (
    SELECT g.id, 0, g.config_json FROM routing_groups g
    WHERE EXISTS (SELECT 1 FROM ordered_paths p WHERE p.id = g.id)
    UNION ALL
    SELECT r.id, p.step, json_remove(r.config_json, p.path)
    FROM repaired r JOIN ordered_paths p ON p.id = r.id AND p.step = r.step + 1
)
SELECT id, config_json FROM repaired r
WHERE NOT EXISTS (SELECT 1 FROM ordered_paths p WHERE p.id = r.id AND p.step > r.step);

-- 升级时修复历史配置；保留实体、秘密、规则和历史版本，失败时由迁移事务回滚。
UPDATE routing_groups
SET config_json = (SELECT config_json FROM routing_priority_repairs p WHERE p.id = routing_groups.id)
WHERE id IN (SELECT id FROM routing_priority_repairs);

-- 与删除语句处于同一事务，包含直接删除和凭据 CAS 删除路径。
CREATE TRIGGER routing_priorities_after_provider_delete AFTER DELETE ON providers
BEGIN
    UPDATE routing_groups
    SET config_json = (SELECT config_json FROM routing_priority_repairs p WHERE p.id = routing_groups.id)
    WHERE id IN (SELECT id FROM routing_priority_repairs);
END;

CREATE TRIGGER routing_priorities_after_key_delete AFTER DELETE ON provider_api_keys
BEGIN
    UPDATE routing_groups
    SET config_json = (SELECT config_json FROM routing_priority_repairs p WHERE p.id = routing_groups.id)
    WHERE id IN (SELECT id FROM routing_priority_repairs);
END;

-- 旧页面或历史配置重新保存时不能重新引入失效优先级；无变化时不触发递归更新。
CREATE TRIGGER routing_priorities_after_insert AFTER INSERT ON routing_groups
WHEN EXISTS (SELECT 1 FROM routing_priority_repairs WHERE id = NEW.id)
BEGIN
    UPDATE routing_groups
    SET config_json = (SELECT config_json FROM routing_priority_repairs WHERE id = NEW.id)
    WHERE id = NEW.id;
END;

CREATE TRIGGER routing_priorities_after_update AFTER UPDATE OF config_json ON routing_groups
WHEN EXISTS (SELECT 1 FROM routing_priority_repairs WHERE id = NEW.id)
BEGIN
    UPDATE routing_groups
    SET config_json = (SELECT config_json FROM routing_priority_repairs WHERE id = NEW.id)
    WHERE id = NEW.id;
END;
