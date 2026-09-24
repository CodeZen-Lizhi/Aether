# 独立审查结果

审查时间：2026-09-15。最新结论：**暂不通过**；最初三项、sender 及合法 Responses 别名/JSON 桥接均已见修复；原始终态归一化仍缺 JSON 对象约束，见末尾“别名落盘后的限定复核”。最终行为结果由 root/runtime 汇总。只审查本任务 apps/crates/frontend 变更及直接调用方，忽略用户既有 AGENTS/spec 大量修改。已读 check.jsonl 全部条目、PRD、design、implement，并应用 trellis-check 与 code-review-and-quality。以下保留初审证据，各节行号对应当次读取快照。

## 必须处理

1. **P1：已输出后缺失协议终态仍可被记为成功。** `apps/aether-gateway/src/execution_runtime/stream/execution.rs:6778` 的 EOF 分支只对原生 Anthropic 显式检查终态；普通 OpenAI Chat 可在输出一段正文、没有 finish_reason/[DONE] 后结束，进入正常完成结算。main 在频道 04:45:09 报告新增 `stream_response_timeout_early_error_retries_but_committed_error_and_eof_do_not` 的 `error=false` 分支实际 `completed`、预期 `failed`，与源码路径一致。runtime 已获知，不能以其它 8 项 deadline 测试通过替代这一失败；修复时同时保持 Responses 合法 incomplete 和已提交后不重放。

2. **P2：隧道成功 headers 后仍可能被首次 watchdog 误杀。** `apps/aether-gateway/src/tunnel/embedded/local_relay.rs:108` 的 `wait_headers` 已取得真实上游 head，但 `:116` 随即 await `record_proxy_upgrade_traffic_success`；该函数在 `maintenance/runtime/proxy_upgrade_rollout.rs:916` 读取数据库，必要时还写入。直到 `execution_runtime/transport.rs:1304` 才调用首次响应完成标记。触发条件是 headers 在首次阈值内到达、随后系统配置读写等待超过剩余首次期限；此时 candidate watchdog 仍看到未响应而报 504。应在收到真实成功 head 后、任何后续 await 前解除首次计时，并保留总期限与取消 guard。普通 direct HTTP 无此插入等待。

3. **P2：总超时丢失预提交期间已观测的响应阶段信息。** `stream/execution.rs:5675` 已捕获首 Data 时间，但 `record_stream_started` 在预读结束后的 `:6139` 才执行。若上游先给 200/心跳、之后直到总期限仍无有效内容，局部 `prefetched_usage_telemetry`/stage trace 被取消丢弃；`chat_retry.rs:675` 的总超时结算复用首输出载荷，固定 `first_byte_time_ms=None`、`provider_response_headers=None`，共享取消上下文也只保存尝试身份。此场景没有先前 streaming 写入可以兜底，最终记录无法表示“已经响应且已有 body”。PRD R2.6 要求区分 headers、首 body、首有效内容和整体耗时。建议只向共享 owner 传递脱敏时间/阶段，补 heartbeat 后总超时的 usage/trace 读回断言。现有首字标签（`frontend/src/features/usage/components/UsageRecordsTable.vue:559`、`RequestDetailDrawer.vue:212`）也尚未说明实际 body 计时口径。

以上均涉及执行边界或公共观测语义，未抢写 runtime/status 文件；机械自修 0 项。

## 已核对的正确方向

- `stream_path.rs` 只给指定 HTTP 聊天流 plan kind 建立新 scope；image/video、nonstream 未进入，WS 未改用此 scope。普通流与流式 compact 共用新期限，旧预算字段仍保留序列化/API 兼容。
- 总期限由请求 accepted 时间起算，首个 provider 在 `prepare_attempt` 绑定一次；重试不重置。request/pump 通过同一锁下的 owner/finished/persistence 状态争用取消，超时先 claim 再 drop future；弱引用 permit hook 让超时可释放仍挂在 Body 上的容量。
- 成功 2xx 解开候选首次 watchdog 和直接/本地隧道首 body 读取限制；有效输出 gate 仍独立服务早期错误预读，不再以旧 deadline 是否存在决定是否检查。
- records/detail/trace 采用请求 lifecycle，候选只保留本轮状态/时长；旧 unknown 状态兜底、图片显式失败保留。前端未删除真终态防陈旧更新；Drawer 不再将选中尝试的错误和180秒时长回写整个请求。
- 新 `stream_total_timeout` 秒字段区分缺失/null，JSON 存储毫秒，范围1–1200秒/最多3位小数；默认900秒且不继承旧90秒预算。create/PATCH/summary 与表单保留未知 config/failover keys，ExecutionTimeouts 新可选字段兼容旧JSON。

