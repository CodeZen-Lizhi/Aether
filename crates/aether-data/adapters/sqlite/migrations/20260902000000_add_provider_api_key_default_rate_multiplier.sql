-- Key 级默认成本倍率：格式级 rate_multipliers 未命中时结算回落到该值（未配置则视为 1）
ALTER TABLE provider_api_keys ADD COLUMN default_rate_multiplier REAL NOT NULL DEFAULT 1;
