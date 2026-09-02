# Quality Guidelines

> Code quality standards for backend development.

---

## Overview

<!--
Document your project's quality standards here.

Questions to answer:
- What patterns are forbidden?
- What linting rules do you enforce?
- What are your testing requirements?
- What code review standards apply?
-->

(To be filled by the team)

---

## Forbidden Patterns

<!-- Patterns that should never be used and why -->

(To be filled by the team)

---

## Required Patterns

<!-- Patterns that must always be used -->

### Convention: provider-ops 架构预设的多认证方式拆分（2026-09，任务 09-02-newapi-auth-split-balance-errors）

**What**: `crates/aether-admin/src/provider/ops/architectures/` 下新增/调整架构预设时，凡凭据存在「二选一」方式（如 访问令牌 vs Cookie），必须在 `supported_auth_types` 中拆成多个 `ProviderOpsAuthSpec` 条目（一个方式一个 schema），而不是把所有字段塞进单一 schema 靠 `any_required` / `conditional_required` 兜底。

**Why**: 前端 `ProviderAuthDialog.vue` 的「认证方式」下拉框只在 `supported_auth_types.length > 1` 时渲染，schema 驱动渲染各方式字段。单一扁平 schema 会让用户看到全部输入框，保存时才报校验错误；拆分后按方式只显示对应字段。

**Example**（参照 `sub2api.rs` 与 `new_api.rs`）:
- 每个条目：`auth_type` 必须取前端 `ConnectorAuthType` 联合（`'api_key' | 'session_login' | 'oauth' | 'cookie' | 'none'`）中的值；display_name 用中文方式名
- 校验用显式 `required`（JSON schema `required` 数组 + `x-validation` type `"required"`），不用 any/conditional 组合
- `credentials_schema`（顶层）指向 `default_connector` 对应方式的 schema
- 存量兼容：沿用旧的 `auth_type` 值给默认方式，避免已保存配置解析不到 schema
- 方式间共享的字段（如 new_api 的 user_id）在每个 schema 中都要出现且必填

**Extensibility**: 新增第三种方式 = 再加一个 `ProviderOpsAuthSpec` 条目 + 对应 schema，前端自动出现新选项，无需前端改动。

---

## Testing Requirements

<!-- What level of testing is expected -->

(To be filled by the team)

---

## Code Review Checklist

<!-- What reviewers should check -->

(To be filled by the team)
