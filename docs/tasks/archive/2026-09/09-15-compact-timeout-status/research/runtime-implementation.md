# Runtime implementation handoff

最终更新（2026-09-15）：最新源码 `cargo test -p aether-gateway --lib stream_response_timeout` **12 passed / 0 failed**。日志 `output/compact-timeout-diagnosis/runtime-final-stream-response-timeout-12.log`。包含本报告后续追加的 owner 受控poll、root非对象payload反例、heartbeat-only计时metadata回读；本代理获准Cargo槽执行后已立即归还。以下较早10/10及等待措辞作为实施过程记录，最终结果以本段为准。

实现代理：timeout-runtime。范围：R2/R3 的 HTTP 聊天/流式 compact 运行时；未修改管理配置、前端、共享 contracts、provider transport network 或生产配置。

## 改动

- `execution_runtime/chat_retry.rs`：独立总期限从请求 accepted 时间起算；首个 provider 的 `stream_total_ms` 绑定一次，缺省 900000 ms；旧首输出预算不再进入 HTTP 流 scope。request/pump 显式共享 owner，锁内重新核对/claim 超时；正常终态与超时各自保护后续持久化。total 错误为 `stream_total_timeout`，candidate failed/504、usage failed/504，保留逻辑耗时与已有 first body 时间，不施加供应商网络/凭据健康处罚。
- `executor/stream_path.rs`：目标 HTTP chat/compact 使用新 scope；排除 image/video/nonstream/WS。
- `execution_runtime/transport.rs`、`transport_failure.rs`、`executor/candidate_loop.rs`：成功真实 2xx head 单独解除候选 watchdog 与后续 body 首次等待；首次设置本身保持原值。有效输出只用于协议预提交识别。global/target permit 通过共享可取走的 guard 在 total 或 Body drop 时释放；重试不重置 deadline。
- `execution_runtime/stream/execution.rs`：协议预读保持独立；pump 继承完整 deadline 并在提交后 total 超时发流内错误。诊断后台任务具备 abort-on-drop。Chat/Responses 在原始上游帧校验协议终态：Chat 需要真实 finish_reason 或完整 DONE，Responses 需要 response.completed/done/failed/incomplete，并沿用 SSE event 字段在 data 无 type 时的回退；已由 JSON bridge 验证的同步响应沿用桥接权威终态；共享 parser.finish 合成的 Finish 不再把 EOF 变成业务成功。item.done 不替代 response terminal，合法 incomplete 保留。
- 同文件将 response-history 记录暂存到本轮现有 Option，在全部发送及生成期限 finish 后写 KV；共享 owner 正常 finish 释放故障 sender，使 HTTP Body EOF 不依赖后续持久化耗时。没有新增敏感正文日志或复制会话给 timeout owner。
- `tunnel/embedded/local_relay.rs`、`execution_runtime/mod.rs`：真实 head 解包后、任何数据库 await 前解除首次等待；流刚打开即安装 cancel/reset guard，覆盖 headers 前取消。隧道接收端已有 streaming 无 body deadline 语义，未改接收端或新建协议字段。

## 验证

- 2026-09-15 最终定向 `cargo test -p aether-gateway --lib stream_response_timeout -- --nocapture`：**10 passed / 0 failed**。日志：`output/compact-timeout-diagnosis/runtime-final-stream-response-timeout.log`。该命令由 main 让出 Cargo 槽后本代理执行，完成后已归还。
- 10 个测试函数含 11 个 Responses 子场景；覆盖真实 Router 的 headers100ms/first1s/body3s/total5s/旧1.8s 成功、heartbeat 后静默、headers-only total、无 headers 首次超时、跨候选不可延长期限、Response 返回后持续输出超时、提交前/后错误和 EOF、owner 交接及慢持久化时 Body EOF/permit 释放。Responses 内部 HTTP framed 路径覆盖 text/compaction item 后 EOF、合法 incomplete、event-only terminal、response.done 和真实 HTTP deadline scope 内的 JSON→SSE 桥接。
- 初轮发现并修复 Chat EOF 合成 Finish 误判成功。后续 snapshot 失败来自测试错误地给 completed 填 incomplete_details，已修正 fixture；旧 JSON bridge fixture 手写 NDJSON 缺闭合括号，已改结构化序列化，并进入新 HTTP deadline scope 验证。
- 本代理最终全部修改 Rust 文件 `rustfmt --check`、`git diff --check` 通过。`cargo test` 完成测试目标编译；main 此前 `cargo check -p aether-gateway --lib` 通过。未单独运行 clippy；编译仅报告本任务未改的 `dispatch/refs.rs:71` unreachable-pattern warning。
- 复用 main 已报告兼容结果：HTTP failures 9、已输出断流 1、容量释放 1、watchdog 10、direct 3、SQLite lifecycle 2、legacy 1、image 1、summary 5、trace 13 通过。后续 status agent trace 修订由 main 再验；未声称上述旧快照验证覆盖后续改动。

## 证据边界

- 本代理 Router 测试主要使用 OpenAI Chat + memory；Responses snapshot/EOF/incomplete 是真实本地 HTTP framed runtime 的内部入口，不冒充完整 compact route。
- SQLite、真实 compact、隧道实际 reset/后续请求及浏览器结果由 main 的隔离 HTTP 验收整合。未调用生产上游。
- 慢持久化的 sender/owner 边界以真实 spawn/receiver/permit 生命周期测试；未单独注入真实 KV backend 延迟。历史写入移动后的真实链路保持待 main 验收。

