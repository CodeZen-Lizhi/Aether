# 调度故障转移最终范围审查

审查人：check_final。日期：2026-09-14（Asia/Shanghai）。本报告是当前工作树的全范围审查快照，不是全部验收通过声明。审查期间 main/integration 继续修复；下列状态以本报告落盘前复读为准。

结论：四项审查发现 N1/N2/N3/N4 均已修复并关闭，定向回归通过；五 packages scoped clippy 曾 exit 0，随后确认一条新增 lint 并由 main 修复，最终 gateway 补跑结果待回填。本 reviewer 未修改实现、旧 fixture 或迁移预期，未运行 Cargo、启动子 agent、提交或调用付费上游。以下过程记录保留各阶段证据；其中历史“待修复/待验证/待 lint”状态由本文末尾最终结论取代。

## 新发现

### N1 / P1：流式目标准入许可在返回响应头/预取内容后释放

- 原发现位置（修复前快照）：`apps/aether-gateway/src/executor/candidate_loop.rs:1508` 的 `_target_permit`；同文件 Responded 返回分支；`apps/aether-gateway/src/execution_runtime/stream/execution.rs:915` 的内部准入跳过。
- 确定链路：watchdog 在实际执行前取得目标许可，但将其保存在函数局部变量。执行被 `scope_chat_probe_session` 包裹后，`chat_attempt_admission_held()` 为真，内部 `execute_in_process_stream` 不再取得目标许可，`DirectUpstreamStreamExecution.upstream_target_permit` 因而为 None。返回响应时，只将全局 `permit_hold` 包入 body，目标许可随 watchdog 返回释放。
- 复现：令 `UpstreamTargetAdmission` 并发上限为 1；第一条 SSE 已输出合法文本、上游保持未终态；此时发送第二个同目标请求。第二个请求能取得被提前释放的目标许可并开始上游调用，实际同时存活两条流。`upstream_admission.rs:99` 的 gate 只受持有中的 permit 约束，因此这不是仅影响统计的问题。
- 影响：绕过本地目标准入，破坏 R7/R19 的容量与资源生命周期契约。现有 SSE probe 排他测试证明的是恢复租约，不能证明普通目标并发许可仍被持有。
- 修复验收：将原目标许可转交响应 body/pump，覆盖上游终态与客户端取消，不重复 acquire；真实并发=1 测试应观察第一条流结束前同目标的第二次上游调用没有开始，结束/取消后许可恢复。
- 已复核修复：integration 已在 `apps/aether-gateway/src/executor/candidate_loop.rs:1677` 将原 `target_permit` 交给 `hold_response_permit`；该 helper 在同文件 `:1830` 将 permit 捕获到响应体流中，覆盖全部 body 读取及 drop。main 在 channel seq 21428 分配修复。状态：代码静态修复成立；integration seq 22152 报告 HTTP16 全通过，其中目标容量回归覆盖有效输出后仍占用、本地满转 K2、中性健康、正常终态/客户端取消释放并复用。原发现 seq 21189。

N1 补充复核：当前 `candidate_loop.rs:1517` 已将 target gate 的 AdmissionTimeout 映射到 `Retry(Candidate)`，没有使用 `await?` 将本地满直接升级成整请求错误；该分支位于健康 capture/真实执行之前。当前 `apps/aether-gateway/src/tests/scheduler_failover/target_admission.rs:45` 明确断言第二请求 200、上游顺序 `[0,1]`，不是 429。候选诊断可记录 429，但不等于客户端响应 429。

容量语义：`upstream_admission.rs:366` 按 URL scheme/host/port + proxy 分桶；同目标不同 K 共享容量，路径/query 不参与，来源是 `AETHER_GATEWAY_UPSTREAM_TARGET_GATE_LIMIT`（`state/app.rs:70`）。它不同于管理端 K 的 `concurrent_limit`；后者在 `crates/aether-scheduler-core/src/candidate/selectability.rs:82` 按 key_id 的活动请求计数过滤。当前真实 targetlimit1 fixture 将两个上游放在不同监听目标，证明目标满时能转可用备用；不能借此声称已证明同 endpoint 不同 K 的独立 K 并发配置。

