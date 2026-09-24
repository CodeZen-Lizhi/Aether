# 普通会话与压缩会话全量超时审计（2026-09-15）

## 范围与结论

读取本机所有现存 usage、request_candidates 及 2026-09-10 至 09-15 网关日志；排除单纯 key_circuit_open、冷却、认证、上游业务拒绝，不将 HTTP 503 一律算作超时。运行中数据库会继续增长；审计脚本快照包含 6750 条 usage。历史候选比 usage 保留更久，缺失主记录不能按普通/压缩分类。

结论：存在三类不同现象，不应统一调大超时处理。

1. 已证实大量本地首有效输出 watchdog 超时（30 秒），以及 90 秒跨候选首输出预算耗尽。
2. 已用安装二进制复现 Responses 普通文本/压缩快照事件被漏识别：已有输出内容仍被判作没有首输出。
3. 最近首字正常、运行约两分钟后失败的请求是 TLS 连接异常关闭；两条请求在同一毫秒断开，且代理日志显示随后切换了出口节点。不能将这类错误归因为 Aether 30/60/90 秒计时器。

## 统计

- 全部现存候选：1081 次 local_stream_candidate_watchdog_timeout，涉及 912 个请求，全部走 `/v1/responses`。
- 其中 428 个请求仍有 usage：普通 chat 最终失败 367 个（379 次超时尝试）、compact 最终失败 35 个（46 次超时尝试）、普通 chat 重试后成功 26 个。
- 能与同一次 candidate 的上游 headers 日志关联的 watchdog 尝试为 38 次，均是 30 秒内 HTTP 200，其中 24 次在 10 秒以内。其余 1043 次未匹配 headers 日志，不能据此补写不存在的首字证据。
- 另有 5 次 stream_failover_budget_exhausted，2 次上游返回 stream_timeout。
- 主记录里另有 10 条“pending 超过 10 分钟”清理错误，这是悬挂记录被维护任务终结，不能当作原请求在网络中完整运行十分钟的证据。此前截图请求的 pending 也可能后续被该任务改为 failed。
- 成功对照：49 条普通 chat、16 条 compact 首批数据 <30 秒且总耗时 >90 秒；最长普通 chat 376972 ms。
- 主记录中首批数据 <30 秒且运行 >30 秒的失败，错误为上游过载、TLS EOF、上游明确拒绝等；没有查到这些已记录首批数据的行以 Stream first byte timeout 终结。

逐候选脱敏记录：`output/timeout-records-audit/timeout-attempts.csv`；结构化日志关联：`output/timeout-records-audit/audit.json`；生成脚本 `audit.py`。未输出正文、密钥、请求头或订阅凭据。

## 具体请求

| 请求前缀 | 类型 | 首部/首批数据 | 总耗时 | 原因 |
| --- | --- | --- | --- | --- |
| b4710372 | chat | headers 3699 ms | 60625 ms | 第一候选 watchdog 30006 ms，最终 503；不能认定 60 秒是单个计时器 |
| f5c0d65e | chat | headers 6279 ms | 35601 ms | watchdog 30006 ms |
| b47a513c | chat | first_data 6328 ms，response_ready 11117 ms | 122431 ms | TLS peer closed connection without sending close_notify |
| b1c79e1b | chat | first_data 16554 ms，response_ready 22371 ms | 132359 ms | 同上 |
| 5c2cd7c9 | chat | first_data 6884 ms | 248047 ms | completed 成功 |
| ff1b0e6a | chat | first_data 8669 ms | 321389 ms | completed 成功 |
| 50012af4 | compact | first_data 6247 ms | 187716 ms | completed 成功 |

注意：监控 first_byte_time_ms 可能记录的是首批上游数据；真正解除首有效输出门槛的时刻对应 response_ready 之前的 useful 判定。首部、首 SSE 数据、有效内容、完整完成是不同里程碑。

## 本机代理关联

