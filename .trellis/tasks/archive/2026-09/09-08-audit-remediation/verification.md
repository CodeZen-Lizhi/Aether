# 修复与验证记录

本任务已接续原会话 `01a065a1-b9b7-7c53-a990-bdfc0da23047` 的审查结果，完成 R1–R7 的代码修复和局部验证。工作分支为 `slim-personal`，基线为 `7692fde7d32ff359ed5bbd38f2ae4cdbe3d9da4b`。验证完成时实现留在工作区，尚未提交或 push；后续提交授权见文末。

## 变更摘要与影响文件

| 要求 | 结果 | 主要文件 |
|---|---|---|
| R1 导出/导入/复制 | 默认域覆盖最新迁移后的 77 张业务表；补回支付配置和历史奖励；退役域明确拒绝；导入单事务 UPSERT，外键延迟至提交检查 | `crates/aether-data/runtime/src/lifecycle/export.rs`、`export/sqlite.rs`、现有 `export/tests.rs` |
| R2 驱动/CLI | runtime 仅保留 SQLite feature，删除未接入旧驱动代码；CLI 仅接受 SQLite 和 single-node，旧 URL 在连接前报错；历史 manifest driver 仍可读 | runtime `Cargo.toml`、`src/backend/`、`src/driver/`、`src/lifecycle/`；gateway `src/main.rs` |
| R3 钱包 | 认证、会话和设备检查后按用户查询真实钱包；复用已有金额序列化；无钱包为零余额有限模式，查询错误走原 500 链路 | gateway `handlers/public/support/auth_session.rs`、`state/runtime/auth/user_provisioning.rs`、模块导出及现有登录回归 |
| R4 路由 | 简化路径忽略退役模型/Key overlay，保留实体 Key 优先级；前端保存保留外部规则条件、阶段、动作和 stop；完整 resolver 语义保留 | `aether-routing-core/src/{actions,policy}.rs`；gateway `routing/resolver.rs`、`scheduler/config.rs`；前端 schedulingStrategy、RoutingProfiles 及现有回归 |
| R5 维护工具 | schema check 读取真实 SQLite 迁移；删除旧 PostgreSQL 构建输入；修正 runtime/schema/backfill 文档和 auth_modules 注释 | runtime `build.rs`（删除）、`schema/compose_schema.sh`、相关 README；schema `src/lib.rs`；SQLite `src/auth_modules.rs` |
| R6 孤立前端 | 删除 StripePaymentDialog、announcement helper、仅供 Stripe 使用的 paymentInstructions，清理导出、独占翻译和 Stripe 依赖 | frontend `src/components/common/`、`src/utils/`、`src/i18n/messages.ts`、`package.json`、`package-lock.json` |
| R7 交付 | 类型/编译、局部回归、代码和 SQL 审查完成；同步数据、网关、路由及前端规范 | `.trellis/spec/`、本任务记录和开发日志 |

没有修改任何历史 `.sql` 文件，没有新增测试文件，没有连接用户实际数据库。数据库回归仅使用内存库或本轮临时目录内的 SQLite 文件；HTTP 回归使用测试自身的本地服务。

## 命令与实际结果

以下 Rust/schema 命令在仓库根目录执行。测试执行限制为 60 秒；纯测试编译单独限制为 120 秒；超时会终止整个进程组。

| 命令 | 结果 |
|---|---|
| `cargo check -p aether-data --all-features --offline` | 通过，14.82 秒；原先激活空 feature 导致的 48 个编译错误已消除 |
| `cargo check -p aether-data --no-default-features --offline` | 最终通过，6.34 秒；本轮引入的未使用导入/helper 告警已清除 |
| `cargo check -p aether-gateway --lib --bin aether-gateway --offline` | 通过，50.89 秒 |
| `cargo test -p aether-data --lib lifecycle::export::tests --offline` | 最终 16/16，通过；总计 12.41 秒，含编译 |
| `cargo test -p aether-data --lib backend:: --offline` | 18/18，通过 |
| `cargo test -p aether-data --lib lifecycle:: --offline` | 迁移 12 项、backfill 6 项通过；该次 export 有一项旧夹具失败，修正后已在上述 16/16 中复验 |
| `bash crates/aether-data/runtime/schema/compose_schema.sh check` | 通过：`ok generated logical schema`、`ok sqlite/baseline`，4.73 秒 |
| `cargo test -p aether-data-schema --lib --offline` | 8/8，通过 |
| `cargo test -p aether-routing-core --lib policy:: --offline` | 7/7，通过，含简化与完整 resolver 的差异、实体 Key 优先级和混合规则 |
| `cargo test -p aether-gateway --bin aether-gateway --no-run --offline` | 通过，81.37 秒 |
| `cargo test -p aether-gateway --bin aether-gateway --offline` | 54/54，通过，4.44 秒 |
| `cargo test -p aether-gateway --lib --no-run --offline` | 最终通过，117.54 秒 |
| `cargo test -p aether-gateway --lib routing::resolver::tests:: --offline` | 2/2，通过；静态捷径直接与 core simplified resolver 比较 |
| `cargo test -p aether-gateway --lib gateway_handles_auth_login_locally_without_proxying_upstream --offline` | 1/1，通过；有限钱包 HTTP 金额及无限/无钱包序列化断言 |
| `rustfmt --check --edition 2021 --config skip_children=true <本轮修改的 29 个 Rust 文件>` | 通过 |
| `bash -n crates/aether-data/runtime/schema/compose_schema.sh` | 通过 |
| `git diff --check` | 通过 |
| `git diff --name-only -- '*.sql'` | 空输出，历史 SQL 未修改 |
| `python3 .trellis/scripts/task.py validate .trellis/tasks/09-08-audit-remediation` | 通过，implement/check context 各 4 条 |

