# 前端用户侧盘点报告（2026-09-01）

探查代理产出，作为 deletion-checklist.md 的证据留存。行号为盘点时快照。

## 1. 路由

**`frontend/src/router/routes/public.ts`**（11 行）
- 仅 `/` → `views/public/Login.vue`，`meta: { requiresAuth: false }`（L5-10）。
- 无注册/找回密码页面；`api/auth.ts` 的 `authApi` 只有 login/logout/getCurrentUser/refreshToken/getAuthSettings（L73-107）。`auth.login.registerNow` 等 i18n key 是孤儿。

**`frontend/src/router/routes/dashboard.ts`**（整文件可删）
- `/dashboard`，MainLayout，`requiresAuth: true`，5 子路由：`''`→shared/Dashboard（L10-14）、`api-keys`→user/MyApiKeys（L15-19）、`usage`→shared/Usage（L20-24）、`settings`→user/Settings（L25-29）、`models`→user/ModelCatalog（L30-34）。

**`frontend/src/router/routes/admin.ts`**（保留）
- `/admin`，`requiresAuth + requiresAdmin`，子路由：dashboard、keys、providers、models、routing(+new)、usage、system。

**`frontend/src/router/index.ts`** 守卫（L16-47）
- `ensureUserLoaded`（guards/authGuard.ts：有 token 无 user 时 fetchCurrentUser）。
- `requiresAuth` = `to.matched.some(record => record.meta.requiresAuth !== false)`（L27）；未登录存 `sessionStorage['redirectPath']` 后 `next('/')`（L29-31）。
- `requiresAdmin`（L35-39）→ checkAdminAccess（guards/adminGuard.ts L8-17）：`!canAccessAdmin` → 重定向 `/dashboard`（L13）。
- 登录后重定向（guards/homeGuard.ts）：L35 `canAccessAdmin ? '/admin/dashboard' : '/dashboard'`；L23 isFromApp 含 `from.path.startsWith('/dashboard')` 与 `'/guide'`；L29-33 消费 redirectPath。
- `routes/index.ts` 按 public→dashboard→admin 拼接。

## 2. auth store

`frontend/src/stores/auth.ts`
- `login` L57-91；`role` 来自 `authApi.getCurrentUser()`（GET /api/users/me）。
- `isAdmin` L52、`isAuditAdmin` L53、`canAccessAdmin` L54、`canOperateAdmin` L55（store 外零引用，死代码）、`isAuthenticated` L35。
- 引用清单：App.vue:120、navigation.ts:23/26/29、MainLayout.vue:469/475-476、adminGuard.ts:11、homeGuard.ts:35、Login.vue:98、Dashboard.vue:810/812-813、Usage.vue:47/51/55/65/78、features/usage 组件 isAdmin prop（约 20 处）。

## 3. 用户侧视图（views/user/，整目录可删）

**MyApiKeys.vue**（1651 行）
- 密钥表格（桌面 L68-252 + 移动卡片 L253-388 + 分页）；创建/编辑对话框 L401-569；新密钥展示 L571-633；CC Switch 导入 L635-781；CLI 安装对话框 L783-898；删除确认 L900-910。
- API：getApiKeys L1114、createApiKey L1476、updateApiKey L1462、deleteApiKey L1518、toggleApiKey L1533、getFullApiKey L1393/1548、getClientConfig L1327/1394、getAvailableModels L1328、createApiKeyInstallSession L1201。
- 独有依赖：features/api-keys/utils/ccswitchImport.ts。

**Settings.vue**（603 行）——含需迁移功能
- 基本信息（updateProfile L473）+ 修改/设置密码 L36-93（changePassword L480，成功后 logout 跳 `/` L491-493）→ **唯一需迁移功能**。
- 登录设备 L97-207（listSessions L405、updateSessionLabel L525、revokeSession L542、revokeOtherSessions L559）。
- 偏好 L209-279（getPreferences L421、updatePreferences L442/572，联动 useDarkMode）。
- 独有依赖：utils/passwordPolicy、types/session（formatSessionMeta）。

