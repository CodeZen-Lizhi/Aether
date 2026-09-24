# 故障转移方案完整性检查

记录日期：2026-09-13。状态：只读代码核查与规划讨论稿，不是已验证的运行结果或最终实施方案。

本记录保留阶段性建议与证据，后续决定以 [最终 PRD](../prd.md)、[技术设计](../design.md) 和 [实施计划](../implement.md) 为准。R1-R23 已逐项确认，包含最新接受的有状态续接边界；下文“待确认”描述记录当时状态，不构成新的阻塞问题。本版技术默认随整体规划审阅。

## 结论

已确认请求内有限重试与健康熔断互不绑定，以及 10 分制的连续加重和成功恢复公式。核心可实现，但还不足以覆盖整个聊天故障转移。主要缺口是错误分类、重试配置语义、熔断恢复、亲和迁移和有状态 Responses 续接。

没有必要根据“8 次乘 30 秒等于 4 分钟”的假设重做调度：当前切换候选不以耗尽健康为前提。错误分类与基本恢复规则现已收敛；请求内等待参数等尚待最终规划，不能把讨论中的例子当成现有行为。

## 已核实的代码事实

下列路径相对项目根目录，函数名用于后续定向研究。

| 范围 | 现状 | 证据 |
|---|---|---|
| 健康预算 | 阈值 8，普通步长 1/8，429 排除；成功时 consecutive_failures 减 1 而非清零，并非严格连续计数 | apps/aether-gateway/src/orchestration/health.rs: project_local_failure_health, project_local_success_health |
| 其他熔断触发 | 5 分钟窗口至少 10 样本且成功率低于 20% 可熔断；恢复期失败可立即重新熔断 | 同文件 project_local_key_circuit_failure_with_success_rate |
| 探测恢复 | 从 0.25 起步，后续三次成功回到 1，与新 +1/10 规则不能直接混合 | 同文件 CIRCUIT_RAMP_INITIAL_HEALTH, CIRCUIT_RAMP_REQUIRED_SUCCESSES |
| 失败投影 | 429 冷却并清亲和；凭据失效类可立即熔断；部分格式有额外的健康作用域过滤 | apps/aether-gateway/src/orchestration/effects.rs: record_health_failure_effect, local_candidate_failure_should_apply_key_effects |
| 尝试配置 | failover_rules.max_retries 优先，其次 endpoint/provider 字段；后两者的值 2 视为旧默认忽略；无明确配置为 1，总槽位限制为 1..99 | apps/aether-gateway/src/orchestration/attempt.rs: local_attempt_slots_from_transport |
| 前端配置 | 提供商表单“最大重试次数”直接写 provider 字段；不能从标签推断实际总尝试数 | frontend/src/features/providers/components/ProviderFormDialog.vue |
| 候选执行 | Responded 与 Retry 分开，Retry 可选择 candidate/credential/endpoint/provider 跳过范围，不要求先健康归零 | apps/aether-gateway/src/executor/candidate_loop.rs；execution_runtime/sync/execution.rs |
| 格式差异 | claude:messages 的 429 默认换 credential，5xx 换 endpoint；仅增加尝试槽位不保证同 K 重试 | apps/aether-gateway/src/orchestration/classifier.rs: classify_anthropic_failure_disposition |
| 流式超时 | watchdog 默认 30000 ms，计划 first_byte_ms 覆盖；获取准入许可之后才启动计时，终态处理开始后可继续等待 | apps/aether-gateway/src/executor/candidate_loop.rs: resolve_stream_candidate_watchdog_timeout, execute_stream_candidate_with_watchdog |
| 本地过滤 | 并发、额度、OAuth 无效、熔断、冷却、探测占用有对应 skip reason；已到探测时间的零分 K 有例外 | crates/aether-scheduler-core/src/candidate/selectability.rs |
| 亲和 | 部分可重试错误清缓存；cache affinity 成功才记住目标；失败目标匹配可扩大到同 provider+endpoint，不限精确 key | apps/aether-gateway/src/orchestration/effects.rs: record_attempt_failure_effect, local_scheduler_affinity_matches_failed_target, remember_successful_local_scheduler_affinity |
| 排序 | fixed_order 不比较动态健康/并发/延迟；cost_based 比较 rate multiplier，不代表实际 token 费用最小 | crates/aether-scheduler-core/src/ranking/modes.rs；apps/aether-gateway/src/ai_serving/planner/candidate_ranking.rs |
| 健康持久化 | 已有锁和 CAS，成功有写入节流，需要核实分数未满时是否会漏掉一次回血 | apps/aether-gateway/src/orchestration/effects.rs: record_health_success_effect, provider_key_health_success_persist_gate_allows |
| Responses WS | 独立 response.create 与带 previous_response_id 的续接不同；续接校验原候选、所有权和物理绑定，不得把原 ID 任意发给另一个 K | docs/operations/codex-responses-websocket-probe.md；docs/WebSocket-Mode.md；apps/aether-gateway/src/handlers/proxy/websocket/responses/continuation.rs, binding.rs, connection.rs, upstream.rs |

