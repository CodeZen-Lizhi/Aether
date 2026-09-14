Active task: .trellis/tasks/09-13-review-scheduler-failover

你已经是 trellis-implement 子 agent，直接实现，不再 spawn implement/check。用户已批准完整方案，状态 in_progress。读任务产物、implement.jsonl 和 implementation-contracts.md。独占 config/admin/frontend 范围：ExecutionTimeouts 新字段 stream_failover_budget_ms: Option<u64>；aether-admin 配置保存读取/校验；gateway 将该字段投影到真实 ExecutionPlan（不改 executor/candidate_loop、orchestration/attempt、candidate_ranking/materialization）；相关前端 provider/routing 配置表单、预算与健康状态展示。

落实旧存储2由一次修正两次且不遮蔽显式覆盖，规范 max_attempts 含首次1..99，保存/刷新实际值与来源一致，新字段在 failover_rules 配置对象持久化可用并保留未知字段。orchestration/attempt.rs 由 retry_runtime 修改，你不要改；请尽快写 research/config-contract.md 明确其 resolver 应读的 JSON 结构、新旧优先级、预算投影来源，并通过 channel 报给 main。公开 struct 新字段造成其他已有 struct literal 编译问题可做必要机械补全，但不要碰其他 owner 的活跃文件，报给 main 集成。前端沿用现有控件/i18n/响应式；真实调度健康不要伪装成历史成功率。完成相关前端测试/type-check/定向eslint，后端 crate 针对检查，避免 gateway cargo 并发。不要提交、生产变更或付费模型调用。