### N2 / P2：重复 pending enqueue 后曾使用后到副本结算

- 原位置：`apps/aether-gateway/src/orchestration/health_settlement.rs:308` 的 enqueue 与原 `:318` 的 `process_pending(&state, &pending)`；`apps/aether-gateway/src/orchestration/effects.rs:164` 的终态观察时间重取。
- 仓储证据：`crates/aether-data/runtime/src/repository/provider_catalog/memory.rs:1280` 使用 entry/or_insert 保留首份事实；`crates/aether-data/adapters/sqlite/src/provider_catalog.rs:2187` 使用 `ON CONFLICT(id) DO NOTHING`。原返回类型 `Result<(), _>` 不能告诉调用者哪份事实已被接受。
- 复现顺序：同 attempt 的 429 / Retry-After 30 在 t0 已 enqueue，随后 CAS/存储失败留在 pending；同 identity 在 t0+10 重投，effects 重新取 observed_at；重复 enqueue 保留 t0 的记录，但原即时路径使用 t0+10 的本地副本结算。冷却变成 t0+40，随后删除 t0 的原 pending。如果两份终态内容不同，也会出现后到内容抢先计分。已有仓储 immutable-enqueue 测试不能覆盖 gateway 消费错误副本。
- 契约：`health-pending-contract.md` 明确要求重复 enqueue 后消费持久化的首次事实；R19/A7 要求原始时间、事实和 identity 不随重投改变。
- 已复核修复：main 已将 enqueue 改为返回 `ProviderCatalogKeyHealthPendingFact`；memory 在写锁内返回 entry 的 clone，SQLite 在同一写事务内 SELECT 首份 payload；`health_settlement.rs:315` 只对返回的 canonical fact 执行 process。enqueue 超时/错误时不再猜测本地副本为权威事实，保留诊断，已落盘事实可由 drain 重试。
- 状态：代码静态修复成立；新增 `effects.rs:2098` 同 attempt 不同 timestamp/penalty 回归断言首次基础 1 分及原始 Retry-After 截止。integration seq 22163 报告该 gateway 回归 1 passed、memory/SQLite health_pending 各 1 passed。原发现 seq 21042；main 接手 seq 21047。

### N3 / P2：HTTP 200 的 HTML/纯文本/空响应仍漏过新的异常成功分类

- 原发现位置（修复前快照）：`apps/aether-gateway/src/execution_runtime/sync/execution.rs:2214`，条件仅在 `body_json.as_ref().is_some_and(...)` 时检查 `invalid_chat_sync_success`；同文件 `:2265` 仅对状态 >=400 结算失败。
- 确定链路：`execution_runtime/transport.rs:4558` 的 `build_execution_response_body` 对非 JSON HTML/纯文本保留 base64 body，对空响应返回 None。之后 SSE error 提取没有匹配，状态保持 200，故没有进入基础 2 分失败结算。OpenAI chat 正常计划使用 `openai_chat_sync_finalize`（`ai_serving/planner/standard/openai/chat/decision/request.rs:619`）；`execution_runtime/fallback.rs:144` 对 explicit finalize 且存在 bytes 的情况也不会因缺少 JSON 转移。
- 复现：K1 返回 `200 / Content-Type: text/html / <html>maintenance</html>`，K2 正常；另覆盖 `text/plain` 和空 body。当前新 validator 不识别这些无合法模型终态的成功状态。确定影响是 K1 不按上游无效响应扣基础 2 分；最终客户端错误包装需以真实路由回归断言，不将静态分析夸大为已经观测到回血。
- 与旧 F1 的区别：旧 JSON `{"error":...}` 路径已有真实回归通过，本项是新 validator 的 None 分支漏检，不重新报告旧 JSON 路径仍未修。
- 修复验收：非 JSON 的无效成功进入统一失败结算与有限重试；同时保留合法 forced-upstream-SSE 的既有聚合/完整终态处理，不能把所有 bytes 响应一律判错。
- 已复核修复：main 在 `sync/execution.rs:2225` 先尝试既有 SSE 聚合，再以 `is_none_or` 将缺失或无效终态提升 502；新增 `apps/aether-gateway/src/tests/scheduler_failover/http_failures.rs:77` 覆盖 HTML/纯文本/空 body，并另保留合法 forced SSE 回归。状态：原 HTML/纯文本/空 body 漏检已修复；integration seq 22152 报告 HTTP16 全通过，包含三类坏 body 与合法 OpenAI chat forced SSE。但此修复另引入 N4，不能由 chat 格式的通过证据覆盖 Responses 同格式。原发现 seq 21204。

