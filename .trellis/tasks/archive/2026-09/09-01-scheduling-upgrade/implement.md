# 实施记录（P0 + P1 + R10 + R11 后端）

> 状态: 后端全部完成 · 日期: 2026-09-02 · 分支: slim-personal（工作区含用户并行 slim 改动）

## 已交付

### P0-1/2/3（错误三分类 + Retry-After 冷却 + 亲和解绑）
- `crates/aether-scheduler-core/src/health.rs`: `UpstreamFailureClass`（CredentialDead/RateLimited/Transient，from_status_code: 401/402/403→凭证死、429→限流、其余→瞬时）; `ProviderKeyRateLimitCooldown`（project 指数梯 30s→1m→2m→4m 封顶 10m，Retry-After 优先且 >10m 拒信）; 读取/判定函数。
- `apps/aether-gateway/src/orchestration/health.rs`: `parse_retry_after_secs`（秒数+HTTP-date 双格式，过去日期/超界拒绝）; `project_local_failure_health` 跳过 429 并保留限流字段; `project_local_rate_limit_cooldown`（防写放大: 窗口有效且移动 ≤5s 则不写）; 成功投影清零限流字段。
- `apps/aether-gateway/src/orchestration/effects.rs`: `record_health_failure_effect` 三分类分流——429→限流冷却+亲和解绑; 凭证死→`record_credential_dead_circuit_effect`（1 次即开长熔断，复用 `project_local_key_circuit_open` 指数探测）+解绑; 瞬时→原路径。`unbind_scheduler_affinity_for_failed_candidate` 复用（P0-3）。
- `selectability.rs`: 新 skip reason `key_rate_limit_cooldown`。
- 流式/非流式失败路径接线 Retry-After 解析（stream/execution.rs、sync/execution.rs、execution_failures.rs）。

### P1-6/7（成功率窗口熔断 + 恢复爬坡）
- `project_local_key_circuit_failure_with_success_rate`: 5min 窗口成功率 <20% 且 ≥10 样本 → 开熔断（reason `success_rate_window_20pct`）; `append_request_result_window` 容量 8→64。
- `project_local_key_circuit_closed_with_ramp`: 探测成功关熔断时进入爬坡（`ramp_remaining_successes=3`）; `project_local_ramp_success_health` 健康分 0.75 线性爬回 1.0; 爬坡期失败立即重开熔断（effects 中 ramp_active→success_rate_breached=true）。

### P1-4/5（在途数 + 延迟 EWMA 排序信号）
- `SchedulerRankableCandidate` 增 `inflight_count`/`latency_ewma_ms`; `SchedulerRankingContext` 增 `include_inflight`/`include_latency` 开关。
- CacheAffinity 链插入两档（健康之后、种子哈希之前）; 在途少者优先; 延迟 EWMA α=0.2、样本 <5 视为缺席（不惩罚冷启动）。
- `apps/aether-gateway/src/scheduler/latency_tracker.rs`（新文件）: 进程内 per (provider,endpoint,key,format) EWMA tracker，10k 容量，无持久化。
- 信号注入在 planner 的 `build_rankable_candidate`; 在途数来自最近候选记录（与并发上限同源）。
- system_config 开关: `ranking_inflight_signal`（默认开）/ `ranking_latency_signal`（默认关，采集先行）。

### R10（成本优先模式 + LoadBalance 软删）
- `SchedulerRankingMode::CostBased`（scheduler-core；原名 Economy，后应用户要求全栈改名 cost_based）+ 比较器: 亲和 > 倍率升序 > 优先级 slot > 健康 > 动态信号 > 哈希。
- 倍率读自 key 行 `rate_multipliers[api_format]`（仅 成本优先模式查询 DB，`with_rate_multiplier` 守卫非法值）。
- 贯通枚举链: `RoutingSchedulingMode::CostBased` → `SchedulerSchedulingMode::CostBased` → `AiRankingSchedulingMode::CostBased`; `parse_scheduler_scheduling_mode` 认 `cost_based`。
- LoadBalance 软删: gateway 配置解析 `load_balance` → CacheAffinity + warn 日志; routing 层 `RoutingSchedulingMode::LoadBalance` 转换映射 CacheAffinity; 枚举保留（反序列化兼容）。

