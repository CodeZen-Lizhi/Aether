# 审查与验证

## 本项目采用的审查约定

本节将用户全局约定和已有 Review 技能落实到 Aether。Aether 当前是 Rust/Vue 项目，专门审查使用可用的 `code-review-and-quality`；Java/Go 技能只在任务实际涉及相应语言时采用。SQL、权限、并发和性能疑点纳入同一次相关审查，复用已读调用链和验证结果。

审查从本次变更及直接调用方开始，问题必须有触发条件、代码依据和影响。纯文档可做静态检查；小 diff 若改变权限、事务或状态边界，仍按实际风险深入。常规局部修改不扩展为全仓扫描、架构改造或重复审核。

## 从验收行为选择证据

| 变化 | 最小有意义的证据 |
| --- | --- |
| Spec、注释或文档导航 | 内容与源文件对照、链接及占位符检查 |
| 纯计算/排序 | 受影响 crate 中既有公共入口测试，含真正的边界 |
| HTTP 错误或业务接口 | 实际 Router 的 status/body/header，加涉及的后续读取 |
| 调度、流式、重试、亲和、健康度 | 本地 scripted upstream 的调用次序/次数和结算后的状态，不能仅测公式 |
| SQL、迁移、导入导出 | 临时 SQLite 的成功/失败/回滚/重开后读取 |
| UI、缓存或保存 | 用户操作后的结果、刷新/重复操作/过期响应；运行条件具备时做最小浏览器流程 |
| Tauri IPC、钥匙串、进程管理 | 原生边界验证；浏览器 fixture 不能代替原生证据 |

范围内存在行为缺陷时修复根因，并只重验受影响路径。没有新修改、失败或未解决疑点时，已有检查不重复跑。编译、lint 和测试各自只证明对应范围；未运行的行为明确记为未验证。

## 可用命令与范围

命令在仓库根目录运行。先从 `Cargo.toml` 和实际测试模块确定目标：

- 单包编译：`cargo check -p <实际包名>`。
- 单包库测试：`cargo test -p <实际包名> --lib <实际测试过滤词>`；没有 lib 的包选择其实际 target。确认确实执行了目标测试，零测试通过不算行为证据。
- 单包格式检查：`cargo fmt -p <实际包名> --check`。
- 网关路径示例：`cargo test -p aether-gateway --lib tests::scheduler_failover`；只在调度故障转移相关变更时采用。该测试使用本地上游，并读回健康状态，源码见 [scheduler_failover](../../../apps/aether-gateway/src/tests/scheduler_failover.rs)。
- 数据迁移示例：`cargo test -p aether-data --lib lifecycle::migrate::tests`；具体 feature 和导出测试参考 [数据层 README](../../../crates/aether-data/runtime/README.md)。

[Rust CI](../../../.github/workflows/rust-ci.yml) 定义远端的完整 fmt/clippy/nextest 工作流。完整 CI 不等于每次本地修改都需重跑全 workspace；涉及广泛公共契约时再扩大范围。隔离验证使用临时数据库、本地上游与测试凭据。

前端命令及保存一致性见 [前端验证规范](../frontend/quality-guidelines.md)。交付写清修改、实际证据及剩余限制；审查结论不自动授权提交、push、发布或生产操作。
