# Aether 调度现状盘点（slim-personal @ 111929e72）

> 任务: 09-01-scheduling-upgrade · 方法: 全仓 Explore 扫描 + 关键文件复核
> 重要事实: Aether 是 **Rust** workspace（axum + tokio + sqlx/SQLite，Redis 可选），非 Go。

## 1. 请求全链路（一条 /v1/messages 的生命周期）

1. axum 路由: `apps/aether-gateway/src/api/ai/registry.rs:22-80`（OpenAI/Claude/Gemini 三套 POST 模式 + catch-all proxy）。
2. 前门 `proxy_request`（`apps/aether-gateway/src/handlers/proxy/mod.rs:835`，2687 行文件）:
   全局并发 permit（:872）→ 认证/路由分类（:995）→ IP/模型权限 → **前门用户 RPM**（:1349-1388）→ body 缓冲。
3. planner（`apps/aether-gateway/src/ai_serving/planner/`）向 scheduler 要**排名后的候选列表**:
   SQL join（provider⨝endpoint⨝key⨝models，`crates/aether-data/adapters/sqlite/src/candidate_selection.rs:43-53`）
   → 运行时快照（最近 128 条 `request_candidates` + key 行，`scheduler/candidate/runtime.rs:31,45`）
   → 可选性过滤（`resolution.rs:8` → `aether-scheduler-core/src/candidate/selectability.rs:42-125`）
   → 排序（`ranking.rs:16` → `aether-scheduler-core/src/ranking/modes.rs:12`）。
4. 尝试循环: `run_ai_attempt_loop`（`crates/aether-ai/serving/src/attempt_loop.rs:95-159`）；动态懒加载变体 `run_dynamic_attempt_loop`（`apps/aether-gateway/src/executor/candidate_loop.rs:775`）。
5. 上游执行: `execution_runtime/transport.rs`（7733 行，reqwest/wreq）；流式框架 `stream/execution.rs` + `stream_pump.rs`。

## 2. 池模型

无 `pools` 表。池 = **(provider → endpoint → key/OAuth 账号) 三元组集合**，经 models/global_models 与统一模型名关联（`model_provider_model_mappings` 带每映射 priority/api_format）。

- `StoredProviderCatalogProvider`（contracts `provider_catalog/types.rs:178`）: provider_type / is_active / monthly_quota_usd / concurrent_limit / max_retries / proxy / stream_first_byte_timeout_secs
- `StoredProviderCatalogEndpoint`（:304）: api_format（如 `openai:chat`、`claude:messages`）/ base_url / header_rules / body_rules / health_score
- `StoredProviderCatalogKey`（:406）: auth_type / 加密凭据 / rpm_limit+learned_rpm_limit / concurrent_limit / 429 计数 / **health_by_format / circuit_breaker_by_format（JSON 列）** / oauth_invalid_reason / upstream_metadata（OAuth plan/quota 快照）

`crates/aether-provider/pool/` 现在只承载 OAuth 账号配额适配器（ProviderPoolService、plan 档位、5h/周窗口推导）。

## 3. 选择算法（现状）

`select_minimal_candidate`（selection.rs:51）单选 / `collect_selectable_enumerated_candidates_with_skip_reasons`（:199）全量排名。

**不是加权随机也不是 round-robin，是确定性种子哈希洗牌**: `seeded_rank_hash`（modes.rs:189-209）= SHA-256(`seed(=当前秒) : salt : provider_id : endpoint_id : key_id : model : original_index`)——排序每秒重掷、秒内稳定（重试可复用列表尾部）。

三种模式（`scheduler/config.rs:6-11`）:
- `FixedOrder` / **`CacheAffinity`（默认）** / `LoadBalance`
- CacheAffinity 比较链（modes.rs:41-55）: capability → **亲和命中** → 跨格式降级 → priority slot（slim 后恒中性）→ tunnel bucket → 格式偏好 → **健康分桶** → 亲和/种子哈希
- `LoadBalance`（modes.rs:66-80）: capability → 跨格式降级 → **种子哈希分布槽**（priority_mode 决定按 provider 还是按 key 分桶）→ tiebreaker。**没有任何真实负载信号**——本质是每秒重掷的静态随机。
- slim 后所有 priority 字段硬编码中性（`aether-scheduler-core/src/candidate/enumeration.rs:84-91`），DB 优先级列已删。