### R11 后端（配置面瘦身）
- R11-1/2: `resolve_routing_policy_simplified`（routing-core 新入口）——忽略 `allowed_models` 门槛与 `model_policies`（区分模型），规则/动作（供应商优先级、scheduling、header/body patch）保留; gateway resolver 全量切换到该入口。
- R11-4: `SetKeyPriority` 动作保留功能（旧配置不静默变行为），UI 不再产出。
- R11-5: admin 供应商列表按系统默认组 `SetProviderPriority` 升序排序（未配置落 i32::MAX 尾部，tie 按名），payload 输出 `priority`。
- R11-6: keys_grouped 页对 `api_formats` 空的 key 回退用供应商活动端点格式展示（不再隐形）。

## 验证

- `cargo check` 全绿（scheduler-core / routing-core / ai-serving / gateway lib）。
- 新增测试 16 个全绿: rate_limit_cooldown×6（分类映射/Retry-After 优先与拒绝/指数梯封顶/到期边界/payload 回读）、cost_based_and_signal×7（倍率排序/平级回落/亲和压成本/在途决胜/延迟低者胜/样本不足缺席/确定性）、simplified_resolution×3（allowlist 忽略/model_policies 忽略/规则仍生效）。
- `cargo fmt --all` 完成。
- 既有失败隔离: scheduler-core 2 个失败（codex_live_*）与 gateway lib test 156 个编译错误均为**用户并行 slim 改动的既有问题**（stash 验证: 无我的改动时反而 251 个错误——我的改动还修复了其中约 95 个）。clippy 因网络 TLS 无法安装组件，以 check+测试替代。

### R11-8 前端 UI 重设计 + R11-7 多组入口移除（第二批完成）
- `frontend/src/views/admin/RoutingProfiles.vue` 全量重写为单页形态: 调度模式三选一（缓存亲和[默认·推荐]/固定顺序/成本优先，各带一句话代价说明）、供应商拖拽排序（原生 HTML5 drag + ↑↓ 按钮，行首序号即调度顺序=故障转移顺序）、Key 行内展开（优先级数字输入 + 每格式倍率输入，倍率经 PUT key 即时保存）。
- 纯逻辑抽到 `frontend/src/features/routing/utils/schedulingStrategy.ts`（parse/build 往返、load_balance→cache_affinity 软删映射、系统默认组查找、顺序→优先级），vitest 8 例全绿。
- 保存: updateRoutingGroup + publishRoutingGroup 写回单份策略（`ui_provider_priority` 规则承载 set_provider_priority/set_key_priority 动作）; 无组时首次保存自动创建系统默认组。
- R11-7: 删除策略分组列表/新建/删除/绑定/试运行入口; 路由收敛为 `/admin/routing` 单路由（`routing/new`、`routing/:groupId` 移除）; 删除 5 个旧组件（RoutingGroupEditor/GroupList/ModelPolicyEditor/PriorityPolicyEditor/DryRunDialog）与旧 allowed-models spec。
- API 补充: `listAdminProviders`（GET /api/admin/providers，R11-5 的优先级排序列表）、`getEndpointKeysGroupedByFormat`（grouped-by-format，含 R11-6 回退）。
- Key 优先级持久化说明: slim 迁移已删 key 表优先级列，Key 优先级经单份策略 overlay（set_key_priority）持久化——比较器链 `routing_overlaid_candidate` 原生支持，语义与 R11-4 决策一致。
- 验证: `vue-tsc --noEmit` 零错误; vitest 全量 111 文件 654 测试全绿（含新增 8 例）; `vite build` 成功。

## 未完成（明确遗留）

- R11-7 策略单份化的存量多组归并迁移（resolver 已按"读默认组"运行，存量多组数据未归并）。
- PRD R8（上下文超限 fallback）/R9（尝试轨迹可视化）按计划不做。

> 归档订正（2026-09-02）：R11-8 前端 UI 与 R11-7 入口移除已于第二批完成（见上节）；gateway 侧 p0_failure_class_tests / latency_tracker tests 的编译阻塞已由 09-02 预存质量债修复解除，测试已生效。

## 本次触碰文件

crates: scheduler-core（health/selectability/lib/ranking×3）、routing-core（policy/actions/lib）、ai-serving（candidate_ranking）
gateway: orchestration（health/effects/mod）、scheduler（config/latency_tracker[新]/mod）、planner（candidate_ranking）、routing/resolver、execution_runtime×3、admin summary/list、public keys_grouped、provider_key_auth
