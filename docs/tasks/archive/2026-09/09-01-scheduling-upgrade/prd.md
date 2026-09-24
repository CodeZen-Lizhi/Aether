# 调度优化：融合 Bifrost/LiteLLM/one-api/Higress 精华

## Goal

在不推翻 Aether 现有调度底座的前提下，吸收四个开源网关的精华，补齐四块短板：错误三分类接入冷却/熔断、429 冷却期限、排序动态信号、成功率窗口熔断。**保留**现有作用域故障转移、流式预取门、半开探测、自适应 RPM、会话亲和、DB 持久化状态。

- 研究依据: `research/01-gateway-comparison.md`（Bifrost/LiteLLM/one-api/Higress/AIBrix 源码对比）
- 现状依据: `research/02-aether-current-state.md`（slim-personal @ 111929e72 全链路盘点）

## 约束（不可违背）

1. **正常路径行为不变**: CacheAffinity 会话粘性、prompt cache 命中、种子哈希的秒内稳定性（重试复用列表尾部）必须保持。
2. **状态写入仍走乐观锁围栏**（`ProviderKeyHealthStateUpdate` 的 `expected_*`），不引入新的写竞争面。
3. 单用户 slim 形态: 所有新增状态优先进程内存（含在途数、EWMA、成功率窗口），不强制 Redis；DB 只在 key 行既有 JSON 列上扩展。
4. 不引入 worker 池/请求队列（Aether 为 async tokio 直发模型）。
5. 错误透传原则保持: 全候选耗尽仍返回上游原始错误。

## Requirements（按实施批次）

### 批次一 P0（改动小、收益大，先做）

**R1 错误三分类接入冷却与熔断**（来源: Bifrost executeRequestWithRetries 的三分法 + LiteLLM per-exception 计数）

把失败划为三类，分别影响不同状态:
- `credential-dead` 凭证/配额死（401、403、insufficient_quota、oauth_invalid）→ **快速通道**: 1 次即开长熔断（复用 `project_local_key_circuit_open` 的硬阻断路径），OAuth 触发 ForceRefresh（已有）。
- `rate-limited` 限流（429，含限流语义的错误消息）→ **不计入**失败冷却窗口与熔断连败计数；只喂自适应 RPM 学习（已有路径）；触发 R2 的冷却期限。
- `transient` 瞬时（5xx、transport、超时）→ 维持现有 60s/8 败冷却 + 8 连败熔断计数。

改动点: `classifier.rs` 的分类结果把错误类传入 effects；`aether-scheduler-core/src/health.rs` 的 `project_local_key_circuit_failure` / `is_candidate_in_recent_failure_cooldown` 增加错误类维度。

**R2 429 冷却期限（Retry-After 落地）**（来源: Bifrost 429 池轮空 + one-api）

- 解析 429 响应的 `Retry-After` 头（秒数或 HTTP-date）；无该头时用指数试探（30s→1m→2m→4m，封顶 10m）。
- 写 key 级 `rate_limit_cooldown_until_unix_secs`；选择时未到期的 key 直接 skip（新 skip reason `key_rate_limit_cooldown`，进 `collect_selectable_enumerated_candidates_with_skip_reasons` 的跳过审计）。
- 到期自动回归，无需探测（限流不是故障）。

**R3 冷却/熔断中的 key 自动解除会话亲和**（现有机制闭环补丁）

- 绑定的候选进入 credential-dead 熔断或 rate-limit 冷却时，立即失效对应亲和缓存条目（扩展 `effects.rs:464` 的 `local_scheduler_affinity_matches_failed_target` 逻辑到这两个新场景），会话下次请求重绑到健康 key，避免反复打回冷却 key。

### 批次二 P1（动态化，第二步）

**R4 排序加"在途数"比较器**（来源: LiteLLM least-busy + Higress inflight 信号）

- 在排序链的种子哈希之前插入一档: 同健康等级下先比 key 级当前在途请求数（读 `ProviderPoolInFlightGuard` 现有计数，只读、不改写路径）。
- CacheAffinity 与 LoadBalance 两个模式都生效；FixedOrder 不动。