## 纠正早期审查

1. 8 次是旧失败预算，不等于单请求强制重试 8 次；成功仅退还一个预算，前文“连续失败”的说法过度简化。
2. 4 分钟是假设等待健康耗尽的算例，不是现有网关已测得时长。
3. 先前把动态信号关闭直接列为 P1，证据不足。fixed_order 本就不消费动态信号；planner、routing policy、minimal scheduler 职责有差别。本轮不预设打开所有信号，更不能因此改变人工排序。
4. 先前以 stream helper 没用 plan_kind 推断视频取消/删除重试为 P1，未证明对应流式入口可达，不能作为已确认缺陷或本轮修改依据。
5. “只影响当前 K”是用户目标，不代表现有全部 failure scope 和亲和解绑逻辑已经满足。
6. 先前 cargo 命令未取得可核验最终结果，不能称测试通过；本轮只读核查并写文档，不跑新的编译或付费模型请求。

## 错误分类设计

429/上游并发满、503/504 基础 1 分及暂不增加基础 3 分档已经由用户确认。用户随后接受助手设计具体分类，要求暂态故障轻扣、明确不可用重扣，并明确允许余额不足重扣。以下是据此收敛的设计，不代表已实现。先确定失败归因，再决定扣分；HTTP 状态码不直接等同于真实根因。

| 分类 | 建议 | 边界 |
|---|---|---|
| 中转 503/504 | 已确认基础 1 分，按配置有限重试 | 503 是暂时过载/维护，504 是网关等待上游超时；正文明确指向其他原因时按实际归因；用户终止规则仍优先 |
| 中转 500/502 | 基础 2 分，按配置有限重试 | 未知内部异常/无效上游响应归为较重故障，但不据此判定永久失效。若可信的结构化错误明确表示暂时限流/容量不足，按暂态规则基础扣 1 分 |
| 等待模型开始输出超时 | 基础 1 分 | 不把正常慢推理直接当严重故障；超时口径和每次尝试配置另行确定 |
| 可归因上游的 DNS/TLS/连接拒绝、协议损坏、无效成功响应或异常断流 | 基础 2 分 | 排除网关/本地配置/公共网络故障及客户端取消；异常 200 要有协议或明确匹配证据。已交付有效内容的断流只结算，不透明重放；单纯等待超时仍基础扣 1 分 |
| 429 / 上游并发满 | 已确认：基础 1 分并配合退避/冷却，计入连续失败加重 | 持续容量不足也会消耗健康并归零；冷却时无实际调用不扣分，具体冷却参数待定；取代旧实现的“429 不扣分”行为 |
| 中转 401/402/403/无效 K 字样，根因不确定 | 基础 2 分并按配置有限重试 | 不因单次不确定的中转状态码直接熔断，也不能放宽调用者鉴权；可证实的凭据失效仍按专门路径处理 |
| 上游明确证明当前 K 余额/额度耗尽、凭据不可用 | 直接置零，停止普通尝试并进入可恢复熔断 | 必须同时确认错误类型与目标归属；不把中转内部某线路余额耗尽当成当前 K 永久失效，不自动扣其他 K 的分。OAuth 可刷新的到期先走现有有界刷新路径，不能仅凭到期判定无效 |
| 本地已知凭据无效、额度耗尽 | 硬过滤或专门凭据恢复，不靠普通扣分耗尽 | OAuth 刷新与模型尝试预算需明确，不能无限刷新重试 |
| 请求参数错误、客户端取消、本地并发过滤 | 不扣分、不增加连续计数、不回血 | 取消不能被 watchdog 再记成上游超时；中性事件不能被算成完整成功 |

