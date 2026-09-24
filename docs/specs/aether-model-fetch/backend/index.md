# aether-model-fetch 开发入口

范围：`crates/aether-model-fetch/`。以当前 workspace 的包定义和实际导出定位代码。

## Pre-Development Checklist

- [包定义](../../../../crates/aether-model-fetch/Cargo.toml) 与 [实现入口](../../../../crates/aether-model-fetch/src/lib.rs)：核对 feature、依赖和直接调用方。
- [共享模块边界](../../backend/architecture.md)：跨包改动时读取。
- [共享错误与资源规则](../../backend/error-handling.md)：错误、日志、超时、取消改动时读取。
- [共享数据规则](../../backend/database-guidelines.md)：仅涉及 SQL、repository、Schema 或事务时读取。

## Quality Check

- 按 [共享验证规范](../../backend/quality-guidelines.md) 选择本次改动的最小证据，测试目标从本包配置和源码确认。
- 新的稳定模块约束写在本目录并加入索引；共性规则修改共享文档，不重新复制模板。
