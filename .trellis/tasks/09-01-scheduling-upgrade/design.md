# 调度升级技术设计（融合方案）

> 任务: 09-01-scheduling-upgrade · 依据: `research/01-gateway-comparison.md`、`research/02-aether-current-state.md`、`prd.md`
> 行号基于 slim-personal @ 111929e72，实现时以当时 HEAD 为准（本文件重在定位与结构，不逐行背书）。

## 0. 设计原则

1. **加法不改骨架**: 不改请求主链路（proxy → planner → 排名 → 尝试循环 → transport），只在三个既有"接缝"注入新逻辑: ① classifier 的分类结果（错误类）② effects 的状态写路径（新状态维度）③ ranking 的比较器链（新动态信号档位）。
2. **状态分家**: 凭证死 / 限流 / 瞬时三类失败写**不同的状态字段**，互不污染计数（现状最大的问题就是共用一套计数）。
3. **DB 只扩既有 JSON 列**（约束 3）: key 行的 `health_by_format` JSON 增加限流冷却子字段；不建新表。纯内存信号（EWMA、成功率窗、在途快照）不落库。
4. **每个 P1 动态信号带 system_config 开关**，默认 R4 开、R5 采集先行（排序默认关），灰度可控。

## 1. 核心数据结构

### 1.1 错误三分类（R1，纯新增枚举）

```rust
// crates/aether-scheduler-core（或 orchestration 既有分类模块旁）
pub enum UpstreamFailureClass {
    /// 凭证/配额死: 401、403、insufficient_quota、oauth_invalid、account_deactivated
    /// → 1 次即走长熔断快速通道（复用硬阻断路径）
    CredentialDead,
    /// 限流: 429（含限流语义消息）→ 不计冷却/熔断，喂 RPM 学习 + 限流冷却期限
    RateLimited { retry_after_secs: Option<u64> },
    /// 瞬时: 5xx、transport、超时 → 维持现有 60s/8 败冷却 + 8 连败熔断
    Transient,
}
```

判定入口: `orchestration/classifier.rs`。`classify_local_failover` / `classify_anthropic_failure_disposition` 已产出 disposition（NextCredential/NextEndpoint/stop）与 token action，在同处**并行**产出 `UpstreamFailureClass`（不动现有返回结构，作为新字段挂入失败上下文）。判定规则:

- status 401/403 + provider 错误码 `insufficient_quota` / `account_deactivated` / oauth_invalid 映射 → `CredentialDead`
- status 429 / Retry-After 头存在 / 既有 429 语义匹配 → `RateLimited`（同时携带解析出的 retry_after）
- 其余可重试失败 → `Transient`
- 与 LocalFailoverPolicy 的用户自定义 stop 规则正交: 用户配置 stop 仍然 stop（分类只决定"若继续尝试，状态怎么记"）。

### 1.2 限流冷却（R2，扩展 key 行 JSON）

```rust
// health_by_format JSON 内每格式条目新增（aether-data contracts 中对应 struct 扩字段）
pub struct ProviderKeyFormatHealthState {
    // ... 既有字段（health_score、consecutive_failures 等）
    /// R2: 限流冷却到期（unix secs）。到期后参选，无需探测。
    pub rate_limit_cooldown_until_unix_secs: Option<i64>,
    /// R2: 连续 429 计数（无 Retry-After 时的指数试探 30s→1m→2m→4m，封顶 10m）
    pub consecutive_rate_limits: u32,
}
```

生命周期:

```
429 命中
  ├─ 有 Retry-After → cooldown_until = now + retry_after（HTTP-date 解析为秒）
  └─ 无 → cooldown_until = now + min(30s * 2^(n-1), 10m)，n=consecutive_rate_limits+1
选择时（selectability）: now < cooldown_until → skip reason `key_rate_limit_cooldown`
非 429 成功 → 字段清零（consecutive_rate_limits 一并清）
非 429 失败 → 不动这两个字段（只属于限流域）
```