## 验证和剩余限制

- 本审查执行 `git diff --check -- apps crates frontend`：通过。按 main 明确安排未运行任何 Cargo、前端全量测试或浏览器；未提交、发布或修改生产配置。
- 复用证据：config-handoff.md 记录 contracts 3项、ProviderForm 11项、type-check/eslint 通过；usage-status 频道记录前端136项与最新 Drawer/Timeline 42项通过。main 当前 runtime 9项为8过1失败，尚非最终修复后结果。
- 已静态核对新增 Router 超时测试、SQLite lifecycle API 测试和前端合并/组件测试；前者主要是 OpenAI Chat + memory，SQLite lifecycle 测试手工推进 usage/candidate。它们分别证明各自层次，不能称为“真实 compact 重试→SQLite→API→前端合并”的同一完整自动链路。
- 待 main 补齐/记录实际 compact、真实 tunnel（含 reset 后后续请求）、客户端取消、终态与 deadline 同时就绪，以及 HTTP CRUD + SQLite 重开/浏览器保存清空的最终证据。Responses snapshot 单元/内部流测试不能替代真实 compact route。
- 额外待核边界：`stream/execution.rs:6681/:6839/:6908` 的 response-history KV 持久化仍在 `finish_stream_request_deadline(:6981)` 之前；pump 中 `mark_chat_attempt_terminal` 不暂停 deadline。建议以受控延迟确认“协议终态已确定/已发送、历史写入未完”不会被同一生成计时器取消；当前 owner 单测直接调用 finish，未经过这一真实路径。此项尚未复现，不列为已确认缺陷。
- 尝试经 trellis-channel 发送即时发现给 main/runtime，因 sandbox 无权写用户目录中的频道 `.lock` 文件而返回 EPERM；没有绕过权限。共享本报告与最终回复用于交接。

## EOF 定向复核补充

收到 main 的 8过1失败结果后进一步追踪，根因不止 EOF 分支漏检查：

- `crates/aether-ai/formats/src/formats/openai/chat/stream.rs:371` 的 OpenAIChatProviderState::finish 在已开始但未完成时补出 canonical Finish；同文件 `:1895` 的 OpenAIResponsesProviderState::finish 也补出 stop/tool_calls。
- `crates/aether-ai/formats/src/formats/shared/stream_core/format_matrix.rs:304` 的 observer.finish 调用上述 parser.finish，再由 `:378` 的 observe_frame 把补出的 Finish 记为 `observed_finish=true`。该文件 `:3020` 现有测试已明确钉住 Responses EOF 补 stop 的共享行为。
- Gateway `stream/execution.rs:731` 的 finalize_stream_usage_observer 会走此路径，后续 missing-observed 检查就可能被这个合成 true 绕过；因此修复需区分原始上游终态与 finish() 补出的终态。不要直接全局修改共享 parser，以免改变本任务排除的 WS 行为。
- 读取中的 runtime 补丁已开始新增 OpenAIChatDone tracker，但尚在编辑。最终 review 需确认检查用的是上游原始帧（含预读+后续跨块数据），在 rewriter.finish 补出客户端成功事件前判缺终态；并覆盖 Responses 文本/compaction item.done 后 EOF，不把 item.done 当 response.completed。Responses 的合法 response.incomplete 仍需认作合法协议终态。
- 仅将 usage 改 failed 不足够；同一失败应到达 candidate/health 与支持的流内错误反馈，且不得重放已提交内容。

本补充仍为静态复核，未启动 Cargo，也未抢写实现。

