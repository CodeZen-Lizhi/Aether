# R1 状态一致性实现

- records/active/detail 使用 usage 生命周期；仅无有效生命周期的历史记录和显式图片失败保留候选兜底。正常聊天重试间隙不会由单个 failed/504 候选推断请求失败。
- trace 总状态/总耗时读取 usage，候选自己的失败、状态码、错误和耗时仍保留。前端 active+错误字段不再直接显示失败；真正终态的防陈旧保护保持。
- 修复 RequestDetailDrawer 独立的 active+error→failed 判断，以及 trace 把单轮错误/耗时回写列表的问题。有请求生命周期时，trace 仅补充显式图片进度。
- 新增两条临时 SQLite + 真实 HTTP Router 测试：先持久化失败候选，确保没有下一轮、没有 retryable 元数据；依次读取 records/active/detail/trace，创建重试，再终结成功或失败并重复读取。断言整条 300008 ms 与单轮 180003/119008 ms 分离。另保留历史兜底和图片失败 API 回归。

## 已执行验证

前端六个测试文件共 136 项通过：status、recordSync、useUsageData、UsageRecordsTable、HorizontalRequestTimeline、RequestDetailDrawer.pricing。最后两组件修改后重验 42 项通过。type-check、修改文件 ESLint、修改 Rust 文件 rustfmt、diff-check 通过。

## 浏览器 fixture

`frontend/scripts/fixtures/request-detail-layout.html` 支持：

- `?status=retry-gap`：只有首轮 failed/504，请求 pending、总耗时 181 秒。
- `?status=retry-gap&lifecycle=streaming`：同一失败候选，请求保持 streaming。
- `?status=retry-started`：第二次尝试已开始，请求仍进行中。
- `?status=final-failed` / `final-success`：重试后请求终结，整体 300008 ms。

关闭抽屉后显示真实 UsageRecordsTable，可点击行或“打开请求详情”重开；组件 requestState 通过 recordSync 合并回列表。fixture 模块语法已检查。

main 回传的真实浏览器结果：retry-gap 表格等待中、请求链路进行中、单轮 failed/504 保留，整耗时 181 秒与单轮 180 秒分离，关闭重开维持 active；final-failed 列表与详情请求均显示失败/300.01 秒，第二轮独立显示 119.01 秒。该证据来自本地真实组件 fixture，不代表真实供应商调用。总期限节点暂沿用通用错误提示，按本轮确认保留，不新增配置。

## 共享 Rust 验证入口及边界

实现已就绪；usage-status 未占用 Cargo。请由 main 统一运行或分配构建槽：

- `gateway_admin_usage_retry_lifecycle_sqlite`（2 项）
- `gateway_derives_legacy_admin_usage`（1 项）
- `gateway_preserves_admin_usage_explicit_image_failure`（1 项）
- `observability::usage::summary_routes::tests`
- `observability::monitoring::tests::trace`

这些 Router 测试证明持久化生命周期快照经公开读取 API 的一致性，不代替真实运行时重试调度验证。最终本地合成上游 HTTP/SQLite 验收由 main 负责；未重复旧安装版基线、未修改生产配置、未提交。

## 收尾：首字指标口径说明

已核对运行时：`stream/execution.rs` 的 `first_stream_event_telemetry` 使用本次执行开始至首个 body 块/Data 帧被网关观察到的耗时，持久化为 `first_byte_time_ms`；启动/心跳可能计入，有效正文另行识别。`request_diagnostics.rs` 的端到端值加入候选开始前的调度/重试耗时。非流式 transport 可能使用响应头计时，所以新增说明明确限定 HTTP 流式。

仅在 UsageRecordsTable 的表头/性能 tooltip、RequestDetailDrawer 的首字指标 tooltip 添加口径，并在 i18n/messages.ts 加英文映射；未改数值、设置或样式。按收尾要求只做 type-check、三个修改文件 ESLint 与 diff-check，均通过；未重跑全套测试。
