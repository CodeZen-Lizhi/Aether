# 网关质量与健康策略

先按 [共享验证规范](../../backend/quality-guidelines.md) 选择本次修改的证据。网关专门契约由 [本包索引](index.md) 导航。

## 聊天健康策略的权威来源

聊天 HTTP/SSE/Responses WS 的健康、重试和 probe 行为以 [chat-failover](chat-failover.md) 为准。旧文档中“429 不计健康失败、probe 成功恢复 0.75、随后三次回满”的规则已被聊天 policy v2 替代，不适用于当前聊天路径；其他协议保留自己的现有行为。

源码依据是 [chat_health.rs](../../../../apps/aether-gateway/src/orchestration/chat_health.rs) 的 `CHAT_HEALTH_POLICY_VERSION = 2` 与固定点计分，以及 [scheduler_failover](../../../../apps/aether-gateway/src/tests/scheduler_failover.rs) 的本地上游和读后状态断言。分数对外为 0–10，内部归一化为 0–1；两次 500 后备用成功时，原 Key 是 0.6，备用成功不能给原 Key 回血。

修改候选准入、probe 所有权、终态或数据库结算时，检查真实执行与后续状态，而不只测试纯投影。具体矩阵与回归入口只在 chat-failover 中维护，避免出现第二套数值规则。