## 新版稳定契约核对

已按 root 通知重读更新后的 `docs/specs/aether-gateway/backend/chat-failover.md`。HTTP 成功真实 headers 解除首次等待、900秒独立逻辑期限、旧值兼容、request/pump 所有权、usage lifecycle、取消中性归因均与批准 PRD 一致。health 公式、探针规则、其它协议及既有 incomplete 约束保留；文档没有把待执行验收宣称为通过。

建议 root 汇总时补三处精确措辞（本 review 未改 spec）：

1. `chat-failover.md:47` 的 `Nonstream/compact` 容易被读成所有 compact 均排除；改为 `Nonstream requests (including nonstream compact), WS, image and video ...`，明确流式 compact 已属前句范围。
2. `:74` 的 `records candidate failed/504` 加上“仅当前仍在执行的 candidate；重试间隙只结算请求，不覆盖已结束尝试”。当前 `scope_attempt` 清除 snapshot 的实现正是在保留此边界。
3. `:66` 仅写记录 event types/timings，尚未固定 PRD R2.6 的四个里程碑。补明 headers、首 body、首有效内容、整体结束各自记录；总超时保留已有里程碑，UI 标签解释口径。可同处明确首次缺省30秒。

这次仅核对规范，不代表产品审查问题已关闭。实现修复与最终测试/验收结论继续由 root 汇总。

## 修复代码复核

已收到 runtime 的 EOF/计时修复说明，静态确认：HTTP Chat 在 rewriter.finish 前检查上游完整 DONE 或实际 finish_reason；缺失时进入既有 midstream failure 路径。新的原始帧 tracker 同时纳入 Responses，item.done 本身不算 response terminal。`local_relay.rs` 已在真实 head 到达后、流量数据库操作之前标记已响应；共享 timing 记录 headers/body/effective，总超时 seed 保留 first_byte_ms，headers 不填 body 时间。三项原始缺陷从“待改代码”转为“代码已改、待 root 定向验证”；首 body 测试当前增加 headers-only 为 None、持续输出后 timeout 保留 Some 的断言，预提交 heartbeat-only 的四阶段 metadata 读回仍未见新增断言。

**新增 P2：Responses tracker 与既有解析契约不一致，合法完成被误判 EOF。** `apps/aether-gateway/src/execution_runtime/stream/execution.rs:4284` 的 `SseTerminalPolicy::OpenAIResponses` 只读取 data JSON 的 type，且枚举仅 completed/failed/incomplete；在 `:6880` 处 Responses 又只能靠 tracker.completed 放行。以下原来接受的输入因此会在已输出后被改判 stream_missing_terminal_event/502：

- `event: response.completed` + `data: {"response":{...}}`（data 不重复 type）。已有 `format_matrix.rs:2401` 的 `terminal_observer_uses_event_type_when_data_omits_type` 明确验证该行为，`:207` 会把 event 字段补入 payload。
- `data: {"type":"response.done","response":{...}}`。`crates/aether-ai/formats/src/formats/openai/chat/stream.rs:1864` 明确把 response.done 与 response.completed 同分支解析。

建议 provider tracker 复用/对齐既有事件归一化和有效终态语义，再用两个小边界加 item.done+EOF 验证。不能只依赖 finalizer.finish 后的 observed_finish（会再次引入原始合成终态问题），也不应通过修改共享 parser 的既有兼容行为消除差异。

未写产品文件、未运行 Cargo；runtime patch 的统一复跑结果仍由 root 记录。

## 最终定向复审（1–3项、计时与终态所有权）

按 root 最新要求只读复核当前修复，不重复全范围。**结论：最初三项根因均已有对应代码修复；EOF 修复仍有两类 P2 兼容问题，暂不整体通过。**

