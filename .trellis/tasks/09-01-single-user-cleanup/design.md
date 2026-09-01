# 技术设计：单用户化改造

对应 `prd.md`。盘点证据见 `research/frontend-inventory.md`、`research/backend-inventory.md`。

## 1. 目标架构

```
改造前                                改造后
─────────────────────────────        ─────────────────────────────
/public  → Login（角色无关）          /public → Login（不变）
/dashboard（user/audit 用户区）        （删除）
  ├ Dashboard.vue（双模式）            ├ /admin/dashboard → 同组件（仅 admin 分支）
  ├ MyApiKeys.vue                      │
  ├ Usage.vue（双模式）                ├ /admin/usage → 同组件（仅 admin 分支）
  ├ ModelCatalog.vue                   │
  └ Settings.vue（个人设置）           ├ /admin/settings ← 迁移自 user/Settings.vue
/admin（管理区）                       ├ 其余 /admin 页面不变
  ├ dashboard/keys/providers/...       │
  └ usage/system                       │
后端                                   后端
  /api/users/me/*（用户全家桶）        /api/users/me/{,password,sessions*,preferences*}
  /api/admin/*（含 users_manage）      /api/admin/*（users_manage 删除）
  roles: user/admin/audit_admin        单一 admin（role 仍写 "admin" 进 JWT）
```

## 2. 前端设计

### 2.1 路由与守卫

- `routes/dashboard.ts` 整文件删除；`routes/index.ts` 去掉该 import。
- `guards/adminGuard.ts`：`canAccessAdmin` 恒真后此守卫仅剩防御意义。**保留守卫**但 fallback 从 `'/dashboard'` 改为 `'/admin/dashboard'`（防脏数据/旧 token 中 role 缺失时白屏）。
- `guards/homeGuard.ts`：登录后目标恒 `/admin/dashboard`；清理 `'/guide'` 与 `from.path.startsWith('/dashboard')` 残留；`sessionStorage['redirectPath']` 深链恢复流程不变。
- `router/index.ts` 的 `requiresAuth !== false` 判定逻辑不变（`/admin` 全部需登录）。
- `App.vue:120-122` 外部 token 变更时的非 admin 重定向目标同步改 `/admin/dashboard`。

### 2.2 「个人设置」整页迁移（唯一的功能迁移，已确认）

- `views/user/Settings.vue` → `views/admin/ProfileSettings.vue`，挂 `/admin/settings`（name: `ProfileSettings`）。
- 保留功能块：基本信息（用户名，`meApi.updateProfile`）、修改密码（`meApi.changePassword`）、登录设备（sessions 系列）、偏好（`getPreferences/updatePreferences`，联动 `useDarkMode`）。
- 依赖随迁：`types/session.ts`（formatSessionMeta）、`utils/passwordPolicy.ts`、i18n `breadcrumb` 语义调整。
- `MainLayout.vue` 侧边栏底部与移动菜单的设置入口改链 `/admin/settings`；`currentRoleLabel` 删用户分支。

### 2.3 共享视图去双模式

- **Dashboard.vue**：删除 `isAdmin` computed 分支消费——模板 L240-340（本月统计）、L360-433（用户折线/柱状图）、脚本 L912/1126/1157/1261-1273（userMonthlyCost 加载链）；`dashboardModeLabel` 简化为固定 ADMIN。API 不变（`/api/dashboard/stats` 后端仍按 role 返回，admin 分支即目标形态）。
- **Usage.vue**：`isAdminPage` 由 `route.path.startsWith('/admin')` 改为常量 `true`；删除热力图/活跃请求/统计的 `meApi` 分支（L240/444）；`loadAdminUsers` 的 `meApi.getProfile()` 改用 `authStore.user`（解除对 me.ts 的最后一处 admin 运行时依赖）；删除用户专属本地筛选/分页分支（L349-434、L809-833）与 2 列布局。
- **useUsageData.ts**：删除 L136/262/400/434 的用户分支，恒走 `usageApi`。
- usage 四个表格组件的 `isAdmin` prop：阶段 1 保留恒 true 传参，阶段 4 再内化（避免一次改太深）。

