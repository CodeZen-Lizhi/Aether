# 逐文件删除清单

图例：`[删]` 整文件/目录删除 · `[改]` 修改保留 · `[迁]` 迁移后原文件删除。行号为盘点时快照。

---

## 阶段 1：前端用户侧下线

### 1A. 个人设置迁移（/admin/settings）

| 动作 | 文件 | 说明 |
|---|---|---|
| `[迁]` | `frontend/src/views/user/Settings.vue` | → `views/admin/ProfileSettings.vue`；保留：基本信息(用户名)、修改密码、登录设备、偏好；删除面向「个人账户」的文案 |
| `[改]` | `frontend/src/router/routes/admin.ts` | 新增 `settings` 子路由 `/admin/settings`（name: ProfileSettings） |
| `[改]` | `frontend/src/layouts/MainLayout.vue` | 设置入口改链：L155-162（桌面）、L293-300（移动）`/dashboard/settings` → `/admin/settings`；L456-461 `isNavActive` 删 `/dashboard` 特判；L469-478 `currentRoleLabel` 删用户分支 |
| `[改]` | `frontend/src/layouts/main-layout/navigation.ts` | L29-46 用户菜单分支整段删；L83-88 `/dashboard/settings` 面包屑特判删；`canAccessAdmin` 入参保留（恒真，结构不动） |
| `[保留]` | `frontend/src/types/session.ts` | Settings 登录设备依赖（formatSessionMeta），随迁保留 |
| `[保留]` | `frontend/src/utils/passwordPolicy.ts` + `__tests__` | 改密表单校验，随迁保留 |

### 1B. 解除 admin 对 me.ts 的依赖（先于 1C，防运行时报错）

| 动作 | 文件 | 说明 |
|---|---|---|
| `[改]` | `frontend/src/views/shared/Usage.vue` | L253 `loadAdminUsers` 的 `meApi.getProfile()` → `authStore.user`；L181 `isAdminPage` → 常量 `true`；删 L240/444 meApi 热力图/活跃请求分支；删 L349-434 用户本地筛选、L809-833 用户分页；模板 L33 三元、L57-69 用户 2 列布局、L77-78 isAdmin 传参简化 |
| `[改]` | `frontend/src/features/usage/composables/useUsageData.ts` | 删 L136 用户分支（meApi.getUsage）、L262/L434 meApi 调用；import meApi 移除 |
| `[改]` | `frontend/src/views/shared/Dashboard.vue` | 删模板 L240-340（用户本月统计）、L360-433（用户趋势/成本图）；删脚本 L810-813 isAdmin/modeLabel 简化、L912/1126/1157/1261-1273 用户数据链与 `userMonthlyCost`；保留 admin 分支为唯一路径 |
| `[改]` | `frontend/src/features/usage/components/Usage*Table.vue`（4 个） | 阶段1 仅保持 isAdmin 恒 true 传参不动；阶段 4 内化 |

### 1C. 删除用户侧路由与视图

| 动作 | 文件 | 说明 |
|---|---|---|
| `[删]` | `frontend/src/router/routes/dashboard.ts` | 整文件（/dashboard 5 子路由） |
| `[改]` | `frontend/src/router/routes/index.ts` | 去 dashboardRoutes import 与展开 |
| `[删]` | `frontend/src/views/user/MyApiKeys.vue`（1651 行） | 个人密钥管理 + CC Switch 导入 + CLI 安装对话框（后端 501，功能死） |
| `[删]` | `frontend/src/views/user/ModelCatalog.vue`（343 行） | 用户模型目录（admin 有 /admin/models） |
| `[删]` | `frontend/src/views/user/components/UserModelDetailDrawer.vue` | 仅 ModelCatalog 使用 |
| `[删]` | `frontend/src/views/user/model-catalog-helpers.ts` | 仅 ModelCatalog 使用 |
| `[删]` | `frontend/src/views/user/__tests__/`（3 个 spec） | MyApiKeys.ccswitch / ModelCatalog / model-catalog-helpers |
| `[删]` | `frontend/src/api/public-models.ts` | 仅 ModelCatalog/Drawer/helpers 消费 |
| `[删]` | `frontend/src/features/api-keys/utils/ccswitchImport.ts` + spec | 仅 MyApiKeys 消费 |
| `[删]` | `frontend/src/features/api-keys/utils/userKeyPayload.ts` + spec | 本就是零引用死代码 |
| `[改]` | `frontend/src/api/me.ts` | 大瘦身：删 getApiKeys/createApiKey/getApiKeyDetail/getFullApiKey/getClientConfig/deleteApiKey/toggleApiKey/updateApiKey/createApiKeyInstallSession/getUsage/getActiveRequests/getAvailableProviders/getAvailableModels/getEndpointStatus/updateApiKeyProviders/updateApiKeyCapabilities/getModelCapabilitySettings/updateModelCapabilitySettings/getIntervalTimeline/getActivityHeatmap 及孤儿类型；保留 Profile/UserPreferences/ChangePasswordRequest/UserSession 与 profile/password/sessions/preferences 方法 |
| `[删]` | `frontend/src/api/__tests__/me.spec.ts` | 被测方法已删（或改写为仅覆盖保留子集） |

