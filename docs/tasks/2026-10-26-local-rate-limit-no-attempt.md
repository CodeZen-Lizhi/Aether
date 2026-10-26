# 本地 Key 限流冷却不再伪装上游失败

## 请求、现状与关联

用户观察到请求 `47371e3b-e0ef-41f1-8178-69173aa67568` 在 `20:42:27` 因 Key `3425` 的 10 分钟限流冷却被本地跳过，却显示为 `503`、伪造上游响应、`1791225528.45s` 耗时和错误时间范围；同一 Key 的实际上游 `429` 发生在 `20:32:27`。用户询问健康度为何不下降，并要求本地冷却展示真实原因和恢复时间。

结论：这是 Key 限流冷却（`key_rate_limit_cooldown`），不是熔断（`key_circuit_open`）。冷却跳过没有请求上游，也没有结算健康度，因此健康度保持 `0.9` 符合现有限流和健康策略。

关联 Spec：

- `docs/specs/frontend/index.md`：“请求详情首屏与尝试分页”。
- `docs/specs/aether-gateway/backend/chat-failover.md`：健康结算、冷却与验证矩阵。

## 本次变化与决定

- 本地冷却导致全部候选被跳过时，客户端返回 `429` 和 `Retry-After`，不再返回 `503`。
- trace 的 `attempted_only` 查询在零上游调用时使用 `no_attempt`，不再复用 usage 的 `failed`。
- 请求详情只画真实上游调用；本地冷却展示“本地限流冷却中，未发送到上游”、命中 Key、可靠恢复时刻和剩余时长，不展示请求级 HTTP 结果、上游响应或本地处理耗时。
- 管理详情依据本地跳过原因判定“无上游调用”，即使记录保留了 candidate/provider key 标识也不生成 `upstream_error`。
- 冷却跳过不重新结算 Key 健康度。真实上游 `429` 才扣减健康。

## 验收与实际检查

- A1：真实路由产生上游 `429` 后，下一次请求在冷却期不发上游，返回本地 `429`、`Retry-After`，usage/trace/候选状态一致。已用 `rate_limit_cooldown_rejects_locally_without_upstream_attempt_or_response` 通过。
- A2：冷却请求的前后健康度保持 `0.9`。真实路由测试已断言两个 Key 均不变化。
- A3：管理详情不把本地冷却记录归因为上游错误。`detail_payload_exposes_local_rate_limit_cooldown_without_upstream_response` 使用真实 candidate/provider key 形状通过。
- A4：请求详情和失败提示不显示 `503`、上游响应或错误耗时，恢复时刻与剩余时长可见。前端四份测试共 67 项、类型检查及真实 Chromium 七场景回归通过。
- A5：独立本地冷却响应契约包含 `429`、`Retry-After`、`provider_key` scope 和本地原因文案；专项 Rust 测试通过。

## 状态、收敛与下一步

状态：进行中。核心实现、类型检查、基础契约回归、真实浏览器回归和 Delivery Guard 七项检查均已通过，待独立 reviewer 结论和最终收敛。未提交、未推送、未发布。