### 2.4 auth store 收敛

- 删除 `isAuditAdmin`、`canOperateAdmin`（零引用）；`isAdmin`/`canAccessAdmin` 保留（守卫/布局在用，role 仍来自 `GET /api/users/me`）。
- `User` 类型 `role` 字段保留（后端仍返回 "admin"），不强行改常量判断，降低后端联动面。

### 2.5 删除后不可再引用的资产（grep 验收基线）

`views/user/`、`@/api/me`（除 Settings 迁移件与保留子集）、`api/public-models.ts`、`features/api-keys/utils/ccswitchImport.ts`、`features/api-keys/utils/userKeyPayload.ts`、`/dashboard` 字符串（用户侧语义）、`meApi.getUsage|getApiKeys|getAvailableModels|...`。

## 3. 后端设计

### 3.1 路由分类器（`control/route/`）改造原则

后端是「先分类、后分发」的 axum frontdoor。删除端点时**必须同步删分类器 arm**，否则出现「分类命中、无 handler」的 501 抖动（现状 management-tokens/ccswitch/install-sessions 就是这种半成品状态）；未分类路径会落入 AI 兜底 proxy 或 404，也不可接受。

### 3.2 `users/me` 最小子集（保留）

| 端点 | 保留理由 |
|---|---|
| `GET /api/users/me` | `handle_auth_me`（auth_session.rs:317）同时服务 `/api/auth/me`，前端 `authApi.getCurrentUser` 依赖 |
| `PATCH /api/users/me/password` | admin 自改密码（Settings 迁移件调用） |
| `GET/DELETE/PATCH /api/users/me/sessions*` | 登录设备管理 |
| `GET/PUT /api/users/me/preferences` | 主题/语言/时区偏好 |
| `PUT /api/users/me` | 用户名修改（待确认，默认保留） |

删除：`/api/users/me/api-keys*`（含 providers/capabilities 子路径、install-sessions）、`/usage*`、`/providers`、`/available-models`、`/client-config`、`/model-capabilities`、`/api/monitoring/my-*`、`/api/ccswitch/usage`。

### 3.3 死路由分类器删除清单（501 半成品）

- ccswitch：`public_support.rs:178-190` 分类 + `resolution.rs:1213-1223` 鉴权签名 `aether:ccswitch_usage`。
- 用户侧 management-tokens：`public_support.rs:230-318, 399-416`。**注意**：`management_tokens` 表与 `get_management_token_with_user*`（`data/state/auth.rs:1032-1112`）被 internal/tunnel trusted-header 通道使用，**底层保留**。
- `install/*`、`install-tunnel/*`、`/i/*`：`public_support.rs:472-483`。
- oauth family：`control/route/oauth.rs` + `support.rs` 分发器 oauth arm（本就 501）。
- install-sessions 两处分类 arm：`public_support.rs:275-288`、`control/route/admin/observability_families.rs:77-84` + `handlers/admin/auth/api_keys/shared.rs:105` 残留辅助。

### 3.4 `users_manage` 删除（阶段 3）

- 删 `handlers/admin/users/` 目录 + `operations_families.rs` 中 users/user-groups/billing 分类。
- **先搬移**被 `handlers/admin/auth/api_keys/mod.rs` 复用的工具函数（`generate_admin_user_api_key_plaintext` / `hash_admin_user_api_key` / `masked_user_api_key_display` 等）到 `auth/api_keys` 侧共享模块。
- 删 `register_local_auth_user`（`user_lifecycle.rs:794`、`data/state/auth.rs:826`，生产零调用）。
- `is_admin_users_route`（`users/routes.rs:20`）与 `resolution.rs` 中 users_manage 相关判定同步移除。

### 3.5 角色体系收敛（阶段 3）