`Input-国际线路` 的代理配置指向名为“代理”的手工 HTTP 节点 `127.0.0.1:7890`。Clash Verge 本机 `verge.yaml:54` 为 `auto_close_connection: true`。

两条请求在 2026-09-15 00:11:18.931/932 报 TLS EOF（Aether `aether-gateway.2026-09-15.log:502,504`）。Clash `service_2026-09-15_00-07-53.log`：

- 120、149 行：00:09:06 / 00:09:16，aether-gateway 到 input.codes 经新加坡专线 02。
- 380、387 行：00:11:19，重连改经台湾专线 01。
- 574 行：00:12:49，后续请求又改经香港专线 05。

官方设置说明：自动关闭连接会在代理组选中节点或代理模式变动时关闭已建立连接，来源 https://github.com/clash-verge-rev/clash-verge-rev/blob/dev/src/locales/zh/settings.json 。该设置开启、出口切换、同步断开三项高度吻合，但日志没有明确记录每次“关闭所有连接”的触发者，不能把所有历史失败都归因于节点切换。

建议在 Clash 设置/杂项设置关闭“自动关闭连接”，长请求期间保持出口稳定。未修改 Clash 设置；该建议不会修复网关自身的 Responses 事件识别缺陷，也不能防止出口本身故障。

当前 Input-国际线路 已被设置为 300 秒单次、600000 ms 总预算；其他所查供应商仍为默认。两分钟后的 TLS EOF 仍发生，因此继续增大该供应商超时不解决这组断线。

## 确认的代码缺陷与反证

`apps/aether-gateway/src/execution_runtime/stream/execution.rs:4674` 只识别特定 delta、function_call added、response.completed 等事件，漏掉带非空结果的 response.output_text.done 和 response.output_item.done。普通 Responses 兼容输出可能只提供快照而没有增量；compact item.done 在仓库既有 fixture `crates/aether-ai/formats/src/formats/shared/sync_products.rs:4957` 中明确存在。

隔离模拟上游在 100 ms 发出有效内容，3 秒后 completed，Aether 候选阈值设为 1 秒：

- 正常非空 text.delta：完整成功，可超过 1 秒及 2.5 秒逻辑预算（见 diagnosis.md 追加验证）。
- 普通 chat，text.done：1 秒 watchdog，网关不返回已收到内容；直连正常。结果 `output/compact-timeout-diagnosis/run-1789402704625145000/results.json`。
- compact，output_item.done + compaction.encrypted_content：同样 1 秒 watchdog。结果 `output/compact-timeout-diagnosis/run-1789402719020199000/results.json`。
- 超过 256 KiB 的 response.created 后接正常 delta，再延迟 3 秒完成：成功，不能把“大启动帧必定触发超时”作为结论。结果 `output/compact-timeout-diagnosis/run-1789402736447191000/results.json`。

复现命令见脚本 `output/compact-timeout-diagnosis/reproduce.py` 的 DIAG_OUTPUT_KIND / DIAG_OPERATION 参数。只使用 localhost 与合成凭据，生产正文未重放。

虽然代码缺陷已复现，历史超时请求没有事件正文或事件类型时间线，尚不能证明 367 条普通失败全部由该遗漏导致。

## 30/60/90 的实际含义

- 30 秒：候选在首有效输出前的 watchdog；headers/启动事件不会解除；识别有效输出后解除。
- 60 秒：本路径的上游空闲监控只写日志，不取消 HTTP 流；非流式与 tunnel 的后备值是其他路径。
- 90 秒：首次有效输出前，跨选择/排队/重试/候选共享的预算。完整输出不限为 90 秒。
- 上游或本机代理异常关闭连接可在任意时刻中止，包括已出首字后的 2–3 分钟。

实施方向：修复有效内容判定、超时 usage 终态及错误归因，补不含正文的阶段时间证据；保留现有显式配置，不以修改全局超时或忽略 TLS EOF 代替修复。