### N4 / P2：非 JSON 成功校验误用跨格式聚合，破坏 Responses 同格式终态扩展兼容

- 准确位置：`apps/aether-gateway/src/execution_runtime/sync/execution.rs:2231`，缺失 JSON 时无条件调用 `aggregate_standard_chat_stream_sync_response`，随后 `:2236` 将 None 提升为 502。
- 确定链路：`crates/aether-ai/formats/src/formats/shared/sync_products.rs:1935` 的通用 Responses 聚合调用 `ensure_no_unknown_openai_responses_stream_events(body, false)`；终态 output 的 `program` 等扩展由 `formats/openai/chat/stream.rs:1330` 产生 UnknownEvent，通用聚合失败并返回 None。现有同格式 finalizer 在 `sync_products.rs:870` 明确以 true 调用相同验证器，允许完整 authoritative terminal 的扩展 output 原样保留，同时继续拒绝未知中间事件。
- 可复现输入：client/provider 都为 `openai:responses`、`needs_conversion=false`、同步客户端/forced 上游 SSE，返回 `event: response.completed`，data 为 `{"type":"response.completed","response":{"id":"resp_extension","object":"response","status":"completed","output":[{"type":"program","call_id":"p1","fingerprint":"fp1"}]}}`。现有 `sync_products.rs:4717` 的 `same_family_responses_stream_uses_terminal_response_as_authoritative_snapshot` 已有更完整的同类 fixture。
- 影响：原同格式可合法透传的完整成功在到达 finalizer 前被判 502，随后扣当前 K 健康并可能重试/转移。新 `valid_forced_upstream_sse_is_still_accepted_for_a_sync_client` 仅覆盖 OpenAI chat，不能检出这个 Responses 分支。
- 修复方向：同格式 Responses 沿用既有 authoritative helper 及严格生命周期验证，再按适用范围使用通用聚合；不要把跨格式的未知事件约束全局放松。补真实 Responses 路由回归，断言一次上游调用、原 output 保留、无健康惩罚。
- 状态：原发现已于 channel seq 22294 发给 main。main 已添加 `sync/execution.rs:661` 的 `aggregate_chat_sync_success`，传入原 report_kind/context/body_base64；同格式 authoritative helper 的 Ok(Some) 原样接收，Ok(None) 才通用聚合，Err 直接 None，不绕过生命周期检查。生产 validator `:2252` 已调用新 helper。`:3045` 起两项 `responses_sync_success` 模块回归分别覆盖扩展终态保留/跨格式拒绝，以及未知中间事件拒绝。定向静态复核通过，N4 代码问题关闭；稳定 snapshot 的 `responses_sync_success` 两项 passed（0.00s），同一构建产物 `http_failures` 九项 passed（11.81s，RUST_MIN_STACK=33554432）。前两项是 N4 模块边界验证，后九项是既有真实 HTTP failures 路由回归，不将两者合并描述为新增 N4 真路由测试。最终 scoped clippy 待结果。本 reviewer 未运行 Cargo。

## 全范围核对结果

已读 `check.jsonl` 全部引用的 PRD/design/implement、旧初审、pending/HTTP/integration/config 交接，以及本次相关的 routing、frontend、gateway transport、data SQLite 和包级规范。已检查当前 tracked diff、staged 范围（为空）和新增 untracked Rust 模块；没有仅凭 `git diff` 忽略新增文件。

