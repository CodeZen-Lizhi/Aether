# 调度核心验证入口

调度领域规则由 [src/lib.rs](../../../../crates/aether-scheduler-core/src/lib.rs) 导出，策略语义读取 [路由契约](../../aether-routing-core/backend/simplified-routing.md)。

纯排序、亲和或候选规则变更先验证本包受影响用例；改变聊天候选顺序、准入或状态时，还需按 [聊天契约](../../aether-gateway/backend/chat-failover.md) 检查真实网关调用及后续状态。范围和重复检查原则见 [共享验证规范](../../backend/quality-guidelines.md)。该文件保留为历史任务 JSONL 的有效入口。