普通 1/2 分故障都计入连续失败附加分；明确不可用直接置零，不再为凑满次数发请求。未知 5xx/传输失败默认基础 2 分；未知 4xx 不盲目套用服务故障扣分或重试，先依据已有协议分类与用户终止规则处理，并记录未识别归因。不能只实现分数，让重试和健康各自猜测根因。

识别顺序：本地错误与取消排除 → 经过供应商适配、可确认作用域的结构化错误码 → 已验证的供应商错误模板 → 状态码/传输类型兜底。只解析错误响应或协议错误事件，不扫描正常模型文本中的“余额不足”。结构化字段仍需适配语义，不能假设所有中转都遵循同一错误码定义。恢复规则见 PRD R17，间隔上限与旧触发器处置仍见 D3。

### 基础 3 分与 503/504 的讨论

用户已接受 503/504 基础 1 分、暂不增加基础 3 分档；随后追问 500/502、上游连接和 TLS 故障是否适合基础 2 分，并将具体错误分类交由助手设计。采用上表的 1/2 分与明确不可用直接置零规则；之后已接受 PRD R17 的基本恢复规则。

前文基础 3 分是说明不同扣分的算例，未定义可执行的严重错误分类。助手当前倾向先使用基础 1/2 两档，不为了凑齐档位而新增模糊的“严重”类别；已证实不可用的目标仍走明确过滤/恢复流程。若增加基础 3 分，必须给出可识别且不会与连续加重重复计算的判据。

按 [RFC 9110 15.6.4](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.6.4)，503 表示暂时过载或维护；按 [15.6.5](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.6.5)，504 表示网关未及时收到上游响应。标准没有规定健康扣分；这两个状态码都不能单独证明 K 已失效。建议的基础 1 分体现对偶发暂态错误的容忍，不是所有情况下都仅扣 1 分，也不是让当前请求坚持用这个 K 到归零。

满分 10 且连续同类计分失败、无回血时，既定附加分会使不同基础分得到如下结果：

| 基础分 | 实际扣分至归零 | 第几次归零 |
|---|---|---|
| 1 | 1、1、2、2、3、3 | 6 |
| 2 | 2、2、3、3 | 4 |
| 3 | 3、3、4 | 3 |

因此基础 3 加上现有连续加重，会把同 K 的三次真实失败变成一次熔断；这属于容忍度选择，不能声称是 HTTP 标准要求或已证明的最优参数。

按 [RFC 9110 15.6.1](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.6.1)，500 表示服务遇到意外情况无法完成请求；按 [15.6.3](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.6.3)，502 表示网关收到无效上游响应。将它们设置基础 2 分，是本项目拟议的容错权重，不是标准规定的严重级别，也不证明它们一定比 503/504 更难恢复。

## 必须补齐的执行契约

### 请求与健康计次

建议一次真实上游 attempt 对应一次终态计分。同一逻辑请求里两次真实调用失败，扣两次；候选汇总、异步重复报告不得重复扣分。总尝试数、追加重试数、候选数要明确命名。配置 2 的目标是用户理解的两次总尝试，旧配置兼容须审阅。

并发结果按同作用域串行提交更新，复用现有锁/CAS。考虑重复事件、迟到旧成功覆盖新熔断、凭据轮换前结果污染新凭据。严格连续计数在计分失败时增加、完整成功清零，中性事件不回血。

### 冷却与熔断恢复

零分不能永久过滤，保留正常、熔断、半开探测的状态差别。探测需要独占和租约释放。已接受首次 1 分钟后允许探测、失败翻倍且受配置上限约束、完整探测成功恢复 1 分并按成功 +1 回升，以及确认充值/更新凭据后提前允许探测。到期只授予探测资格，不承诺准点后台调用；实际恢复到被检测之间可能有延迟，普通请求使用其他可用 K。恢复资格不等于强制切回，仍遵循各模式排序与亲和约束。

固定 8 次、成功率窗口、旧快速恢复不能未经讨论叠加到新公式上。若移除它们，必须明确新分数模型如何处理交替成功失败的抖动。

429 已确认扣分；冷却中的 K 不能被当前请求立刻反复调用。需决定等待到期且预算允许再试，或本次跳过。长 Retry-After 不能无限挂起请求，也不能截短后宣称已遵守上游要求。

