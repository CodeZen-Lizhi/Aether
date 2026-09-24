# 后端用户侧与角色系统盘点报告（2026-09-01）

探查代理产出，作为 deletion-checklist.md 的证据留存。路径省略前缀 `apps/aether-gateway/`。

**架构前提**：axum "compatibility frontdoor"。`src/router.rs:28-47` 只挂 6 组粗粒度函数 + 兜底 `proxy_request`。`/api/...` → `handlers/proxy/mod.rs:836 proxy_request` → `control/route/mod.rs:172 resolve_control_route` 路由分类 → 鉴权解析 → 命中 family 由本地响应构建器处理，未命中 501（`handlers/public/support.rs:58 build_unhandled_public_support_response`）或 503。因此删端点必须同步删分类器 arm。

## 1. 路由注册总表

顶层挂载：`mount_core_routes`（api/core.rs:15，gateway-manifest/readyz/health）、`mount_operational_routes`（api/ops.rs:9，/_gateway/metrics + /_gateway/audit/*）、`mount_public_support_routes`（api/backend/public.rs:6，/v1/models、/v1beta、/api/public/*、/api/modules/auth-status、/api/capabilities*、/install/*、/install-tunnel/*、/i/*、/）、`mount_internal_routes`（api/backend/internal.rs:13）、`mount_admin_routes`（api/backend/admin.rs:6，/api/admin/{*admin_path}）、兜底（router.rs:38 AI 代理）。

### a) 公开认证端点（family auth/auth_public/oauth；分类器 control/route/public_support.rs:100-136）

| 方法 | 路径 | handler |
|---|---|---|
| POST | /api/auth/login | handlers/public/support/auth.rs:33 |
| GET | /api/auth/me | handlers/public/support/auth_session.rs:317（build_auth_me_payload L164 含 role） |
| GET | /api/auth/refresh | auth_session.rs:344 |
| GET/POST | /api/auth/logout | auth_session.rs:721 |
| GET | /api/auth/settings | support.rs:124-132 |
| GET | /api/oauth/providers、/api/oauth/{p}/authorize、/api/oauth/{p}/callback | 分类器 control/route/oauth.rs:3；**分发无本地分支 → 501** |

- 无注册/自助开号端点。唯一账号初始化：src/state/bootstrap_admin.rs:205 bootstrap_admin_from_env（ADMIN_EMAIL/ADMIN_USERNAME/ADMIN_PASSWORD；:273-284 create_local_auth_user_with_settings(..., "admin", ...)）；:256 已有 active admin 时跳过。无 first-run HTTP 端点。

### b) 用户侧端点（family users_me/dashboard/monitoring_user/ccswitch）

分类器：public_support.rs:137-177（dashboard/monitoring/ccswitch）、:191-416（users_me + management-tokens）；分发 support.rs:108-111 → handlers/public/support/user_me_routes.rs:21。

| 方法 | 路径 | handler |
|---|---|---|
| GET | /api/users/me | auth_session.rs:317（user_me_routes.rs:33） |
| PUT | /api/users/me | user_me_profile.rs:67 |
| PATCH | /api/users/me/password | user_me_profile.rs:196 |
| GET | /api/users/me/sessions | user_me_sessions.rs:62 |
| DELETE | /api/users/me/sessions/others | user_me_sessions.rs:91 |
| PATCH/DELETE | /api/users/me/sessions/{id} | user_me_sessions.rs:189/140 |
| GET/POST | /api/users/me/api-keys | user_me_api_keys.rs:300/507（create → create_user_api_key L565-581） |
| GET/PUT/PATCH/DELETE | /api/users/me/api-keys/{id} | user_me_api_keys.rs:362/650/782/846 |
| PUT | /api/users/me/api-keys/{id}/providers、.../capabilities | user_me_api_keys.rs:885/1020 |
| POST | /api/users/me/api-keys/{id}/install-sessions | 分类器 public_support.rs:275-288；**user_me_routes match 无 arm → 501** |
| GET | /api/users/me/usage、/usage/active、/usage/interval-timeline、/usage/heatmap | user_me_usage.rs:978/1324/1433/1498 |
| GET | /api/users/me/providers、/available-models、/client-config | user_me_catalog.rs:290/137、user_me_profile.rs:43 |
| GET/PUT | /api/users/me/preferences、/model-capabilities | user_me_preferences.rs:173/202、:147/365 |
| GET/POST/PUT/PATCH/DELETE | /api/me/management-tokens[...] | 分类器 :230-318/399-416；**无本地 handler → 501**（数据层 data/state/auth.rs:1032-1112、state/catalog.rs:66-92） |
| GET | /api/dashboard/stats、recent-requests、provider-status、daily-stats | user_me support/dashboard.rs:15（要求用户 JWT；admin Dashboard 前端也在用，后端按 role 返回不同字段） |
| GET | /api/monitoring/my-audit-logs、/rate-limit-status | support/monitoring.rs（family monitoring_user） |
| GET | /api/ccswitch/usage | 分类器 :178-190（鉴权签名 aether:ccswitch_usage，resolution.rs:1223，identity-only :1213-1219）；**分发无分支 → 501**；前端 features/api-keys/utils/ccswitchImport.ts:47 调用 |
| GET | /api/capabilities、/api/capabilities/user-configurable、/api/capabilities/model/* | family capabilities（:417-443） |
| GET | /api/public/site-info、providers、models、search/models、stats、global-models、health/* | support.rs:133-380 内联 |
| GET | /v1/models、/v1beta/models[/{id}] | family models（需 API key auth_context） |
| GET | /install/*、/install-tunnel/*、/i/* | family install（:472-483）；**无本地 handler → 501** |

### c) admin 端点（route_class=admin_proxy，/api/admin/ 前缀）

分类器 control/route/admin.rs:31 → 8 family 文件；本地构建 handlers/proxy/local.rs:44 → handlers/admin/routes.rs:3。模块：model/provider（providers_manage、global_models_manage）、observability（usage/stats/monitoring/provider-query）、auth/api_keys（standalone key，/api/admin/api-keys*；install-sessions 分类 observability_families.rs:78-84，**routes.rs:35-85 match 无 arm → 503**）、users（users_manage，/api/admin/users*、user-groups*、users/{id}/api-keys*、sessions*、billing/*）、routing、system、endpoint（endpoints/adaptive）、payment/request（billing/payments）。

internal：control/route/internal.rs:4（/api/internal/gateway/{resolve,auth-context,decision-*,plan-*,report-*,execute-*,finalize-*} + tunnel）。

## 2. 角色系统（src/roles.rs）

常量 ROLE_USER/ROLE_ADMIN/ROLE_AUDIT_ADMIN（roles.rs:1-3）。调用点：

| 函数 | 调用位置 |
|---|---|
| can_access_admin_console | control/auth/resolution.rs:343（JWT role）、:370（DB 复查）、control/auth/credentials.rs:208（trusted header x-aether-admin-user-role）、state/runtime/auth/user_lifecycle.rs:74（默认用户组）、handlers/admin/users/groups.rs:285 |
| can_write_admin_console | 仅 roles.rs:39 测试。**生产无调用** |
| is_full_admin_role | handlers/admin/users/batch.rs:643/663、lifecycle/update.rs:154/157（防最后 admin 降级） |
| is_audit_admin_role | 仅被 can_access 引用 |
| normalize_assignable_user_role | handlers/admin/users/batch.rs:501、shared.rs:241 |

**鉴权拦截**：无 axum 中间件式 AdminGuard。本地 JWT：resolution.rs:323 resolve_local_admin_principal → :343 token role → :370 DB user is_active/is_deleted/role → :384-397 session 校验。trusted-header：resolution.rs:296 + credentials.rs:181-230（headers 见 constants.rs:57-63）。未通过 → admin_principal=None → proxy/mod.rs:1150-1166 拒绝。用户侧拦截：各 handler 调 auth_session.rs:254 resolve_authenticated_local_user（JWT + session，不查角色）。

## 3. 用户管理端点（后端完整保留，UI 已删）

handlers/admin/users/routes.rs:104-195 分派：create_user（lifecycle/create.rs:19，:100 角色归一化）、list_users（reads.rs:18）、get/update/delete_user（reads.rs、update.rs:23、delete.rs:14）、admin 重置密码（update.rs:233-253）、batch_action_users（batch.rs，:501/:643/:663 角色校验）、resolve_user_selection、user_groups CRUD/成员/默认组（groups.rs）、用户会话列表/踢出（sessions.rs）、给任意用户建/改/删/锁/看明文 key（users/api_keys/，is_standalone=0，sqlite/src/auth.rs:510-525）、billing/entitlements、grant-plan。

注册：无 HTTP 端点；register_local_auth_user（data/state/auth.rs:826、user_lifecycle.rs:794）生产零调用（死代码）。OAuth/LDAP 自助开户数据层入口：get_or_create_ldap_auth_user（user_lifecycle.rs/user_provisioning.rs:115）、create_oauth_auth_user（data/state/auth.rs:273），受 /api/auth/settings 开关控制。

## 4. 密钥模型

- 表：crates/aether-data/adapters/sqlite/migrations/20260403000000_baseline.sql:25-50 — api_keys(user_id TEXT NOT NULL, key_hash UNIQUE, is_standalone INTEGER DEFAULT 0)；:52 索引。无独立表。
- 快照：crates/aether-data/contracts/src/repository/auth.rs:4 StoredAuthApiKeySnapshot（user_id/username/user_role/user_is_active/... + api_key_is_standalone）。
- 创建者：standalone 仅 admin 端点 POST /api/admin/api-keys（mutation_routes.rs:153-171，:155 user_id=operator_id 即当前 admin；配独立钱包 initialize_auth_api_key_wallet，data/state/auth.rs:666）。普通 key：用户侧 user_me_api_keys.rs:507（→ sqlite/src/auth.rs:109 create_api_key）+ admin 给用户建（handlers/admin/users/api_keys/，is_standalone=0）。
- 鉴权：credentials.rs:397 hash_api_key（SHA-256）→ resolution.rs:929-936 read_auth_api_key_snapshot_by_key_hash_strong → sqlite/src/auth.rs:20-42 **FROM api_keys JOIN users ON users.id = api_keys.user_id**；auth.rs:200 find_api_key_snapshot 三种方式。钱包：wallets 表按 user_id 或 api_key_id 关联（data/state/auth.rs:677-707）。resolution.rs:1197-1200 admin_bypass_limits = user_role=="admin" && !api_key_is_standalone。
- **结论**：删用户侧后普通 key 无处创建（两个创建入口都删）；standalone key 只依赖当前 admin 的 user_id；JOIN users 与快照要求 users 行存在且 active（resolution.rs:370、sqlite/src/auth.rs:8-10）。**users 表不能物理删除，只能保留一行 admin。**

## 5. 数据库迁移（crates/aether-data/adapters/sqlite/migrations/）

| 迁移 | 内容 |
|---|---|
| 20260403000000_baseline.sql | users(:1，role/auth_source/is_active/is_deleted/allowed_*/rate_limit)、api_keys(:25)、user_sessions(:140)、management_tokens(:101)、user_preferences(:120)、wallets(:572，user_id UNIQUE 与 api_key_id UNIQUE 双向)、wallet_transactions(:591)、audit_logs(:54 user_id 可空)、usage(:774 user_id/api_key_id 可空，无 FK)、stats_*、auth_modules(:471)、oauth_providers(:480)、user_oauth_links(:516)、ldap_configs(:498)、gemini_file_mappings、video_tasks、request_candidates |
| 20260509120000_add_user_groups.sql | user_groups + user_group_members（纯用户侧） |
| 20260510120000_normalize_empty_user_policy_modes.sql | users 加 *_mode 4 列 |
| 20260511120000_exclude_admins_from_default_user_group.sql | admin 移出默认用户组 |
| 20260507120000_add_management_token_permissions.sql | permissions |
| 20260519000000_add_referrals_privacy_required_announcements.sql | user_invite_codes（**FK→users(id) ON DELETE CASCADE**）、user_referrals、隐私列 |
| 20260821000000_migrate_legacy_codex_live_permissions.sql | allowed_api_formats 数据迁移 |
| 20260520000000/20260512000000 等 | 弱相关（feature_settings 挂 user_id） |

