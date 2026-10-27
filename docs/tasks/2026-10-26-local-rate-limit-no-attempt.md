# 本地 Key 限流冷却不再伪装上游失败

## 请求、现状与关联

用户观察到请求 `47371e3b-e0ef-41f1-8178-69173aa67568` 在 `20:42:27` 因 Key `3425` 的 10 分钟限流冷却被本地跳过，却显示为 `503`、伪造上游响应、`1791225528.45s` 耗时和错误时间范围；同一 Key 的实际上游 `429` 发生在 `20:32:27`。用户询问健康度为何不下降，并要求本地冷却展示真实原因和恢复时间。

结论：这是 Key 限流冷却（`key_rate_limit_cooldown`），不是熔断（`key_circuit_open`）。冷却跳过没有请求上游，也没有结算健康度，因此健康度保持 `0.9` 符合现有限流和健康策略。

关联 Spec：

- `docs/specs/frontend/index.md`：“请求详情首屏与尝试分页”。
- `docs/specs/aether-gateway/backend/chat-failover.md`：健康结算、冷却与验证矩阵。

## 本次变化与决定

- 真实上游 `429` 的流式最终回退与候选耗尽都保留客户端 `429`，并在存在可靠冷却截止时返回 `Retry-After`；没有本地窗口时保留并限制上游建议值，不超过 600 秒。provider-private 的 HTTP 200 SSE 语义错误也按解析后的语义状态返回，不再透传 HTTP 200。usage 同步进入 `failed/429` 终态并记录实际请求耗时，不再等待维护任务清理。流式非 2xx 候选同时记录本次实际候选耗时。
- 已观测上游响应时，终态耗时若因毫秒截断得到 `0`，usage seed 规范化为 `1ms`；这避免已结束请求看起来仍为待处理，同时不改变本地冷却记录的零耗时语义。
- 只有候选循环确认最终保留某条上游响应时才写 usage 终态，中间候选失败不能提前结束整个请求；延迟终态保留最后候选的执行计划和真实 telemetry。
- 真实 `429` 结算后读取同一 Key 健康快照中的冷却起止，写入 usage metadata 和候选 `extra_data`；请求详情只在真实 `429` 的 Key 详情显示“限流时间”，旧记录仅有可靠截止时显示截止时刻。
- 列表与详情的 pending→终态同步使用 usage 生命周期快照判断：`created_at` 的毫秒精度与 `updated_at` 的秒级截断落在同一秒时，不能把终态误判为旧快照；真正早于上一秒的快照仍会被拒绝。
- 本地冷却导致全部候选被跳过时，客户端返回 `429` 和 `Retry-After`，不再返回 `503`。
- trace 的 `attempted_only` 查询在零上游调用时使用 `no_attempt`，不再复用 usage 的 `failed`。
- 请求详情只画真实上游调用，唯一例外是全部候选仅因本地限流冷却跳过时复用同一三框路径；Key 标记“未请求”，请求结果标记“未发送”和预计恢复时刻，不展示请求级 HTTP 结果、上游响应或本地处理耗时。
- 详情首屏概览把原“总耗时”位置改为“本地限流预计恢复时间”，展示真实恢复时刻；头部不显示请求级 `503`。
- 管理详情依据本地跳过原因判定“无上游调用”，即使记录保留了 candidate/provider key 标识也不生成 `upstream_error`。
- 冷却跳过不重新结算 Key 健康度。真实上游 `429` 才扣减健康。
- 请求详情供应商框改为细实线外框，移除内层重复虚线；Key 展开详情的“时间”改为“请求时间”，真实 `429` 增加与健康快照同源的“限流时间”“剩余冷却”，剩余冷却每秒刷新。
- 限流冷却跳过候选保留同一窗口起点和截止并投影到 trace；本地跳过详情显示“HTTP 未请求”“耗时 未请求上游”、限流起止、动态剩余冷却及“没有上游响应、不消耗上游额度”，不生成上游响应面板。

## 验收与实际检查

- A1：真实路由产生上游 `429` 后，下一次请求在冷却期不发上游，返回本地 `429`、`Retry-After`，usage/trace/候选状态一致。已用 `rate_limit_cooldown_rejects_locally_without_upstream_attempt_or_response` 通过。
- A2：冷却请求的前后健康度保持 `0.9`。真实路由测试已断言两个 Key 均不变化。
- A3：管理详情不把本地冷却记录归因为上游错误。`detail_payload_exposes_local_rate_limit_cooldown_without_upstream_response` 使用真实 candidate/provider key 形状通过。
- A4：请求详情路径与首屏概览均不显示 `503`、上游响应或本地处理耗时，恢复时刻与剩余时长可见。抽屉回归 18 项、前端四份测试共 68 项、类型检查及真实 Chromium 七场景通过。
- A5：独立本地冷却响应契约包含 `429`、`Retry-After`、`provider_key` scope 和本地原因文案；专项 Rust 测试通过。
- A6：单候选真实流式 `429` 耗尽时，客户端返回 `429`，usage 为 `failed/429` 且 `response_time_ms > 0`，最终候选耗时非空，usage metadata 与候选 `extra_data` 的冷却起止一致。`stream_rate_limit_exhaustion_returns_429_and_persists_cooldown_window`、上游超长 `Retry-After` 同步/流式上限用例及 `rate_limit` 35 项均通过。
- A7：列表与详情同一秒 pending→failed/429 快照会接受终态并带出耗时，前一秒快照仍拒绝。`recordSync.spec.ts` 17 项、`useUsageData.spec.ts` 16 项及 `Usage.vue` 共用判定通过。
- A8：请求详情真实 Chromium 七场景通过，包含供应商只保留细实线外框、无内虚线、429“限流时间”中文及英文格式、请求时间标签与移动端内部滚动；相关组件和 i18n 测试共 46 项通过。
- A9：当前改动 `cargo check -p aether-gateway -p aether-scheduler-core -p aether-usage-runtime`、`scheduler_failover` 25 项、`aether-ai-serving` 83 项、scheduler request candidate 10 项、usage request metadata 22 项和 `vue-tsc -b` 均通过。
- A10：provider-private 预取流在 HTTP 200 内出现语义 `429` 时，最终响应为 `429`、带 1–600 秒 `Retry-After` 和原错误正文；usage 立即写入 `failed/429`、实际耗时和错误正文。`gateway_returns_error_body_when_prefetch_detects_embedded_stream_error` 通过。
- A11：本地冷却与真实 `429` 使用同一三框路径和 Key 详情；每个本地跳过 Key 可点击并显示“未请求”、自身冷却截止、动态剩余冷却，详情显示限流起止且无上游响应。前端四份测试 69 项、类型检查和真实 Chromium 七场景在最终文件上复验通过。
- A12：上游超长 `Retry-After` 的同步/流式终态耗时不因亚毫秒观测写成零；`rate_limit` 36 项通过，原先会间歇失败的用例连续 30 次通过。`observed_provider_terminal_usage_never_keeps_zero_elapsed_time` 确认仅已观测响应头/响应体时规范化为 `1ms`，传输中断和无上游调用仍保持零耗时。

## 状态、收敛与下一步

状态：实现、自检和适用验证已完成；高风险 Delivery 的独立 verifier 因用户明确要求本轮仅用主线程而未生成，机器门禁待用户确认例外。当前工作区改动未提交、未构建或发布。

收敛：A1-A12 均有实际通过结果，Spec、任务与实现一致；真实上游 `429`、provider-private 语义 `429`、usage 终态、零耗时边界、列表/详情同步、细实线供应商框、请求时间和限流窗口已覆盖。尚未对真实生产上游或新桌面包复验。
