# 调度与故障转移初审

日期：2026-09-13。状态：实施中快照审查，非最终验收、非合并批准。

按 main 指令只读检查实现；仅新增本报告。未修改活跃实现文件，未 spawn，未运行 cargo、模型请求或生产操作。已读取 check.jsonl 全部引用、PRD、design、implement、implementation-contracts，以及 storage/config/ranking 交接契约和相关包规范。

下列复现场景来自静态调用链分析，尚未由 check 执行集成复现。health_policy、retry_runtime、storage/config/websocket/ranking 在审查期间持续修改文件，位置以落盘前读取为准。所有问题须在稳定版本复核；不能把接入中的缺口称为最终交付缺陷。

## 当前实现中的具体问题

### F1 / P1：同步异常 200 没有进入失败结算，仍可进入成功效果

- 位置：`apps/aether-gateway/src/execution_runtime/sync/execution.rs:2214`、`:2511`、`:2713`；`apps/aether-gateway/src/execution_runtime/fallback.rs:166`。
- 触发：OpenAI 同步上游返回 HTTP 200、JSON `{"error":{"type":"server_error","message":"failed"}}`，没有用户成功匹配规则。也应覆盖有效 JSON 但缺少合法模型终态的响应。
- 链路：transport 保留 HTTP status，sync 只对 Gemini 无效成功和非 JSON SSE 错误进行状态提升；`status_code >= 400` 才调用新的分类/扣分。`resolve_core_sync_error_finalize_report_kind` 能识别 embedded error，但后续成功效果仍用原始 `status_code < 400` 判断，未使用模型终态事实。
- 影响：异常 200 至少绕过失败扣分和默认故障转移；走到上述成功分支时会 +1、清连续失败并迁移亲和。不能以 HTTP body 读取完毕证明完整模型成功。对应 A4/A8、R10/R12。
- 建议复核：真实 `/v1/chat/completions` 非流式入口，K1 返回异常 200、K2 正常，读取 K1 健康和下一请求亲和。应使用已归一化错误事实控制失败/成功结算与重试，保留显式终止规则。
- Owner：retry_runtime + health_policy，当前仍在接入。

### F2 / P1：直接传输失败仍丢失来源，统一以 502 进入旧效果入口

- 位置：`apps/aether-gateway/src/execution_runtime/stream/execution.rs:3137`、`:3292`；`apps/aether-gateway/src/execution_runtime/sync/execution.rs:326`、`:1905`、`:2064`；`apps/aether-gateway/src/orchestration/effects.rs` 的 `record_health_failure_effect` 聊天分支。
- 触发：流式上游在响应头之前发生 FirstByteTimeout；令 transport 时限短于 candidate watchdog，可确定先走 transport error 分支。另测本地代理/请求构建配置错误。
- 链路：SSE `InProcessStreamExecutionError::Transport(err)` 无论类型都提交 `HealthFailure { status_code: 502, UseDefault }`；该兼容入口重新按 `UpstreamResponse` 分类。sync 的 `SyncExecutionFailure::from_transport` 同样未保留完整健康归因。
- 影响：真实首输出超时可能扣基础 2 而非 1；本地配置错误也无法保留中性归因。watchdog 的新 `UpstreamTimeout` 分支正确并不覆盖这些先返回的 transport 分支。对应 A4、R7/R10。
- 建议复核：分别注入传输首输出超时、上游连接拒绝、已知本地代理配置失败，检查 1/2/0 基础分及同一 attempt 的单次结算；在 typed transport error 尚未丢失处生成事实。
- Owner：retry_runtime；implementation-contracts 已说明 transport failure plumbing 正在接入，本项不是宣称 owner 已完成后又出现回归。

### F3 / P1：HTTP 状态引用只绑定“本次第一个候选”，没有绑定旧响应的原 K

- 位置：`apps/aether-gateway/src/execution_runtime/chat_retry.rs:213`、`:229`；`apps/aether-gateway/src/ai_serving/planner/standard/openai/responses/decision/request.rs` 的 `resolve_local_openai_responses_candidate_payload_parts`。
- 触发：先由 K1 创建可续接 Responses ID；使 K1 冷却或将 K2 调到首位，再通过 HTTP 提交该 `previous_response_id`，使用保留该字段的同格式 Responses endpoint。
- 链路：新 tracker 的 `continuation_target` 初始为空，第一次 `prepare_attempt` 直接把当前 plan 的 provider/endpoint/key 设为绑定，未查询该 response ID 的已有归属。之后阻止本请求继续换 K，不能阻止第一次就将 K1 的旧引用交给 K2。
- 影响：有状态续接仍可跨 K 发出旧引用。若格式转换或既有适配先移除引用，检查最终 plan body 的这个门也看不到原始续接事实。对应 A8、R23；只验证 WS 的 ownership/physical binding 不覆盖此 HTTP 路径。
- 建议复核：真实 HTTP 同格式请求，先保存 K1 响应 ID，再改变可用性/顺序，断言 K2 收到零条含旧引用的请求；无法证明原绑定时应明确失败。另核对既有跨格式 history expansion 与本轮 R23 的边界，由 main 统一决策，check 未改写旧历史机制。
- Owner：retry_runtime + main；HTTP continuation 正在补接，WS 原有 ownership 校验不能直接复用为 HTTP 已验收证据。