定向补查：`crates/aether-scheduler-core/src/health.rs:682` 的 `ProviderKeyRateLimitCooldown::project` 在缺少可用 Retry-After 时，从 30 秒翻倍至 600 秒；它还将超过 600 秒的上游值退回本地阶梯。当前实现不能作为新策略已支持快速同 K 重试、完整尊重长冷却的证据。用户已接受 PRD R19：逻辑请求在同一 K 的重试额外等待累计上限 2 秒，超出就转下一候选，但保留 K 完整冷却；上游未提供时间时的退避参数仍需最终定稿。

总预算已确认于 PRD R20：可配置默认 90 秒，限定流式聊天首输出前。`apps/aether-gateway/src/executor/candidate_loop.rs:1303` 的 watchdog 读取 first_byte_ms；同文件 `stream_candidate_watchdog_ignores_total_timeout_for_stream_upstream` 等测试表明不能把 total_ms 当成该 watchdog 已执行的跨候选总时限。`apps/aether-gateway/src/handlers/proxy/websocket/responses/upstream.rs:29` 的握手使用 first_byte_ms/total_ms/30 秒中的较小值，但这同样不能证明整个模型轮次有统一总预算。实施时应在逻辑请求范围贯穿单调时钟截止时间，不能每换 K 或开始重试就重置；全局预算耗尽取消不能重复按独立上游超时扣分，未尝试的 K 不扣分。

### 排序与亲和

排序决定首次选谁和故障后的候选顺序；健康分不能把 fixed_order 隐性改成负载均衡。建议临时失败先保留亲和，耗尽同 K 尝试后允许备用候选；备用 K 完整成功后迁移本会话亲和，人工排序不因此迁移。该迁移规则尚待确认。

成本模式已确认 PRD R22：沿用已配置 K 倍率，在同等模型/能力/兼容条件下低倍率优先，仅同倍率优先亲和，不宣称实际账单最低，不默认给所有模式打开动态重排。代码入口 `apps/aether-gateway/src/ai_serving/planner/candidate_ranking.rs:147` 读取 default_rate_multiplier；`crates/aether-scheduler-core/src/ranking/modes.rs:34` 当前比较器把 cached_affinity_match 放在倍率前。应同时验证实际亲和数据来源、比较次序和完整成功后的写入，不能仅以比较器单测证明真实排序生效。

### HTTP 与 WebSocket

分别核验 HTTP 响应提交前、已交付 SSE 内容、WS 首事件、整轮结束和有状态续接。HTTP 200、心跳、response.created 与有效模型/工具调用进展不可混淆。

WS 新独立轮次的连接/握手失败能否换候选，需检查生产入口，不能由 HTTP 推断。带 previous_response_id 的续接保持原物理绑定，不由网关重建上下文；绑定不可用明确失败。HTTP Responses 如有同类状态引用，也要核查。实际用户是否走 WS 仍未验证。

D5 面向用户说明中的证据：`apps/aether-gateway/src/handlers/proxy/websocket/responses/continuation.rs:1` 的所有权注册明确禁止跨物理 provider binding 续接；`docs/operations/codex-responses-websocket-probe.md:95` 描述非空 previous_response_id 固定候选校验，独立 response.create 则重新计划；`docs/WebSocket-Mode.md:379` 说明当前已存在窄范围 Codex 额度耗尽独立轮次换 K，不能由此推断所有 WS 错误都可自动转移。有状态引用的可恢复错误和完整上下文重建应由客户端处理，具体客户端自动恢复能力未经验证。

### 状态、性能与可观测性

一次 attempt 一次权威健康更新；成功写入节流不能破坏“完整成功每次 +1”。优先复用已有状态存储，不新增数据库/服务。旧归一化分数、旧失败预算与新严格连续计数需版本/迁移设计，不能直接把已有低健康 K 重置为满分。

诊断记录应能解释生效尝试数、错误分类、基础/附加分、健康前后值、连续次数、切换/跳过原因、熔断/探测时间和响应交付状态。逻辑请求与真实 attempt 分开计量，不暴露 K 内容或模型正文。

## 收敛后的规划骨架

### 现有超时与 90 秒建议

本次继续沿生产入口只读核查；以下是仓库默认与代码契约，不是读取运行实例配置或实测延迟。