Skip reasons（selectability.rs:42-125）: provider_quota_blocked / account_quota_exhausted / oauth_invalid / provider_concurrency_limit_reached / provider_key_concurrency_limit_reached / **key_circuit_open** / **key_health_score_zero** / **key_rpm_exhausted**。

## 4. 重试 / 故障转移（现状，强项）

尝试循环（attempt_loop.rs:95-159）: 预排名迭代；`Retry{scope}` 失败 → 压入 `AiAttemptRetryFilter`（:161-189）**按失败作用域剪枝后续候选**:
- `Credential` → 跳过同 key_id；`Endpoint` → 跳过同 endpoint_id；`Provider` → 跳过同 provider_id；`Candidate` → 不剪枝
- 全部候选耗尽 → 返回 `Deferred`（**透传上游原始错误体**，不合成）

错误分类（`apps/aether-gateway/src/orchestration/classifier.rs`）:
- `classify_local_failover`（:299-345）: 可配置 `LocalFailoverPolicy`（stop/continue status codes、error_stop_patterns/success_failover_patterns 正则、transport 开关）。默认 **≥400 全部重试**（含 400/401/429）。
- `classify_anthropic_failure_disposition`（:178-282，仅 claude:messages）: 400 stop；401 → NextCredential+token ForceRefresh；403 → NextCredential；404 → NextEndpoint；413 stop；**429 → NextCredential（CredentialModel scope，限流视为 credential+model 粒度）**；529 → NextEndpoint+Provider scope；500-599 → NextEndpoint。其他格式走 `failure_disposition_from_local_classification`（:142）= 纯 NextCandidate。scope→AiAttemptRetryScope 映射: `execution_runtime/mod.rs:50-68`。
- transport 错误默认换下一个候选（candidate_loop.rs:1023-1032）；`ProviderTransferTracker`（candidate_loop.rs:500-600）限每 provider 传输预算。

## 5. 跨请求状态（现状，DB 持久化 + 乐观锁）

- **失败冷却**: `aether-scheduler-core/src/health.rs:6-7` `FAILURE_COOLDOWN_WINDOW_SECS=60` / `THRESHOLD=8`；同 (provider,endpoint,key) 60s 内 ≥8 败进入冷却；**一次成功立即清零**（:73）。
- **per-key 熔断（按 api_format 独立）**: 8 连败开（`orchestration/health.rs:7`）；`project_local_key_circuit_failure`（:143）**指数探测退避 1→2→…→32 分钟**（:270 `next_circuit_probe_interval_minutes`）；`is_provider_key_circuit_open_at`（scheduler-core health.rs:287-345）按 `next_probe_at_unix_secs` **半开探测**；成功关闭（:233）。`account_deactivated_401` 类硬阻断直接开（:78）。
- **健康分**: health_by_format JSON；失败投影 `project_local_failure_health`（base 0.6-0.75，每次额外连败 -0.15，地板 0.2）；成功 → 1.0。0 分 → key_health_score_zero skip。
- **自适应 RPM 学习**: learned_rpm_limit + 预留比例 + 置信度衰减（scheduler-core health.rs:10-24；`provider_key_rpm_allows_request_since` :201）；429 计数持久化在 key 行；`orchestration/adaptive.rs:60`。
- **OAuth 硬阻断**: runtime.rs:261-350（account_banned/suspended/oauth_token_invalid/[REFRESH_FAILED]+过期）。
- **在途计数/需求**: `provider_pool_demand.rs` —— `ProviderPoolInFlightGuard`（:43）本地 DashMap 或 Redis sorted-set（120s TTL+30s 续租）；EMA 需求快照。**只用于并发上限，不参与排序。**
- 写路径: `apply_local_execution_effect`（orchestration/effects.rs:261）→ `ProviderKeyHealthStateUpdate` 带 `expected_*` 乐观锁围栏（contracts types.rs:166-175）。
- 维护任务: maintenance/runtime/{provider_checkin, provider_quota_alert}。

