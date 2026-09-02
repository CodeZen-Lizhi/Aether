-- 单用户化收尾：drop 纯用户侧旧表（含数据，破坏性；回滚只能还原执行前的 db 备份）
-- 保留 auth_modules（/api/auth/settings 本地登录开关仍在用）。
-- users/api_keys/wallets/user_sessions/user_preferences/management_tokens 与 audit/usage/stats 表结构不动。

DROP TABLE IF EXISTS user_group_members;
DROP TABLE IF EXISTS user_groups;
DROP TABLE IF EXISTS user_invite_codes;
DROP TABLE IF EXISTS user_referrals;
DROP TABLE IF EXISTS user_oauth_links;
DROP TABLE IF EXISTS oauth_providers;
DROP TABLE IF EXISTS ldap_configs;

-- referral_rewards.referral_id 的外键指向上面已 drop 的 user_referrals。
-- 残留悬挂外键会让 DELETE FROM users 在级联动作编译时报
-- "no such table: main.user_referrals"，因此重建该表去除悬挂外键
-- （列、其余外键与索引保持不变；有效 referral 的奖励行会随
-- user_referrals 的 ON DELETE CASCADE 在上面 DROP 时一并清除）。
CREATE TABLE referral_rewards_rebuilt (
    id TEXT PRIMARY KEY,
    referral_id TEXT NOT NULL,
    inviter_user_id TEXT NOT NULL,
    invitee_user_id TEXT NOT NULL,
    reward_type TEXT NOT NULL,
    trigger_point TEXT NOT NULL,
    source_order_id TEXT,
    idempotency_key TEXT NOT NULL UNIQUE,
    amount_usd REAL NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    wallet_transaction_id TEXT,
    reversed_amount_usd REAL NOT NULL DEFAULT 0,
    pending_reversal_amount_usd REAL NOT NULL DEFAULT 0,
    failure_reason TEXT,
    admin_operator_id TEXT,
    admin_note TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(inviter_user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY(invitee_user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY(source_order_id) REFERENCES payment_orders(id) ON DELETE SET NULL
);

INSERT INTO referral_rewards_rebuilt (
    id, referral_id, inviter_user_id, invitee_user_id, reward_type, trigger_point,
    source_order_id, idempotency_key, amount_usd, status, wallet_transaction_id,
    reversed_amount_usd, pending_reversal_amount_usd, failure_reason, admin_operator_id,
    admin_note, created_at, updated_at
)
SELECT
    id, referral_id, inviter_user_id, invitee_user_id, reward_type, trigger_point,
    source_order_id, idempotency_key, amount_usd, status, wallet_transaction_id,
    reversed_amount_usd, pending_reversal_amount_usd, failure_reason, admin_operator_id,
    admin_note, created_at, updated_at
FROM referral_rewards;

DROP TABLE referral_rewards;
ALTER TABLE referral_rewards_rebuilt RENAME TO referral_rewards;

CREATE INDEX IF NOT EXISTS idx_referral_rewards_inviter_status
  ON referral_rewards (inviter_user_id, status, created_at);
CREATE INDEX IF NOT EXISTS idx_referral_rewards_inviter_created
  ON referral_rewards (inviter_user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_referral_rewards_created
  ON referral_rewards (created_at);
CREATE INDEX IF NOT EXISTS idx_referral_rewards_source_order
  ON referral_rewards (source_order_id);