- `roles.rs`：删除 `ROLE_AUDIT_ADMIN`、`is_audit_admin_role`、`normalize_assignable_user_role`（users_manage 删除后无调用方）；保留 `can_access_admin_console` 收敛为 `is_full_admin_role` 别名（`resolution.rs:343/370`、`credentials.rs:208` 仍调用）。
- JWT/DB 中 role 字段**保留写入 "admin"**，不做 schema 变更——`StoredAuthApiKeySnapshot.user_role`、限流旁路判定（`resolution.rs:1197-1200` `admin_bypass_limits`）继续工作。
- `can_write_admin_console` 生产无调用，随删。

### 3.6 明确保留（易误删清单）

`find_user_auth_by_id`、user_sessions 全套（admin 会话校验 `resolution.rs:384-397` 共用）、`count_active_admin_users`（bootstrap）、bootstrap_admin_from_env、`/api/auth/settings`、`/api/public/*`、`/v1/models`、wallet 双挂结算、`/api/internal/gateway/*`、trusted-header 管理通道底层。

## 4. 数据模型边界

- **不动 schema**：`users`（留一行 admin）、`api_keys`（`JOIN users` 鉴权）、`wallets`（user_id/api_key_id 双挂）、`user_sessions`、`user_preferences`、`management_tokens`、audit/usage/stats。
- **清理迁移（已确认执行，阶段 4）**：drop `user_groups`、`user_group_members`、`user_invite_codes`、`user_referrals`、`oauth_providers`、`user_oauth_links`、`ldap_configs` 共 7 张纯用户侧表（含数据，破坏性；执行前备份 `data/` 下 db 文件）。`auth_modules` **保留**（`/api/auth/settings` 本地登录开关仍在用）。
- 既有库升级路径：迁移前无需数据修复——admin 行已存在（bootstrap 幂等），普通 key 与钱包数据不受影响仍可鉴权读取；仅 7 张旧表内的推荐码/推荐关系/OAuth 绑定等数据随之清除。

## 5. 语义变更点（需要知晓的行为差异）

1. **普通（非 standalone）key 从此无处创建**：用户侧与 admin-给用户建 key 的两个入口都删。既有普通 key 仍可鉴权（快照 JOIN 不变）；admin 自己的普通 key 享受 `admin_bypass_limits` 旁路——语义不变，建议日常全部用独立密钥页的 standalone key。
2. **用户级限速/允许列表设置页消失**：`/model-capabilities`、`/preferences` 中仅 key 维度策略随用户侧删除；admin 侧 key 级设置（StandaloneKeyFormDialog）不受影响。
3. **模型目录用户视图消失**：需要看可用模型走 `/admin/models`（管理视图）或 `/v1/models`。
4. **`/api/dashboard/stats` 请求方只剩 admin**：后端用户字段分支代码阶段 4 可删，行为无差异。

## 6. 验证与回滚策略

- 每阶段一个 commit（阶段 3 可再拆 3a/3b）；验证矩阵：`vue-tsc --noEmit` + `vitest run`（frontend）、`cargo check` + `cargo test -p aether-gateway -p aether-data`、手动回归（登录→改密→建 standalone key→代理调用→用量页→系统设置）。
- 回滚 = revert 对应 commit；阶段 1-3 数据库零迁移，前端回滚无数据影响；阶段 4 的 drop 迁移不可逆，回滚 = 还原执行前备份的 db 文件。
- 风险最高的两处：`handlers/admin/users/` 工具函数搬移（阶段 3）、`control/route` 分类器删 arm 后的路径落兜底——每删一处用 `curl` 打对应路径确认 404/401 而非 501/503。

## 7. 阶段依赖关系

```
阶段1（前端下线）─── 无后端依赖，可先行
阶段2（后端收缩）─── 依赖阶段1完成（Settings 迁移件决定保留哪些 me 端点）
阶段3（users_manage/角色）─── 依赖阶段2（避免同文件反复冲突）
阶段4（DB 清理/收尾）──── 依赖阶段3（已确认执行）
```
