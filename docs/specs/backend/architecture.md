# 模块与运行边界

## 当前分支的产品范围

[README](../../../README.md) 定义个人单机版本；当前运行支持 SQLite 与 memory。旧目录、历史 SQL、枚举名称或 workspace 中的残留依赖不代表相应产品能力仍启用。新增调用前核对实际路由、feature 与启动配置，不能根据名字恢复已经移除的商业化、多用户或多数据库功能。

包路径以 [Cargo workspace](../../../Cargo.toml) 为准。`aether-gateway` 位于 `apps/aether-gateway`；`aether-data` 位于 `crates/aether-data/runtime`，不要按 crate 名猜路径。

## 放置代码

- 准入领域规则放在 [aether-admission-core](../../../crates/aether-admission-core/src/lib.rs)；该库明确不依赖 Tokio、HTTP、数据库或 Redis。运行期并发 permit 与响应生命周期归 [aether-runtime](../../../crates/aether-runtime/base/src/admission.rs)。
- Provider 领域描述与资格规则归 [provider/core](../../../crates/aether-provider/core/src/lib.rs)；上游 URL、headers、认证、转换策略与诊断组装复用 [provider/transport](../../../crates/aether-provider/transport/src/lib.rs)。不要在每个 handler 再做一套解析。
- 候选引用、调度效果、序列是 [dispatch-core](../../../crates/aether-dispatch-core/src/lib.rs) 的契约；聊天的完整调用策略还涉及 gateway 编排，不能仅根据 `core` 名称搬动整条流程。
- 网关公共请求上下文归 [gateway/control](../../../crates/aether-gateway/control/src/lib.rs)；具体鉴权、持久化和 handlers 仍由适配层负责。[gateway/execution](../../../crates/aether-gateway/execution/src/lib.rs) 当前导出流帧和执行限制，不应把未来迁移目标描述为已完成架构。
- 数据分层遵循 [数据规范](database-guidelines.md)。前端 HTTP 状态、Tauri IPC 与网关业务契约分别沿用现有边界。

## 修改共享契约

先追踪定义、直接调用方和序列化边界。保留字段缺省、显式 null、时间单位、错误分类及旧数据读取语义。只有存在真实复用与变化点才引入新抽象；局部修复不连带迁移整个 workspace。

对调度、重试、亲和、健康度修改，读取 [聊天故障转移契约](../aether-gateway/backend/chat-failover.md) 和 [路由契约](../aether-routing-core/backend/simplified-routing.md)。图片、视频、embedding 不自动继承聊天策略。
