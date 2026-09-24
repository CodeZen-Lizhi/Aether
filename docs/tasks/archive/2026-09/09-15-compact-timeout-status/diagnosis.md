# 诊断与证据（2026-09-15）

## 真实失败请求

SQLite 以 `mode=ro` 打开：`/Users/zhenglizhi/Library/Application Support/com.aether.desktop/aether.db`。
日志：同目录 `logs/aether-gateway.2026-09-15.log`。仅读取状态、时间与事件字段，未重放正文或导出凭据。

| 时间 / 日志行 | 已确认事实 |
| --- | --- |
| 10:49:56.560 / 43684 | 接收 909d896c-9581-4519-9c63-7836b6ad68c5，POST /v1/responses |
| 10:49:56.561 / 43687 | 请求体 1177147 bytes，缓冲 elapsed_ms=0 |
| 10:50:02.109 / 43706 | 第一轮 HTTP 200 响应头，5534 ms |
| 10:52:56.576 / 44057 | 第一轮首有效输出 watchdog，timeout_ms=180000；candidate failed/504，latency_ms=180003 |
| 10:52:57.524 / 44062 | 第二轮重试开始，对应截图详情 2/2 的开始时间 |
| 10:53:04.302 / 44081 | 第二轮 HTTP 200 响应头，6778 ms |
| 10:54:56.576 / 44496 | 请求整体 HTTP 504，elapsed_ms=300014 |

最终 usage：compact / failed / 504，response_time_ms=300008，first_byte_time_ms=null，错误 `Streaming failover budget exhausted before useful model output`。第二轮候选 cancelled/504，error_type=stream_failover_budget_exhausted，latency_ms=119056。

此次最终状态已经落库，不能套用旧任务里“usage 永远 pending”的结论。180 秒是第一轮等待，300 秒才是本次逻辑请求的整体首输出预算。

只读检查时供应商 input 当前值已是单轮 300 秒、总预算 420000 ms；不能用当前值倒推运行中请求采用的快照。执行记录是 180000 / 300000，运行中的计划和总截止时间不会因用户后来改配置自动延长。

## 成功对照与耗时解释

紧随其后的请求 30483f13 于 10:54:57.383 开始（日志 44497）。10:55:02.293 收到响应头（44516）。10:58:05.932 首个有效事件 `response.output_item.done`（45021），elapsed_ms=188533，first_data_ms=4899，first_output_timeout_ms=300000。随后 HTTP 200 完成（45022），最终 usage completed/200，response_time_ms=188669。

因此本机已有“首批数据约 4.9 秒，但首有效压缩结果要 188.5 秒”的真实成功样本。早期网络响应不表示压缩结果已生成。三条连续失败 1c12fad2、44727dfb、909d896c 均是约 300 秒总预算耗尽，不应据此描述所有压缩都会失败。

OpenAI 官方说明压缩会处理已有上下文，生成承载状态与推理的加密 compaction item；它不是本地 ZIP/gzip。参考 https://developers.openai.com/api/docs/guides/compaction 。文档未给出本供应商此请求的内部耗时分解。失败请求没有首有效事件，无法区分供应商推理、排队及兼容事件异常，不能保证延长等待必然成功，也不能证明截图 high 是独立压缩耗时的根因。

## 状态冲突的代码路径

1. `apps/aether-gateway/src/handlers/admin/observability/usage/summary_routes.rs:312`：对 active usage 从当前候选推断终态，并把候选 latency_ms 写成整体 response_time_ms。`:357` 只有 error_flow.retryable=true 且 decision=retry_next_candidate 才跳过失败推断。
2. 真实第一轮候选的 extra_data 没有 error_flow；watchdog 在 `apps/aether-gateway/src/executor/candidate_loop.rs:1654` 写 failed/504。第一轮结束与第二轮创建之间约 0.95 秒存在把整个请求推断为 failed 的窗口。
3. `frontend/src/features/usage/composables/useUsageData.ts:398` 与 `frontend/src/features/usage/utils/recordSync.ts:131` 将 failed 作为不可回退的状态，随后较新的 pending 重试快照被拒绝。
4. 详情重新读取时第二轮已启动，候选不再推断终态；`detail_routes.rs:239` 同一 helper 返回空覆盖，故能得到 pending。截图符合“列表缓存旧单轮失败、详情看到当前重试”。没有保存截图时刻的 HTTP JSON，具体轮询取到旧状态的瞬间仍是据时序与代码的推断。
5. 另有直接展示冲突：`frontend/src/features/usage/utils/status.ts:242` 把 pending+504 显示为 failed，而 `:288` 的详情总状态保留 pending。这个冲突已以当前真实 helper 运行复现。

