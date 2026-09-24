# aether-gateway 开发入口

范围：`apps/aether-gateway/`。以当前 workspace 的包定义和实际导出定位代码。

## Pre-Development Checklist

- [包定义](../../../../apps/aether-gateway/Cargo.toml) 与 [实现入口](../../../../apps/aether-gateway/src/lib.rs)：核对 feature、依赖和直接调用方。
- [共享模块边界](../../backend/architecture.md)：跨包改动时读取。
- [共享错误与资源规则](../../backend/error-handling.md)：错误、日志、超时、取消改动时读取。
- [共享数据规则](../../backend/database-guidelines.md)：仅涉及 SQL、repository、Schema 或事务时读取。

## 本包契约

- [Administrator JSON Backup Contract](admin-json-backup.md)
- [Authenticated Wallet Summary](auth-wallet-summary.md)
- [Chat Scheduling and Failover](chat-failover.md)
- [Dashboard Stats API Contract (`/api/dashboard/stats`)](dashboard-stats-api.md)
- [External Model Catalog Reliability](external-model-catalog.md)
- [Provider Model Test API Contract](model-test-api.md)
- [网关质量与健康策略](quality-guidelines.md)
- [Transport Diagnostics and SOCKS DNS](transport-diagnostics-and-proxy.md)

## Quality Check

- 按 [共享验证规范](../../backend/quality-guidelines.md) 选择本次改动的最小证据，测试目标从本包配置和源码确认。
- 新的稳定模块约束写在本目录并加入索引；共性规则修改共享文档，不重新复制模板。
