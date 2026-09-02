# New API 认证方式拆分与余额查询错误信息透传

## Goal

用户在供应商列表添加 New API 站点并配置「查询余额」时，遇到两类体验问题：

1. 访问令牌和 Cookie 是二选一的凭据，但表单把所有输入框平铺展示，靠保存时校验兜底；而 Sub2API 预设会先让用户选「认证方式」再显示对应字段。New API 应回归同样的交互。
2. 查询余额失败（如 401）时只报干巴巴的「认证失败」，而 New API 上游会返回具体的失败原因（如「未提供 New-Api-User」「New-Api-User 与登录用户不匹配」「access token 无效」），这些信息对用户排查至关重要，应当透传。

同时修正一个误导性描述：凭据 schema 中「使用 Cookie 时用户 ID 可选」与 New API 上游行为不符——上游对所有方式都强制校验 `New-Api-User` 请求头，用户 ID 实际必填。

## Requirements

### R1 拆分认证方式（后端预设）

- `new_api` 架构的 `supported_auth_types` 拆为两个条目：
  - 「访问令牌」：字段为 站点地址、访问令牌、用户 ID（用户 ID 必填）。访问令牌的描述必须明确是「个人设置生成的系统访问令牌」，不是 `sk-` 开头的模型调用令牌。
  - 「Cookie」：字段为 站点地址、Cookie、用户 ID（用户 ID 必填，并保留粘贴 Cookie 时自动解析填充的 field hook）。
- 「访问令牌」条目沿用现有 `api_key` auth_type 标识（保证已保存的存量配置仍能解析出 schema），作为默认选中项。
- 「Cookie」条目使用新的 auth_type 标识（需与 ConnectorAuthType 允许值兼容，参考 sub2api 的 `session_login` 用法）。
- 两个 schema 的校验规则改为显式必填（不再是 `any_required` / `conditional_required` 的兜底组合）。
- 顶层 `credentials_schema` 与 `default_connector` 语义保持与 sub2api 一致（指向默认方式的 schema）。

### R2 余额查询失败时透传上游错误信息（网关 handler）

- SingleRequest 路径的余额查询，当上游返回非 2xx 且响应体是合法 JSON 时，从响应中提取业务错误信息（New API 的 `message` 字段）并拼入返回给前端的错误 message（如「认证失败：无权进行此操作，未提供 New-Api-User」）。
- 上游无 `message` 字段或响应体非 JSON 时，退回现有固定文案，行为不回退。
- Cookie 失效的特殊文案分支（`query_balance_cookie_auth_errors`）语义保留。

### R3 不破坏现有行为

- 已保存的 new_api 供应商配置（auth_type=api_key）在编辑/查询余额时行为不回退。
- 查询余额成功路径、签到探针（checkin probe）、Sub2API/usage_api/generic_api 路径不受影响。

## Acceptance Criteria

- [ ] 前端选择 New API 模板时出现「认证方式」下拉框，含「访问令牌」「Cookie」两项；选「访问令牌」只显示 站点地址/访问令牌/用户 ID，选「Cookie」只显示 站点地址/Cookie/用户 ID。
- [ ] 两种方式下用户 ID 均为必填；访问令牌方式不填访问令牌、Cookie 方式不填 Cookie 时保存校验报错。
- [ ] 粘贴 Cookie 时用户 ID 仍自动解析填充（field hook 保留）。
- [ ] 模拟上游返回 401 且 body 含 `{"success":false,"message":"无权进行此操作，未提供 New-Api-User"}` 时，余额查询结果 message 包含该上游信息；body 无 message 时仍显示原固定文案。
- [ ] 相关 Rust 测试更新并通过（含新增/调整的错误透传断言与架构 spec 断言）；前端构建/类型检查通过。
- [ ] `cargo test`（涉及的 crate）与前端 lint/tsc 无回归。

## Constraints

- 不改上游协议语义：请求头拼装逻辑（Authorization / New-Api-User / Cookie）不变。
- 前端是 schema 驱动的，预期零前端改动即可出现选择器；若需前端配合（如 auth_type 类型联合）按最小改动处理。

## Notes

- 上游鉴权事实依据（调研已确认，new-api v0.10.9 `middleware/auth.go`）：`/api/user/self` 需 session 或 `Authorization`（剥 Bearer 后按 access_token 查库），且**必须**带 `New-Api-User: <数字用户ID>` 且与登录用户一致，否则 401。
- CC Switch 的 NEW_API 模板同样使用「访问令牌 + 用户 ID」调用 `GET /api/user/self`，`quota/500000` 换算美元，可作对照。
