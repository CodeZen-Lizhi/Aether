# 实施协调契约

用户已批准实施；不要再次进入规划确认。当前任务已 in_progress。

## 所有权

- health_policy（原生 agent）：orchestration/{health,effects,classifier,mod}.rs、scheduler-core/src/health.rs。负责 ChatFailureFacts、评分、冷却、恢复和 effect 集成。
- retry_runtime（原生 agent）：orchestration/attempt.rs、executor/candidate_loop.rs、execution_runtime 同步/流式/transport_failure plumbing、serving attempt loop。负责有限重试、2 秒等待、90 秒预算。
- storage（channel）：aether-data 仓储契约/runtime/SQLite、gateway state/catalog 薄适配。负责健康结算事务幂等 API，不修改 effects.rs；将 API 写入本目录 storage-contract.md 供 health owner 接入。
- config（channel）：contracts/plan.rs 的新 timeout 字段、aether-admin 配置 API、gateway 计划配置投影（不改 candidate_ranking/materialization）、前端 provider/routing 配置。负责新预算生效投影、旧值 2 迁移、字段验证和展示。attempt.rs 归 retry_runtime，先写 config-contract.md 约定解析 API。
- websocket（channel）：handlers/proxy/websocket/responses/。负责独立轮次共享重试预算与 pinned continuation 边界，消费健康统一事实，不改 effects/普通 HTTP 执行。
- ranking（channel）：scheduler-core/ranking、gateway candidate_ranking，负责倍率和同价亲和真实读取；effects 亲和写入归 health_policy，请将必要调用契约交给主会话集成。

## 已确定共享字段

- ExecutionTimeouts.stream_failover_budget_ms: Option<u64>，单位毫秒，serde 可缺省；默认 90000，独立于 total_ms。
- 配置持久化字段命名 stream_failover_budget_ms，与现有 failover_rules 对象保留未知字段；实际计划投影与管理展示由 config owner 说明。
- retry_runtime 可以在 ProviderTransferTracker 保持单调 started_at 和逐 K/格式累计重试等待，保证跨候选不重置；普通总尝试 resolver 归其所有。
- 已落盘健康 API：orchestration::{chat_health_policy_applies, classify_chat_failure_for_plan(plan,status,response_text,ChatFailureSource), ChatFailureSource::{UpstreamResponse,UpstreamTimeout,UpstreamTransport,UpstreamProtocol,Neutral}, ChatFailureFact{penalty,rate_limited,reason}, ChatHealthPenalty::{Neutral,Points(u8),Unavailable}}；调用 LocalExecutionEffect::ClassifiedHealthFailure(LocalHealthFailureEffect, ChatFailureFact)。不要另造映射。stream failure helper 已接线。
- 仓储去重必须与健康/CAS 同一原子提交，正常 compare_and_update 兼容保留；storage owner 尽快写出最小新 API。

验证：避免多个 cargo 同时编译 gateway；各 owner 先独立静态检查、fmt 指定文件和小 crate 测试，gateway 完整针对性测试由主会话串行调度。不要全仓格式化，不提交/发布/真实模型调用。

## 2026-09-13 实施接入更新

- health_policy：storage-contract.md 已提供原子结算接口与 gateway wrapper。请尽快接入 record_chat_health_effect；需要从 attempt 开始捕获 identity/credential/epoch，不能在结算时重新捕获。原 API CAS 保留供 lease 使用。
- retry_runtime：config-contract.md 已提供 aether_contracts::chat_retry::resolve_chat_max_attempts；ExecutionTimeouts.stream_failover_budget_ms 已落盘。stream_path 目前读取单独 system config，必须改为实际 provider failover_rules 的预算，且保留逻辑请求起点、跨候选不重置。只依赖 system config 会导致前端保存无效。
- health_policy：请完成长探测租约续期、取消释放以及 CostBased 的同价亲和成功写入。ranking owner 负责读取与排序；不要把暂时失败直接清除亲和，备用完整成功后再迁移。
- integration：真实测试入口位于 tests/scheduler_failover.rs；main 负责 mod 注册。使用生产配置路径，不加仅供测试的预算捷径。
- retry_runtime：health 新增 capture_chat_health_attempt(state, plan) -> Result<Option<Value>> 和 CHAT_HEALTH_ATTEMPT_REPORT_FIELD。请在 HTTP sync/SSE 每个真实模型尝试前调用，并把值写入该次 report_context 贯穿所有成功/失败 effects。不能只靠 candidate_id 默认回退，否则旧 epoch/凭据隔离不成立；不得在结算时才 capture。
- websocket：main 已通过 channel 要求 WS 各 attempt 接入相同 capture。允许 WS owner 在 attempt_lifecycle.rs begin 接线。
- ranking：正在独占补齐无 policy 的跨页排序、scheduler/affinity.rs 的 CostBased 读取与亲和并发顺序元数据/缓存保护。effects.rs 仍归 health_policy，待 ranking-contract 写出最终消费 API 后接入。
- health_policy：Retry-After 当前仓储时间精度为秒，current_unix_secs 向下取整可能使 1 秒要求被缩成不足 1 秒；请保证实际截止不会早于接收时刻加时长（可保守向上取整），并继续遵守每 K 累计等待 2 秒上限。

## health_policy 接口更新

- 原子仓储已经接入 effects.rs 的 `record_chat_health_effect`。新增 `capture_chat_health_attempt(state, plan).await? -> Option<Value>`，执行入口需在真实调用开始前捕获一次并把值放入 report_context[CHAT_HEALTH_ATTEMPT_REPORT_FIELD]，field 值为 `chat_health_attempt`。只能在每次真实 attempt 开始时生成，终态重投不可重新生成。该值仅包含 SHA256 凭据摘要、epoch、随机 attempt ID、开始时间，不包含密钥密文。
- **retry_runtime/main 必须修改 recovery.rs 的 apply_provider_failure_disposition**：当前 `SameCredential` 仍错误映射为 `StopLocalFailover`；应该与 NextCandidate 一样映射 `RetryNextCandidate`，同时 execution_runtime 的重试 scope 保持 Candidate。classifier 新聊天分支已经返回 SameCredential。
- 冷却/健康/熔断同次投影、完整成功 +0.1、严格连续计数、旧计数迁移、CostBased 本地配置亲和写入已实现。effects 仍由 health_policy 独占。
- attempt 捕获还需要保证 plan 所用凭据与读取的 generation 一致；如果 planner 缓存了旧凭据，捕获当前代次后仍执行旧 plan 会绕过代次保护。最好在实际 transport snapshot 验证后再捕获。
- 长探测续租/取消释放目前尚未接入：原 claim enum 不返回 owner token，且 planner/candidate_loop 都可能在执行前预占。仅在健康模块新增定时器无法证明租约所有权。需要 main/retry_runtime 统一 claim 到真实 attempt，持有有 owner token 的 guard 到终态；现有 60 秒 claim 不能直接视为满足 A6。
- 健康定向测试已写，包括重复结算、旧 epoch 成功、连续成功不被节流丢弃。一次 cargo test 遇主会话构建锁后已取消，暂无本 agent 运行通过声明。
