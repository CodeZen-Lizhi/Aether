# 原子导入的事务接入调查

调查范围：只读检查现有代码与 SQLx 0.8.6 源码；未编译、未连接任何数据库。本文为主 agent 要求持久化的研究结果，未修改实现。

## 建议与可实施边界

新增只服务管理员备份的显式 `AdminSystemImportSession`，由 `DataBackends` / `SqliteBackend` 创建，拥有 `sqlx::Transaction<'static, Sqlite>`。Gateway 保留格式归一化、加密、ID 映射、merge 决策和响应组装；所有参与导入的读取和写入通过 session 的同一连接完成。只提取本次导入实际使用的 SQL 为接受 `&mut SqliteConnection` 的 helper，不改全项目仓储 trait、不引入 task-local 事务或全库 staging。

建议调用顺序：

1. 解析整个文档、执行不依赖数据库的校验与加密准备。
2. 如需修改 external-models selector，按原接口锁顺序先取得相应 runtime mutation lock。
3. `pool.begin_with("BEGIN IMMEDIATE")`；在连接上设置 `PRAGMA defer_foreign_keys = ON`。
4. session 读取目标状态；在同一事务内执行配置、代理、模型、渠道 Key、路由策略、管理员 API Key、统计、资料、偏好和会话撤销。
5. 组装成功 payload；只有所有必需步骤成功才 `commit().await`。
6. 提交成功后同步失效应用缓存、按既有语义尽力清理 external-models runtime KV；释放 runtime lock，返回已组装的成功 payload。

`BEGIN IMMEDIATE` 在读 merge 依据之前取得 SQLite 写锁，使前台、独立后台池和其他连接上的并发写不能在导入决策期间改变目标状态。session 必须直接查连接，不能走 AppState / GatewayDataState 的缓存 reader。事务内不能再调用持有普通 pool 的仓储方法，也不能调用某个原方法后让其偷偷通过 pool 重读结果。

注意双层返回值：现 handler 使用 `Result<Result<Value, (StatusCode, Value)>, GatewayError>`。`Ok(Err(...))` 同样必须 rollback，不能只处理外层 `Err`。预期错误路径建议显式 `rollback().await`；取消/提前退出由 SQLx Transaction 的 Drop 兜底。commit 失败也必须保持失败结果，不做成功刷新。

## 现有构造与可复用模式

- `crates/aether-data/runtime/src/backend/mod.rs:38,87`：`DataBackends` 组合 SQLite backend 与 read/write 仓储；`sqlite()` 可访问具体 backend。
- `runtime/src/backend/sqlite.rs:68,74,85`：`SqliteBackend` 保存 config 和 pool；各仓储工厂传递 `pool_clone()`。
- `runtime/src/backend/transactions.rs`：`DataTransactionBackends` 目前为空，`has_any()` 固定 false；没有现成跨仓储事务接口。
- `apps/aether-gateway/src/data/state/core.rs:396`：`GatewayDataState::from_config` 创建 `DataBackends`，认证、provider、routing 等 reader 再套缓存。
- `apps/aether-gateway/src/state/core.rs:425`：`with_data_config_and_background_isolation` 可以创建两个指向同一数据库的独立前后台 pool。
- `apps/aether-gateway/src/state/app.rs:365`：AppState 为 Clone，但 data、runtime_state 和大量缓存均为 Arc。克隆不是副作用隔离。
- `apps/aether-gateway/src/state/core.rs:189`：`replace_data_state` / `replace_foreground_data_state` 会清缓存、改变 scheduler epoch、重建 tunnel、配置 request-candidate queue；不能作为无副作用的 transaction adapter。
- `crates/aether-data/adapters/sqlite/src/candidates.rs:338,366,384` 已采用 `&mut SqliteConnection` helper，仓储入口开启事务后复用，适合作为提取风格参考。

## 最小连接层改动清单

以下路径均相对仓库根目录。保留现有普通仓储入口：入口 acquire/begin 后调用提取的 helper；导入 session 调同一 helper，并由外层 session 独占 commit/rollback。