**R5 key/端点级延迟 EWMA 采集与排序**（来源: AIBrix 延迟建模简化版 + Higress feedback EWMA α=0.2）

- 非流式记总时延、流式记 TTFT；key 粒度内存 EWMA（5 分钟窗口语义，α=0.2），进程内存即可不入库。
- 作为排序在途数之后的第二动态信号；数据不足（样本 < 5）不参与比较，避免冷启动抖动。

**R6 成功率滚动窗口熔断**（来源: one-api MetricDisableChannel）

- 5 分钟窗口内成功率 < 20% 且样本 ≥ 10 → 开熔断（与现有 8 连败熔断取"或"），恢复复用现有半开探测（指数退避）。
- 进程内存环形计数即可（per (provider,endpoint,key,format) 桶）。

**R7 恢复爬坡防抖**（来源: LiteLLM 恢复思路的防抖改造）

- 熔断关闭后健康分从 0.75 起步（而非 1.0），连续 3 次成功才回满 1.0；恢复期再失败 → 立即重开熔断并按指数退避推进下一探测点。
- 改 `project_local_key_circuit_closed` / `project_local_failure_health`。

### 批次三 P2（可选，不做也成立）

**R10 成本优先（CostBased，新增调度模式；原名“经济模式”，应用户要求改名）+ 移除 LoadBalance（软删）**（用户决策: 场景仅 Codex/Claude Code 长会话，无无状态流量，LoadBalance 无用武之地；CostBased 对标 LiteLLM cost-based routing 的单模型特化版）

- 新增 `CostBased` 调度模式: 同请求模型内按 **Key 倍率升序**参与排序（替代优先级档位），倍率最低的候选优先服务；亲和粘性照旧（选中后粘住，缓存不碎）；故障转移复用"名单往下走"——最便宜失败换次便宜。
- **移除 `LoadBalance` 模式（软删）**: 删比较器分支与配置枚举; 反序列化兼容——旧配置/历史 routing_groups 中存量的 `"load_balance"` 值读取时自动映射为 `cache_affinity` 并打日志，不报错不迁移。
- 前提（用户确认成立）: 请求必带统一模型，所有候选服务同一模型，倍率即唯一成本依据，无需"倍率×底价"换算。
- 倍率相同: 回落该 routing group 配置的优先级，再相同回落秒级哈希（确定性不变）。
- R4/R5 动态信号（在途数/延迟）档位插入 CacheAffinity 与 CostBased 两个模式的链（亲和仍压在最前），不再为 LoadBalance 保留插入点。
- 预期行为: 最便宜 key 吃满流量，429 冷却/RPM 学习自动轮休时流量去次便宜——稳态成本最低且无需人工调权; 便宜但不稳的 key 由健康分档位（排在成本之后）自然压后。
- 依赖: Key 倍率字段（见需求优化清单"Key 倍率计费"项）落地后排序可读; 模式骨架与比较器档位可先行实现、倍率缺失时退化为回退排序（同优先级缺失行为）。

**R8 上下文超限 fallback**（来源: LiteLLM context_window_fallbacks）: 识别 context_length_exceeded 类错误，自动尝试映射到更大上下文窗口的候选模型映射。个人网关价值有限。

**R9 尝试轨迹可视化**: `request_candidates` 表已有全量尝试记录（最近 128 条已被调度读取），补 admin 查询端点展示 per-request 的候选排名、skip reason、失败作用域链。纯展示，无调度逻辑。

### R11 调度配置面瘦身（用户逐条拍板，全部为 UI/配置维度删减，不动排序机制）