## 已执行的最小复现

- 在 frontend 目录用 TypeScript transpileModule 原样加载 status.ts，输入 pending/504/Stream first effective output timeout；输出 list=failed、detail=pending，并以 exit 1 断言冲突。没有替换业务函数。
- 同样原样加载 recordSync.ts（仅将 cyberError 模块转换成 data URL），输入 10:52:56 failed，再输入较新 10:52:57 pending；结果 accepted=false，仍为 failed。这里观察的是防回退规则如何保留错误终态，不建议直接取消全部终态保护。
- 安装版网关 + localhost 合成上游 + 临时 SQLite，只复制真实库 schema。`DIAG_FIRST_TIMEOUT=1 DIAG_BUDGET_MS=1800 DIAG_OUTPUT_DELAY=3 DIAG_EXPECT_FAILED_TERMINAL=1 DIAG_ONLY_GATEWAY=1 python3 output/compact-timeout-diagnosis/reproduce.py`，复现单轮 watchdog/504，最终 usage failed/504。结果 `output/compact-timeout-diagnosis/run-1789441241152374000-0c49761a/results.json`。
- 同脚本改 `DIAG_FIRST_TIMEOUT=5`，仍在 1804 ms 被总预算切断，最终 usage failed/504。结果 `output/compact-timeout-diagnosis/run-1789441287186764000-ab43af3d/results.json`。证明只延长单轮计时器不足。

本轮未修改产品代码，未做修复后验收。两轮重试间隙经真实列表/详情 API 再到前端状态合并的完整自动回归，列为实施首项门禁。

## 用户澄清后的语义结论

本轮用户明确要求：首批上游响应体数据已达后，首字节期限必须解除，后续处理只能受独立整体期限等结束条件约束。此前诊断的“首有效输出 watchdog”描述的是现有代码，不代表用户认可该产品语义。补充确认：candidate_loop.rs:1375 使用 first_byte_ms，却在 stream/execution.rs:5996 有效内容判定后才解除；stream_first_data 的记录位于 :5663。现有设置名、监控指标与取消条件确实不是同一个里程碑。

300 秒的 stream_failover_budget_ms 是跨重试的首有效输出预算，不是流式完整响应总超时；transport.rs:2940 非流式总超时对 is_stream 返回 None。不能因为要保留整体期限，就把这两个值不加说明地混用。600/900 秒的压缩专用首有效输出方案已撤回，转为修正首字节语义。历史诊断和复现结果仍有效，修复方向以最新 PRD/design 为准。

## 最新语义确认：成功响应头即结束首次等待

用户进一步明确，首次限制是等待上游开始响应；HTTP 200 本身即可结束该阶段，不需要等 body，更不需要等文本或压缩结果。后续只接受独立完整响应时间限制。本次不新增静默超时，也不新增首有效输出超时。此前规划中的“headers-only 仍须等到 body 才能解除首次限制”已被这一明确决定替代。

当前实现中的“有效压缩结果”具体是识别到 compaction 项中非空 encrypted_content（stream/execution.rs:4705）；有效输出标记在 :5996 调用 chat_retry::mark_useful_stream_output（chat_retry.rs:455），同时解除候选 watchdog 与首输出总预算。这是现有应用的语义内容判据，不是 HTTP 协议对首次响应的要求。