写路径: 走既有 `apply_local_execution_effect`（orchestration/effects.rs:261）→ `ProviderKeyHealthStateUpdate`（乐观锁围栏照旧）。**防写放大**: 仅当 cooldown_until 变化超过 5 秒才落库（429 风暴时不逐次写）。

### 1.3 熔断状态机 v2（R1 快速通道 + R6 窗口触发 + R7 爬坡）

```
                ┌──────────────────────────────────────────────┐
                │ closed（正常服务）                              │
                └──────┬───────────────┬───────────────┬───────┘
        8 连败(transient)   5min 成功率<20%      credential-dead
        （既有路径）        且样本≥10（R6）      1 次即触发（R1）
                       │               │               │
                       ▼               ▼               ▼
                ┌──────────────────────────────────────────────┐
                │ open（熔断，per (key, format)，既有结构）        │
                │ next_probe_at = now + 1→2→…→32min（既有指数）  │
                └──────────────────────┬───────────────────────┘
                  到达探测点（既有半开） │
                                       ▼
                ┌──────────────────────────────────────────────┐
                │ half-open probe（既有）: 放行试探请求            │
                └──────┬───────────────────────────┬───────────┘
              试探成功  │                    试探失败 │
                       ▼                           ▼
        ┌──────────────────────────┐   回 open，探测间隔指数推进（既有）
        │ ramping（R7 新增，0.75 分）│
        │ 连续 3 成 → 满血 1.0      │──恢复期失败──→ 立即回 open（R7）
        └──────────────────────────┘
```

- R1 快速通道: `CredentialDead` 直接调用既有 `project_local_key_circuit_open`（orchestration/health.rs:78 的硬阻断同路径），跳过连败计数。
- R6 窗口: 进程内存环形计数器 per (provider, endpoint, key, format)，容量 5min/桶粒度 30s（10 桶环形）。触发时**写既有的熔断 JSON**（复用 open 状态与探测字段），不发明第二套熔断表示。仅 429 不计入窗口（限流域已隔离）。
- R7 爬坡: `project_local_key_circuit_closed`（health.rs:233）改为"半开关闭"，健康分写 0.75 而非 1.0，并置 `ramp_remaining_successes=3`；`project_local_failure_health` 成功分支在 ramp 期递减计数，减到 0 才写 1.0；ramp 期失败 → 直接重开熔断（探测间隔从当前指数档继续，不回 1min）。

### 1.4 排序动态信号（R4 在途 / R5 延迟 EWMA）

```rust
// 排名上下文扩展（aether-scheduler-core ranking types）
pub struct SchedulerRankingContext {
    // ... 既有
    pub inflight_by_key: HashMap<ProviderKeyId, u32>,     // R4: 只读快照
    pub latency_by_key: HashMap<ProviderKeyId, LatencyEwma>, // R5: 样本数+EWMA
    pub dynamic_signals_enabled: DynamicSignalsConfig,    // 开关
}

pub struct LatencyEwma { pub samples: u32, pub ewma_ms: f64 } // α=0.2，samples<5 不参与比较
```

注入位置（`modes.rs` 比较器链，插在种子哈希**之前**、健康分桶**之后**）:

```
capability → [亲和命中(CacheAffinity)] → 跨格式降级 → priority slot(中性) →
  tunnel bucket → 格式偏好 → 健康分桶 →
  ★ compare_inflight (R4) → ★ compare_latency_ewma (R5) →   ← 新增两档
  亲和/种子哈希 → identity → original_index
```

- `compare_inflight`: `left.inflight.cmp(&right.inflight)`（少者优先）。`LoadBalance` 模式插在分布槽之后、tiebreaker 之前。
- `compare_latency_ewma`: 任一方 samples<5 → `Ordering::Equal`（不惩罚冷启动）；否则 EWMA 低者优先。
- 数据源: R4 快照自 `ProviderPoolInFlightGuard`（provider_pool_demand.rs:43，读接口已有，只读）；R5 由 effects 在响应完成时更新内存 `SchedulerLatencyTracker`（DashMap<(provider,endpoint,key,format)>，流式记 TTFT、非流式记总时延——TTFT 采集点在 `stream_pump.rs` 的 `await_stream_first_byte` 旁）。
- **确定性说明**: 种子哈希的"秒内稳定"语义保留——动态信号不变时同秒排序不变；信号变化（在途数变）本身就该反映。验收标准据此表述。