**R11-1 删"区分模型"维度**: 调度策略只留"统一"维度（全局一套策略）。routing_groups 的 per-model policies（`matching_model_policies` 匹配循环）从配置面移除; 模型映射（候选池圈定，SQL join）**不受影响**——删的是"每模型策略差异"，不是"每模型候选池"。
**R11-2 删模型白名单（RestrictModels）**: routing_groups 的 `RestrictModels` action 从配置面移除。单用户自用无治理需求。
**R11-3 删优先级模式的 GlobalKey 档**: 调度策略页"优先级模式"只留/只展示"供应商"一种——该选项本身从 UI 消失（单一选项无展示意义）。`RoutingSetPriorityMode::GlobalKey` 枚举软删（保留反序列化兼容，UI 入口移除）。排序心智定稿: 先按供应商优先级 → 供应商内部按 Key 优先级（现状 Provider 分支默认行为）。
**R11-4 删调度策略页的 Key 优先级设置入口**: `SetKeyPriority` overlay 的 UI 移除; **Key 优先级字段保留且继续参与排序**，入口回归供应商管理添加/编辑 Key 处设置（Key 管理页展示保留）。
**R11-5 供应商列表按调度优先级同步重排展示**: 供应商管理列表排序与调度优先级一致（现按 id/创建时间脱节）。
**R11-6（顺手修）Key 列表 grouped-by-format 展示不全**: `api_formats` 为空的 key 当前被整把跳过（`keys_grouped.rs:119`）——对这类 key 按其供应商端点的格式回退展示，解决"只显示部分供应商 Key"。注意: 该字段为空不影响调度（调度走 SQL join 不看此字段），纯展示缺陷。

**R11-7 调度策略全局唯一（single instance）**: 现状 `routing_groups` 为多组设计（表支持任意多行 + `is_system_default` 默认组 + `routing_group_bindings` 主体绑定 + `routing_group_versions` 版本历史）。单用户场景砍掉多组心智: **全局只留一份策略**——多组 CRUD、组间绑定/切换、组选择器 UI 全部移除; 保留单一系统默认组（DB 表结构不动，逻辑上按"只有一行"运行）; 现存多余组做一次性归并/停用迁移。策略版本历史表保留（发布/回滚能力不丢）。

**R11-8 调度配置 UI 整体重设计**: 调度策略页按 R11 全部删减后的最终形态重画，目标是一个单页、无嵌套、无切换的配置体验:

```
┌─ 调度策略（全局唯一一份）─────────────────────────┐
│                                                    │
│  调度模式    ( ) 缓存亲和（默认·推荐·保会话缓存）      │
│             ( ) 固定顺序（严格主备）                  │
│             ( ) 成本优先（倍率低者优先·需 Key 倍率）    │
│                                                    │
│  供应商优先级（拖拽排序，即调度顺序 = 故障转移顺序）      │
│   ① 中转站A   [key×3 ▸]  ←行内展开该供应商 Key        │
│   ② Claude官方 [key×1 ▾]      及其 Key 优先级设置       │
│   ③ 备用中转B  [key×2 ▸]                            │
│                                                    │
│  （已删维度: 区分模型/模型白名单/优先级模式/Key级覆盖/   │
│    负载均衡/多策略组——入口全部不存在）                  │
└────────────────────────────────────────────────────┘
```

- 三个交互原则: ①供应商拖拽排序与 R11-5 的列表同步展示同一数据源; ②Key 行内展开设置 Key 优先级（R11-4 的入口回归处，含倍率展示——为 成本优先模式铺垫）; ③调度模式切换时给一句话说明各自代价（固定顺序=无缓存亲和; 经济=省钱但最便宜 key 先吃满）。
- 已删配置的兼容: 旧数据里的多余组/绑定/Key overlay 读取时静默忽略并日志提示（与 R11 验收标准一致），不迁移不报错。

调度策略页最终形态: **一份策略 = 一个调度模式（三选一）+ 一套供应商优先级（含 Key 优先级/倍率内嵌）**。

调度策略页最终形态: **一套供应商优先级 + 一个调度模式三选一（FixedOrder/CacheAffinity/CostBased）**。

### 明确不做（有意排除）

- Bifrost 式 worker 池 + 有界队列 + 退避占坑（async 直发无此问题）
- KV/前缀缓存打分、LoRA 亲和（自托管 vLLM 集群场景，不适用）
- 加权随机（被"种子哈希 + 健康分 + 动态信号"替代，语义更稳）
- 主动健康检查探测线程（现有半开探测以真实流量探测，比合成探测请求更真实且零成本）
- 流式提交后重放/续传（协议所限，全行业未解）

## Acceptance Criteria

