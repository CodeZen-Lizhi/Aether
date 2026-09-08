# 技术设计

## 变更边界

最小缺口是 Responses 转换器未忽略 `ping`，以及现有传输错误字符串/派生 Debug 的凭据 URL 保护不完整。直接在已有转换器和传输错误边界修复，不通过路由、API 响应或前端拦截改变业务语义。

## R1

- 来源：上游 `88d2b002b`。
- 修改 `crates/aether-ai/formats/src/formats/openai/chat/stream.rs` 的 Responses 事件分类。
- 在 `crates/aether-ai/formats/src/formats/shared/stream_core/format_matrix.rs` 保留端到端协议转换回归，证明心跳前后内容和终止事件正常。

## R2

- 参考上游 `36e9d21e3` 与其既有 sanitizer 的前置逻辑，按本地实现移植，不 cherry-pick 大型安全重构 `579f2c7cc`。
- 主要边界：`apps/aether-gateway/src/execution_runtime/transport.rs` 的 reqwest/wreq/Hyper 错误链格式化、`sanitize_upstream_url_text`、`ExecutionRuntimeTransportError`。
- 基线 sanitizer 只删除 query/fragment，未删除 userinfo；当时派生 Debug 可展开底层 URL 和 source。本次修复及回归覆盖直接字符串、typed error、内嵌多个 URL、Unicode 与普通错误，最终证据见 `research/verification.md`。
- 优先复用或扩展现有 helper。保留 host/path 和错误语义，不照搬上游把地址全部替换为 `redacted.invalid` 的策略，不引入全量隐私脱敏模块。
- 边界以独立引号、分组或空白等明确诊断结构为依据；一个 URL 的 query/fragment 可以包含另一个 URL、逗号、分号或单引号，不能仅凭这些内容拆成新诊断字段。原先裸逗号列表测试的假设过强，改为明确分组列表；歧义值仍在外层 query/fragment 内完整清除。
- 禁止改原始上游响应体/捕获值、状态码、熔断、失败预算、首字节超时、候选切换及请求传输选项。仅保护诊断表示中的秘密。

## R3

- 来源：上游 `b08fa3bdb`。该提交依赖上游的 DNS guard，并改变 `socks5` 的解析方式，不可直接移植。
- 本地既有设计和前端测试明确接受 `socks5` 与 `socks5h`，应保留 library 定义的 local/remote DNS 区别。
- 使用本机 SOCKS mock 检查送到代理的地址类型；远程 DNS 使用 `.invalid` 域名，不接触真实供应商。HTTP、浏览器指纹 HTTP 与 WS 中仍有实际调用的路径需要覆盖。
- 测试通过时仅补必要契约回归并记录跳过上游策略；发现分支自身缺陷时只修相应路径。

## R4

- `ec95f2ca1` 同时涉及网关、Tunnel 客户端及 SETTINGS/流控协议，不能作为一个普通补丁整体套用。
- 核对节点类型与运行组件，只输出计数和模式，不读取/输出代理凭据。
- 不把只读的历史 SQLite 文件当成当前运行库。无法确认使用时不声称生产环境未使用；依用户授权可跳过条件性升级。

## 验证与兼容性

- 先为实际缺陷建立失败用例，再做最小修复。
- 使用本机隔离 mock 和已有 in-memory/SQLite 测试底座；不更改运行数据库、网络代理或容器。
- 格式、受影响包测试与 Clippy 完成后，独立复核原始错误展示、请求选项、单用户/SQLite 边界。
- 初次交付保留未提交差异供用户审阅；后续依据用户“合并回 `slim-personal`”的明确授权提交本任务改动并进行本地合并。
