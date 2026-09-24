# CC Switch、SUB2API、New API 流式超时对照

核对日期：2026-09-15。来源为三个官方仓库 main 的固定提交：

| 项目 | 提交 | 核对范围 |
| --- | --- | --- |
| farion1231/cc-switch | 42ac174dbc42e0cf50a50e60c5f2c3dcecca4560 | 普通 HTTP 流、Claude→Responses 转换、表单及默认配置 |
| Wei-Shaw/sub2api | badfad8b7248b8aac0e6b503a06e392aa31cb294 | OpenAI Responses HTTP、响应头、首语义输出、流数据间隔 |
| QuantumNous/new-api | 5caafd3d84dc74c8b0d081524f57a534b3980cb0 | 共用 relay HTTP client 与 SSE scanner，OpenAI Responses 调用方 |

只读源代码与官方文档，没有运行这些项目、修改配置或复用其用户数据。源码副本在 /tmp/aether-timeout-research-20260915/；固定提交链接才是持久证据。当前主分支不代表用户供应商部署版本。

## CC Switch

- 普通流式处理显式区分 first_byte_timeout 与 idle_timeout。第一个 stream chunk 成功后，is_first_chunk=false，后续每次 stream.next() 使用独立 idle 期限。[response_processor.rs:701](https://github.com/farion1231/cc-switch/blob/42ac174dbc42e0cf50a50e60c5f2c3dcecca4560/src-tauri/src/proxy/response_processor.rs#L701)。
- 普通默认首字节 60 秒、静默 120 秒，Claude 初始化值为 90/180 秒；非流式默认 600 秒。界面首字节范围 1–120 秒，静默可填 0 关闭。[官方说明](https://github.com/farion1231/cc-switch/blob/42ac174dbc42e0cf50a50e60c5f2c3dcecca4560/docs/user-manual/en/4-proxy/4.3-failover.md#L117)；[表单](https://github.com/farion1231/cc-switch/blob/42ac174dbc42e0cf50a50e60c5f2c3dcecca4560/src/components/proxy/AutoFailoverConfigPanel.tsx#L70)。
- 200 headers 不足以代表首包：先等待 headers，再 prime 第一个 body chunk。[forwarder.rs:2367](https://github.com/farion1231/cc-switch/blob/42ac174dbc42e0cf50a50e60c5f2c3dcecca4560/src-tauri/src/proxy/forwarder.rs#L2367)、[prime_streaming_response:2641](https://github.com/farion1231/cc-switch/blob/42ac174dbc42e0cf50a50e60c5f2c3dcecca4560/src-tauri/src/proxy/forwarder.rs#L2641)。每阶段使用相同设置，不可声称它采用一个端到端首字节绝对截止时间。
- 这些流式 timeout 检查与故障转移开关相关；关闭故障转移时 streaming_timeout_config 返回 0/0。[handler_context.rs:259](https://github.com/farion1231/cc-switch/blob/42ac174dbc42e0cf50a50e60c5f2c3dcecca4560/src-tauri/src/proxy/handler_context.rs#L259)。
- 例外：Claude→Responses 转换在输出前做语义预读，复用 streaming_first_byte_timeout 包裹每次 stream.next()。每次有 chunk 会进入下一次等待；没有独立的从请求起算到语义结果的绝对计时器，持续小块可能延长预读，缓存达 256 KiB 会提交。[forwarder.rs:2555](https://github.com/farion1231/cc-switch/blob/42ac174dbc42e0cf50a50e60c5f2c3dcecca4560/src-tauri/src/proxy/forwarder.rs#L2555)。因此不能把 CC Switch 所有路径描述成“第一个 body 后再也不用首字节参数”。
- 普通 reqwest 流式请求用 24 小时底层请求期限，避免沿用短非流式总时长；不是绝对无限制。[forwarder.rs:2354](https://github.com/farion1231/cc-switch/blob/42ac174dbc42e0cf50a50e60c5f2c3dcecca4560/src-tauri/src/proxy/forwarder.rs#L2354)。

## SUB2API

- 通用响应头等待默认 600 秒；OpenAI/Codex 独立 response header timeout 默认 0（关闭）。[config.example.yaml:210](https://github.com/Wei-Shaw/sub2api/blob/badfad8b7248b8aac0e6b503a06e392aa31cb294/deploy/config.example.yaml#L210)。
- 首语义输出有独立配置 openai_first_output_timeout_seconds，默认 0（关闭）。普通正值允许 30–600 秒；high/xhigh/max 可另设 30–1800 秒 override。基础为 0 时 helper 直接关闭，不能只设 high override 就认为生效。[config.go:3300](https://github.com/Wei-Shaw/sub2api/blob/badfad8b7248b8aac0e6b503a06e392aa31cb294/backend/internal/config/config.go#L3300)、[openai_first_output_timeout.go:232](https://github.com/Wei-Shaw/sub2api/blob/badfad8b7248b8aac0e6b503a06e392aa31cb294/backend/internal/service/openai_first_output_timeout.go#L232)。
- 启用后是从 attempt startTime 起算的绝对语义输出期限（包括 headers 等待），不是每个 chunk 重置；完整进展事件后 stopFirstOutputTimer。[response_handling.go:213](https://github.com/Wei-Shaw/sub2api/blob/badfad8b7248b8aac0e6b503a06e392aa31cb294/backend/internal/service/openai_gateway_response_handling.go#L213)、[完成进展事件:304](https://github.com/Wei-Shaw/sub2api/blob/badfad8b7248b8aac0e6b503a06e392aa31cb294/backend/internal/service/openai_gateway_response_handling.go#L304)。默认关闭计时器不等于取消所有输出前缓存。
- 上游流数据间隔独立配置 stream_data_interval_timeout，默认 180 秒，0 关闭；图片另设 900 秒。扫描上游行更新 lastReadAt，独立 ticker 用距最近读取的间隔判断，不要求有效文本才更新。[config:480](https://github.com/Wei-Shaw/sub2api/blob/badfad8b7248b8aac0e6b503a06e392aa31cb294/deploy/config.example.yaml#L480)、[读取与超时:849](https://github.com/Wei-Shaw/sub2api/blob/badfad8b7248b8aac0e6b503a06e392aa31cb294/backend/internal/service/openai_gateway_response_handling.go#L849)。轮询式检查可能晚于精确静默期限，不可说一定恰好在 lastRead+180 秒触发。
- 下游 keepalive 默认 10 秒；它防止客户端/中间代理认为下游静默，但不会把上游 lastReadAt 刷新。不能拿网关自己发心跳当上游仍有进展。

## New API

- RELAY_RESPONSE_HEADER_TIMEOUT 默认 1800 秒，0 关闭，仅限制等待响应头，收到 headers 后不再约束 body 传输。
- RELAY_TIMEOUT 默认 0（不限制完整请求），非零写入 http.Client.Timeout，会涵盖 body 读取。因此短值可以中断正常长流。[service/http_client.go:95](https://github.com/QuantumNous/new-api/blob/5caafd3d84dc74c8b0d081524f57a534b3980cb0/service/http_client.go#L95)、[环境配置:64](https://github.com/QuantumNous/new-api/blob/5caafd3d84dc74c8b0d081524f57a534b3980cb0/.env.example#L64)。
- STREAMING_TIMEOUT 默认 300 秒。共用 scanner 每读到一行，先 ticker.Reset 再筛选 data 内容；上游 event 行、注释/心跳、空行也能重置，并不等首段文本或压缩结果。[stream_scanner.go:244](https://github.com/QuantumNous/new-api/blob/5caafd3d84dc74c8b0d081524f57a534b3980cb0/relay/helper/stream_scanner.go#L244)。它是流数据间隔超时，不是从请求发起后固定 300 秒总时限。
- RELAY_IDLE_CONN_TIMEOUT 是连接池闲置连接回收，与活动 SSE 请求中两段数据之间的等待不同。不要当作流式空闲超时使用。
- OpenAI Responses 使用上述共用 scanner。[relay_responses.go:81](https://github.com/QuantumNous/new-api/blob/5caafd3d84dc74c8b0d081524f57a534b3980cb0/relay/channel/openai/relay_responses.go#L81)。结论仅覆盖已追踪路径，不泛化到每种插件/特殊供应商。

## 对 Aether 规划的影响

1. 用户要求的“首批 body 到达后解除首字节超时”有 CC Switch 普通路径作为直接参照。
2. 语义输出等待作为策略本身存在，但 SUB2API 将其独立命名并默认关闭；不能把 Aether 的首字节监控与有效内容 watchdog 混成一项。
3. 已收到 body 不等于所有期限消失。若 4.9 秒后到 188.5 秒之间完全没有新上游行/块，独立静默限制仍可能阻断；若有定期上游心跳，按数据间隔的期限会被重置。当前真实请求没有中间事件时间线，无法推断换用它们一定成功。
4. 对用户要的“允许慢压缩持续运行”，首字节、上游数据间隔、可选语义输出、完整响应期限应分别说明。语义输出策略不默认强加；数据间隔应可独立关闭或按操作设置，不能未经确认新增另一个 180 秒截止条件。
5. 不改变已确定的 R1 状态一致性任务。仅补充研究证据，未批准实施，也未选择新的总期限或静默默认值。
