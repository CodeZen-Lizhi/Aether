# Aether Spec 索引

当前规范按真实 workspace 与前端组织。先找受影响包，再读相关共享规范和业务契约；不要一次加载所有包。

- [共享后端规范](backend/index.md)：模块、错误/资源、数据、审查与验证。
- [前端规范](frontend/index.md)：已有 UI 与业务展示契约，以及状态验证入口。
- [维护指南](guides/spec-maintenance.md)：初始化后的补齐和任务结束后的增量更新。

## Rust 包

| 包 | 源码目录 | 规范入口 |
| --- | --- | --- |
| `aether-admin` | `crates/aether-admin` | [开发入口](aether-admin/backend/index.md) |
| `aether-admission-core` | `crates/aether-admission-core` | [开发入口](aether-admission-core/backend/index.md) |
| `aether-ai-formats` | `crates/aether-ai/formats` | [开发入口](aether-ai-formats/backend/index.md) |
| `aether-ai-serving` | `crates/aether-ai/serving` | [开发入口](aether-ai-serving/backend/index.md) |
| `aether-billing` | `crates/aether-billing` | [开发入口](aether-billing/backend/index.md) |
| `aether-cache` | `crates/aether-cache` | [开发入口](aether-cache/backend/index.md) |
| `aether-contracts` | `crates/aether-contracts` | [开发入口](aether-contracts/backend/index.md) |
| `aether-crypto` | `crates/aether-crypto` | [开发入口](aether-crypto/backend/index.md) |
| `aether-data` | `crates/aether-data/runtime` | [开发入口](aether-data/backend/index.md) |
| `aether-data-contracts` | `crates/aether-data/contracts` | [开发入口](aether-data-contracts/backend/index.md) |
| `aether-data-query` | `crates/aether-data/query` | [开发入口](aether-data-query/backend/index.md) |
| `aether-data-schema` | `crates/aether-data/schema` | [开发入口](aether-data-schema/backend/index.md) |
| `aether-data-sqlite` | `crates/aether-data/adapters/sqlite` | [开发入口](aether-data-sqlite/backend/index.md) |
| `aether-desktop` | `apps/aether-desktop/src-tauri` | [开发入口](aether-desktop/backend/index.md) |
| `aether-dispatch-core` | `crates/aether-dispatch-core` | [开发入口](aether-dispatch-core/backend/index.md) |
| `aether-gateway` | `apps/aether-gateway` | [开发入口](aether-gateway/backend/index.md) |
| `aether-gateway-control` | `crates/aether-gateway/control` | [开发入口](aether-gateway-control/backend/index.md) |
| `aether-gateway-execution` | `crates/aether-gateway/execution` | [开发入口](aether-gateway-execution/backend/index.md) |
| `aether-gateway-frontdoor` | `crates/aether-gateway/frontdoor` | [开发入口](aether-gateway-frontdoor/backend/index.md) |
| `aether-gateway-tunnel` | `crates/aether-gateway/tunnel` | [开发入口](aether-gateway-tunnel/backend/index.md) |
| `aether-gateway-workers` | `crates/aether-gateway/workers` | [开发入口](aether-gateway-workers/backend/index.md) |
| `aether-http` | `crates/aether-http` | [开发入口](aether-http/backend/index.md) |
| `aether-integration-tests` | `crates/aether-testing/integration` | [开发入口](aether-integration-tests/backend/index.md) |
| `aether-loadtools` | `crates/aether-testing/loadtools` | [开发入口](aether-loadtools/backend/index.md) |
| `aether-model-fetch` | `crates/aether-model-fetch` | [开发入口](aether-model-fetch/backend/index.md) |
| `aether-provider-core` | `crates/aether-provider/core` | [开发入口](aether-provider-core/backend/index.md) |
| `aether-provider-pool` | `crates/aether-provider/pool` | [开发入口](aether-provider-pool/backend/index.md) |
| `aether-provider-transport` | `crates/aether-provider/transport` | [开发入口](aether-provider-transport/backend/index.md) |
| `aether-routing-core` | `crates/aether-routing-core` | [开发入口](aether-routing-core/backend/index.md) |
| `aether-runtime` | `crates/aether-runtime/base` | [开发入口](aether-runtime/backend/index.md) |
| `aether-runtime-state` | `crates/aether-runtime/state` | [开发入口](aether-runtime-state/backend/index.md) |
| `aether-scheduler-core` | `crates/aether-scheduler-core` | [开发入口](aether-scheduler-core/backend/index.md) |
| `aether-task-core` | `crates/aether-task/core` | [开发入口](aether-task-core/backend/index.md) |
| `aether-task-runtime` | `crates/aether-task/runtime` | [开发入口](aether-task-runtime/backend/index.md) |
| `aether-test-support` | `crates/aether-testing/support` | [开发入口](aether-test-support/backend/index.md) |
| `aether-testkit` | `crates/aether-testing/testkit` | [开发入口](aether-testkit/backend/index.md) |
| `aether-tunnel` | `apps/aether-tunnel` | [开发入口](aether-tunnel/backend/index.md) |
| `aether-usage-core` | `crates/aether-usage/core` | [开发入口](aether-usage-core/backend/index.md) |
| `aether-usage-runtime` | `crates/aether-usage/runtime` | [开发入口](aether-usage-runtime/backend/index.md) |
| `aether-video-tasks-core` | `crates/aether-video-tasks-core` | [开发入口](aether-video-tasks-core/backend/index.md) |
| `aether-wallet` | `crates/aether-wallet` | [开发入口](aether-wallet/backend/index.md) |

## 历史边界

原 `aether-data-postgres`、`aether-data-mysql`、`aether-oauth`、`aether-pool-core` 的纯模板目录已移除：它们不在当前 Cargo workspace 中。历史任务文字记录保留，不作为当前能力声明。数据能力读 [SQLite lifecycle](aether-data/backend/sqlite-lifecycle.md)，Provider 池与认证相关代码从当前 [provider-pool](aether-provider-pool/backend/index.md) 和 [provider-transport](aether-provider-transport/backend/index.md) 定位。