前端检查由 routing 工作项在 `frontend/` 中执行，最后一轮均通过：

```bash
./node_modules/.bin/vue-tsc -b

./node_modules/.bin/eslint \
  src/features/routing/utils/schedulingStrategy.ts \
  src/features/routing/__tests__/schedulingStrategy.spec.ts \
  src/views/admin/RoutingProfiles.vue \
  src/components/common/index.ts \
  src/i18n/messages.ts

node --experimental-require-module --disable-warning=ExperimentalWarning \
  ./node_modules/vitest/vitest.mjs run \
  src/features/routing/__tests__/schedulingStrategy.spec.ts \
  src/features/routing/__tests__/routingPolicy.spec.ts \
  src/features/providers/utils/__tests__/providerPrioritySort.spec.ts \
  src/i18n/__tests__/i18n.spec.ts
```

Vitest 为 4 个文件、29/29 通过。锁文件已通过 `npm install --package-lock-only --offline --ignore-scripts --no-audit --no-fund` 校准，diff 仅移除 Stripe 相关依赖，没有升级其他依赖。对被删除前端对象的跟踪源码消费者检索为空。

## 审查和反馈闭环

按 `trellis-check`、`code-review-and-quality`、`sql-code-review` 完成静态审查，主线程负责整合，并由独立工作项复核数据/runtime/schema 和 CLI/钱包。范围内没有剩余必须修复的问题。

- SQL 检查：导出域和辅助表白名单覆盖当前 77 表，`payment_gateway_configs` 主键为 `provider`；动态标识符来自已验证 schema/白名单，值使用参数绑定。UPSERT 保留未提供字段，避免 REPLACE 的级联删除。未知非空列、含行旧域和提交外键失败均报错并回滚；空历史旧域仍兼容。
- 调用链检查：真实钱包由认证用户 ID 查询，错误传播至 HTTP 500，金额复用现有 serializer；没有修改结算核心。简化 resolver 与完整 resolver 分离，静态捷径归一化一致，Key 实体优先级仍参与排序。
- 保存检查：移除精确 `ui_model_scheduling:` 命名空间内的旧调度动作，保留混合 mutation 与 stop；相似外部 ID 不误删。新全局规则遇到 `i32::MIN` 时，仅把最小端连续优先级组加一，保留同级关系和实际执行顺序；已有回归同时断言输入不变。
- 修正过的验证缺陷：原父子夹具指向已删除用户组；替换后的 `api_keys.user_id` 及新增回滚夹具的 `models.provider_id` 实际没有声明外键，不能证明级联/提交语义。最终分别改用真实的 `referral_rewards -> users` 和 `background_task_events -> background_task_runs`，16 项 export 回归重新通过。
- 修正过的编译问题：gateway lib 测试首次发现钱包 helper 经私有导入无法再导出，已修正 `support.rs` 的 crate 内导出，最终 lib 测试编译与执行通过。
- 有界超时：gateway production check 与 CLI 测试首次在 60 秒编译限额触发超时；后续分离纯编译和执行，在 120/60 秒限额内成功。未把旧二进制或超时当作测试通过。
- 未修改文件 `apps/aether-gateway/src/dispatch/refs.rs:71` 有一个测试编译 `unreachable_patterns` 告警；不影响本次通过结果。

## 兼容性、验证盲区与回滚

- 部署和构建只能配置 SQLite/single-node；旧 PostgreSQL/MySQL URL、driver feature 和 multi-node 选项会明确失败。历史 JSONL 的 driver 元数据、v1/v2 格式、时间戳/二进制兼容保留；含退役域数据的旧备份明确拒绝，不能静默丢行。
- 默认导出/复制和 omit-body 使用最新迁移的临时数据库验证，保留路由快照、支付配置、历史奖励和二进制数据；没有对生产备份体量或全部历史数据形态作验证。当前导出仍沿用内存中组装 JSONL 的既有设计。
- 钱包有限模式覆盖真实测试 HTTP 链路；无限/无钱包覆盖序列化。仓库查询失败到 HTTP 500 的分支完成静态审查，未添加故障注入用例。
- 没有运行全仓测试、全量构建、浏览器验证或生产数据库操作；现有定向检查之外的行为不声明已验证。
- 本轮无 Schema 或真实数据变更；可按导出/runtime、网关钱包/CLI、路由/前端三个功能组撤回本轮代码 diff。若后续已保存路由配置，需要同时还原保存前配置，单独回退代码不会恢复已退役字段。

## Trellis 收尾

已将 SQLite lifecycle、认证钱包摘要和简化路由往返约束写入各包 spec，并更新索引。初次归档与日志使用 `--no-commit`，当时尚未获得提交授权；未运行技能默认自动提交，也未把基线 commit 冒充本轮实现 commit。

## 后续提交授权

用户在修复验收后明确要求“提交代码 push”。修复已提交为 `ad7ac23ab199c736b9250e7895efa39bf10a17c9`（`fix: 修复单用户与 SQLite 运行一致性问题`），目标远端分支为 `origin/slim-personal`。提交前确认本地与远端基线一致，`git diff --check` 及暂存区检查通过；本轮提交操作未再修改应用代码，沿用上述验证结果。