| 路径 | 已验证行为 | 证据 |
|---|---|---|
| HTTP 流式候选 watchdog | 默认 30 秒，first_byte_ms 可覆盖；获取 execution gate 后才启动，终态开始后可继续等待；每次执行候选重新创建计时器 | apps/aether-gateway/src/executor/candidate_loop.rs:1296, 1417 |
| HTTP 候选循环 | 静态/动态循环逐个执行候选；已核对循环未维护跨候选共享的首输出截止时间，动态取下一候选的 planning_timeout 也是逐次调用 | crates/aether-ai/serving/src/attempt_loop.rs:104；apps/aether-gateway/src/executor/candidate_loop.rs:773, 974 |
| 普通非流式与 compact | 普通非流式默认 300000 ms，openai responses compact 默认 1200000 ms，total_ms 覆盖；with_non_stream_total_timeout 包裹单次传输工作 | apps/aether-gateway/src/execution_runtime/transport.rs:61, 1068, 1353, 2890, 2935 |
| 流式传输 | 默认 first_byte_ms 为 30000 ms；非流式 total timeout helper 对流式返回 None；传输首字节、响应头和模型有效内容不能混为一谈 | apps/aether-gateway/src/execution_runtime/transport.rs:2918, 2942, 2960 |
| Responses WS | 握手默认上限 30 秒；发送 response.create 后有首事件默认 30 秒和独立的整轮 terminal timeout；connection supervisor 另有 1 小时寿命限制，不能称 WS 完全没有总时限 | apps/aether-gateway/src/handlers/proxy/websocket/responses/upstream.rs:29；responses/turn.rs:535, 552, 906；responses/session.rs:329；apps/aether-gateway/src/handlers/proxy/websocket/session.rs:15 |
| 实际尝试次数 | 无明确覆盖默认一次；endpoint/provider 表字段为 2 作为旧默认忽略，failover_rules.max_retries 为 2 可生效为两个槽位；failure scope 还可能跳过剩余槽位 | apps/aether-gateway/src/orchestration/attempt.rs:153, 165, 173 |

用户已接受流式聊天首输出前可配置 90 秒兜底及收窄后的适用范围，不改变普通非流式/compact 的完整响应超时，不取代 WS 已有整轮/连接时限，也不越过有状态续接约束。90 秒基于“首 K 两次各 30 秒后仍给备用 K 约一次机会”的折中，不是行业标准、已测最优值或成功保证。总预算耗尽可在尝试次数用完前结束；快速成功立即返回。若正常请求单次就需超过 30 秒才输出，需先据实测调整单次时限，总预算延长无助于该 K 的单次成功。

参考 [AWS Builders' Library](https://d1.awsstatic.com/builderslibrary/pdfs/timeouts-retries-and-backoff-with-jitter.pdf) 的方法：根据下游延迟分布和可接受的误超时率选择单次 timeout，考虑公网额外延迟与超时实际覆盖范围。该资料不推荐模型网关使用固定 90 秒；本方案总预算同时受用户可接受等待时间约束。未读取真实上游时延与客户端/前置代理生效超时，不能保证用户端一定能等满 90 秒。

### 后续步骤

以下只是顺序草案，不是实施授权或最终 implement.md。

1. 定稿基础分、重试/冷却、亲和、恢复与 WS 范围，补齐可观察验收条件。
2. 贯通页面配置、transport snapshot、attempt slots 和 failure scope，处理值 2 的兼容语义。
3. 修改现有健康投影为 10 分、阶梯加重、成功 +1 和严格连续计数，确定旧熔断规则与迁移。
4. 对齐同步/SSE/WS 终态、超时、错误正文、取消与重试边界，避免重复计分。
5. 接通熔断、独占探测、亲和、后续读取与本地容量约束。
6. 用两个可控本地上游复现快 500、延迟超时、429、异常 200、断流、备用成功和全部耗尽；核对真实调用次数/顺序/后续健康。
7. 三种模式分别验收；覆盖并发终态、旧凭据结果、探测互斥和 WS 绑定。前端标签/配置若改动，做最小真实页面流程。

决策收敛后才写最终 design.md / implement.md 并请用户确认。目标是每个承诺都有边界、失败结果和重复验证方法，而不是宣称方案完美。
