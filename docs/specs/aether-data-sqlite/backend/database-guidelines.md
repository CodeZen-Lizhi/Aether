# SQLite Adapter 数据规范

请求路径 SQL、池与 migration 的归属见 [共享数据规范](../../backend/database-guidelines.md)，导入导出规则见 [SQLite lifecycle](../../aether-data/backend/sqlite-lifecycle.md)。

以本包 [src/lib.rs](../../../../crates/aether-data/adapters/sqlite/src/lib.rs) 的 repository 导出和 [migrations](../../../../crates/aether-data/adapters/sqlite/migrations) 的完整执行序列定位实现。新增查询使用参数绑定，事务/唯一约束按实际 Schema 核对；通用 DTO 放 contracts，runtime 负责组装。该文件保留为历史任务 JSONL 的有效入口。
