# 实施与验证

- [x] 固定本地/上游基线，记录用户授权与不变约束。
- [x] 检查相关 specs、现有代理设计与上游补丁。
- [x] Responses worker：先复现 `ping` 转换失败，再做两文件修复，运行协议转换回归。
- [x] 传输错误：先复现凭据泄露，再修 Display/Debug/错误链边界，保留可用诊断。
- [x] 核验 SOCKS 的现有 local/remote DNS 契约；用隔离 mock 覆盖 HTTP/WS。
- [x] 完成 Tunnel 适用性核验，必要性与依赖不满足时记录跳过。
- [x] 检查 diff 中没有 Gemini、调度、SQLite、UI、新配置和新能力变更。
- [x] 运行受影响包测试、Rust 格式检查、Clippy/type-check；根据失败原因调整范围，避免无依据地反复全仓测试。
- [x] 由独立检查 agent 复核需求、兼容性与回归测试，修复范围内发现的问题。
- [x] 保存验证记录并完成 Trellis 收尾；初次按未提交状态交付，后续提交与本地合并遵循用户明确授权，不 push、不部署。

## 分工与命令

- 主 agent：范围判断、SOCKS/Tunnel 核验、整合与最终验证。
- 实现 worker：按明确文件边界实施；不得二次委派，不得改任务范围或 Git 分支。
- 共享 Cargo 目标目录；避免同时跑 Cargo。需要跑检查时向主 agent说明，主 agent 串行协调。
- 预期检查：`cargo test -p aether-ai-formats --lib`、受影响 gateway 模块测试、`cargo fmt --all -- --check`、`cargo clippy -p aether-ai-formats -p aether-gateway --lib --tests -- -D warnings`；精确命令和结论写入 research/verification.md。
