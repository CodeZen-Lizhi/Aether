# R1 Responses ping 移植记录

基线：`58c445ddc`。来源：上游 `88d2b002be8f5147a61c76014b0ff55c3998bfcd`（`fix(formats): ignore Responses ping stream events`）。

## 改动与行为边界

- `crates/aether-ai/formats/src/formats/openai/chat/stream.rs`：采用上游同一行修复，将 `ping` 与 `keepalive` 一同忽略；心跳不生成 canonical frame，不启动或终止正文流。
- `crates/aether-ai/formats/src/formats/shared/stream_core/format_matrix.rs`：新增 `ignores_openai_responses_ping_without_interrupting_text_or_completion`，通过真实 `StreamingStandardFormatMatrix::transform_line` 检查 Chat 与 Responses 两个客户端。
- 回归依次输入流首 ping、正文 `po`、正文间 ping、正文 `ng`、显式 `response.completed`。每次 ping 输出必须为空；正文必须恰好为 `pong`；完成事件之前没有结束标记；完成事件产生一次正常结束；再次 `finish` 不产生重复输出。
- Responses 客户端覆盖既有 `provider_stream_event_api_format = openai:responses` 覆盖路径。保留原有 keepalive、正文、已知/未知事件和错误转换测试。
- 生产代码仅改变 Responses 心跳分类；未修改 Gemini、路由、调度、代理配置或任务状态。

## 验证

| 阶段 | 实际命令 | 结果 |
| --- | --- | --- |
| Red，仅新增回归 | `cargo test -p aether-ai-formats --lib ignores_openai_responses_ping_without_interrupting_text_or_completion` | exit 101；0 passed、1 failed、729 filtered out。流首 ping 返回 `unsupported_stream_event` 错误 SSE，空输出断言失败。 |
| Green，加入单行修复 | 同上 | exit 0；1 passed、0 failed、729 filtered out。两个客户端分支均执行通过。 |
| 包内测试 | `cargo test -p aether-ai-formats --lib -- --quiet` | exit 0；730 passed、0 failed、0 ignored。包含既有错误转换与未知事件、终止事件回归。 |
| 包内格式 | `cargo fmt -p aether-ai-formats -- --check` | exit 0。 |
| 包内 lint/type-check | `cargo clippy -p aether-ai-formats --lib --tests -- -D warnings` | exit 0。 |
| 补丁空白检查 | `git diff --check -- crates/aether-ai/formats/src/formats/openai/chat/stream.rs crates/aether-ai/formats/src/formats/shared/stream_core/format_matrix.rs` | exit 0。 |

R1 无已知未解决问题。共享 Cargo 检查已结束并通知主 agent；全工作区格式与网关检查由主 agent 整合其他修复后执行。
