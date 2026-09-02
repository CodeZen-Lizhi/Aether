# 开源 AI 网关调度与故障转移横向对比研究

> 任务: 09-01-scheduling-upgrade · 调研日期: 2026-09-01 · 方法: 逐项目阅读 GitHub 源码（非文档转述）

## 结论先行

没有全能冠军，分层看:

1. **多 provider 转发的策略完备度冠军: LiteLLM** —— "错误分类 → allowed_fails 分桶计数 → 失败率阈值 → 冷却 → 重试时重选健康部署"是完整可调参的策略链。
2. **熔断生命周期完整性冠军: one-api** —— 唯一同时做到"自动禁用 + 主动探测恢复"的项目（禁用: 单次硬错误/成功率窗口；恢复: 定时 testAllChannels 探测成功才重新启用）。
3. **动态选路算法冠军: Higress ai-endpoint-picker / AIBrix** —— 多信号打分和延迟建模高一个量级，但限定自托管同构集群（vLLM 端点），不是多 provider 故障转移。
4. **单请求内工程精度冠军: Bifrost** —— key 轮换语义、尝试轨迹审计、SSE 首块错误处理最细；但跨请求状态为零。

对 Aether 的启示: Bifrost 的错误分类精度 + LiteLLM 的冷却策略 + one-api 的成功率熔断 + Higress 的动态信号思想，补进 Aether 已有的底座（见 02-aether-current-state.md）。

## 一、Bifrost (maximhq/bifrost, Go) —— 单请求内的工匠

### 请求全链路

```
Transport → handleRequest/handleStreamRequest (core/bifrost.go:5196/5346)
  ├─ 1. PreRequestHooks: routing 插件 CEL 规则 → governance.LoadBalanceProvider 加权随机选 provider
  │      + 按权重降序自动生成 fallback 链 (plugins/governance/main.go:366)
  ├─ 2. tryRequest → RunLLMPreHooks (semanticcache 在此短路命中缓存)
  ├─ 3. 入 ProviderQueue (每 provider 有界 channel + 固定 worker 池) bifrost.go:5604
  ├─ 4. requestWorker → 构建 key 池 (bifrost.go:6791)
  ├─ 5. executeRequestWithRetries: 选 key → 失败分类 → 换 key/退避重试 (bifrost.go:6074)
  ├─ 6. RunPostLLMHooks (governance 记账)
  └─ 7. 主 provider 失败 → 顺序试 fallback (bifrost.go:5283-5335)
```

### 调度分层（初始选路）

- 第 1 层: routing 插件，CEL 表达式匹配（模型/请求类型/预算限流状态/复杂度分级），rule 内 target 带权重再加权随机，可级联（max depth 防环）。
- 第 2 层: governance `LoadBalanceProvider`（plugins/governance/main.go:366）——VK provider configs，过滤链: 黑名单 > 白名单 > 预算违规 > 限流违规，剩余加权随机；**选中后把剩余 provider 按权重降序自动生成 fallback 链**（负载均衡与故障转移同一份配置驱动）。
- 第 3 层: key 级加权随机（core/keyselectors/weightedrandom.go，权重全 0 退化为均匀随机）。
- 所有层都是**静态权重随机**，不感知延迟/并发/错误率。

### 重试与 key 轮换（executeRequestWithRetries, bifrost.go:6074）——精细度最高

| 错误 | 判定 | 动作 |
|---|---|---|
| 401/402/403 | per-key 永久失败 | 进 `deadKeyIDs`（请求内永不复用），换 key，**跳过退避** |
| 429（含消息文本启发式 `IsRateLimitErrorMessage`） | per-key 暂时失败 | 进 `usedKeyIDs` 轮空，换 key，**保留退避**（账号级限额跨 key 共享）；池轮空后重置 usedKeyIDs 重开一轮 |
| 5xx/网络 | 瞬时失败 | 同 key 重试 |
| 上游拒绝回放加密 reasoning | 特例 fail-soft | 剥掉 encrypted_content 额外给一次（每请求最多一次） |

