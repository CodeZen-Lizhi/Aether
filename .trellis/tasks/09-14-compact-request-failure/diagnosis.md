# 会话压缩失败诊断

## 结论

截图请求 `58b85d96-ffff-4c50-bfa2-d69b71b422b8` 的直接失败机制已确认：Responses 流式压缩沿用普通流式首有效输出等待规则，前两次候选各在约 30 秒被 watchdog 中断，第三次在逻辑请求的 90 秒总预算到期时被取消，网关返回 504。不能把它归因为请求体大小限制，也不能据 HTTP 200 保证供应商最终一定能完成压缩。

另已确认独立缺陷：总预算到期后只终结 candidate，没有终结 usage；HTTP 504 对应的 usage 长期停在 pending，详情缺少最终错误和耗时。

## 本机证据（只读、脱敏）

来源：`~/Library/Application Support/com.aether.desktop/logs/aether-gateway.2026-09-14.log` 与同目录 `aether.db`，数据库以 `mode=ro` 打开；未导出凭据或会话正文。

| 项目 | 结果 |
| --- | --- |
| 实际请求 | POST `/v1/responses`，`openai_responses_stream`，usage.request_type=compact，is_stream=1 |
| 接收请求 | 23:21:42.178，日志 12287 行 |
| 请求体 | 1,367,110 bytes；完整缓冲成功，日志 12290 行 |
| 第一次上游 | HTTP 200，6,550 ms 收到响应头，日志 12292 行 |
| 第一次失败 | 30,004 ms，local_stream_candidate_watchdog_timeout，日志 12324 行 |
| 第二次上游 | HTTP 200，5,080 ms 收到响应头，日志 12364 行 |
| 第二次失败 | 30,006 ms，同一 watchdog，日志 12516 行 |
| 第三次上游 | HTTP 200，6,021 ms 收到响应头，日志 12537 行 |
| 最终结果 | 第三候选 cancelled / stream_failover_budget_exhausted；总 90,034 ms，HTTP 504，日志 12603 行 |
| usage 终态 | 仍为 pending，status_code/error_message/response_time_ms 均空 |
| 两个候选供应商配置 | request_timeout 与 stream_first_byte_timeout 均未显式配置 |

对照请求 `d63b90ca-9c83-4e9f-90ca-592dbf44610d`：同模型、最终同供应商 input 的普通流式会话成功；请求体 18,382,141 bytes（约 17.5 MiB）、输入 164,237 token、首字 12,098 ms、总耗时 20,705 ms。比失败请求大很多，说明字节体积本身不是本次已观察到的拒绝原因。

审计配置为 body_capture_mode=none，四类 body_state 均 disabled。因此无法回放原请求，不能判定上游当时究竟没有发有效内容，还是发了网关未识别的压缩事件；也不能证明放宽超时后原请求必然成功。

## 代码依据

- `crates/aether-ai/formats/src/formats/openai/responses/mod.rs:217`：普通 Responses 请求中的 compaction_trigger 标记为 compact 操作；它与旧 `/responses/compact` 是不同传输形态。
- `crates/aether-provider/transport/src/network.rs:14`：未配置时首字节超时默认 30 秒。
- `apps/aether-gateway/src/executor/candidate_loop.rs:1349`：候选 watchdog 直接取 first_byte_ms，不区分压缩操作；`:1566` 开始计时，`:1583` 仅 terminal_started 豁免。
- `apps/aether-gateway/src/execution_runtime/stream/execution.rs:4644`：response.created/in_progress/queued 属于启动帧；`:4674` 的有效输出判断只认可特定文本、推理、工具事件及 completed，未单独处理 compaction item。
- `apps/aether-gateway/src/execution_runtime/chat_retry.rs:304`：有效输出通知同时解除候选 watchdog 与请求总预算。
- `apps/aether-gateway/src/executor/stream_path.rs:41`：普通 Responses 流式计划被 90 秒首有效输出预算包裹，旧 Compact 计划不在该名单。
- `apps/aether-gateway/src/execution_runtime/transport.rs:63`、`:2940`：旧非流式 Compact 的默认总超时是 1,200 秒，不适用于本次流式 Responses 压缩。
- `apps/aether-gateway/src/execution_runtime/chat_retry.rs:425`：总预算到期 drop future，仅写 candidate cancelled，未补 usage 终态。

