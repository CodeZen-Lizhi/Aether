# 独立复核记录

Reviewer：`/root/backport_review`（独立 `trellis-check`）；任务基线：`58c445ddc`。

已读 `check.jsonl` 全部上下文、PRD、design、implement、各项 research，以及 `trellis-check` / `code-review-and-quality`。审查遵守原始个人版设计、Gemini 排除、仅适用缺陷修复、无提交或部署的约束。

## Findings (fixed)

最终源码静态复核和独立回归复核通过，本任务内无未解决的代码发现。以下问题由 reviewer 提供最小复现、原 R2 worker 在共享源码中修复，reviewer 未修改产品源码。

### P1：合法单引号及伪装字段造成凭据残留

- File：`apps/aether-gateway/src/execution_runtime/transport.rs`，`sanitize_error_detail` / `diagnostic_url_suffix_is_balanced`。
- Issue：单引号曾截断合法 userinfo / fragment；进一步仅根据 `;label=` 判断外闭又会留下 `url='socks5h://host.test/path?key=first';token=second-secret'` 或括号版的后半秘密。
- Fix：限定候选 URL 范围，保留 authority 内合法标点，核验候选外闭之后的引号/括号配对；存在未配对外闭时延后边界。既保护完整秘密，也保留 `;cause=(TLS)`、`;cause='TLS'` 以及下一 quoted source 内合法的 userinfo apostrophe。
- Evidence：原始 userinfo/query/fragment 复现、伪字段变体、source 与 cause 对照均在最终独立 harness 通过；仓库 table-driven 回归覆盖 Display、Debug / alternate Debug。

### P1：把 query/fragment 内嵌 URL 当成独立诊断 URL

- File：`apps/aether-gateway/src/execution_runtime/transport.rs`，`sanitize_error_detail`。
- Issue：逗号或分号紧邻另一个 scheme，不足以证明外部列表边界；即使形如 `urls=[URL?token=opaque,https://private-value.test/super-secret]`，后半仍可能是 query 秘密。
- Fix：移除裸逗号/分号拆分的全部特殊分支。不明确的嵌套值随外层 query/fragment 整体删除；真正多 URL 诊断使用空白或独立引号/分组边界。旧的过强裸列表期望改为 `urls=[(URL1),(URL2)]`，未以生产配置变化迎合测试。
- Evidence：普通与方括号包裹的 comma / semicolon / fragment 情形全部通过；多个独立 URL 的 host/path、Unicode 和后续原因仍保持。

### P2：最后一个闭括号吞掉后续诊断原因

- File：`apps/aether-gateway/src/execution_runtime/transport.rs`，`sanitize_error_detail`。
- Issue：`rfind` 选取最后一个闭括号，导致 `(URL);cause=(TLS)`、方括号原因或 `,source=(第二 URL)` 被整体吞掉。
- Fix：依据候选外围分组、URL 内部分组与后缀配对确定边界，保留路径括号、IPv6、嵌套外围 `((URL))` 及后续 source/cause。
- Evidence：原始括号/方括号与嵌套 redirect + source 复现，连同指定保留对照，最终全部通过。

### P2：无路径 URL 的 authority 扫描跨入后续 peer 字段

- File：`apps/aether-gateway/src/execution_runtime/transport.rs`，`diagnostic_url_userinfo_end` 的调用边界。
- Issue：`url=(https://host.test);peer=user@example.test` 曾输出 `url=(https://example.test/`，因为 `rfind('@')` 跨过真正外闭，把 peer 邮箱误当成 URL userinfo。单引号版和带凭据的无路径 URL 同样受影响。
- Fix：计算 userinfo 时只检查候选 URL 内的 authority；不读取候选外闭后的字段。
- Evidence：无路径 URL 的括号/引号、有/无 userinfo，以及带路径对照全部通过；原主机与 peer 字段保持正确。

## Findings (not fixed)

没有尚未处理的本任务代码问题。以下既有全仓质量问题未扩大修复，保留个人分支原意与本次范围：