### 1D. 守卫/重定向/登录

| 动作 | 文件 | 说明 |
|---|---|---|
| `[改]` | `frontend/src/router/guards/adminGuard.ts` | L13 fallback `'/dashboard'` → `'/admin/dashboard'` |
| `[改]` | `frontend/src/router/guards/homeGuard.ts` | L35 恒 `/admin/dashboard`；L23 删 `'/guide'` 与 `/dashboard` 前缀残留 |
| `[改]` | `frontend/src/views/public/Login.vue` | L98 登录成功重定向改 `/admin/dashboard` |
| `[改]` | `frontend/src/App.vue` | L120-122 非 admin 访问 /admin 的 replace 目标改 `/admin/dashboard` |
| `[改]` | `frontend/src/stores/auth.ts` | 删 `isAuditAdmin`（L53）、`canOperateAdmin`（L55，零引用）；保留 isAdmin/canAccessAdmin |

### 1E. i18n 与测试清理（阶段 1 收尾）

| 动作 | 位置 | 内容 |
|---|---|---|
| `[删]` | `frontend/src/i18n/messages.ts`（zh+en 双份） | `nav.group.resources/account`、`nav.modelCatalog`、`nav.apiKeys`、`nav.usageStats`、`auth.role.user`、`breadcrumb.personalSettings` |
| `[删]` | 同上（孤儿顺手清） | `guide.*`（88×2，/guide 路由已不存在）、`nav.walletCenter/billingCenter/myReferral/walletManagement/billingManagement`、`auth.login.registerNow/noAccount/contactAdmin/oauth*/demoMode/adminLocal/ldap` |
| `[改]` | `frontend/src/layouts/main-layout/__tests__/navigation.spec.ts` | 删 'builds user navigation' 用例（L24/37/58） |
| `[改]` | `frontend/src/router/guards/__tests__/homeGuard.spec.ts` | 'routes by role' 用例改写（L49） |
| `[改]` | `frontend/src/views/shared/__tests__/Dashboard.spec.ts` | 删 'ordinary user wallet card'（L143-144） |
| `[删]` | `frontend/src/views/shared/__tests__/Usage.record-filters.spec.ts` | normal-user 筛选用例（L15-38）；文件若仅此内容则整删 |
| `[改]` | `frontend/src/features/usage/composables/__tests__/useUsageData.spec.ts` | 删 vi.mock('@/api/me') 与用户分支用例 |
| `[改]` | `frontend/src/i18n/__tests__/i18n.spec.ts` | 若断言 key 数量则同步 |

**阶段 1 验证**：`vue-tsc --noEmit` + `vitest run` 全绿；手动：登录直达 /admin/dashboard；/dashboard 直输不可达；/admin/settings 四个功能块可用；独立密钥页正常。

---

## 阶段 2：后端用户端点收缩

均位于 `apps/aether-gateway/src/`。

### 2A. 删除用户侧 handler 与分类 arm

| 动作 | 文件 | 说明 |
|---|---|---|
| `[删]` | `handlers/public/support/user_me_api_keys.rs` | 用户 keys CRUD/providers/capabilities（L300-1020+） |
| `[删]` | `handlers/public/support/user_me_usage.rs` | /usage、/usage/active、/interval-timeline、/heatmap（L978-1498+） |
| `[删]` | `handlers/public/support/user_me_catalog.rs` | /providers、/available-models（L290/137） |
| `[删]` | `handlers/public/support/monitoring.rs`（my-audit-logs、rate-limit-status） | family monitoring_user |
| `[改]` | `handlers/public/support/user_me_routes.rs` | match 删上述 arms，保留 auth-me/password/profile/sessions/preferences 分支 |
| `[改]` | `handlers/public/support/user_me_profile.rs` | 删 `client-config`（L43），保留 profile/password（L67/L196） |
| `[改]` | `handlers/public/support/user_me_preferences.rs` | 删 model-capabilities（L147/365），保留 preferences（L173/202） |
| `[改]` | `control/route/public_support.rs` | 删分类 arm：users_me 的 keys/usage/catalog/client-config/model-capabilities/monitoring_user/ccswitch（L178-190）/用户侧 management-tokens（L230-318, 399-416）/install-sessions（L275-288）/install,install-tunnel,i（L472-483） |
| `[删]` | `handlers/public/support/support/dashboard.rs`（user 侧）与 `support/monitoring.rs` family 注册 | /api/dashboard/stats、/api/monitoring 用户分支——**保留端点本身**（admin Dashboard 在用），删用户字段分支与 monitoring_user family |
| `[改]` | `control/auth/resolution.rs` | 删 `aether:ccswitch_usage` 签名（L1213-1223）；users_me 已删路径的鉴权分支收敛 |