- **原1 / EOF：Chat 根因修复成立。** 上游预读与后续 chunk 使用同一个增量 tracker，完整 DONE 跨块识别；Chat parser 在 EOF 合成 Finish 时 finish_reason 为 None，真实 finish_reason 保留下来，新增判断在 rewriter.finish 前拦截缺终态并走失败反馈。HTTP scope 限定保留，未全局改 parser。Responses item.done 不被当作 response terminal。行为结果待 root 复跑。
- **原2 / 隧道：代码问题已消除。** `local_relay.rs:116` 在真实 head 到达后立即 mark，随后才执行数据库流量记录；guard 在等待期间仍持有，标记内部只接受新 HTTP scope 的成功 2xx。未见 relay 外层200误作真实上游 head。
- **原3 / 计时：代码问题已消除。** `chat_retry.rs:845` 每个实际 attempt capture 时清空 timing；headers/body/effective 的 elapsed 使用同一个原始请求起点，`first_byte_ms` 则保留本轮首 Data 耗时。重试不重建 started、不再绑定 deadline。新 attempt 之前的重试间隙保留上轮身份/观测但清除 candidate snapshot，因此超时只结束请求，不覆写上轮候选。timing 只包含数值，timeout seed 保留首body且 headers 不填该值。
- **所有权 / history：未发现重复取消归属。** request/pump 交接和 timeout claim 使用同一状态锁；正常 finish 后同一计时器不再取消持久化。当前所有 response-history 获取点只暂存 record，唯一 KV 写入已移到 `stream/execution.rs:7046`，在 `finish_stream_request_deadline(:7037)` 之后；此前“慢 KV 被生成期限取消”的待核代码窗口已移除。并发与慢持久化行为仍由 root 验证。

仍需处理的 EOF 兼容问题：

1. **已报告 P2 仍存在：事件归一化/别名缺失。** 当前 `stream/execution.rs:4284` 仍只接受 data.type 中 completed/failed/incomplete，拒绝现有 parser 已支持的 `event: response.completed` + 无type的data，以及 `response.done`。证据与修复建议见上一节，不重复展开。
2. **新增 P2：合法同步 JSON 转 SSE 被要求提供原始 SSE 终态。** `:5911` 的 `maybe_bridge_standard_sync_json_to_stream` 已接受合法 Responses JSON 并生成 authoritative terminal summary/SSE，将 `sync_json_stream_bridge_active_for_report=true`；provider 原文仍是 JSON，tracker 不可能看到 SSE terminal。`:6866` 的新增 EOF guard 没排除这个桥接分支，且 Responses 没有 Chat 的 finish_reason 放行条件，故正常 Responses JSON 在 HTTP stream scope 下会被改判 `stream_missing_terminal_event/502`。应让已由桥接 finalizer 验证的 JSON 使用其权威终态，不套原始 SSE 标记要求。已有 `execute_execution_runtime_stream_bridges_sync_json_body_from_remote_runtime_to_sse`（`:11725`）直接调用内部 execution、没有新 HTTP deadline scope，不能拦住这个回归；定向验收需走真实 HTTP scope 或给同一用例加该scope。

本轮没有产品修改、Cargo 或重复前端测试。上述行号是本轮快照；最终行为证据、追加兼容修复及是否通过由 root 汇总。

## Sender 修复复核与第二轮验证交接

- **Sender 所有权修复静态通过。** `chat_retry.rs:160` 将 HTTP pump 的故障 sender 移入共享 deadline owner，外层 future 不再另留同一个克隆；`:440` 的正常 finish 关闭该 sender，超时分支则在 `:193` 取走并发送协议错误。`stream/execution.rs:7037` 在全部正常/错误末端发送之后 finish 并 drop 主 sender，随后才 await history KV；idle monitor 也不持有 sender。未发现正常生成结束后因这个克隆继续阻塞 Body EOF 的窗口。
- `chat_retry.rs:1355` 的边界单测现在断言 receiver 已 EOF 时 pump 尚未完成，再等模拟持久化越过原 deadline 完成。它覆盖 owner/sender 生命周期，**没有注入真实 KV 延迟，不能作为真实慢 KV 端到端证据**。
- 已直接读取 root 第二轮日志 `output/compact-timeout-diagnosis/rust-checks-1789448249945925000/0-stream_response_timeout.log:4`，确认 `stream_response_timeout_terminal_persistence_survives_generation_deadline ... ok`。因此 sender 克隆问题已获“代码复核 + 受控慢持久化 owner 单测通过”的证据，不仅是移动 history await 的静态判断；本 reviewer 未重复运行 Cargo。
- 复用 root 频道 04:59:47 的第二轮结果：`stream_response_timeout` 仍为 8过1失败，但原 Chat EOF 用例已通过；当前失败是 snapshot 矩阵等待 usage 终态超时。runtime 已接管其定位与定向 Cargo 槽，合法 incomplete 的既有状态分类应保留，不能只为新断言改成 completed。
- 截至本次读取，上节两类 P2（Responses event/type 与 response.done 兼容、同步 JSON→SSE 桥接）仍未落盘；继续保留开放状态，待 runtime 完成后仅复核这两处。本轮不因旧的 ready 消息并行启动 Cargo，遵循 root 最新统一验证分工；未修改产品文件或重复前端检查。