- 合法 Responses incomplete 的协议摘要仍是正常终态且不重放；现有 `aether-usage/runtime/src/write.rs` 会把 incomplete_details 提取为错误，usage 保持原有 failed/200 呈现。这是原有跨层语义差异，本代理未扩大修改 usage 所属模块；已向 main/review 报告。新增测试断言保持其现有结果，同时没有 stream_missing_terminal_event。

运行时实现与定向检查已交付；整体任务的真实 compact/SQLite/隧道/浏览器验收由 main 完成，不能以本报告代替。

## Owner 交接与可重试终态复核

- `chat_retry.rs::run_stream_request_deadline` 的 select 前快照只决定是否轮询 timer；真正 timeout 分支在 `drop(future)` 前重新持有同一个 `budget.state` 锁，核对 `finished`、`pump_owned != pump`、`terminal_persistence` 及最新 deadline。仅在全部允许时于锁内设置 finished 来 claim 取消；交接与 finish/persistence 标记也使用该锁。因此在 timer 就绪前发生的交接/终态保护不能被旧 owner 的快照覆盖。
- 已通过 `stream_response_timeout_request_hands_off_to_pump_before_return`：request future 在交接后仍运行30ms，pump期限10ms/生成100ms；request不会误取消交接，pump按总超时结束且不继续生成，Body仍持guard时permit已释放。`stream_response_timeout_terminal_persistence_survives_generation_deadline` 同样通过，验证正常finish后100ms持久化跨过10ms期限，Body先EOF。
- `record_first_output_timeout` 全仓唯一产品调用在 `candidate_loop.rs::build_exhaustion` 的最终耗尽分支。可重试 `AttemptTimeout` 仅变为候选 Retry，不调用它。上游明确错误的 attempt 结算通过 `mark_chat_attempt_terminal` 暂停该轮持久化取消，`scope_attempt` 返回后清除此暂停且不设置finished；HTTP首有效输出也不调用永久finish。
- 已通过真实候选测试 `stream_response_timeout_total_deadline_is_shared_across_real_candidates`：第一供应商1400ms失败后第二轮真实执行，第二供应商配置5000ms也不能延长首轮3000ms逻辑期限。证明可重试失败未关闭总期限。此复核只读取代码并复用已过证据，未另启Cargo或改root正在接手的execution.rs。

## 后续受控轮询回归（等待 root 统一运行）

按 root 继续完善测试且不另启 Cargo 的指令，新增 `stream_response_timeout_rechecks_owner_after_polling_generation`，覆盖 handoff / finished / persistence 三种状态在 monitor 快照之后、generation 首次 poll 内改变的顺序。timer 已耗尽；不发送 Notify，避免将通知当正确性前提。断言旧 owner 不能 claim 取消，保护中的 future 可以完成。只修改 chat_retry.rs 测试，rustfmt / diff-check 通过；新增用例尚未运行，已通知 root 纳入最终统一过滤器。上面的 10/10 是此前已执行快照，不包含这个新增用例。

## 首 body 与候选/usage 结算边界复核

- Headers 观察只写 `response_headers_elapsed_ms`；首 Data 事件沿用原 telemetry/ttfb 口径写入共享 `first_byte_ms`，total payload 原值保留。headers-only=None 与已输出后 total=Some 的断言已有通过证据；外部 heartbeat-only SQLite 读回由 root 矩阵验证。
- HTTP 共享 pump 在生成/发送结束后先 finish 总期限，再执行历史、health、usage 持久化，最后写 candidate Success。提前同步终结也在 usage handoff 入口 finish。故候选已成功但 usage 存储尚待完成时，同一生成期限不会再改写终态；若 timeout 先锁内 claim，它先丢弃生成 future，再持久化 candidate/usage failed504。两类记录未改为同一数据库事务，瞬时持久化差异仍须由权威 usage lifecycle 投影，不将该代码顺序等同于数据库原子写。

## 预读心跳后 total 的回读断言（等待 root 统一运行）

现有 `stream_response_timeout_headers_without_body_expire_only_at_total_deadline` 扩为 headers-only / heartbeat-only 两分支，真实 Router 返回后从 usage repository 读取 `stream_timing`、`first_byte_time_ms`、`end_to_end_first_byte_time_ms`、`end_to_end_time_ms`。有心跳则保留首 body，首有效内容仍空；仅 headers 不填首 body。两分支都验证总超时504、候选终态、健康中性、容量释放和后续成功。只改测试，格式/diff检查通过；尚未运行，已交root最终12项过滤器（函数数未增加）。真实SQLite读回和隧道reset仍由root外部矩阵负责；history真实KV延迟限制仍按上文记录。

## 最终12项执行结论

本轮编译44.85秒，测试12.77秒，无失败；仅未改动的 dispatch/refs.rs unreachable-pattern warning。新增 owner 三种状态受控poll、Responses非对象payload反例、预读心跳后total的metadata持久化回读均已通过，不再标为未执行。实际 Router 的memory repository证据不等同于root外部真实SQLite/compact/隧道验收；真实KV backend受控延迟仍未注入。无需新增产品修复，最终源码稳定，Cargo槽归还，等待root整合验收。
