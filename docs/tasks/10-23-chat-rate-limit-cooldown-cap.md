# 聊天 Key 限流冷却上限

## 请求、现状与关联

Magpie 的 `openai:responses` Key 曾收到上游 429，响应建议等待 82,610 秒，Aether 因聊天路径保留完整 `Retry-After` 而将 Key 冷却到次日。用户确认聊天 Key 的本地冷却最多 10 分钟，不遵循更长的上游时间。关联 [Chat Scheduling and Failover](../specs/aether-gateway/backend/chat-failover.md) 的限流冷却规则。

## 本次变化与决定

将上游 `Retry-After` 和本地聊天限流冷却统一限制为最近一次限流后最多 600 秒。冷却起点存入现有 `health_by_format` JSON 的独立字段，普通错误不得重置；旧记录缺少该字段时回退 `last_failure_at`，避免旧数据继续挡住 Key。冷却仍按 Key 和 API 格式独立，429 健康语义、现有冷却后探测、RPM/并发限制不变。不新增 SQLite schema 字段、迁移、管理 API 或可编辑设置。

## 验收与实际检查

- A1：超长上游建议和历史截止的有效时间不超过 600 秒；短建议仍精确保留。
- A2：本地 scripted upstream 验证实际候选顺序、后续冷却筛选和健康状态，确认长冷却仍跳过当前请求但不会延续至次日。
- A3：检查健康分、短建议重试、熔断探测和非聊天冷却回归。

## 实际检查

- `cargo test -p aether-scheduler-core --lib rate_limit_cooldown`：9 项通过，覆盖上游超长建议封顶、旧截止回退及有效期边界。
- `cargo test -p aether-gateway --lib tests::scheduler_failover`：22 项通过；本地 scripted upstream 实际观察 3600 秒 `Retry-After` 只保存最多 600 秒冷却、当前请求故障转移、后续请求仍跳过处于冷却的 Key。
- `cargo test -p aether-gateway --lib rate_limit_cooldown`：5 项通过；`cargo test -p aether-gateway --lib unrelated_failure_does_not_extend_rate_limit_cooldown`：1 项通过。
- `cargo fmt --all -- --check` 与 `git diff --check`：通过。
- delivery 完整门禁：4 项通过，结果保存在 `.cache/delivery/rate-limit-cooldown-10m/run-3/result.json`；新鲜度复核通过。

## 状态、收敛与下一步

状态：代码验收完成，尚未安装或发布。Spec、实现和测试一致；没有变更 SQLite schema。当前安装运行的 Aether.app 是旧二进制，未替换或重启它，也未进行生产发布。后续要让当前桌面应用采用该逻辑，需要构建并安装新的桌面候选包。