SQLite 无声明式 FK（api_keys.user_id 等只是普通列+索引）；唯一显式 FK 是 user_invite_codes→users CASCADE。**建议保留 users 表只留一行 admin**；role 列保留（登录/JWT/resolution/batch/update 仍在读写）。

## 6. 认证机制

- 自制 JWT（HS256，auth_session.rs:48-163），secret auth_helpers.rs:52；access 有效期 :65，refresh 7 天（:17）。
- 登录：auth.rs:33 → bcrypt（:98-110）→ auth_session.rs:526 build_auth_login_success_response；claims = user_id/role/created_at/session_id（:540-549，**role 写进 token**）；refresh cookie（auth_helpers.rs:89/114）；session 行落 user_sessions（:583-617）。
- 校验：用户侧 resolve_authenticated_local_user（:254，token→claims→find_user_auth_by_id→session 匹配 client_device_id）；admin 侧见 §2。刷新轮换+防重放（:344-470）。
- 改密：用户自改 user_me_profile.rs:196（校验旧密码，LDAP 拒绝）；admin 重置 lifecycle/update.rs:233-253（不要求旧密码）。密码策略 password_policy_level（bootstrap_admin.rs:111-166）。
- 登出：auth_session.rs:721；撤销方法 data/state/auth.rs:918/933。
- **登录不检查角色**；角色只在访问 admin 时由 can_access_admin_console 拦截。