### P0
- [ ] 模拟上游返回 401: 该 key **第 1 次**失败即进入熔断（不再需要 8 次），熔断信息含 `credential-dead` 类标记；同格式后续请求直接跳过（`key_circuit_open`）。
- [ ] 模拟上游返回 429 + `Retry-After: 30`: 该 key 在 30 秒内的所有选择被跳过（skip reason = `key_rate_limit_cooldown`），30 秒后自动恢复参选；期间 429 未计入失败冷却计数、未推进熔断连败计数。
- [ ] 模拟上游返回 429 无 Retry-After: 按 30s→1m→2m 指数试探，连续 429 时退避递增，成功后清零。
- [ ] 429 后，绑定该 key 的会话亲和条目失效，下次请求绑定其他健康 key（无重复打回）。
- [ ] 5xx 失败行为与现状完全一致（60s/8 败冷却、8 连败熔断、指数探测恢复）。
- [ ] 全候选耗尽仍透传上游原始错误体（现有 Deferred 路径回归通过）。

### P1
- [ ] 两个健康等级相同的 key，在途 0 与在途 5 并存时，新请求优先选在途 0 的 key。
- [ ] 某 key 近期平均延迟显著高于同级（EWMA 差距 > 2 倍且样本充足）时，排序稳定落在同级尾部；样本 < 5 的 key 不受影响。
- [ ] 成功/失败交替的 key（如 1 成 1 败循环）在 5 分钟内成功率 < 20% 且样本 ≥ 10 时进入熔断。
- [ ] 熔断恢复后首个请求失败 → 立即重开熔断且探测间隔按指数推进；恢复后连续 3 次成功 → 健康分回 1.0。
- [ ] 排序仍保持秒内确定性（同一秒内同请求重试得到相同的候选顺序，除动态信号变化外）。

### P2
- [ ] 成本优先模式: 同模型下倍率升序排序生效（倍率 0.4 的 key 排在 1.0 之前）; 选中后亲和粘住; 失败换次便宜的 key。
- [ ] 倍率相同回落组优先级，再回落哈希; 未配置倍率的 key 在 成本优先模式下退化为回退排序，不 panic 不缺席。
- [ ] LoadBalance 软删: 枚举与比较器分支移除后，全量测试无引用残留; 反序列化含 `"load_balance"` 的旧配置映射为 `cache_affinity` 且有日志，不报错。
- [ ] FixedOrder/CacheAffinity 回归不受影响（两种既有模式行为不变）。

### R11（配置面瘦身）
- [ ] 调度策略页不再出现"区分模型"维度、模型白名单、优先级模式选项、Key 优先级设置（四个入口全部移除）。
- [ ] 旧 routing_groups 配置含 per-model policy / RestrictModels / GlobalKey / SetKeyPriority 时，读取不报错，行为回落统一策略 + 供应商优先级模式 + Key 管理字段。
- [ ] 供应商管理列表按调度优先级排序展示，与调度实际顺序一致。
- [ ] `api_formats` 为空的 key 在 grouped-by-format 页面按供应商端点格式回退展示，不再隐形。
- [ ] Key 管理处设置的 key 优先级在排序中生效（同供应商内低值优先）——回归确认。
- [ ] 策略单份化: 不存在多组 CRUD/绑定/切换入口; 现存多组数据归并或停用后调度行为不变; 版本发布/回滚可用。
- [ ] 新 UI: 单页完成"模式三选一 + 供应商拖拽排序 + Key 行内展开设置优先级/倍率"全部操作; 模式切换展示代价说明; 与供应商列表展示同一数据源（改一处两处同步）。

### 回归（所有批次后）
- [ ] CacheAffinity 正常路径: 同会话连续请求命中同一 key，prompt cache 亲和不回退。
- [ ] 既有测试全绿; 新增单测覆盖: 错误三分类映射、Retry-After 解析（秒/HTTP-date/缺失）、冷却到期判断、成功率窗口统计、EWMA 更新、恢复爬坡状态机。

## Notes

- 实施顺序建议: P0-1 → P0-2 → P0-3（同一批回归）→ P1-4 → P1-6（状态类）→ P1-5（采集类）→ P1-7。
- 每批次独立提交，便于回滚；P1 的两个动态信号（R4/R5）各自带开关（system_config），默认可先开 R4（纯读），R5 采集先行、观察后启用排序。
