# 实施记录：调度残留缺口补丁

> 关闭 09-01 升级后仍残留的两个 bug（源自 sess_ff20900d 分析会话的 Bug1/Bug2 残留项）。

## Fix 1：传输层失败接入健康投影与熔断（Bug1 残留）

**根因**：传输层 `Err`（连接拒绝/DNS/TLS/运行时不可用）分支只记 candidate Failed，从不发 `HealthFailure` → 跨请求零惩罚，坏供应商被反复首选。

**修复**：在全部 6 个传输层 Err 分支的 candidate-Failed 记录之后，注入 `HealthFailure { status_code: 502, classification: UseDefault }`——502 为"上游不可达"语义，经三分类归为 Transient（8 连败熔断 + 成功率窗口 + 半开探测全链生效），亲和解绑随 `record_health_failure_effect` 的 Transient 分支自动发生：

- `sync/execution.rs`：真实 Err 分支 + 测试覆盖 Err 分支 + 第三处运行时变体分支（共 3 处）
- `stream/execution.rs`：Transport 分支 + 测试覆盖 Transport 分支（共 2 处，注入时顺带发现并覆盖了第三处变体）

**确认无需修改**：流式 first_byte_timeout/read_timeout 已带 504/502 状态码（投影闸门可过）；AdmissionTimeout 是网关侧背压而非上游故障，有意不记健康。

## Fix 2：手动恢复全量清理（Bug2 残留）

**根因**：恢复健康只写 health/circuit JSON，Codex 配额熔断 KV（TTL 最长 31 天）不清 → 恢复后仍被 key 作用域熔断挡住。

**修复**（`handlers/admin/endpoint/health_builders/keys.rs`）：
- 新增 `clear_codex_quota_breaker_for_key`（orchestration/codex_quota_breaker.rs）——删 key 作用域的熔断 KV；账号作用域刻意不动（需请求头 account id 定位，账号级配额恢复应以账号窗口到期为准，单 Key 恢复不该越权）。
- 单 Key 恢复与批量恢复（recover_all）均在 DB 写成功后调用清理；失败仅告警不阻断恢复。
- `default_key_health_payload` 补齐 `rate_limit_cooldown_until_unix_secs`/`consecutive_rate_limits` 字段，限流冷却随恢复一并清除。
- 会话亲和**有意不清理**：粘住本 Key 的会话在 Key 恢复健康后继续命中是正确行为。

## 验证

- `cargo check -p aether-gateway` 0 错误；routing-core 17 测试全绿；scheduler-core 102 过 / 2 失败（既有 codex_live_*，与本补丁无关）；`cargo fmt --all` 完成。

## 明确不做

- 手动恢复被并发失败覆盖的固有竞态（恢复后上游真坏则应再次熔断，属正确行为）。
- `is_candidate_in_recent_failure_cooldown` 死代码的接回或删除（职责已被熔断器体系覆盖，留待后续清理任务）。
