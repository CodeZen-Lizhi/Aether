# Implementation and validation

## Execution

- [x] 读取当前分支、审查证据、相关历史产品决策与 Trellis 规范；工作区基线干净。
- [x] 写入 PRD/design/本文件并自审；用户已明确授权直接实施，跳过重复确认。
- [x] 激活任务；按文件所有权派发两个独立 trellis-implement 工作项，主 agent 同时实现数据 lifecycle、CLI、钱包修复。
- [x] 建立既有测试反馈；不通过删测试、吞异常或缩小业务语义制造通过。
- [x] R1 默认导出、复制、历史域与当前表覆盖修复。
- [x] R2 SQLite-only runtime、CLI、feature 与部署选项收敛。
- [x] R3 真实钱包摘要及错误链路恢复。
- [x] R4 简化路由退役规则归一、未退役规则往返保持。
- [x] R5 schema 工具、构建、文档和错误注释修复。
- [x] R6 孤立前端源文件和相关依赖清理。
- [x] 逐项整合、直接调用方/数据流与 SQL 审查；修复发现的问题后重复必要局部验证。
- [x] Trellis check、spec 更新与会话记录；无用户提交授权，不运行自动提交。

## Validation plan

具体命令以现有 scripts/Cargo 目标为准；下列限制适用于所有 worker。

- Rust：受影响 crate 的 cargo check（含当前有效 feature 组合）；已有 export、钱包 payload、routing 相关用例，单次 60 秒超时。主 agent 统一运行 Cargo，worker 不并发争抢 target 锁；worker 可报告确切测试名供主 agent 执行。
- 前端：在 frontend 使用 node_modules/.bin/vue-tsc -b、针对编辑文件的 eslint（不使用全仓 --fix）与指定 vitest 文件；由 routing worker 执行。
- Schema：bash -n compose_schema.sh，受影响 schema checker 的 check 路径；不要运行会重写旧迁移的 compose/generate。
- git diff --check；最终 diff 逐项对照 R1–R7，必要的现有回归必须能失败于原 bug。
- 超时限制：后端测试 60 秒，单项其他检查不超过 120 秒；及时终止超时进程树并记录。编译失败先定位，禁止无限重复同一命令或扩展到全仓。

## Review gate

使用 code-review-and-quality 与 sql-code-review。检查边界包括导出完整性/历史版本兼容、绑定参数/表名白名单、事务原子性、钱包鉴权用户映射、简化与完整 resolver 分离、frontend 规则保存语义与依赖锁文件同步。现场历史数据/外部可信调用不在此次执行范围。

## Final result

R1–R7 已实施并完成范围内验证，详见 [verification.md](verification.md)。Trellis 收尾使用 `--no-commit`；用户未授权 git commit/push，不重复请求确认，也不自动提交。

## 后续交付

用户明确授权提交并 push 后，修复已提交为 `ad7ac23ab199c736b9250e7895efa39bf10a17c9`；Trellis 归档及日志作为独立记录提交，目标远端为 `origin/slim-personal`。