**ModelCatalog.vue**（343 行）：模型表格+搜索+分页，UserModelDetailDrawer；仅 meApi.getAvailableModels L308。

**components/UserModelDetailDrawer.vue**（446 行）：仅 ModelCatalog 用，只 import PublicGlobalModel 类型。

**model-catalog-helpers.ts**（46 行）：仅 ModelCatalog + spec。

**__tests__/**：MyApiKeys.ccswitch.spec.ts、ModelCatalog.spec.ts、model-catalog-helpers.spec.ts。

## 4. 共享视图

**views/shared/Dashboard.vue**（1460 行）
- 不直接调 meApi/adminApi，统一 dashboardApi（GET /api/dashboard/stats、/daily-stats），后端按角色返回不同字段。
- `isAdmin = computed(() => authStore.canAccessAdmin)` L810；modeLabel L811-813。
- 脚本分支：L912、L1126、L1157、L1261-1273（userMonthlyCost）。
- 模板分支：L11、L137、L240-340（用户本月统计）、L360-396（用户趋势折线）、L397-433（用户模型成本柱状）、L435/L472、L595/605/616/661。

**views/shared/Usage.vue**（1281 行）
- `isAdminPage = computed(() => route.path.startsWith('/admin'))` L181。
- L237-245 热力图分支（用户 → meApi.getActivityHeatmap L240）；L248-259 loadAdminUsers：**admin 路径也调 meApi.getProfile()（L253）**；L440-444 活跃请求分支（用户 → meApi.getActiveRequests L444）。
- 用户专属本地筛选 L349-434、L809-816、L818-823、L828-833、L773。
- 模板：L33、L42-56 admin 3 列 vs L57-69 用户 2 列、L77-78、L117 RequestDetailDrawer 仅 admin。

**features/usage/composables/useUsageData.ts**：loadStats 分支 L136（用户 → meApi.getUsage L262）、loadRecords 分支 L400（用户 → meApi.getUsage L434）。

## 5. api/me.ts

导出类型：UserSession(re-export L11)、Profile(L13)、UserPreferences(L29)、ProviderConfig(L45)、UsageRecordDetail(L53)、ModelSummary(L111)、ProviderSummary(L127)、ApiFormatSummary(L143)、UsageResponse(L158)、ApiKey(L180)、InstallTargetCli/System(L199-201)、ApiKeyInstallSession(L203)、UserClientConfig(L215)、ChangePasswordRequest(L222)。

meApi（L227）方法：getProfile、updateProfile、changePassword、listSessions、updateSessionLabel、revokeSession、revokeOtherSessions、getApiKeys、createApiKey、getApiKeyDetail、getFullApiKey、getClientConfig、deleteApiKey、toggleApiKey、updateApiKey、createApiKeyInstallSession、getUsage、getActiveRequests、getAvailableProviders、getAvailableModels、getEndpointStatus、getPreferences、updatePreferences、updateApiKeyProviders、updateApiKeyCapabilities、getModelCapabilitySettings、updateModelCapabilitySettings、getIntervalTimeline、getActivityHeatmap。

消费方（grep `from '@/api/me'`，仅 6 文件 + 2 vi.mock）：

| 文件 | 侧别 | 成员 |
|---|---|---|
| views/user/Settings.vue:288 | 用户侧 | meApi 全套 profile/password/sessions/preferences |
| views/user/MyApiKeys.vue:916 | 用户侧 | keys 全套 + install session |
| views/user/ModelCatalog.vue:244 | 用户侧 | getAvailableModels |
| views/shared/Usage.vue:134 | 共享 | getActivityHeatmap(用户)、**getProfile(admin L253)**、getActiveRequests(用户) |
| features/usage/composables/useUsageData.ts:3 | 共享 | getUsage（用户分支） |
| api/__tests__/me.spec.ts:13 | 测试 | toggleApiKey |

- 无任何 admin 视图直接 import @/api/me。me.ts 中 getApiKeyDetail/getAvailableProviders/getEndpointStatus/updateApiKeyProviders/updateApiKeyCapabilities/getModelCapabilitySettings/updateModelCapabilitySettings/getIntervalTimeline 已无视图消费。

## 6. 用户侧专属其他模块

- utils/passwordPolicy.ts — 仅 Settings.vue（+ spec）。
- api/public-models.ts — 仅 ModelCatalog/Drawer/helpers。
- types/session.ts — 仅 api/me.ts + Settings.vue。
- features/api-keys/utils/ccswitchImport.ts — 仅 MyApiKeys.vue（+ spec）。
- features/api-keys/utils/userKeyPayload.ts — 零消费方（死代码）。
- utils/accountBlock.ts、api/auth.ts getAuthSettings()、canOperateAdmin — 无运行时消费方。
- components/ 下无 components/user；用户组件在 views/user/components/。
- 共享保留：useRowClick/useClipboard/useDarkMode/useToast、types/activity、utils/featureSettings、StandaloneKeyFormDialog（admin 用）。

## 7. 导航

- navigation.ts 用户分支 L29-46（if (!canAccessAdmin) 返回 Overview + Resources）可整删；admin 分支 L48-71 保留。
- buildBreadcrumbs L83-88 /dashboard/settings 特判可删。
- MainLayout.vue：/dashboard/settings 入口 L155-162（桌面）、L293-300（移动）；currentRoleLabel L474-478；isNavActive L456-461 /dashboard 特判；退出登录 L163-170/L301-307 与角色无关保留。
- utils/adminNavigationPrefetch.ts 动态解析，无硬编码用户路由，保留。
- App.vue L120-122 非 admin 访问 /admin → replace('/dashboard')。

## 8. i18n

- 用户侧页面基本硬编码中文（grep t( 零命中）。
- 用户侧导航/壳 key：nav.group.resources(L215)、nav.modelCatalog(L222)、nav.apiKeys(L223)、nav.usageStats(L227)、nav.group.account(L216)、breadcrumb.personalSettings(L261/524)、auth.role.user(L17…)、common.settings。
- 孤儿 key 组：guide.* 88 条×2（/guide 已不存在；homeGuard L23 残留 '/guide'）、nav.walletCenter/billingCenter/myReferral/walletManagement/billingManagement、auth.login.registerNow/noAccount/contactAdmin/oauth*/demoMode/adminLocal/ldap。