## 隔离复现

脚本：`output/compact-timeout-diagnosis/reproduce.py`。使用已安装 Aether 网关二进制、从本机数据库只复制 schema 的临时 SQLite、合成密钥及 localhost 模拟上游；不连接真实供应商、不修改真实数据库。

模拟上游立即返回 200 和 response.created，延迟输出 LOCAL_OK。使用缩短阈值加快复现。

1. `python3 output/compact-timeout-diagnosis/reproduce.py`：直连成功；网关 200 ms 候选 watchdog 超时，candidate 错误与真实请求一致。最后回退为 503/no_local_stream_plans，说明该耗尽分支另有误导错误文案，未在本次扩大修复范围。
2. `DIAG_FIRST_TIMEOUT=2 DIAG_EXPECT_SUCCESS=1 python3 output/compact-timeout-diagnosis/reproduce.py`：仅放宽候选阈值，网关 200、收到 LOCAL_OK、usage completed。
3. `DIAG_FIRST_TIMEOUT=5 DIAG_BUDGET_MS=1800 DIAG_OUTPUT_DELAY=3 DIAG_EXPECT_PENDING=1 python3 output/compact-timeout-diagnosis/reproduce.py`：直连成功；网关因逻辑请求预算到期返回 504，usage pending，复现原请求的终态丢失。

脚本断言实际 watchdog 错误、成功输出或 504+pending；生成的 results.json 含候选状态与合成上游记录。

## 建议修复范围

1. 以实际请求操作识别流式压缩，为其设置独立且有界的首有效输出策略；候选阈值和逻辑请求预算必须共同处理。只修改旧 Compact 总超时或供应商普通 request_timeout 对本次流式 watchdog 无效。
2. 明确压缩协议中的有效输出事件，补 compaction item 相关用例；不能把 HTTP 200、心跳或 response.created 当作已成功完成。
3. 总预算取消路径补齐幂等 usage 失败终态，保留 504、明确错误分类和总耗时，刷新页面后仍应一致。
4. 验证长首输出压缩、普通 chat 超时、空心跳、候选重试、504 后查询状态；保持普通请求现有规则。

本次交付为诊断与复现，未修改产品代码或供应商配置；具体阈值及产品修复需后续实施设计。

## 追加验证：已出正文后能否超过首输出阈值

用户明确指出：首输出在阈值内到达后，完整回答超过该阈值不应超时。该预期正确，不应仅通过放宽阈值掩盖正文识别或计时器解除问题。

执行 `DIAG_FIRST_TIMEOUT=1 DIAG_BUDGET_MS=2500 DIAG_OUTPUT_DELAY=0.1 DIAG_AFTER_OUTPUT_DELAY=3 DIAG_EXPECT_SUCCESS=1 python3 output/compact-timeout-diagnosis/reproduce.py`。合成压缩请求仍走普通 Responses 流：模拟上游在 100 ms 后输出文本，再等 3 秒才完成。网关完整成功，HTTP 200，总耗时 3476 ms（超过单次 1000 ms 和逻辑请求 2500 ms 两个阈值），收到 response.completed，usage completed。结果在 `output/compact-timeout-diagnosis/run-1789401025366350000/results.json`。

这证明当前安装二进制的标准文本输出路径并非按整段回答时长截断。生产对照请求 `492d220a-e7fc-4f9a-8e99-8eea5d765c61` 也已成功完成：首字 6071 ms、总耗时 35641 ms。

原失败请求仍只确认在 5–6 秒收到响应头，未保留事件正文，不能称“已收到正文后仍于 30 秒断开”。若真实上游在 30 秒内已发送压缩数据而未被有效输出识别器认可，则是事件识别问题；若直到 30 秒仍只有响应头/启动事件，则是首有效输出确实超过阈值。二者需真实事件类型与时间戳才能最终区分。