退避: `initial * 2^attempt` + 20% 抖动封顶（core/utils.go:147）。跳过退避需确认真换了 key（比较 key ID 防 fixed-key 误判）。全 key 死亡 → 502 `upstream_credentials_exhausted`；全被过滤 → 503。

### 熔断: 没有传统熔断器

全库无失败计数/冷却期/半开探测/健康检查。近似物: ① governance 预算/限流排除（窗口重置即恢复，只覆盖预算限流两类故障）；② per-provider 队列隔离 + DropExcessRequests 负载脱落；③ 请求内 deadKeyIDs（per-request，不跨请求）。**持续 5xx 的上游每个新请求都要先撞一次失败。**

### 流式

首字节前/SSE 首块前可 fallback（`CheckFirstStreamChunkForError` bifrost.go:6460，回归 issue #4788）；提交后断流为终局，无 buffer-replay。

### 优势

- 错误分类与 key 轮换语义同类最细，每条有回归测试对应真实 issue。
- 可观测性顶级: fallback/retry/key-selection 全程 span；`KeyAttemptRecord` 轨迹（哪个 key、失败原因、是否触发轮换）；routing engine log 审计每层决策。
- 负载均衡与 fallback 一体化；并发隔离/优雅 drain/provider 热更新透明重路由扎实；对象池复用。

### 劣势

- 无熔断器/健康探测/跨请求失败记忆（最大缺口）。
- 静态加权随机；fallback 链静态；无 hedging/投机并行。
- 重试退避 sleep 在 worker 内占并发槽位 → 大面积 429 时 worker 被 sleep 占满，靠 DropExcessRequests 兜底。
- 9086 行 bifrost.go，handleRequest/handleStreamRequest 两份近似复制的编排，prepareFallbackRequest 12 路 switch。
- 429 靠消息文本匹配兜底，脆弱。

## 二、LiteLLM (BerriAI/litellm, Python) —— 策略模型的设计师

### 冷却（litellm/router_utils/cooldown_handlers.py）

- 触发分层: 429 立即冷却（单部署组除外）；**`percent_fails > DEFAULT_FAILURE_THRESHOLD_PERCENT` 且分钟内流量 ≥ 最低样本数**才冷却（防偶发误杀）；401/404 等不可重试错误冷却；`APIConnectionError` 不冷却。
- `allowed_fails` 按**异常类型分桶独立计数**（`cache_key = deployment:exception类名`，generic 用 "generic"），计数器 TTL = cooldown_time，窗口与冷却合一，过期自动清零。
- 恢复**纯 TTL 过期**，无半开探测、无灰度。状态默认进程内存，可选 Redis 共享。

### 路由策略（litellm/types/router.py + router.py）

6 种: simple-shuffle（默认）/ least-busy / latency-based / usage-based(-v2) / cost-based / provider-budget-routing。`async_get_available_deployment`（router.py:11985）: pre-routing hook → 过滤健康部署 → 按策略选择。TPM/RPM 计数按分钟桶进 RouterCache。

### 重试编排（router.py:7154 `async_function_with_retries`）

每次重试先重新拉取健康部署列表（排除冷却中），再按策略选下一个——重试天然换部署。Per-exception、per-model-group 的 RetryPolicy；特有 fallback: context_window_fallbacks（上下文超限换小模型）、content_policy_fallbacks；加密内容亲和 pin。

### 优劣

优: 跨请求策略链最完整，per-exception/per-group 可调参，动态策略多。劣: Python 性能；恢复无探测；冷却状态分布式要靠 Redis；无主动健康检查。

## 三、one-api (songquanpeng/one-api, Go) —— 熔断闭环的实践者

### 禁用（monitor/manage.go）

- 单次硬错误即禁用整个 channel: 401 / `insufficient_quota` / `authentication_error` / 消息含 "credit"、"balance"、"已欠费" 等关键词（大小写不敏感）。
- `MetricDisableChannel`: **滚动窗口成功率低于阈值禁用**（`config.MetricSuccessRateThreshold` + `config.MetricQueueSize` 窗口）——朴素被动熔断器。

### 恢复（ShouldEnableChannel + 定时 testAllChannels）

测试请求**传输层和 API 层都成功**才重新启用（`err == nil && openAIErr == nil`），配合定时探测——等价半开探测。是 surveyed 项目中唯一"禁用+主动探测恢复"闭环。

