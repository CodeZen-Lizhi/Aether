# 单用户化：删除用户侧与角色体系

## Goal

将 Aether 收敛为真正的单用户（单 admin 账号）形态：删除前端 `/dashboard` 用户侧区域、`/api/users/me` 用户功能端点组、多角色体系与用户管理后端；保留单一 admin 登录 + 管理台 + AI 代理核心链路。删除清单与分阶段实施规划已产出，**关键决策已于 2026-09-01 确认**（见「已确认决策」），可按阶段进入实施。

## Background

- 前三期 slim 清理（商业化功能：钱包/资金/推荐/池调度/用户管理 UI）已完成，但权限体系与用户侧区域原封未动。
- 现状：前后端仍存在完整的三级角色体系（`user` / `admin` / `audit_admin`），前端按角色渲染两套导航与两套路由区（`/dashboard` 用户侧、`/admin` 管理侧），后端保留全部用户侧端点与用户管理端点。
- 主人实际只用一个 admin 账号，用户侧代码是永远走不到的死区域，且持续制造串行/残留 bug（如独立密钥页钱包列串行、`openAddBalanceDialog` 死引用）。

## Scope（做什么）

1. **前端**：删除 `/dashboard` 路由组与 `views/user/` 全部页面；共享视图（Dashboard/Usage）删除用户分支；导航/守卫/登录重定向收敛到 admin 侧；「个人设置」中有用功能（修改密码、登录设备、偏好）迁移到 `/admin/settings`。
2. **后端**：删除用户侧端点（我的 keys/用量/目录/模型能力等）；删除 501 死路由分类器（ccswitch、用户侧 management-tokens、install/\*、oauth family、两处 install-sessions）；删除 `users_manage` 用户管理端点组与用户注册死代码；保留最小 `users/me` 子集（auth-me、改密、会话、偏好）供 admin 自助使用。
3. **角色体系**：删除 `audit_admin` 区分与角色归一化等用户侧逻辑；`roles.rs` 收敛为单 admin 语义。
4. **数据库**：`users` 表**保留**（仅一行 admin，鉴权 SQL `JOIN users` 硬依赖）；编写清理迁移**删除全部纯用户侧旧表**（user_groups、user_group_members、user_invite_codes、user_referrals、oauth_providers、user_oauth_links、ldap_configs），无保留。

## Non-goals（不做什么）

- 不删除登录认证本身（保留用户名/密码登录、JWT、refresh token、session 机制）。
- 不动 AI 代理核心链路：`api_keys` 鉴权快照（`JOIN users`）、wallet 结算、`/v1/*` 代理、`/api/public/*`、internal/tunnel。
- 不动 `management_tokens` 底层校验（internal/tunnel trusted-header 通道在用，仅删用户侧 501 死路由分类）。
- 不做 UI 改版/重设计，只做删除与必要迁移。
- 数据库除 drop 纯用户侧旧表外不做其他 schema 变更（users/api_keys/wallets/user_sessions/user_preferences/management_tokens 等核心表结构不动）。

## 关键约束（盘点确认的硬依赖）

| 约束 | 证据 |
|---|---|
| `users` 表不可物理删除，只能保留一行 admin | `api_keys.user_id NOT NULL`；鉴权快照 SQL `FROM api_keys JOIN users`（`crates/aether-data/adapters/sqlite/src/auth.rs:41`）；`resolution.rs:370` 校验 user 行 active |
| standalone key 的 `user_id` = 当前 admin 的 user_id | `handlers/admin/auth/api_keys/mutation_routes.rs:155` |
| admin 侧运行时依赖 `meApi.getProfile` 一处 | `views/shared/Usage.vue:253`（单用户筛选器）→ 需改用 `authStore.user` |
| JWT claim 携带 role，多处读取 | `auth_session.rs:540-549`、`resolution.rs:343`、`credentials.rs:208` |
| `handlers/admin/auth/api_keys` 复用 users 模块工具函数 | 删 `handlers/admin/users/` 前须先搬移 `generate_admin_user_api_key_plaintext` 等 |
| 用户侧 CLI 安装会话与 ccswitch 后端均为 501 未实现 | `user_me_routes.rs` match 无 arm；`handlers/admin/auth/api_keys/routes.rs:35-85` 落 `_` → 503 |

## 分阶段实施计划

每阶段独立 commit、独立验证，可单独回滚。

