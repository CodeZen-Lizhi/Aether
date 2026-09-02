# Error Handling

> How errors are handled in this project.

---

## Overview

<!--
Document your project's error handling conventions here.

Questions to answer:
- What error types do you define?
- How are errors propagated?
- How are errors logged?
- How are errors returned to clients?
-->

(To be filled by the team)

---

## Error Types

<!-- Custom error classes/types -->

(To be filled by the team)

---

## Error Handling Patterns

<!-- Try-catch patterns, error propagation -->

### Convention: provider-ops 动作失败时透传上游业务 message（2026-09，任务 09-02-newapi-auth-split-balance-errors）

**What**: `apps/aether-gateway/.../actions/query_balance/mod.rs` 等 provider-ops 动作把上游非 2xx 响应映射为面向前端的错误时，若响应体是 JSON object 且含非空字符串 `message` 字段，必须以「固定前缀文案：上游 message」的形式拼接返回；无 `message` 或体非 JSON 时回退固定文案。

**Why**: 上游的具体失败原因（如 New API 的「无权进行此操作，未提供 New-Api-User」「New-Api-User 与登录用户不匹配」「access token 无效」）是用户排查凭据配置问题的关键信息，只报「认证失败」会迫使用户盲猜。

**注意的边界**:
- 上游「HTTP 200 + `success:false`」属业务解析分支（`parse_query_balance_payload`），本来就会透传 message，不要在非 2xx 分支重复处理
- 各状态码的固定前缀文案（401/403/404/429/其他）保持不变，只做追加
- `query_balance_cookie_auth_errors` 之类按架构特化的文案仍作为主干

---

## API Error Responses

<!-- Standard error response format -->

(To be filled by the team)

---

## Common Mistakes

<!-- Error handling mistakes your team has made -->

(To be filled by the team)
