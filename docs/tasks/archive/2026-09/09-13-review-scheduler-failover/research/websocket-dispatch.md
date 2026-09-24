Active task: docs/tasks/09-13-review-scheduler-failover

你已经是 trellis-implement 子 agent，直接实现，不再 spawn implement/check。用户已批准完整方案，in_progress。读任务产物、implement.jsonl 与 implementation-contracts.md。独占 handlers/proxy/websocket/responses/，完成新健康/重试策略在 WS 独立 response.create 的适用接入、逻辑轮次90秒首有效输出预算（ExecutionTimeouts.stream_failover_budget_ms）、重试间隔同K累计2秒、每次尝试有限。保持 pinned previous_response_id 原目标限制，已向客户端交付不可重放 response/tool 事件后不能透明换K拼接。未知上游部分发送不假定安全。

health_policy agent 正在 orchestration/classifier.rs/mod.rs 发布 ChatFailureFacts 等共享 API，请消费实际落盘签名，不另造状态码规则；不要改 effects.rs。retry_runtime 正在修改 HTTP 的 ProviderTransferTracker/attempt resolver，WS 复用可用公共helper或写 research/websocket-contract.md 说明跨入口需要主会话接线的接口。优先已有 WS quota failover/lifecycle/turn machinery，不能只修文档或留未接线helper。加入针对性 WS 真实本地上游场景回归：独立可重放错误有限重试/备用、首预算不重置、pinned失败不跨K、已交付工具关联不重发、正常长输出不被90秒截断。静态和格式检查可运行，gateway cargo集中由main避免争抢。不要提交/付费模型调用。通过 channel 报告契约与完成结果。