### 阶段 1 — 前端用户侧下线（低风险，纯前端）
1. 「个人设置」迁移为 `/admin/settings`（修改密码、登录设备、偏好设置整页迁移；`Settings.vue` 改名 `AdminProfileSettings.vue` 或保留原名）。
2. 解除 admin 侧对 `me.ts` 的运行时依赖（Usage.vue getProfile → authStore；useUsageData/Usage 用户分支删除；Dashboard.vue 用户模板与脚本分支删除）。
3. 删除 `/dashboard` 路由组、`views/user/` 目录、导航用户分支；守卫与登录重定向统一到 `/admin/dashboard`。
4. 清理孤儿 i18n 与对应测试。

### 阶段 2 — 后端用户端点收缩（中风险）
1. 保留 `users/me` 最小子集：`GET /api/users/me`（auth-me 复用）、`PATCH .../password`、sessions 系列、preferences 系列、`PUT /api/users/me`（用户名，待确认）。
2. 删除用户侧 keys/usage/catalog/client-config/model-capabilities/monitoring handler 及路由分类 arm。
3. 删除 501 死路由分类器：ccswitch、用户侧 management-tokens、`install/*`、`install-tunnel/*`、`/i/*`、oauth family、两处 install-sessions 分类 arm 与残留辅助函数。
4. 删除 `register_local_auth_user` 死代码。

### 阶段 3 — 用户管理后端与角色体系瘦身（中高风险，独立可延后）
1. 删除 `users_manage` 全组端点：`handlers/admin/users/` 目录 + `operations_families.rs` 分类（先搬移被 `auth/api_keys` 复用的工具函数）。
2. 删除 `audit_admin` 区分：前端 `isAuditAdmin`、后端 `is_audit_admin_role`/`can_access_admin_console` 收敛为单 admin 判断。
3. 删除 LDAP/OAuth 数据层入口（`get_or_create_ldap_auth_user`、`create_oauth_auth_user`）。

### 阶段 4 — 数据库清理与收尾（已确认执行）

1. **前置：备份本地数据库**（`data/` 目录，gitignored）——drop 迁移不可逆。
2. 新增 drop 迁移：`user_groups`、`user_group_members`、`user_invite_codes`、`user_referrals`、`oauth_providers`、`user_oauth_links`、`ldap_configs`。
3. `/api/dashboard/stats` 后端用户字段分支删除（前端已不走）。
4. usage 组件 `isAdmin` prop 内化为常量；用户维度监控查询（user_behavior* 等）清除。

## Acceptance Criteria

- [ ] 登录后直达 `/admin/dashboard`，全程无 `/dashboard` 路由可达（直输 URL 落到 admin 或 404→重定向）。
- [ ] `/admin/settings` 可完成：修改密码、管理登录设备、设置主题/语言/时区。
- [ ] 管理台核心页面回归通过：独立密钥、供应商、模型、路由、用量、系统设置。
- [ ] 独立密钥创建的 key 可正常鉴权调用 AI 代理（`/v1/*`），余额/限速正常。
- [ ] `vue-tsc --noEmit`、`vitest run`、`cargo check`、`cargo test`（aether-gateway + aether-data）全绿。
- [ ] 全仓 grep 无 `views/user`、`/dashboard`（用户侧语义）、`meApi`（除保留子集）、`users_me` 死引用残留。
- [ ] users 表仅一行 admin；既有 db 直接升级可用，无需手动数据修复。
- [ ] 清理迁移执行后：纯用户侧 7 张旧表已 drop；全新库与既有库均能正常启动、登录、代理调用。

## 已确认决策（2026-09-01）

| # | 问题 | 决策 |
|---|---|---|
| 1 | 个人设置迁移范围 | **整页迁移**到 `/admin/settings`（基本信息/修改密码/登录设备/偏好全保留） |
| 2 | CLI 一键安装 / CC Switch 导入 | **删干净**（后端本就 501 未实现），前端对话框、API 方法、后端分类 arm 全清 |
| 3 | `/api/admin/users*` 用户管理后端组 | **本次删除**（阶段 3，先搬移被 standalone key 模块复用的工具函数） |
| 4 | 数据库旧表 | **清理，无保留**：阶段 4 写 drop 迁移删 7 张纯用户侧表，执行前备份 |
| 5 | `PUT /api/users/me`（改用户名） | 保留（默认推荐；Settings「基本信息」区块随整页迁移保留） |

## 交付物

- `deletion-checklist.md` — 逐文件删除/修改/迁移清单（本任务核心）。
- `design.md` — 技术设计：目标架构、路由与鉴权改造、数据模型边界、语义变更点。
- `research/frontend-inventory.md`、`research/backend-inventory.md` — 原始盘点（证据与行号）。