### 2B. 删除 OAuth/LDAP 与注册死代码

| 动作 | 文件 | 说明 |
|---|---|---|
| `[删]` | `control/route/oauth.rs` | oauth family 分类（本就 501） |
| `[改]` | `handlers/public/support/support.rs` | 删 oauth 分发 arm |
| `[删]` | `register_local_auth_user` | `state/runtime/auth/user_lifecycle.rs:794`、`data/state/auth.rs:826`（生产零调用） |
| `[改]` | `handlers/public/support/support.rs` auth settings payload | 若删 OAuth/LDAP，`/api/auth/settings` 的 oauth/ldap 开关字段收敛（与 Login.vue 孤儿 key 清理联动） |

**阶段 2 验证**：`cargo check` + `cargo test -p aether-gateway`；curl 回归：`/api/users/me` 200、`/api/users/me/api-keys` 404（非 501）、`/api/ccswitch/usage` 404、登录/refresh/登出正常、`/v1/models` 正常。

---

## 阶段 3：用户管理后端与角色体系瘦身

| 动作 | 文件 | 说明 |
|---|---|---|
| `[改]` | `handlers/admin/auth/api_keys/mod.rs` 等 | 先把复用的 `generate_admin_user_api_key_plaintext` / `hash_admin_user_api_key` / `masked_user_api_key_display` 搬到 auth/api_keys 共享模块 |
| `[删]` | `handlers/admin/users/`（整目录） | lifecycle/batch/groups/sessions/api_keys/payment 等全部 users_manage handler |
| `[改]` | `control/route/admin/operations_families.rs` | 删 users/user-groups/users-keys/billing 分类 arm |
| `[改]` | `handlers/admin/routes.rs` | 删 users 模块挂载与 `is_admin_users_route`（users/routes.rs:20） |
| `[删]` | `roles.rs` 瘦身 | 删 ROLE_AUDIT_ADMIN/is_audit_admin_role/normalize_assignable_user_role/can_write_admin_console；can_access_admin_console 收敛 |
| `[改]` | `control/auth/resolution.rs` / `credentials.rs` | L343/370、L181-230 角色判断随 roles.rs 收敛（trusted-header `x-aether-admin-user-role` 校验保留，只认 admin） |
| `[删]` | LDAP/OAuth 数据层入口 | `get_or_create_ldap_auth_user`（user_lifecycle.rs）、`create_oauth_auth_user`（data/state/auth.rs:273）+ `user_provisioning.rs` 相关 |
| `[改]` | 前端 | 若有 audit_admin 残留字符串/类型一并清（阶段 1 已删大部分） |

**阶段 3 验证**：`cargo test` 全绿；curl：`/api/admin/users` 404、standalone key 创建/编辑/删除正常（工具函数搬移无损）、登录与 admin principal 解析正常、internal/tunnel 心跳正常。

---

## 阶段 4：数据库清理与收尾（已确认执行，无保留）

**前置：备份本地数据库**——`data/` 目录（gitignored）下的 sqlite db 文件复制到库外保存。drop 迁移不可逆，回滚只能靠还原备份。

| 动作 | 文件/内容 | 说明 |
|---|---|---|
| `[备份]` | `data/*.db` | 复制到仓库外目录，记录备份时间 |
| `[删]` | 新增迁移文件（`crates/aether-data/adapters/sqlite/migrations/`；若 postgres adapter 有同构 migrations 目录则同步新增） | drop `user_groups`、`user_group_members`、`user_invite_codes`、`user_referrals`、`oauth_providers`、`user_oauth_links`、`ldap_configs` |
| `[保留]` | `auth_modules` | `/api/auth/settings` 本地登录开关仍在用 |
| `[保留]` | `users`/`api_keys`/`wallets`/`user_sessions`/`user_preferences`/`management_tokens`/audit/usage/stats | 核心表 schema 不动 |
| `[改]` | `handlers/public/support`（dashboard payload） | `/api/dashboard/stats` 用户字段分支删除（前端已不走） |
| `[改]` | `features/usage/components/Usage*Table.vue`（4 个） | isAdmin prop 内化为常量；`UsageRecordsTable.vue:1263-1312` 用户列逻辑（DEFAULT_USER_COLUMNS、userVisibleColumnIds）删除 |
| `[删]` | `handlers/admin/observability/monitoring/user_behavior*` 等用户维度查询 | 阶段 3 已删 users_manage，此处清剩余用户维度统计查询 |

**阶段 4 验证**：全新库与「备份还原库」均能启动、登录、代理调用；`sqlite3 <db> '.schema'` 确认 7 张表已不存在；`cargo test -p aether-data`（含迁移测试）全绿。

---

## 附：保留但需知晓的既有 501/残留（本任务外）

- `/api/me/management-tokens` 底层表与方法保留（tunnel 用），仅删用户侧分类。
- `guide.*` 之外如发现新孤儿 i18n，顺手清并在 commit message 注明。
