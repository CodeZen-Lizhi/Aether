# Aether 共享后端规范

适用于当前 Rust workspace。先读受影响包的索引，再按改动选择下列主题；各包的业务契约优先于此处的一般约定。

| 主题 | 何时读取 |
| --- | --- |
| [模块与运行边界](architecture.md) | 新增模块、调整依赖、跨包迁移 |
| [错误、日志与资源](error-handling.md) | 错误映射、流式处理、超时、取消、日志 |
| [数据与迁移](database-guidelines.md) | SQL、repository、事务、Schema、导入导出 |
| [审查与验证](quality-guidelines.md) | 选择本次改动的验证范围与交付证据 |

这组规则共享给各包，不再为每个 crate 复制五份通用模板。包内文档只记录实际不同的业务规则。