### F4 / P2：非限流错误中的 Retry-After 只约束当前请求，没有保存共享冷却

- 位置：`apps/aether-gateway/src/orchestration/classifier.rs:123`；`apps/aether-gateway/src/orchestration/chat_health.rs:117`；`apps/aether-gateway/src/execution_runtime/chat_retry.rs:21`、`:283`。
- 触发：K1 返回 503 和 `Retry-After: 30`，K2 成功；30 秒内发起第二个独立请求。
- 链路：503 被分类为 `rate_limited=false`，`project_chat_failure` 仅在 `fact.rate_limited` 时持久化冷却。HTTP tracker 能观察完整 30 秒并让本次请求跳过 K1，但其 task-local/逐请求状态不约束下一请求。
- 影响：后续请求仍可立刻调用 K1，绕过上游明确等待期限。R19 的完整期限约束未限定只有 429。429 的新 `project_chat` 保留长值不能证明此情况。对应 A5/R19。
- 建议复核：上述两次真实请求及后续权威健康读取；区分失败评分与显式 Retry-After 冷却是否需要落盘，不把普通无等待提示 500 自动变成共享冷却。
- Owner：health_policy + retry_runtime。

## 已知接入缺口，等待 owner 收敛

### G1：执行前准入、探测所有权和长租约

`candidate_materialization.rs:301`、`:339` 仍可能在规划阶段 claim；`chat_retry.rs:262`、`:309` 只在本请求已有 attempt 时重查，且发生在外层执行 gate 之前。`candidate_loop.rs:295` / 流式 watchdog 的 `acquire_upstream_execution_gate` 之后没有统一、带 owner token 的健康/冷却原子准入。

明确复核场景：预先生成普通 K1 候选并让其排队，另一请求将 K1 熔断，然后释放队列；当前旧候选仍可能开始调用。另将合法探测拖长超过原 60 秒 reservation，再发第二请求；取消探测后立即检查释放。现有 claim enum 不携带所有权，不能把重复 claim 当作续租。此缺口已被 health_policy 在 implementation-contracts 明确承认，由 main/retry_runtime 统一接入，非新增最终 bug 认定。对应 A6。

### G2：凭据代次与真实 plan、完整请求生命周期的连接

本轮复读已看到 `effects.rs:183` 拒绝缺失 identity，`:187` 校验 `planned_chat_credential_fingerprint`，不再走普通 CAS 回退；HTTP 和 WS begin 也已调用 capture。这是有效进展。

仍须证明每个真实入口都在 plan 使用凭据确定之后、真正调用之前携带 planned fingerprint，且没有字段丢失；`capture_chat_health_attempt` 读取当前 key/transport 的摘要本身不能证明旧 plan 用的是相同凭据。先规划、轮换凭据、再释放排队请求，以及旧 circuit epoch 的迟到成功，是必要运行场景。检查缺失字段处理、OAuth 刷新后真实重发计次及内部执行入口，不能只证明手工构造 captured fence 的 effects 单测通过。Owner：health_policy + retry_runtime + websocket，A6/A7。

### G3：亲和并发排序元数据的成功写入

`scheduler/affinity.rs` 已新增 `scheduler_affinity_request_order` 解析；当前读取的 `effects.rs:789` 仍调用原 `remember_scheduler_affinity_target_for_epoch`。ranking-contract 和 implementation-contracts 明确说明排序 token 与 effects 写入正在由 ranking/health_policy 整合。

复核场景：同 session 两请求按旧、新顺序开始，新请求先在 K2 完整成功，旧请求后在 K1 成功，后续请求仍应读 K2；同时覆盖模式切换。已有 epoch 能防配置切换，单独不能排序同 epoch 请求完成。成本比较器、真实亲和读取及 CostBased 成功写入模式判断已接入，不能把这些模块测试视为并发保护已证明。对应 A1/A7。

## 审查期间已变化，不继续列为开放问题