| 文件 | 需要接入同一连接的导入子集 | 要保留的行为 |
|---|---|---|
| `crates/aether-data/adapters/sqlite/src/global_models.rs` | `list_admin_global_models` / `list_admin_provider_models`，`get_admin_global_model_by_id` / `get_admin_provider_model`，`create/update_admin_global_model`，`create/update_admin_provider_model` | `CreateAdminGlobalModelRecord`、`UpdateAdminGlobalModelRecord`、`UpsertAdminProviderModelRecord`、字段校验及 row mapping 可直接复用。create/update 最后的 `get_*` 也必须走当前连接。 |
| `crates/aether-data/adapters/sqlite/src/provider_catalog.rs` | `list_providers`、`list_endpoints_by_provider_ids`、`list_keys_by_provider_ids`；`create/update_provider`、`create/update_endpoint`、`create_key`、`compare_and_update_key_admin_state` | 复用 `StoredProviderCatalogProvider/Endpoint/Key` 和 `ProviderCatalogKeyAdminCasUpdate`、当前 SQL/校验。`create_provider:555` 自己 begin/commit，要把事务 ownership 留在外层入口；CAS 返回 false 必须中止导入。 |
| `crates/aether-data/adapters/sqlite/src/proxy_nodes.rs` | `list_proxy_nodes`、必要的 `find_proxy_node`；`restore_proxy_node:554` → `find_duplicate_proxy_node:146` + `upsert_node:53` | 地址冲突检查与完整节点 restore 必须在同一连接，保留现有 runtime 状态保留逻辑。不要扩大到普通 heartbeat/metrics 写入。 |
| `crates/aether-data/adapters/sqlite/src/routing_profiles.rs` | `find_routing_group`、`list_routing_group_versions`；`create/update_routing_group`、`create_routing_group_version` | 默认策略排他更新、版本选择、版本记录处于同一事务。现 create/update 各自 begin；helper 不能内部提交。无需改非导入用 binding CRUD。 |
| `crates/aether-data/adapters/sqlite/src/auth.rs` | `list_export_api_keys_by_user_ids`；`restore_exported_api_key:396` | 复用 `StoredAuthApiKeyExportRecord`；保留 user_id/key_hash/is_standalone 的 UPSERT guard；rows_affected 为 0 必须失败。 |
| `crates/aether-data/adapters/sqlite/src/users.rs` | `find_user_auth_by_id`、用户名/邮箱占用查询；`restore_admin_profile:621`、`write_user_preferences:1057` 和其结果重读、`revoke_all_user_sessions:1293` | 管理员身份约束、用户名/邮箱唯一约束、密码替换、会话撤销、偏好写入共同回滚。只改导入需要的查询，不改全 UserReadRepository。 |
| `crates/aether-data/runtime/src/backend/system/sqlite.rs` | system config list/upsert 与重读；`import_sqlite_admin_system_usage_aggregates:127` | 抽出 aggregate `_on(connection, ...)` 主体，保留现有 public 方法作为独立事务 wrapper；3 张统计表和 ID 映射复用。 |

推荐把新增 session 放到 `runtime/src/backend/system/` 的独立模块，并经 backend facade 暴露；GatewayDataState/AppState 仅增加创建 session 和提交后失效缓存的薄包装。adapter 现有模块私有，可在 adapter 增加一个公开 `AdminImportConnection` 门面，内部调 `pub(crate)` helper，或在具体已导出的 repository type 上提供静态 `_on` helper，避免把全部模块设为 public。session API 只接受现有 records，不暴露网关 HTTP DTO，不让 runtime 依赖 `aether-admin` / gateway。

若希望进一步压缩 session 方法数，可提供 transaction 内的 typed snapshot + typed mutation batch；但必须把 read snapshot 与 apply batch 置于同一 session 生命周期内，不能在开事务前从普通 reader 生成 merge 计划。

方案选择倾向显式 session 逐项读写，而非 gateway 先普通读库再生成 typed mutation plan：前者更直接复用现有 merge loop 与 record 构造，同时消除计划与写入之间的竞态。顺序导入优先 session 自持 Transaction、使用 `&mut self`。若为兼容仓储 `&self` trait 使用 `Arc<Mutex<Option<Transaction>>>`，每次完整仓储操作只获取一次 guard，内部的结果重读/关联查询必须传同一 `&mut SqliteConnection`，禁止持锁再调用会重锁的 `self.get_*`。commit/rollback 取走 transaction 后的调用必须明确失败，不得静默回退到 pool；无需将导入未调用的方法全部改成 transaction executor。

## Planner / records 复用

现 `import.rs` 是“解析 + 顺序写入”的编排，不是可直接整体复用的纯 planner；大多数构造 record 的逻辑可以保留，仅把 lookup/mutation 路由改为 session。

- 保留 `validate_imported_config_references`、`remap_routing_strategy_config`、`remap_import_proxy`、`build_import_provider_model_record`、`backup_keys.rs` 的凭据构造与验证。
- `backup_proxy.rs::decode_node` 与 merge map 可提取复用，最终写入改为 session。
- `user_backup.rs:426` 已有 `planned_keys`，继续复用 `StoredAuthApiKeyExportRecord`；用户 profile/preferences、aggregate snapshot 都已有可用 record。
- `build_imported_user_usage_total_aggregates` 与 `import_admin_system_user_usage_aggregates` 中合并 supplemental totals 的部分可保留为纯函数，最终 aggregate write 改为 session。
- `provider/write/provider/create.rs:18` 与 `update.rs:17` 的 builder 名称看似纯构造，实际会通过 AppState 查 provider 名称冲突。提取接受现有 provider 集合/冲突事实的纯构造部分；导入传入 transaction 内的结果，避免回到 pool 和缓存。
- 现 `import.rs:1390` 对 `create_routing_group_version` 的 `Option` 结果使用 `let _ =`；若改成强制写入结果应要求 Some，否则整个导入失败。
- aggregate importer 对 unmapped user/key 目前增加 `skipped_unmapped_*` 后继续（`backend/system/sqlite.rs:225,310`）。导入端需要在写入前校验所有引用完整；不要把无法还原的数据误当成用户选择的 merge skip。