### 重试（controller/relay.go）

429/5xx/其他错误重试（400 不重试），每次从缓存重选 channel 并跳过刚失败的；重试次数用尽把 429 改写为"当前分组上游负载已饱和"。代码内自注释 `BUG: bizErr is in race condition`。

### 选路

分组内优先级+权重随机（CacheGetRandomSatisfiedChannel），静态。

## 四、Higress ai-endpoint-picker (alibaba/higress, Go/Wasm) —— 打分算法最先进（限定自托管集群）

`Filter → Normalize → Score → Pick → Feedback` 流水线，六信号各归一化 [0,1] 加权（README_EN 权重默认: queue=2, kvCache=2, prefixCache=3, loraAffinity=0, inflight=1, failure=0）:

- 上游队列深度、KV cache 使用率（vllm:kv_cache_usage_perc）
- **近似前缀缓存**: prompt 语义分段 hash 链估算端点已观察的最长前缀（`0.75 × matched/total + 0.25 × min(matched/8192,1)²`），LRU 容量按 KV block 计
- LoRA 亲和、网关本地在途数
- 失败 EWMA（`feedback.ewmaAlpha: 0.2`，流完成后记 TTFT/总时延/失败 EWMA，per-request lease 防重复计数）

设计哲学克制: 缺信号只减分不剔除（fail-open 到 Envoy 默认 LB）；指标快照容忍 250ms 陈旧；近似误判不影响正确性（引擎侧校验真实 KV 命中）。健康与故障转移交给 Envoy（主动健康检查 + outlier detection）。

## 五、AIBrix (vllm-project/aibrix, Go) —— 延迟建模学术派（自托管 pod 级）

`algorithms/least_latency.go`:

```
score = 队列时间 + (prefill均值/平均prompt tokens × 预估prompt tokens)
              + (decode均值/平均生成tokens × 预估生成tokens)
```

用请求预估 token 数把直方图均值换算成"该请求在此 pod 上的预期总时长"；缺数据的 pod 直接排除不瞎猜；per-pod token 缺失回退全 pod 均值。另有 least_kv_cache / prefix_cache / least_busy_time / least_request / load_balance 等 20+ 算法族与 fallback.go。K8s 就绪即恢复。

## 六、简要提及（本轮未深挖源码）

- **TensorZero** (Rust): variant（模板+模型组合）编排框架，best-of-N 候选、per-variant retries，重心在实验评估而非 provider 池故障转移。
- **Portkey Gateway** (JS): 声明式配置（fallback/loadbalance strategy + on_status_codes + weight），无服务端健康状态。
- **Envoy AI Gateway**: 熔断继承 Envoy outlier detection + circuit_breakers，原语最成熟但非 LLM 错误感知。
- **production-stack / SGLang router**: 自托管 vLLM 的 session/KV/负载感知路由（WebFetch 两次超时未验证源码，结论仅作参考）。

## 七、对 Aether 的取舍（映射到 PRD）

| 来源 | 采纳什么 | 落到 PRD |
|---|---|---|
| Bifrost | 错误三分法（永久/暂时 per-key/瞬时）、429 池轮空思想、SSE 首块错误检查 | P0-1 错误三分类、P0-2 Retry-After 冷却（Aether 的 StreamCommitGate 已覆盖首块检查） |
| LiteLLM | per-exception 分桶计数、失败率阈值+最小样本、恢复防抖 | P0-1、P1-6 成功率窗口、P1-7 恢复爬坡 |
| one-api | 单次硬错误禁用、成功率窗口禁用、主动探测恢复 | P0-1 凭证类快速熔断、P1-6（Aether 已有半开探测，只补窗口触发条件） |
| Higress/AIBrix | 在途数与延迟 EWMA 作为排序信号（简化为两信号，不搬 6 信号全套） | P1-4 在途数比较器、P1-5 延迟 EWMA |
| 不抄 | Bifrost worker 池+队列（Aether async 无此问题）；KV/前缀缓存打分（自托管场景）；加权随机（被种子哈希+动态信号替代） | — |