## 6. 会话亲和（现状，独有优势）

- 默认 CacheAffinity 模式；`SchedulerAffinityCache`（cache/scheduler_affinity.rs:7）TTL 300s / 10k 条 / epoch 失效。
- 亲和键 = api_key_id + api_format + model + client session（scheduler-core affinity.rs:75-90）。
- **9 种客户端会话识别**（client_session_affinity.rs:83-95）: x-aether-session-id/x-aether-agent-id 头 + Codex/Claude Code/OpenCode/Qwen Code/Roo Code/Kilo Code/CherryStudio/OpenUI/官方 SDK 的 body 启发式。
- 命中在 capability 之后的第二优先位（modes.rs:44）；成功记住（selection.rs:103-114）；绑定候选失败时失效（effects.rs:464）。
- 无 LLM 响应缓存（aether-cache 只是 TTL map 工具）。

## 7. 流式（现状，强于 Bifrost）

`execution_runtime/stream/execution.rs`:
- 先读上游响应头帧（:4682-4714）；非 2xx → 收集错误体（:4769-4781）→ 分类 → 换候选（:4873-4947）或带原错误停止。
- **200 也在提交前探测**: success-failover 正则探测（:4717-4764）+ 带界预取 `StreamCommitGate`（:5125-5148，MAX_STREAM_PREFETCH_BYTES/FRAMES，per-format commit policy `stream/commit_policy.rs`）→ SSE 200 内嵌早期错误帧也能转移。
- 提交后失败终局: 帧解码错误 → handle_prefetch_stream_failure（:5257）；首字节看门狗 30s（candidate_loop.rs:50,:1034）；per-provider stream_first_byte_timeout_secs。
- 无中途换供应商、无字节重放（与所有对比对象一致，协议所限）。

## 8. 并发/限流（现状）

- 前门 per-user/key RPM（rate_limit.rs:94，60s 固定窗，fail-open）。
- 全局准入 permit（proxy/mod.rs:872）+ 上游执行门（execution_runtime/admission.rs:38-62，队列预算 → AdmissionTimeout）。
- per-API-key 并发（selection.rs:25-48）；per-provider/key concurrent_limit（selectability.rs:67-96，按 300s 活动窗口计数）；per-key RPM（静态+学习）。
- 无 worker 池队列（async 直发，无 Bifrost 的退避占坑问题）。

## 9. slim 分支剥离情况

已删: pool_member_scores 表、providers/key 优先级列（代码比较器残留但喂常数）、号池管理/OAuth 账号管理端点、支付/订阅/钱包、健康监控 UI。
完整保留: 候选 SQL join、可选性过滤（冷却/熔断/健康/RPM/并发 skip）、种子哈希排名 + CacheAffinity 默认、作用域重试循环、Anthropic disposition 表、自适应 RPM、流式预取门、三层并发准入、在途需求追踪、OAuth 配额适配器、维护任务。

## 10. 差距（对照 01-gateway-comparison.md，PRD 的输入）

1. **冷却/熔断不分错误类型**: 60s/8 败与 8 连败中，429、401、5xx 同权重计数。429 应喂自适应 RPM 而非烧熔断额度；401 凭证死应 1-2 次即快速熔断而非白撞 8 次。
2. **429 无闭嘴期限**: 429 → NextCredential 换下一个，但被限流 key 无 Retry-After 冷却到期记录，高流量下很快弹回再吃一次 429。
3. **排序零动态信号**: LoadBalance = 每秒重掷种子哈希；在途数（数据已有）与延迟（未采集）都不参与排序；CacheAffinity 也只有健康分桶。
4. **熔断触发条件单一**: 只认 8 连败；成功/失败交替的半死 key 永不开熔断；无成功率窗口；恢复后健康分直接 1.0（无爬坡防抖）。