### 1.5 亲和解绑（R3）

扩展 `orchestration/effects.rs:464` `local_scheduler_affinity_matches_failed_target` 的触发条件: 绑定候选进入 ① credential-dead 熔断 ② rate-limit 冷却 时，同样调用既有的亲和失效路径（`cache/scheduler_affinity.rs` 条目失效 + `scheduler/affinity.rs` 重绑语义）。失败发生处（candidate_loop 的 Retry 分支）把 `UpstreamFailureClass` 一并传入 effects，由 effects 决定是否连带失效亲和。

## 2. 改动点总表（按文件）

| # | 文件 | 改动 | 批次 |
|---|---|---|---|
| 1 | `apps/aether-gateway/src/orchestration/classifier.rs` | 失败上下文增加 `UpstreamFailureClass`（401/403/429/5xx/transport 映射 + Retry-After 解析，含 HTTP-date） | P0 |
| 2 | `crates/aether-data/.../provider_catalog/types.rs` | `health_by_format` 条目 struct 增 `rate_limit_cooldown_until_unix_secs`、`consecutive_rate_limits`（+R7 `ramp_remaining_successes`） | P0 |
| 3 | `sqlite/migrations/` | 新迁移: 既有 JSON 列内结构变更（SQLite JSON 无 schema，迁移主要是回填/校验，预计极薄） | P0 |
| 4 | `apps/aether-gateway/src/orchestration/health.rs` | 快速通道熔断（CredentialDead→open）；`project_local_key_circuit_closed` 改半开+爬坡；429 不推进连败/冷却计数 | P0 |
| 5 | `crates/aether-scheduler-core/src/health.rs` | `is_candidate_in_recent_failure_cooldown` 排除 429 记录；新增 `rate_limit_cooldown` 判定函数 | P0 |
| 6 | `crates/aether-scheduler-core/src/candidate/selectability.rs` | 新 skip reason `key_rate_limit_cooldown`（进跳过审计列表） | P0 |
| 7 | `apps/aether-gateway/src/orchestration/effects.rs` | 写路径分流（错误类→对应状态字段，防写放大 5s 阈值）；亲和失效触发条件扩展 | P0 |
| 8 | `apps/aether-gateway/src/executor/candidate_loop.rs` | Retry 分支携带错误类传给 effects | P0 |
| 9 | `apps/aether-gateway/src/provider_pool_demand.rs` | 暴露 key 级在途只读快照接口（数据已有） | P1 |
| 10 | `crates/aether-scheduler-core/src/ranking/modes.rs` + ranking types | 比较器链插入 inflight/latency 两档; `SchedulerRankingContext` 扩展; 开关字段 | P1 |
| 11 | `apps/aether-gateway/src/scheduler/candidate/runtime.rs` | 快照构建时附加 inflight/latency 到排名上下文 | P1 |
| 12 | 新文件 `apps/aether-gateway/src/scheduler/latency_tracker.rs` | `SchedulerLatencyTracker`（EWMA，α=0.2）；TTFT/总时延采集点接线 | P1 |
| 13 | 新文件 `.../scheduler/success_window.rs`（或并入 health.rs） | 5min 环形成功率窗口（10×30s 桶）+ 触发写熔断 | P1 |
| 14 | `apps/aether-gateway/src/scheduler/config.rs` | system_config 开关: `ranking_dynamic_signals`（inflight/latency 各自开关） | P1 |

## 3. 请求生命周期（优化后，一条 429 的完整旅程）

