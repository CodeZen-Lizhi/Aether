Active task: docs/tasks/09-13-review-scheduler-failover

你已经是 trellis-implement 子 agent，直接实现，不再 spawn implement/check。用户已明确批准完整方案，状态 in_progress。读任务产物、implement.jsonl 各引用及 implementation-contracts.md。你的独占范围为 aether-data 仓储契约/runtime/SQLite 与 gateway state/catalog 薄适配，完成 design 的健康结算跨实例幂等和 CAS 原子性、凭据/状态代次保护所需仓储接口。不要改 orchestration/effects.rs、health.rs、其他配置 schema。

优先复用现有健康 CAS + request attempt 终态记录；如必须新增最小去重结构走既有 SQLite migration，memory 语义同步。不要声称仅进程内去重足够。新 API 尽量保留旧 struct 构造与旧方法兼容，单独方法或 wrapper 类型优先。必须写 research/storage-contract.md 说明调用方签名、重复与 CAS 冲突返回语义、代次标记、保留期与重投。通过 trellis channel send scheduler-failover-impl --as storage --to main 报告 API，便于 health agent 集成。完成具体实现和针对性仓储测试；避免运行 gateway cargo，全仓测试禁止。报告路径、验证、未解决边界。不要提交。