本轮可复核的 sender 修复已完成；整体审查仍待上述兼容修复及 root/runtime 的最终行为结果。

## 别名落盘后的限定复核

仅复核 sender/原始终态归一化及已报告的 JSON 桥接例外，没有扩展其它模块审查。

- **Sender 项关闭到当前证据范围。** 修复源码与 root 第二轮受控慢持久化 owner 单测均已核实；真实 KV 延迟未注入，保持上节限制。
- **合法输入兼容已修。** `stream/execution.rs:4289` 对缺失 type 的对象回退使用 SSE event，并增加 response.done；显式 type（包括 null）不被 event 覆盖。`:6879` 排除已经桥接 finalizer 验证的同步 JSON，避免要求其提供原始 SSE 终态。新增 snapshot event_only/done 分支；JSON 桥接测试已纳入 HTTP deadline scope 且断言没有 missing-terminal。测试仍由 runtime 运行，本 reviewer 未宣称其已通过。
- **原归一化 P2 尚有一处输入边界未对齐。** `:4289` 直接使用 Value::get 判断缺失，没有先确认 JSON object。因此正文输出后接 `event: response.completed\ndata: null\n\n`（或数组/字符串）再 EOF，tracker 也会标 completed。既有 `format_matrix.rs:204` 的 apply_sse_event_type 则要求 as_object_mut，非对象不补 type；Responses parser 将其视作未知事件，再由 finish 合成 Finish。gateway `execution.rs:875` 仅按 parser_error/observed_finish 判失败，无法靠 unknown_event_count 拦住这个假终态。此为静态确认的同一归一化边界问题，尚未运行反例；建议 tracker 先要求 JSON object，再按 contains_key/type/event 处理，并加一个 null/数组终态反例。产品文件仍交 runtime 修改。
- runtime 已报告 snapshot 首 case 的失败来自 fixture 给 completed 响应误填 incomplete_details，正在修 fixture；合法 incomplete 的既有 usage 分类将保留。该结果尚未最终回归，不据此变更产品协议结论。

本次限定复核完成；剩余一项终态归一化边界交 main/runtime，等待修复与最终定向结果。

## 定向测试结果与 incomplete 限制

已直接读取最终保存的 `output/compact-timeout-diagnosis/runtime-final-stream-response-timeout.log:466`（原 `/tmp/aether-runtime-timeout-tests.log`）：`stream_response_timeout` **10通过、0失败**（11.85s），含 sender/persistence、JSON 桥接和 snapshot 矩阵。**原4/5的合法别名与 JSON 桥接兼容问题据源码及本轮定向证据关闭**；上节非对象 payload 的 fallback 边界尚无代码修复及反例，仍开放。未另启 Cargo。已核对 `research/runtime-implementation.md` 中相同结果及证据边界；runtime 报告其 owned Rust 的 rustfmt/diff-check 通过，本 reviewer 未重复执行。

`crates/aether-usage/runtime/src/write.rs:2916` 的既有逻辑将 `response.incomplete_details.reason` 提取为错误；本轮 snapshot 用例已保留 incomplete 的 failed/200 和 max_output_tokens 原因，同时断言保留18个tokens、无 stream_missing_terminal_event。这符合本次“不将合法 incomplete 误报缺终态、不扩大修改 usage 分类”的范围。该限制应留给 main 汇总：**合法协议终态仍可能显示 usage failed；这里验证了 token 记录，没有验证计费金额，也不是多候选完整重试路由的独立证据**。真正状态分类是否另行统一，需要单独确定语义，不以本次测试修改掩盖既有差异。