```
请求 → 前门(permit/RPM/鉴权) → planner 要候选列表
  → SQL join → 运行时快照（key 行含 rate_limit_cooldown_until）
  → selectability: 冷却中? 熔断开? 429期限未到? RPM? 并发? → 过滤
  → ranking: capability → 亲和 → 健康 → ★在途 → ★延迟 → 种子哈希
  → attempt#1 → 上游 429 (Retry-After: 30)
  → classifier: RateLimited{retry_after:30} + disposition NextCredential
  → effects: key.A cooldown_until += 30s（防抖落库）; RPM 学习更新;
             亲和条目解绑; 不记冷却/熔断计数
  → attempt#2（已过滤 key.A）→ 成功 → 健康分维持/爬坡递减 → 亲和绑到 key.B
```

## 4. 边界与风险

| 风险 | 缓解 |
|---|---|
| Retry-After 为 HTTP-date 时本机时钟偏差 | 解析失败回退指数试探（30s 起），不信任异常大的值（>10min 封顶） |
| 429 风暴下 key 行写放大 | 5 秒阈值防抖（仅 cooldown_until 实质变化才落库）；进程内缓存当前值 |
| 动态信号破坏排序确定性/可测试性 | 快照注入排名上下文（纯函数比较器保持可测）；开关可回退纯静态排序 |
| EWMA 冷启动抖动 | samples<5 不参与比较；开关默认关（采集先行） |
| 单实例假设（EWMA/成功率窗进程内存） | slim 单用户单进程成立；多实例部署时接受"每实例独立视角"（与 LiteLLM 非 Redis 模式同级），不引入 Redis 依赖 |
| 快速通道误熔断（上游偶发 401 抖动） | 熔断 per (key,format) 粒度而非 provider 级；半开探测 1min 起步自动找回；OAuth 类 401 本就伴随 ForceRefresh 重试语义 |
| LocalFailoverPolicy 用户自定义规则与错误类冲突 | 分类只影响"状态怎么记"，stop/continue 决策仍归 policy（正交） |

## 5. 实施批次与验证

- **批次一（P0）**: 改动点 1-8。单测: 分类映射表（每状态码×格式）、Retry-After 三形态解析、冷却到期边界、快速通道熔断、爬坡状态机。集成: mock 上游 401/429/5xx 三序列，断言 skip reason 与状态字段。
- **批次二（P1）**: 改动点 9-14。单测: 比较器链新档位（含样本不足回退）、EWMA 收敛、窗口触发阈值边界（样本 9/10、成功率 19%/21%）。集成: 双 key 在途不均场景断言选空闲者。
- **回归**: 全量既有测试；CacheAffinity 粘性回归（同会话同 key）；耗尽透传原始错误回归。
- 每批次独立提交独立可回滚；R5 排序默认关闭，观察 EWMA 采集数据后再启用。

## 6. 与四家网关的对照（设计出处速查）

| 设计元素 | 出处 | Aether 落点 |
|---|---|---|
| 错误三分法 + 死 key 跳过等待 | Bifrost `executeRequestWithRetries`（bifrost.go:6559-6650） | 1.1 / R1 |
| per-异常计数隔离 | LiteLLM `cooldown_handlers.py` allowed_fails 分桶 | 1.2/1.3 状态分家 |
| Retry-After 冷却 | Bifrost 429 池语义 + 通用实践 | 1.2 / R2 |
| 成功率窗口禁用 | one-api `MetricDisableChannel` | 1.3 / R6 |
| 半开探测 + 指数退避恢复 | one-api testAllChannels + Aether 既有 | 1.3（复用既有） |
| 恢复爬坡防抖 | LiteLLM 恢复思路的反向加固 | 1.3 / R7 |
| 在途数信号 | LiteLLM least-busy + Higress inflight(权重1) | 1.4 / R4 |
| 延迟 EWMA(α=0.2) | Higress feedback.ewmaAlpha / AIBrix 延迟建模 | 1.4 / R5 |
| 不抄: worker 队列/6 信号全套/加权随机/合成探测 | — | 见 PRD"明确不做" |