| 范围 | 检查内容与结果 |
| --- | --- |
| 健康与结算 | `orchestration/{classifier,chat_health,health,effects,health_settlement}.rs`、data contracts/memory/SQLite、catalog wrappers。公式、版本、成功清 streak、同 K/格式、CAS 重投、receipt 与更新同事务、凭据/熔断代次核对成立；N2 见上。 |
| probe 与取消 | `probe_lease.rs`、circuit/rate-limit wrappers、HTTP `chat_retry.rs`、WS `probe.rs`。原 guard 持有、定期续期、取消释放、owner/source fence、事务内 expiry 与 Duplicate 优先已检查；未把不确定提交当作必然失败。N1 是独立的目标许可缺陷。 |
| 启动与恢复 | `state/core.rs` 注册 singleton health worker，`task_runtime/mod.rs` 注册 daemon；drain 批量 32、保留原事实、过期/失去代次转诊断、成功后精确清理，未重新调用模型或生成迟到亲和。 |
| HTTP 同步/SSE | transport typed source、sync 成功校验、SSE precommit/useful-output gate、候选 watchdog、共享 deadline、实际 capture 后的同一 report、已交付后不 replay。旧 F1 JSON/F2/F3/F4 接线已存在；N3 是非 JSON 分支新增发现。 |
| WS | `client/connection/frame/ownership/quota/session/turn/turn_state/upstream/probe` 及新增 failover/route_smoke 测试。pinned previous ID 不进入独立请求 retry；public response/tool 状态与 effective output 分开；replayable rejection 不包含混合公共输出；每 logical turn 保留 budget、attempt 次数和有限累计等待。未发现额外确定的新问题。 |
| 三种模式 | planner materialization/source/ranking/report context、stream 二次选择、serving ranking、scheduler-core comparator、affinity cache/state 写入。fixed 顺序、同能力/兼容 tier 先倍率后亲和、后页便宜 K、UUIDv7 请求序/epoch 保护已检查；新完整模式不走第二次 pressure/random 选择。 |
| 配置/API/UI | contracts resolver、admin failover merge/endpoint/provider 写读、transport timeouts、provider 表单与详情/i18n。旧 2 不遮蔽有效 override；新 endpoint 显式 2、生效来源、清空继承、无存储值、非聊天旧值、局部规则保存与 UI 不回写未改值已检查。没有更改 routing 保存器，原保留规则契约继续适用。 |
| 迁移/兼容 | 两个 additive SQLite migrations、logical/generated schemas、receipt portable export 与 pending runtime-only、video timeout literal、模块注册均已核对。main 已补 migration 版本预期列表；不把旧 fixture 的已知失败重报为新产品问题。 |
| 测试 | 已查看新增真实 HTTP/WS fixture 的实际监听/鉴权/生产路由和终态后权威读取；检查相关单测新增断言。mock WS helper 与真实 WS route 分开计证据；并发/未执行/损坏输出仍须用能区分正确实现的断言。 |

## 验证证据与限制

本 reviewer 实际执行：只读源码/diff/调用链核对和 `git diff --check`（通过）。TypeCheck、Cargo test/clippy、前端 lint/测试未重跑，遵守本轮明确指令和 main/integration 构建所有权。未以编译通过替代功能正确。

采用已读 `integration-contract.md` 和 `config-contract.md` 的 owner 结果：

- HTTP 真路由 12 passed，独立 WS 真路由 1 passed；WS 模块合计 183 passed，已包含该真路由，不能相加为 184。
- data health：memory 5 + SQLite 5 passed；portable export 覆盖 1 passed。
- scheduler health/ranking：20 / 29 passed；serving attempt/ranking：4 / 4 passed。
- gateway scheduler/cache/affinity：49 / 85 / 89 passed，filter 有重叠，不相加为唯一测试总数。
- gateway diagnostics/task runtime：7 / 8 passed。第一次 diagnostics 运行缺少仓库的线程栈环境而 abort，按配置重跑通过，保留证据边界。
- 前端三个相关组件共 16 passed；type-check 和范围内 ESLint 通过。Drawer 的既有 8 条 unused-variable 仍按原记录保留，非本轮新发现。
- main 已记录真实 Vue 表单 UI fixture 保存/重载和桌面/移动截图；它使用 PATCH stub/localStorage，不是后端持久化或真实 Codex 验证。