sender 已有受控慢持久化 owner 单测通过；真实 KV 延迟未复现的限制保持。root 另行负责最终 HTTP 矩阵及产品验收。

## 最后 metadata 白名单小补丁复核

按 root 请求仅只读核对 `crates/aether-usage/runtime/src/request_metadata.rs` 补丁及 timeout payload 直接生产点，**未发现新问题**。

- `request_metadata.rs:381/:447` 的 copy/move 两条白名单同步加入 stream_timing 与 timeout_trigger，分别沿用现有结构值限额和非空字符串裁剪；merge 与最终 sanitize 均会保留这些字段，未知顶层字段仍过滤。
- `chat_retry.rs:770` 的 timeout payload 复用 end_to_end_time_ms/end_to_end_first_byte_time_ms。HTTP 总/首次超时的 elapsed 取原请求起点，后者取当前尝试已观测的首 body 在逻辑请求中的时间；headers 不填首 body。stream_timing 来自四个 Option<u64>，没有新增正文、密钥或 encrypted_content 复制。
- 新增 `stream_timeout_metadata_survives_both_terminal_merge_paths` 同时比较 borrowed merge、owned merge、sanitize 三入口，覆盖 heartbeat 后仍无有效内容的 null 里程碑及未知字段剔除；静态确认预期有辨别力。本 reviewer 未运行 Cargo，也未据单测源码声称最终落库已通过。
- root 已报告非对象 tracker guard 后11项、SQLite lifecycle 2项及 trace 13项通过；这些最终关闭结论仍由 root 汇总。启动心跳后 total 的实际 metadata 落库正在 root 重建后重验，不能以此前9种HTTP行为正确代替修补后的持久化证据。

本轮小补丁复核完成；未修改产品文件、未改变业务策略或重复运行验证。

## 预提交观测读回断言复核

仅阅读 `tests/scheduler_failover/stream_response_timeout.rs:91` 的最新扩展，未发现新增问题。headers-only/heartbeat-only 两分支都经真实 Router 超时后读取 usage repository：保留 headers 时间、无 first effective output；heartbeat 分支要求 first_byte_time_ms 与首 body 里程碑存在，并核对 end_to_end_first_byte_time_ms；headers-only 分支要求不生成首 body 时间。两分支同时保留 total failed/504、逻辑耗时、单候选、健康中性、容量释放与后续请求成功断言。

这些断言能够检测之前“预提交已收到 body，但终态 metadata 被过滤”的问题。当前只确认测试源码，尚不记为12项最终通过；root 最终过滤器需捕获该文件最新版本。本 reviewer 未改测试或启动 Cargo。

## 主会话最终修复与验证（2026-09-15）

- 非对象 Responses payload 已在 type/event fallback 前排除；null、数组、字符串、显式null type反例及随后合法事件均有定向用例。最新 stream_response_timeout 为11项全通过。
- 整请求 trace 优先 metadata.end_to_end_time_ms，保留 response_time_ms 的单轮语义；SQLite生命周期2项（119008ms/300008ms）与trace13项重新通过。
- 外部HTTP矩阵发现 stream_timing 被usage元数据白名单删除，现已同步copy/move过滤路径，复用现有端到端时钟字段。metadata单测验证两种合并与sanitize保留阶段时间且继续过滤未知字段。测试通过；最终HTTP回读结果见 verification.md。
- 本次新增/引入的已确认审查问题均已修复。保留此前明确记录的验证边界：真实供应商未调用，真实KV延迟未注入；incomplete沿用原分类。

## 交付结论

主会话最终HTTP/SQLite矩阵9/9通过，含此前被过滤的阶段时间回读；HTTP配置CRUD与SQLite重开验证通过。29个修改Rust文件格式及范围diff-check通过。当前任务已确认问题全部关闭，交付门禁通过，限制以verification.md为准。未提交/安装/发布。
