# 继承代理诊断与 429 回归修复

日期：2026-11-06

## 目标与边界

- 修复代码审查发现的两项问题：HTTP 客户端自动继承环境代理但请求记录遗漏，以及 429 错误响应保留测试断言过时。
- 保留环境代理继承与显式代理优先级，遵循依赖的协议选择和 `NO_PROXY`；不新增操作系统代理读取能力。
- 只使用本地隔离服务与合成凭据验证，不继续供应商网络排查，不提交、发布或替换桌面安装包。
- 本次不宣称修复原始 `tls handshake eof`，其根因尚未确定。

## 实现与完成条件

- AI planner 按最终上游 URL 解析环境代理，让执行和请求记录使用同一个快照；记录来源 `inherited`、名称“继承环境代理”和脱敏地址。
- 直连快照关闭客户端自动代理，并与继承客户端分开缓存；HTTP、浏览器指纹、WebSocket、H2C、官方凭据可信判断和 TLS 诊断遵循有效代理状态。
- 429 测试保留候选顺序断言，并断言保留可信上游错误供最终回传。
- 回归核对本地真实代理收到的认证、协议及 bypass 选择、实际网关请求后的记录重读，以及原有调度/代理测试。

## 验证与审查

- `cargo test -p aether-gateway --lib --offline -- tests::scheduler_failover state::proxy::tests retry_scope_tests execution_runtime::transport::proxy_dns_tests execution_runtime::transport::diagnostic_tests explicit_direct_proxy_snapshot`：56 项通过，含完整调度回归、隔离子进程真实路由与最终 usage 重读、HTTP/SOCKS/WS 代理、认证脱敏、直连可信判断及 429 断言。
- `cargo test -p aether-ai-serving --lib decision_payload --offline`：3 项通过，含禁用代理快照的 TLS 路径展示。
- `cargo test -p aether-provider-transport --lib diagnostics --offline`：2 项通过，保留原显式代理的来源和脱敏契约。
- `cargo check -p aether-gateway -p aether-provider-transport -p aether-ai-serving --offline`、三个受影响包的 `cargo fmt --check`、`git diff --check` 均通过。
- 独立 code-review 发现并修复本轮直连快照对官方凭据可信判断及 TLS 路径的兼容问题；复查未发现新的必修缺陷。代理认证仅保留在执行快照，usage/trace 不保存代理用户名或密码。
- 真实路由测试等待 `completed` 后重读，避免把异步写入的 pending 快照误判为最终结果。当前锁定的依赖对 `NO_PROXY=*` 只做域名匹配；本次使用明确的回环 IP 规则验证 bypass，保留依赖既有语义。
- 测试编译仍有 `dispatch/refs.rs` 的既有不可达分支告警，未扩大范围处理。本次未构建或安装桌面候选包，未验证原始供应商 TLS 故障是否消失。