- 原来每个候选分别分配 slots、运行期未限制同 K 总次数；落盘前 `chat_retry.rs:239` 已增加 `max_attempts` 初始化，`:257` 已检查同 K+格式累计次数。需要稳定版本覆盖同 K 多 endpoint 候选，但不再报告“完全没有运行期次数上限”。
- 原来缺失 captured identity 时回退普通 CAS；落盘前 effects 已改为诊断并返回，不再报告该回退仍存在。
- SSE prefetch 失败 helper 初读将非超时都归为协议失败；复读 `execution_failures.rs:370` 已按 `candidate_status_code.is_some()` 保留上游响应来源，不再报告“所有 SSE 429 均扣基础 2”。
- 原来流式入口只读 system config；当前 `stream_path.rs` 建立逻辑预算，`chat_retry.rs:55` 以首个 plan 的 provider timeout 配置设定截止，`aether-provider/transport/src/network.rs:35` 已投影 provider failover_rules。尚待真实跨 K/输出后继续运行的验收，不再报告配置完全无效。
- `recovery.rs` 的 `SameCredential -> RetryNextCandidate` 已修正。main 提供的旧 orchestration 单个失败不能据此认定当前仍坏。

## 本轮证据与范围

- 已检查：共享 classifier/health/effects、HTTP candidate/transport/sync/SSE failure 和成功入口、WS turn/connection/quota/ownership/settlement、memory/SQLite 原子结算、配置 resolver/管理写入/UI payload，以及 ranking/affinity 读写连接。未声称对所有变更行完成最终审查。
- 静态看到 SQLite receipt INSERT 与健康 UPDATE 处于同一事务，冲突回滚 receipt；memory 在同一写锁内完成相同动作。具体并发、重投和存储错误行为采用 storage-contract 中的 owner 运行证据，check 未重复运行。
- main 告知 `cargo check -p aether-gateway --tests` 通过；orchestration filter 为 159 通过、1 失败，可能取到中间版本；真实网关集成 filter 尚在进行。以上均为 main 提供的状态，不是 check 运行结果。
- TypeCheck/Lint/Test：本轮未执行，遵守 main 的构建互斥和只读审查要求。没有以编译通过替代 A1-A11。
- A1/A2：排序与尝试解析已接入，真实 API/后续绑定由 main 集成验证；A3/A7：可见公式及仓储测试，真实终态覆盖仍须复核；A4/A5/A8：优先复现 F1-F4；A6：等待 G1/G2；A9：provider 投影已接入，预算跨 K、有效输出后继续和中性耗尽尚未由 check 运行；A10/A11：UI 和规范化有改动，保存刷新、迁移和非聊天兼容采用 owner/main 证据，check 未作页面验收。

初审完成。等待 main 发起稳定版本的最终全范围复核；届时再决定机械修复与针对性校验。本报告没有授权或执行提交、发布、生产迁移。

## HTTP fixture 失败后的只读补查

main 最新状态：六项真实 HTTP fixture 失败且 `receipts=[]`，integration 独占 gateway cargo；health_policy 已结束，effects 由 main 接手，storage 正在接 probe lease，ranking/retry_runtime/WS 仍活跃。本轮继续不修改这些文件、不执行 cargo。

### T1 / P1：fixture 上游监听路径与 endpoint base URL 不匹配

- 位置：`apps/aether-gateway/src/tests/scheduler_failover/support.rs:167`、`:188`、`:236`；`apps/aether-gateway/src/tests/mod.rs:45`；`crates/aether-provider/transport/src/request_url/mod.rs:154`；`crates/aether-provider/transport/src/url.rs:28`。
- 静态复现条件：fixture 假上游仅注册 `/v1/chat/completions`，`start_server` 返回裸 `http://{addr}`，该值直接写入 endpoint base URL，custom path 未设置。生产 `openai:chat` builder 在 base URL 后拼接 `/chat/completions`，因此命中不存在的 `/chat/completions`，得到 404。
- 影响：`receipts` 在假上游匹配路由的 handler 内才追加，记录的是实际收到的模型请求，而非健康结算 receipt。上述路径不匹配可以同时解释所有用例没有 receipt；不能由此认定六项调度行为均已被真实执行后证明失败，也不能把 effects 结算缺失当作空 receipt 的原因。
- 建议 integration 将 fixture 保存的 endpoint base URL 补为 `{origin}/v1`，保留生产 URL builder 语义，重跑原 filter 后再判断是否还有调用前拒绝或运行期问题。
- Owner：integration；check 仅核对代码契约，没有修改 fixture 或运行该复现。若 owner 正在修复，以其稳定结果为准。