上述通过结果来自本轮新发现修复之前的相应快照，不能自动扩展至 N1/N2/N3 修复后的代码。原 effects/admin/candidate_loop 三项 fixture 和 migration 预期的最新结果应由 main/integration 更新；新 503 Retry-After 场景也以其最终输出为准。

| 验收 | 本次审查结论 |
| --- | --- |
| A1/A2 | 模式/次数/后续亲和已有真实 HTTP 证据；最新修复后最终测试结果仍以 integration 为准。 |
| A3 | 评分序列、混合失败、成功恢复有模块及真实终态子集证据。 |
| A4 | JSON 异常200、typed timeout/connection/local config 已有证据；N3 非 JSON 成功分支尚待修复后回归。 |
| A5 | 429 短/长及后续请求已证明；N2 原事实重投修复需新回归，503 新场景待最终结果。 |
| A6 | due probe 恢复 .1、SSE 持有 owner/取消、存储 expiry 有证据；N1 目标许可生命周期待新回归；新增真实长探测已证明跨初始 60 秒续租、有效输出越过 2 秒预算、换代失租后终止且不重放（下述长测证据）。 |
| A7 | receipt/CAS/generation/持久化重开有证据；N2 的 gateway 重投和并发不等同于仓储 enqueue 单测，仍待新结果。 |
| A8 | HTTP、SSE、WS 分别已有模拟真路由；pinned/工具不重放还含模块证据。实际 Codex 和真实中转协议未验证。 |
| A9 | 默认/配置投影、跨候选耗尽中性和有效输出后放开已有对应证据；不能宣称穷尽所有协议/长推理时序。 |
| A10/A11 | resolver/API/UI 保存刷新、健康显示/成功率区分和保留旧配置已查；管理 fixture 修正后的最终结果待汇总，UI fixture 不能证明数据库刷新。 |

续审补充证据（integration channel seq 22152 / 22163）：HTTP 真路由 16 passed / 25.03s，包含 N1、N3 和新增 503 Retry-After；N2 gateway canonical 回归 1 passed / 1.19s，memory/SQLite health_pending 各 1 passed。上述替代对应修复前的“待回归”状态；未重跑的其他范围不扩大声明。第一轮 scoped clippy 发现 `sync/execution.rs:2179` 的 `never_loop` 并退出 101，main 接手；最终 clippy 尚待修复稳定后的结果。

审查范围已完成。本 reviewer 修复 0 项；累计发现 4 项，N1/N2/N3 的 owner 修复已静态复核且有定向运行证据，N4 已静态确认修复并有两项定向运行证据；四项发现均关闭，最终 scoped clippy 尚待结果。

## 长探测续审证据

已只读核对 `apps/aether-gateway/src/tests/scheduler_failover/probe_lifetime.rs:17` 与 integration-contract 的结果：真实路由定向测试 1 passed / 83.18s，实际等待 62 秒跨越初始 60 秒租约，断言同 owner、截止延期且仍在有效期；有效输出后越过配置 2 秒预算，独立请求使用备用。手动替换 circuit epoch/owner 后原 body 在 30 秒内返回 IO error，健康保持 0、上游调用无重放、旧 owner 清理未删除新 owner。此项补齐原长租约和输出中失租的未验证范围；这是 N4 修复前快照，不扩展为 N4 通过证据。

本次读取时 `sync/execution.rs:2231` 仍是 N4 原通用聚合调用，main 的稳定信号尚未收到；遵循 main 暂停新编译的协调要求，尚不发送最终 N4/lint ready。

N4 定向收尾：生产 helper 与两项新增模块测试已复核，never_loop 已改表达式 block；已向 main/integration 发送 ready，可执行 `responses_sync_success` filter 和最终 scoped clippy。未重复全范围审查或已通过的长探测。