## 7. CLI 安装会话（install-sessions / ccswitch）

| 端点 | 分类器 | 本地 handler |
|---|---|---|
| 用户侧 POST /api/users/me/api-keys/{id}/install-sessions | public_support.rs:275-288 | **无** → 501 |
| 管理侧 POST /api/admin/api-keys/{id}/install-sessions | observability_families.rs:77-84 | **无**（routes.rs:35-85 落 _ → 503）；残留辅助 shared.rs:105 admin_api_key_install_session_id_from_path |
| POST /api/admin/proxy-nodes/install-sessions | operations_families.rs:294-301 | proxy-nodes 模块（与 CLI key 无关） |

- 无 install code 落库表（全仓 grep install_session 于 aether-data 为空）。前端调用存在（api/me.ts:332、api/admin.ts:946）——两端点均为「已分类、未实现」。
- ccswitch：仅前端脚本（ccswitchImport.ts:47）+ 分类器与鉴权签名（resolution.rs:1213-1223），响应 501。management-tokens 用户侧同理。

## 8. data/state 运行时状态

data/state/auth.rs（约 1600 行）：
- 仅用户侧使用：user_groups 全套（:93-224）、user_oauth_links（:226-361）、user_preferences（:363-397）、update_user_model_capability_settings/update_user_feature_settings（:441-465）、get_or_create_ldap_auth_user（:613）、register_local_auth_user（:826 死代码）、initialize_auth_user_wallet（:652）、用户审计/公告读取（runtime.rs:369-535、:439）、list_wallet_payment_orders_by_user_id（runtime.rs:866-909）。
- 两类共用（删除会牵连 admin）：find_user_auth_by_id/identifier（:73-91）、create_user_session/touch/revoke（:431-933）、count_active_admin_users（:776 bootstrap）、create/delete_local_auth_user*（:496-576 admin 建号）、key 读写（:1436-1564）、get_management_token_with_user*（:1045-1069 trusted-header）。

