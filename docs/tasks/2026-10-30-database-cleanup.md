# 使用记录数据库清理

日期：2026-10-30

## 问题

桌面版请求记录配置为保留 3 天，每日 03:00 自动清理；候选记录保留 7 天。自动任务此前正常执行并删除过数千条请求记录，但应用没有在 10-30 03:00 保持运行，当天任务被错过。同时，清理只会删除 `usage` 主记录，不会删除没有外键约束的 `usage_settlement_snapshots`，导致旧结算快照持续累积。

清理前数据库约 1.14 GB，包含 17,564 条 usage、111,725 条结算快照，其中 94,161 条没有对应 usage。

## 处理

- 在线使用 SQLite backup API 创建一致性备份：
  `~/Library/Application Support/com.aether.desktop/backups/manual-cleanup-20261030-110435/aether.db`
- 按现有 3 天保留策略删除 6,406 条旧 usage；关联的 HTTP 审计、正文和路由快照由外键级联删除。
- 删除清理后全部无对应 usage 的结算快照，避免继续占用空间。
- 保留 55 天统计日汇总、配置、密钥、钱包、候选记录及其他业务数据。
- 运行 `PRAGMA quick_check`、`PRAGMA integrity_check`、`PRAGMA foreign_key_check` 和 WAL 被动检查点。

## 结果

- usage、结算快照、HTTP 审计、路由快照均剩余 11,111 条，且一一关联。
- 无对应 usage 的结算快照由 94,161 条降为 0。
- `quick_check`、`integrity_check` 和 `foreign_key_check` 均通过。
- 数据库文件仍约 1.09 GB，但已有 159,937 个空闲页，后续写入会优先复用；完整压缩需要独占数据库，本次为保持网关在线未执行。
- 清理后网关继续监听 `127.0.0.1:8084`，`/health` 返回 200。

## 后续

- `delete_old_usage_records` 应同步删除对应结算快照，避免下次自动清理再次产生孤儿数据。
- usage 清理应增加启动补偿：应用错过 03:00 后，在下次启动时补跑一次有效期检查。
- 待用户允许短暂停机时，对数据库执行一次 `VACUUM`，将文件大小收缩到有效数据规模。