稳定 snapshot 验证更新：owner 报告 `responses_sync_success` 2 passed / 0.00s，同 artifact `http_failures` 9 passed / 11.81s；按仓库配置 RUST_MIN_STACK=33554432。五 packages 最终 scoped clippy 已启动，未取得最终输出前不标通过；长探测不重复。

## 最终结论

四项发现全部关闭：N1 目标许可完整响应生命周期及本地满转备用、N2 canonical pending 首份事实结算、N3 非 JSON HTTP 200 无效成功分类、N4 Responses 同格式 authoritative 终态扩展兼容。没有尚未处理的已确认审查发现。

最终证据：

- HTTP 真路由共 17 项：原 16 项通过（25.03s），另长 probe 1 项通过（83.18s）。长 probe 实际等待 62 秒跨越初始 60 秒租约，验证同 owner 续期、有效输出越过首输出预算，以及 epoch/owner 替换后终止、无重放/回血并保留新 owner。
- 稳定 snapshot 的 N4 `responses_sync_success` 模块测试 2 passed（0.00s）；同一构建产物的 `http_failures` 真路由回归 9 passed（11.81s，RUST_MIN_STACK=33554432）。后者属于上述 HTTP 场景的定向重跑，不另加到 17 项。
- WS 模块 183 passed，已包含独立真实 WS 路由 smoke；不能另加为 184。
- canonical pending gateway 1 passed（1.19s），memory/SQLite health_pending 各 1 passed；其余已记录的数据层、调度、亲和和配置证据保持原覆盖范围。
- 五 packages scoped clippy 曾 exit 0（1m13s），日志保留于 `output/scheduler-failover-verification/clippy.log`，首次日志 `clippy.first.log` 也保留。main 随后更正：逐行核对确实命中新增 `apps/aether-gateway/src/execution_runtime/stream/execution.rs:5974` 的 `needless_option_as_deref`（lib/test 重复报告），撤回此前“未命中新行”判断。main 已在直接 return 的 prefetch failure 分支改为直接传 `retry_scope_out`，无行为变化；integration 仅补跑 gateway scoped clippy，不重复行为测试。最终 lint 状态以这次补跑的实际结果为准，目前待回填；不能表述为零 warnings。

验证边界：上述为本地模拟上游及仓库测试，实际 Codex 客户端与真实 relay 未测试；长时 WS 生命周期仍未测试，HTTP 长 probe 的通过不能替代长 WS 证据。N4 的同格式扩展/跨格式拒绝/未知中间事件拒绝由定向模块测试证明，不声称新增 N4 真路由专项已经运行。

最终审查完成，发现 4 项、owner 修复并关闭 4 项、开放 0 项。本次仅更新最终结论，没有扩展审查范围或运行新的检查。

最终 lint 更正：此前“warnings 未命中新增行”的过早判断已撤回。四项行为发现仍关闭，行为回归证据不变；最终 gateway lint 补跑结果待 integration 回报。


## 主会话最终验证结论

2026-09-14：N1–N4 的代码问题和对应定向回归均已关闭。新增的 stream Option reborrow 机械修正后，主会话运行 `cargo clippy -p aether-gateway --lib --tests -- --no-deps` 返回 exit 0，18.36s；日志为 `output/scheduler-failover-verification/clippy-gateway-final.log`。既有警告保留，按当前 diff 逐行核对没有新行警告，不表述为零警告。此前五包 scoped clippy 通过仍有效。

最终行为证据包含 16 项真实 HTTP/SSE 回归、另 1 项 83.18s 的真实长探测、183 项 WS（含真实路由）、Responses 扩展兼容 2 项及同构建 HTTP failures 9 项。后两组不与 HTTP16 相加为新的唯一样本总数。完整计数和限制见 verification-log.md。实际 Codex/中转未验证；长时间 HTTP 续租/输出中失租已覆盖，同等长时间 WS 路由未覆盖。实现可在本地审阅，未提交或部署。