data/state/runtime.rs：迁移/维护（:163-236）、统计与钱包（:339-970+）；list_admin_wallets 与 list_wallet_payment_orders_by_user_id 混在同类。

state/runtime/auth/：user_lifecycle.rs（角色→默认用户组 :60-88、注册死代码 :794）、sessions.rs、api_keys.rs、user_provisioning.rs（LDAP/OAuth 开户+钱包初始化 :115/:245/:301）、../api_key_exports.rs、../user_preferences.rs。

缓存层 cache/auth_runtime.rs、cache/auth_context.rs：缓存 StoredAuthApiKeySnapshot（含 api_key_is_standalone、user_role），admin 限流旁路与用户限流共用同一快照。

## 9. 连带影响与风险

1. **users 表不能删**：api_keys.user_id NOT NULL + JOIN users（sqlite/src/auth.rs:41）+ resolution.rs:370 active 检查 + standalone key user_id=admin 自己（mutation_routes.rs:155）。删 users 会让所有 key 鉴权失效。
2. **wallets 双外键**（user_id 用户钱包 / api_key_id standalone 钱包，baseline :572-590）：删用户钱包路径要保住 api_key 钱包；结算聚合（runtime.rs:339-347）双钱包共用。
3. **admin_bypass_limits 语义**（resolution.rs:1197-1200）：admin 普通key绕过限流。全 standalone 后该旁路不触发；建议保留逻辑不动。
4. **共用鉴权底座**：resolve_authenticated_local_user 与 resolve_local_admin_principal 共用 JWT 解码/find_user_auth_by_id/user_sessions/session touch；删 /api/users/me 不影响 admin 登录，但删 user_sessions/session 相关会打断 admin 会话校验（resolution.rs:384-397）。
5. **role 进 token 且多处读**：JWT claim、resolution.rs:343、credentials.rs:208、StoredAuthApiKeySnapshot.user_role、batch/update.rs。固化单 admin 时同步简化或保留常量 "admin"。
6. **admin users_manage 整组删除需先搬工具函数**：handlers/admin/auth/api_keys/mod.rs:1-6 复用 users 模块 generate_admin_user_api_key_plaintext/hash_admin_user_api_key/masked_user_api_key_display。
7. **默认用户组逻辑残留**：user_lifecycle.rs:60-88、groups.rs:285、create.rs:130；configured_default_user_group_id 读系统配置。
8. **trusted-header 管理通道**：management_tokens（credentials.rs:181-230，tunnel 节点冒充 admin）与用户侧 /api/me/management-tokens（501 死路由）不是同一消费者——删用户侧路由别误删底层 token 校验。
9. **纯用户侧可安全删除**：/api/users/me/* 除保留子集（user_me_*.rs 相关）、monitoring_user、ccswitch 分类+签名、用户侧 management-tokens 分类、install-sessions 两个 501 端点、/api/capabilities/user-configurable、oauth family、register_local_auth_user、user_invite_codes/user_referrals 相关。
10. **公开目录端点受众混淆**：/api/public/*、/v1/models 名义用户侧但 admin 控制台与 CLI 也消费；sanitize_public_model_config_for_user 可放宽，端点保留。
11. **统计与审计表**：stats_hourly_user/stats_user_daily/usage.user_id/audit_logs.user_id 仍写 admin 的 user_id；删用户维度查询（observability/monitoring/user_behavior*）时注意 /_gateway/audit/*（api/ops.rs:12-31）仍按 user_id+api_key_id 组织。
12. **501/503 语义**：删路由优先改分类器（public_support.rs/operations_families.rs），否则「分类仍在、handler 缺失」501 抖动——management-tokens/ccswitch/install-sessions 即此状态。