## 提交前后副作用

必须留在事务内：`user_backup.rs:490` 触发的 `revoke_all_user_sessions`。生产实现更新的是 SQLite `user_sessions`，不是纯内存动作。提交后撤销会话会出现密码/配置已生效而撤销失败，无法满足整体回滚。

必须推迟到提交成功后：

- `state/core.rs:918::remember_system_config_write` 会立即把新值插入共享 `system_config_cache`，并修改 scheduler/auth/frontdoor/transport 派生缓存。transaction 路径不要调用它。
- `state/catalog.rs:512..665` 与 `state/routing_profiles.rs` 的常规写 wrapper 立即 invalidate routing caches。失败导入不应走这些 wrapper。
- `state/runtime/api_key_exports.rs:232` 和 `state/runtime/auth/user_lifecycle.rs:434` 会立即失效 auth cache；改为提交后统一失效。
- 成功提交后清 foreground 和 background data 的 system-config/auth/provider/routing reader 缓存，并清 AppState system config、auth context、provider routing/transport、frontdoor 默认 RPM、scheduler affinity 等实际受影响派生缓存。`invalidate_auth_context_cache` 已同时处理前后台认证 reader；`invalidate_provider_routing_caches` 目前只处理 foreground data，新增总刷新时应补 background reader。
- `import.rs:747` 调 `apply_admin_external_models_config_update`；后者在 `handlers/admin/model/external_cache.rs:263` 写数据库后删除 RuntimeState KV，并进行 payload 重读。应复用输入校验/selector 逻辑，将 selector SQL 纳入 session，把 runtime KV 清理留到提交后。现代码明确 KV 清理是 best effort，v2 cache envelope 用 selector ID 防止读错缓存；不得在 DB commit 后因派生清理失败返回“导入失败、已回滚”。
- 不在提交后再执行可能失败的数据写入、SQL 重读或成功 payload 序列化。若未来有必须可靠执行的外部动作，需要另立持久化执行机制，不能把它伪装成 SQLite 事务的一部分。

## 既有原子 JSONL / copy 能力的边界

- `runtime/src/lifecycle/export/sqlite.rs:104::import_sqlite_plan` 已使用一个 transaction、`PRAGMA defer_foreign_keys = ON`、主键 UPSERT 和单次 commit；适合复用事务约定、行校验和回滚测试结构。
- 它处理物理数据库行，不提供业务 merge、名称匹配、管理员 ID 改映射或重新加密；目前入口只有 pool，不能直接包住现有 HTTP import。
- `runtime/src/lifecycle/export.rs:798::copy_database_records` 是导出源库后把物理行导入目标库；它会建立新 pool，仅目标导入阶段原子，不是已经运行中的 AppState 的事务。
- `runtime/src/lifecycle/export/tests.rs:470` 有晚期行失败回滚，`:519` 附近有 deferred FK commit 失败回滚和 pragma 重置测试，可供本次测试设计参考。

## 必须避免

- 外层 `pool.begin()`，内部仍 `.execute(&self.pool)`：这些 query 会申请别的连接，完全不在外层事务内。
- 仅把 pool max_connections 设为 1：持有 transaction 后仓储再 acquire 会自锁；裸 BEGIN/COMMIT 配合 pool 或内部 begin 也会破坏 ownership。
- 克隆 AppState / GatewayDataState 然后替换 backend：共享 Arc 缓存/runtime_state，且已构造的 repo 保留原 pool。
- 失败后导出备份回填：回填本身会失败，期间其他请求可看到部分结果，也会覆盖并发写。
- 无锁 shadow/staging 全库替换或 JSONL 回灌：覆盖其他写入，并且物理主键 UPSERT 不等同业务 merge；文件替换还不覆盖既有 WAL/连接。
- 全局 task-local 自动捕获事务、把 SQLite pool type alias 改成广域 executor wrapper：改动面与审计难度远大于本次导入需要。

## 最小验收建议

使用临时迁移后的 SQLite；不触碰用户库。至少覆盖：配置晚期错误回滚早期 provider/model 写入；完整导入用户尾部 SQL 失败回滚配置/API Key/统计/密码和会话撤销；强制 commit 时 FK 失败；失败后共享缓存仍返回导入前的值；成功后前后台缓存可见新值；并发独立连接看不到未提交部分数据。可用临时库 trigger `RAISE(ABORT, ...)` 制造晚期写失败，比只测解析错误更能证明事务接入正确。

## SQLx 核实来源

- Cargo.lock 锁定 SQLx 0.8.6；本机 registry `sqlx-core-0.8.6/src/pool/mod.rs:391` 确认 `begin_with` 返回拥有连接的 `Transaction<'static, DB>`。
- 同版本 `transaction.rs:116,124,264` 确认 commit/rollback 与 Drop rollback；`sqlx-sqlite-0.8.6/src/transaction.rs:17` 确认自定义 begin statement 被实际送入连接并校验 transaction 状态。
- Context7 查询的官方 docs.rs 索引同样确认 begin_with/Transaction 语义；索引最新签名已变化，具体实现以锁定的 0.8.6 源码为准。
