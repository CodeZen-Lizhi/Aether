# 验证结果

2026-09-09：实现与范围内质量检查完成，未提交、push 或部署。

## 已通过

| 检查 | 结果 |
|---|---|
| `cargo test -p aether-gateway --lib handlers::admin::request::system` | 21 通过，包含旧用户文件解析、来源重复项、取消、缓存、真实 FK 提交失败和完整用户往返 |
| `cargo test -p aether-gateway --lib tests::control::admin::system` | 17 通过 |
| `cargo test -p aether-gateway --test admin_unsigned_identity_headers` | 9 通过，包含 8 个备份行为用例 |
| `cargo test -p aether-data-sqlite` | 88 单元测试、6 个事务集成测试通过 |
| `cargo test -p aether-admin system` | 32 通过 |
| `cargo test -p aether-data --lib backend::system` | 2 通过 |
| `cargo test -p aether-data-contracts --lib repository::auth` | 16 通过 |
| `cargo check -p aether-data --no-default-features` | 通过 |
| SQLite schema compose `check` | 通过 |
| 前端两份备份 Vitest 文件 | 27 通过 |
| `npm run type-check -- --pretty false` | 通过 |
| 前端 7 个改动文件定向 ESLint | 通过 |
| 38 个改动 Rust 文件的定向 `rustfmt --check`、`git diff --check` | 通过 |

总计 218 个范围内自动化测试通过；另有一次真实旧文件的隔离验证。
Gateway 的 lib-test 编译仍有原有 `dispatch/refs.rs:71` unreachable-pattern warning，
不属于本次改动；未抑制或扩展无关修复。

## 真实旧备份

只读读取下载目录中的既有旧备份，通过真实认证路由导入独立临时 SQLite 后再导出。
原有 5 个全局模型、7 个提供商、15 个渠道 Key、1 个代理节点、34 项系统配置、
独立 Key 及三类统计的条数保持一致，系统配置值逐项一致。
旧文件未包含管理员，目标管理员与登录保持有效。
验证用 Rust 文件和临时数据库已清理；没有将真实备份内容或凭据写入仓库 fixture。

## 审查闭环

- 修复普通 Key 查询过滤独立 Key 导致备份漏项的问题。
- 保存并恢复 `is_locked`，避免导入后意外解锁。
- 统计不再过滤全零/仅实际成本行，不再强制完成状态。
- 导出列表不再过滤损坏成员，未知 API 格式失败。
- Endpoint 空重试次数按继承值原样恢复。
- 退役默认分组配置保持原值，非空未知/退役 domain 仍按完整性要求失败。
- 提交、缓存失效与 lease 释放在可独立完成的收尾任务中，取消窗口已有回归覆盖。
- 后段配置和偏好写入触发真实 SQL 错误时，前序配置、Key、统计、密码与会话均回滚；移除触发器后可立即重试成功。
- 独立审查最终未发现未解决的 Required / Critical 问题。

## 删除请求

已按明确授权删除 `/Users/zhenglizhi/Aether` 及其中未提交修改、备份和数据，并再次确认路径不存在。
当前工作仓库为 `/Users/zhenglizhi/otherProjects/Aether`。