## 9. 测试

删：views/user/__tests__/ 3 个、api/__tests__/me.spec.ts、utils/__tests__/passwordPolicy.spec.ts（若保留 passwordPolicy 则不删）、features/api-keys/utils/__tests__/ccswitchImport.spec.ts、userKeyPayload.spec.ts、utils/__tests__/accountBlock.spec.ts。
改：navigation.spec.ts（user 用例 L24/37/58）、homeGuard.spec.ts（L49）、auth.spec.ts（角色断言）、Dashboard.spec.ts（wallet card L143-144）、Usage.record-filters.spec.ts（L15-38）、useUsageData.spec.ts（vi.mock L33）、UsageRecordsTable.spec.ts（user/admin 列逻辑）、loginRedirect.spec.ts（回归）。

## 10. 连带影响

1. admin 侧对 me.ts 两处运行时依赖：Usage.vue:253 getProfile；useUsageData/Usage 用户分支（删 /dashboard 后恒 admin）。
2. 重定向硬编码 4 处：homeGuard.ts:35、Login.vue:98、adminGuard.ts:13、App.vue:121 → /admin/dashboard；'/guide' 残留清理。
3. Settings.vue 修改密码必须迁移；登录设备/偏好建议一并迁移。
4. MainLayout /dashboard/settings 入口会成死链；currentRoleLabel 用户分支删。
5. shared 视图删用户分支后更简单；usage 组件 isAdmin prop 恒 true，可后续内化。
6. me.ts 删除后无类型残留风险；8 个方法本就无消费方可一并瘦身。
7. 可整删资产清单见 checklist。
8. i18n 清理含 guide.88×2 等；i18n.spec 若断言 key 数需同步。
9. isAuditAdmin 区分可删；canAccessAdmin 仍被守卫/布局使用；canOperateAdmin 死代码。
10. sessionStorage redirectPath 流程与角色无关，保留。
