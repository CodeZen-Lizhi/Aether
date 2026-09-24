# 数据访问与迁移

## 权威边界

[数据层 README](../../../crates/aether-data/runtime/README.md) 定义分层：[contracts](../../../crates/aether-data/contracts/src/lib.rs) 持有 DTO、repository traits 和错误；[SQLite adapter](../../../crates/aether-data/adapters/sqlite/src/lib.rs) 持有池、请求路径 SQL 和可执行 migrations；runtime 组合后端、memory 实现及维护流程。

默认 SQL 后端只有 SQLite。历史 PostgreSQL/MySQL Schema、元数据及错误枚举不提供连接能力；不要据此新增全驱动测试矩阵或恢复旧 adapter。完整兼容行为见 [SQLite lifecycle](../aether-data/backend/sqlite-lifecycle.md)。

## Schema 与写入

- 当前 Schema 以 [可执行 SQLite migrations](../../../crates/aether-data/adapters/sqlite/migrations) 的完整序列为准；logical/generated SQL 包含历史形状，不能直接推断现存表、列和外键。
- 已发布迁移保持版本与校验和；结构变更新增迁移，并核对 contracts、SQLite repository、memory 对应行为、备份覆盖及调用方。导入或回填需用临时数据库验证，不连接用户工作数据库做验证。
- 外部值通过参数绑定进入 SQL；动态表名/排序字段从明确允许的集合选择。事务覆盖业务原子性，更新失败不能被成功响应或默认值掩盖。
- JSON、布尔、时间、金额按实际 Schema 和当前转换代码处理，不用语言类型猜数据库单位。特别保留 lifecycle 契约记录的历史时间单位与导入兼容语义。
- JSONL 导入和后台管理 JSON 备份是不同协议，分别遵循 [SQLite lifecycle](../aether-data/backend/sqlite-lifecycle.md) 和 [admin backup](../aether-gateway/backend/admin-json-backup.md)，不要混用版本或覆盖语义。

## 并发与重试

健康结算修改同时核对 memory/SQLite、唯一身份、receipt、条件更新与回滚。数据库失败重试的是原终态事实的写入，不能重新调用模型，也不能生成新 attempt 身份。细节只维护在 [聊天结算契约](../aether-gateway/backend/chat-failover.md)。

## 验证选择

从受影响 repository 或 `lifecycle::{migrate,backfill,export}` 的现有测试开始，验证真实 SQL、回滚和写后读取。Schema 审计可执行 `bash crates/aether-data/runtime/schema/compose_schema.sh check`；该命令只检查文件，不证明迁移运行成功。驱动 feature 变化才补 README 中相关编译组合，普通查询改动无需全部执行。
