# 调研：New API 上游鉴权与余额查询事实（已核实 v0.10.9）

## 上游接口

- 余额查询 = `GET /api/user/self`（`router/api-router.go` selfRoute 组，经过 `middleware.UserAuth()`）。**上游没有独立的 `/api/user/balance` 端点**（Aether 的 mod.rs 里 `/api/user/balance` 只是 action_config 缺省时的 fallback，new_api 预设 config 用 `/api/user/self`，不受影响）。
- 返回 `{"success":true,"data":{...,"quota":<原始额度>,"used_quota":...}}`；美元换算 = quota ÷ 500000（QuotaPerUnit，站点可配）。
- 签到端点存在：`GET/POST /api/user/checkin`。
- 失败时（含 401）body 形如 `{"success":false,"message":"无权进行此操作，未提供 New-Api-User"}`。

## 上游鉴权（middleware/auth.go authHelper，v0.10.9）

1. 先查 session（gin session cookie）。有 username 则用 session 身份。
2. 无 session 时读 `Authorization` 头；`model.ValidateAccessToken` 会 `strings.Replace(token, "Bearer ", "", 1)` 剥掉一个 Bearer 前缀，然后按 DB 的 `access_token` 字段查用户。**这里的 access token 是「个人设置 → 生成系统访问令牌」，不是 `sk-` 开头的模型调用令牌。**
3. **无论哪种方式，都必须带 `New-Api-User: <数字用户ID>` 头且与登录用户 ID 一致**，否则 401：
   - 「无权进行此操作，未提供 New-Api-User」
   - 「无权进行此操作，New-Api-User 格式错误」
   - 「无权进行此操作，New-Api-User 与登录用户不匹配」
   - access token 无效时：HTTP 200 + `{"success":false,"message":"无权进行此操作，access token 无效"}`（注意是 200，不是 401）

## CC Switch 对照（farion1231/cc-switch，`src/components/UsageScriptModal.tsx`）

NEW_API 模板：`GET {{baseUrl}}/api/user/self`，头 `Authorization: Bearer {{accessToken}}` + `New-Api-User: {{userId}}`；extractor：`remaining = data.quota / 500000`，`used = data.used_quota / 500000`。与 Aether 实现一致，佐证「访问令牌 + 用户 ID」是标准用法。

## Aether 相关代码位置

- 架构预设（本次拆分目标）：`crates/aether-admin/src/provider/ops/architectures/new_api.rs`
  - `supported_auth_types` 目前只有一项 `api_key`（"New API Key"），schema 内含 base_url/api_key/cookie/user_id 四字段。
  - 对照 sub2api.rs：两项 auth types（session_login「账号密码」/ api_key「Refresh Token」），`default_connector: Some("session_login")`，顶层 `credentials_schema` 指向默认方式的 schema。
- 请求头拼装：`crates/aether-admin/src/provider/ops/verify.rs` `admin_provider_ops_verify_headers` 的 `"new_api"` 分支（Authorization Bearer + New-Api-User + Cookie，哪个填了带哪个）——**本次不改**。
- 余额查询错误映射（本次透传目标）：`apps/aether-gateway/src/handlers/admin/provider/ops/providers/actions/query_balance/mod.rs` 非 2xx 分支，`admin_provider_ops_execute_json_request` 返回 `(status, response_json)`，非 2xx 且 JSON 合法时 response_json 可用。
- 前端选择器开关：`frontend/src/features/providers/components/ProviderAuthDialog.vue` `v-if="currentAuthTypes.length > 1"`，schema 驱动，预期零前端改动；`ConnectorAuthType` 类型定义在 `frontend/src/api/providerOps`，新增 auth_type 标识时确认其取值。
- 前端 field hook（Cookie 解析用户 ID）：`frontend/src/features/providers/auth-templates/field-hooks.ts` `parse_new_api_user_id`，schema 里经 `x-field-hooks` 挂在 cookie 字段上，拆分后需保留在 Cookie 方式的 schema 中。
- 相关测试：`apps/aether-gateway/src/tests/control/admin/provider_ops.rs`（含 New-Api-User 断言）、`crates/aether-admin/src/provider/ops/actions.rs` 测试模块、`apps/aether-gateway/src/tests/architecture/admin_provider.rs`。