- 未修改 `58c445ddc` 的 gateway 全包已有 25 项失败，和修复初轮的失败名单完全一致。完整名单及比较结果见 `baseline-comparison.json`；涉及 routing/quota、流式 fixture、SQLite 备份、已裁剪模块相关架构断言等，不能据此恢复用户主动删除的功能。
- 基线全仓格式已有 5 个文件、8 个差异块。
- 基线严格 Clippy 先受 scheduler `health.rs` 的已有 `manual_range_patterns` 阻挡；`--no-deps` 展开的 20 条 gateway 诊断也已记入基线，包含旧 transport 测试的 MutexGuard 和未修改的 `dispatch/refs.rs:71`。不通过抑制 lint 或扩大业务改动制造全绿。

以上是已验证的基线问题，不能称全包 lint / tests 通过。主 agent 已完成最终修复版的同命令对照：格式和 Clippy 诊断与基线一致；最终整包另有一次未改动 Gemini 用例失败，立即独立复跑通过，根因未确定。完整结果见 `verification.md`。

## Scope and coverage review

- R1 production diff 只给原 `keepalive` 分支增加 `ping`，没有整体忽略未知事件。真实转换矩阵回归覆盖 Chat / Responses、流首与正文间心跳、完整正文、唯一完成事件和重复 finish。
- R2 只修改既有错误格式化边界。typed enum 字段、状态和 source 结构未变；同步执行 fallback 仍按 enum 分类。没有捕获体、原始响应、调度或超时流程变更。
- SOCKS 四项测试使用真实普通 / browser HTTP 和 WebSocket 入口，只监听 loopback 临时端口。`.invalid` 目标由 fixture 接管，fixture 不连接目标；断言 CONNECT 地址、端口、Host/path 和握手结果。
- 对锁定依赖的 `Domain("[::1]")` 仍要求能解析为数字 loopback，不能放过原始 `localhost`；研究记录明确这只证明 DNS 位置，不保证所有外部代理的 IPv6 兼容。
- WebSocket re-export 和测试模块均为 `#[cfg(test)]`。没有生产可见性、Gemini、SQLite、Tunnel 协议、依赖版本、UI 或代理配置变更。
- Scoped specs 已记录心跳、诊断和 SOCKS scheme 契约；本次没有平台配置或生成模板同步需求。

## Verification

- 独立 sanitizer 回归：**20 passed**。从最终 `transport.rs` 原样抽取 helper，以 `rustc` 和已经构建的锁定 `url` / `regex` 依赖在临时目录编译执行；`reqwest::Url` 使用其同一 `url::Url` 别名。仅复测已报告的问题与约定对照，未扩大范围。
- `git diff --check`：通过。
- Formats：复用已记录的 **730 tests passed**、fmt 和严格 Clippy / type-check 通过证据。
- Gateway 定向验证：worker 的最终诊断套件 **14 passed**；SOCKS **4 passed**（8 种配置组合）；原传输套件此前 **31 passed / 1 ignored**。主 agent 的最终整包再次包含这些测试，失败名单没有 transport 用例。
- 基线对照：**2530 passed / 25 failed / 7 ignored**；修复初轮 **2545 passed / 25 failed / 7 ignored**，失败集合完全相同。主 agent 的最终整包为 **2547 passed / 26 failed / 7 ignored**：包含原有 25 项失败，额外 Gemini pro/flash 选择用例在同一代码独立复跑 **1 passed**。该波动没有通过修改或过滤 Gemini 掩盖，不能称全包通过。
- Format / Lint：formats 通过，本次 6 个 Rust 文件单独格式检查通过。整仓格式差异与基线归一化后逐字一致；gateway 严格 Clippy 和 `--no-deps` 的诊断分别与基线一致，无新增。主 agent 已将命令、完整比较和遗留项写入 `verification.md`，本 reviewer 未占用 Cargo。
- TypeCheck：定向 Rust 测试和独立 helper 编译通过；主 agent 的最终 `cargo check -p aether-gateway --lib --locked --offline` **通过**。

代码复核结论：此前报告的问题均已闭环，没有本任务范围内的阻塞发现；全仓既有质量问题如实保留记录。
