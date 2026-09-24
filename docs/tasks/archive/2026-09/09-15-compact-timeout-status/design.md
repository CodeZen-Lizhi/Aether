# 技术方案：首次响应、完整期限与请求终态

状态：已批准并进入实施。需求以 prd.md 为准，完整期限缺省 900 秒。

## 1. 生命周期与请求状态

请求 lifecycle 是列表、卡片、详情总状态的共同依据；candidate 只代表单轮尝试。active usage 不能因最新 candidate failed/504 或缺少 retryable 元数据而提前合成为终态。候选耗时与请求耗时独立。

修改 admin usage records/detail 的候选终态兜底，先判断权威请求生命周期。保留无 lifecycle 历史兼容及图片显式终态；不通过删除所有兜底隐藏旧数据问题。前端 status helpers 同口径处理 active+候选错误；recordSync 真终态防陈旧保护继续保留，不靠允许任意 failed→pending 回退修补。

入口：apps/aether-gateway/src/handlers/admin/observability/usage/{summary_routes,detail_routes}.rs，frontend/src/features/usage/utils/{status,recordSync}.ts、composables/useUsageData.ts、components/{UsageRecordsTable,RequestDetailDrawer,HorizontalRequestTimeline}.vue。

## 2. 两个独立计时状态

每轮请求有首次响应状态；整个逻辑请求有不可延长的完整截止时间。成功 headers 标记该轮已响应，只解除首次期限，不触发完成、计费或健康成功。收到失败 headers 进入错误路径，临时 1xx 不解除。下一轮重新持有自己的首次等待，但共享原整体截止时间。

首次限制需要核查并同步：candidate_loop.rs 的 watchdog、execution_runtime/transport.rs 的 request.send 及 await_stream_body_first_item、stream/execution.rs 的 framed reader/预读路径、隧道网关侧响应解包。对本次聊天 HTTP 流式，在成功 headers 后不再使用首次配置等 body。

完整期限由逻辑请求级共享上下文保存 started/deadline/取消原因/终态归属。首次确定的执行供应商总期限绑定到请求起点；候选切换和响应头不能延长。上下文显式传给响应 pump，不能仅依赖 task_local 或包裹返回 Response 的 future；返回 Response 后仍需覆盖上游读取、转换和客户端转发。预算到期取消上游并使用既有失败终结路径释放 permit/probe、完成 usage/candidate 结算。终结持久化不被同一生成期限中断。

现有参考：chat_retry.rs:288 的预算上下文和 :552 的 request scope、:149 的 spawn_chat_stream_pump、candidate_loop.rs:1375 的 watchdog；这些用于复用生命周期方法，不继续复用“首有效内容到达才解除”的语义。

旧 stream_failover_budget_ms 对本次 HTTP 聊天流退出执行计时，避免仍有第三个隐藏的首有效输出期限。其他协议消费点先核实并保留既有行为，不能全局机械删除共享字段。

## 3. 配置、公共契约与兼容

- 现有 stream_first_byte_timeout 存储/API 字段保持兼容，界面改名“首次响应超时”，明确成功 headers 到达即结束。缺省维持 30 秒、范围维持 1–300 秒，不改已保存值。
- 新增独立 API 字段 stream_total_timeout（秒，1–1200，允许 null 清除），统一校验并按现有 provider.config JSON 模式存储 config.stream_total_timeout_ms。create、PATCH、直接 config 写入、summary readback 和备份恢复使用同一语义；未知配置与原 failover_rules 保留。不为单个可扩展配置增加 SQLite 列迁移。
- 缺省 900000 ms；读回应包含原始覆盖值及生效值/缺省来源，表单清空后可验证恢复 900 秒。普通聊天与压缩共用，无独立压缩控件。
- 新增 ExecutionTimeouts.stream_total_ms（可选、serde default）传播独立完整流式期限；原 total_ms 继续表示其现有非流式用途。若实施时复用等价的已有请求上下文字段，必须满足相同序列化/兼容验收，不能把 total_ms 静默转义。
- 管理界面移除本次 HTTP 聊天流的“首输出总预算”可编辑入口，换成明确的“流式请求总超时”。旧 stream_failover_budget_ms 保留数据/API 兼容但不冒充新字段；升级不使用旧 90/300 秒作为完整流式期限。
- 隧道：现有 RequestMeta 的 stream_first_byte_timeout_ms 负责上游开始响应；网关必须依据解包后的真实上游 headers，不能把 relay HTTP 200 当上游成功。总期限由网关请求 owner 始终执行，并通过现有 reset/取消释放远端流；若接收端需独立限时，扩展可选剩余时长字段并同步发送/接收端，不依赖旧节点支持新字段来实现网关总期限。非聊天隧道时间语义保持。

读写入口：apps/aether-gateway/src/handlers/admin/provider/{shared,write,summary}/，crates/aether-admin/src/provider/，crates/aether-provider/transport/src/network.rs，crates/aether-contracts/src/{plan,tunnel}.rs，frontend/src/api/endpoints/providers.ts、ProviderFormDialog.vue 及 i18n。隧道消费入口 apps/aether-tunnel/src/tunnel/stream_handler.rs 与 crates/aether-gateway/tunnel/。

## 4. 错误预读、提交和终态

stream/execution.rs:5470 附近的 useful-output 预读承担 HTTP 200 后早期 response.failed 的候选切换；这一协议策略与计时器独立保留，不能因 current_first_output_deadline 变成 None 而意外改变提交时机。首有效内容判断继续用于业务识别和指标，不作为超时条件。

明确上游错误/网络错误立即处理。协议要求完成事件时，缺失终态不能按 200+EOF 判成功；合法 incomplete 保留原处理，不扩大成无条件重试。已向客户端交付协议/工具/模型状态后，不拼接或透明重发；按现有协议错误事件或连接错误反馈失败。终态统一包含真实错误类别与完整 elapsed，不把总期限当“首字节超时”，不伪造上游 HTTP 状态。

总期限是本地策略取消，保留独立归因；不把它误归为已证明的网络/凭据失败。既有上游明确错误的健康分类、尝试次数、Retry-After 等契约不变。race 由单一终态 owner 解决，完成/取消/超时只能选一个终态事实。

## 5. 观测与验证边界

分别记录响应头、首 body、首有效内容、最终时间；不记录新增敏感正文或 encrypted_content。原“首字”指标不静默改成首 headers，改用准确标签/详情说明，让用户区分已经响应和有效内容尚未产生。

验收采用本地合成上游、临时 SQLite、真实 HTTP API 与最小前端流程。必须证明成功 headers 后长等待可完成、总期限在 headers 返回后的 body 阶段仍有效，以及总超时后资源和状态收尾。仅类型/lint/单测通过不代表真实链路正确。

更新稳定契约时精准修改 docs/specs/aether-gateway/backend/chat-failover.md 的首有效内容预算相关段落及直接相关前端规范，保留用户当前大范围 spec 修改。更改的兼容说明与验证结论进入任务记录。

## 6. 风险与回退

默认完整期限首次真正覆盖长流，超过 15 分钟会结束，应在设置明确展示并允许修改。没有静默期限时，已响应但挂起的请求可能等到总期限。向客户端尚未提交的预读期间，客户端自身超时仍可能先结束请求，解除网关首次限制不等于能改变外部客户端限制。

本次不改生产配置或数据库结构，旧配置数据保留，源码可独立回退；不能把旧数据保留当作新旧运行语义完全相同的保证。HTTP 流与非流式/WS/图片需要明确分支，避免共享 helper 改动扩散。
