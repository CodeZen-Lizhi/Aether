Active task: .trellis/tasks/09-13-review-scheduler-failover

storage 的后续独立实施：请实现长探测租约续期和取消释放。你已完成原子仓储，现在原生 health_policy 已交付，不再编辑文件。你独占 orchestration/circuit_probe.rs、rate_limit_probe.rs、必要新 probe_lease.rs 和 health.rs 纯投影 helper、mod.rs 相关导出（其他导出请保留）。不改 effects.rs（main接手）或 HTTP/WS执行入口（retry/WS仍活跃）。

当前 try_claim 只给 Acquired/Unavailable，60秒半开预占没有owner token。设计要求：单探测占用必须跨实例CAS；租约带owner token和credential/circuitepoch；真实执行期间有界定期续期；完整终态/取消/drop时只释放自己的租约，不动其他请求新占用，不给健康加减分；失去租约不得静默继续宣称独占；进程崩溃仍靠TTL释放。复用现有key health/circuit CAS，避免新表/服务。rate-limitprobe同时考虑，严禁释放/续租清除新的RetryAfter冷却或新熔断。

需要给执行者一个简单guard或managed lease API，把调用契约写 research/probe-lease-contract.md，并通过channel通知main。HTTP流式需持有到body终态而非Response创建时；WS到turn终态。由于调用方在并行实现，可先API+单测然后main接线，不得自称仅helper已实现功能。旧 try_claim签名可兼容桥接，但执行入口最终必须用owner API。

你的范围还可以修复chat probe取消旧标记被遗留等直接相关问题，不能把所有业务异常算作lease丢失。使用fake时钟/投影测试与memory CAS并发证明：一名持有人、过60秒仍不可再占、drop释放、旧owner不清新owner/新epoch、失效后不能续租。此轮不要cargo（main正编译gateway），格式指定文件可做。

输出实际API和接入限制，主会话负责生产入口贯通。不要提交/付费模型调用。请继续到可集成模块完成。
