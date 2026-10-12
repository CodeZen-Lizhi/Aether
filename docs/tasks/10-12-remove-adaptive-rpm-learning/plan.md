# 移除自适应 RPM 学习

日期：2026-10-12  
状态：已完成

## 目标与范围

按用户配置的 `rpm_limit` 管理每条供应商密钥的 RPM 限制，移除自动学习出的 RPM 值及相关运行功能。保留数据库现有 Schema；历史 `learned_rpm_limit` 数据不再参与调度、更新、管理 API、备份恢复或页面展示。未设置 `rpm_limit` 表示该 Key 不设 RPM 限制。

用户故事：作为 Aether 管理者，我希望每条 Key 的 RPM 行为只受自己设置的限制控制，以避免系统根据上游流量自动生成并应用另一个限制值。

业务链路：Key 表单配置 `rpm_limit` → 保存到现有字段 → 调度器仅按该字段判断请求容量 → Key 列表仅展示手动配置值。

## 验收条件

1. 配置固定 RPM 时，请求准入按配置值工作；即使数据库保留旧学习值，也不覆盖手动值。
2. `rpm_limit` 为空时，旧学习值不限制请求；运行中成功/429 请求不再写学习值。
3. Key 表单将空值说明为“不限”；详情页不显示学习数字或自适应标签。
4. 自适应管理 API 不再读取或更改学习状态；新的 Key API 响应不暴露学习值。
5. 不修改数据库迁移和实际 Schema；旧备份仍可读，但导入后不恢复学习值。

## 验证

- 调度器测试验证仅配置 `rpm_limit` 生效，空值不受旧学习值影响。
- Gateway 运行测试验证 429 / 成功回写不再改变 RPM 学习状态，并检查已移除管理接口的结果。
- 前端相关测试、类型检查验证表单及展示契约；符合条件时使用本地浏览器/桌面候选入口复核真实页面。
- 检查 SQLite Schema 与迁移文件未变化；更新本任务验证记录后再交付。

## 实际验证记录

- `cargo check -p aether-gateway` 通过。
- `cargo fmt --all -- --check`、`git diff --check` 通过。
- `cargo test -p aether-scheduler-core`：106 项通过；包含预聚合 RPM 准入测试。
- `cargo test -p aether-data`：215 项通过；覆盖 SQLite/内存候选记录按 Key 聚合及备份相关数据契约。
- SQLite/内存按 Key 聚合定向测试各 1 项通过；修复 SQLite 聚合查询括号后，`cargo test -p aether-data-sqlite --lib aggregates_recent_attempts_for_a_key_without_a_global_row_limit`（含 `--no-default-features`）均通过。
- Gateway 定向测试通过：退役自适应路由 2 项、Key 创建与 RPM 边界 1 项、RPM 超过全局候选窗口 1 项、完整备份往返 1 项。
- 前端 `npm run type-check`、密钥相关 Vitest 3 个文件 18 项、`npm run build:with-typecheck` 通过。测试保留现有 `size="sm"` 的 Vue stub 警告；构建仅有旧 Browserslist 数据提示。
- 未修改 SQLite 迁移或 Schema；工作区未提交、未 push、未发布，也未连接正在运行的桌面网关做浏览器验收。
