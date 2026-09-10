# Docker → macOS 桌面数据迁移调查

调查范围：只读源码与分支差异，没有访问 Docker、实际数据库、环境凭据或 Keychain，没有启动服务、构建或执行迁移。实际数据源确认、备份、导入和原生验收由主任务执行。

## 结论

若源为兼容的 SQLite，保留全部数据的首选是**一致性数据库快照 + 原数据库加密密钥**，在隔离副本完成迁移和验证后交给桌面宿主。这样保留管理员和所有关联实体的原 ID，避免逻辑导入重新映射。不能只复制正在使用的 SQLite 主文件；本项目使用 WAL。

管理页面的“完整数据”是配置与用户聚合备份，不包含逐请求历史、所有统计维度、完整路由版本/绑定及全部钱包/结算数据。它适合业务配置和汇总数据迁移，不能作为全部历史已迁移的证据。

## 当前分支和源分支有差异

- 本次 desktop 分支 HEAD：`0942819e8b338f6a30aa64a5f7b8d2447a1b309f`；桌面实现另有本地改动。
- 调查时 `slim-personal`：`a31c317366f6346735b36cef671e2db0e5cb8236`，提交标题“fix: 取消备份版本限制并保证完整导入导出”。
- `a31c31736` 新增 `request/system/backup_state.rs` 和 SQLite 备份事务作用域；整体导入在同一事务内执行，导出使用同一读事务。当前 desktop 分支尚未包含这些变更。
- 该提交还移除了导出文档的 `version` 字段，并按实际内容判断兼容性。当前 desktop 导入仍强制聚合版本 `2.0`、配置版本 `3.0`、用户版本 `2.0`。**如果 Docker 已运行新提交，其管理 API 导出不能直接交给当前 desktop 导入器。**不要仅补版本字符串就宣称格式已兼容。
- 原 worktree 工作区 clean 不代表两分支相同；本次仅读取差异，没有修改原 worktree。

## 当前 desktop 管理备份覆盖范围

入口见 `apps/aether-gateway/src/handlers/admin/system/core/system_routes.rs:69`：

- `GET /api/admin/system/config/export` → 配置 JSON；`POST /api/admin/system/config/import` 接收其顶层字段，可增加 `merge_mode`。
- `GET /api/admin/system/data/export` → `{ version, exported_at, config_data, user_data }`；`POST /api/admin/system/data/import` 接收同形 JSON，顶层 `merge_mode` 会传给两个部分。
- `merge_mode` 为 `skip`、`overwrite`、`error`，缺省 `skip`。聚合/用户导入因当前管理员已存在而拒绝 `error`。

| 数据 | 管理 JSON 备份的实际行为 | 完整 SQLite/CLI 路径 |
| --- | --- | --- |
| 管理员 | 必须恰好一个有效本地管理员；源管理员 ID 映射为当前操作者 ID。`overwrite` 恢复用户名、邮箱、密码哈希、权限与偏好；保留目标用户 ID 和创建时间。`skip` 不恢复管理员资料/偏好 | 保留原行与原 ID；desktop 可复用唯一有效本地管理员，不要求旧密码可登录 |
| 客户端 API Key | 普通与独立 Key 均包含；按 key hash 匹配，现有 Key 保留目标 ID，新 Key 生成 UUID；关联到当前管理员；保留 hash 和限流/累计字段 | 保留原 ID、归属、hash、密文及全部字段 |
| 渠道和上游 Key | Provider 按名称匹配；Key 按原 ID 或解密后的凭证匹配。新 Provider 会获得新 ID，映射用于用户权限和默认路由。上游 API Key、OAuth/service-account auth config、Provider Ops 敏感凭证可重加密 | 原 ID 和数据库密文完整保留 |
| 模型/端点 | Global model、Provider model、endpoint 包含；按名称/格式重建关联，部分实体 ID、时间及运行健康状态不会逐字保留 | 原行保留 |
| 路由 | 仅系统默认分组的当前配置；映射 Provider/Key 引用。覆盖时写目标的新版本，不恢复原版本历史 | `routing_groups`、`routing_group_versions`、`routing_group_bindings` 都包含 |
| 用量统计 | 仅 `stats_daily`、`stats_user_daily`、`stats_daily_api_key`，加用户/API Key 累计字段。无法映射的用户或 Key 日统计会跳过并计数；累计补差会折算进某天汇总 | 逐请求 `usage`、结算快照、计数增量以及所有注册统计表均包含 |
| 请求明细/审计 | 不包含逐请求 usage、HTTP audit、body blob、routing snapshot、audit logs | 默认全部包含；不要传 `--omit-request-body-details` |
| 钱包、结算、认证附属数据 | 管理配置/用户文档没有完整钱包/支付/结算账本、auth_modules、management_tokens、sessions 等部分 | 相关当前业务表包含 |
| 系统设置/代理节点 | 持久化 system configs、代理节点配置包含；敏感 system config 重新加密，代理用户名/密码也在备份中。退役默认用户组配置可能归零 | 原数据库字段保留；桌面端口、目录、自启动和 Keychain 不属于数据库备份 |

主要证据：`request/system/export.rs:68,160,185`、`request/system/user_backup.rs:64,234,315`、`request/system/import.rs:841,1113,1300,1455`、`crates/aether-data/runtime/src/repository/system.rs:78`、`crates/aether-data/adapters/sqlite/src/users.rs:621`。

### 密钥与管理员细节

- 管理导出用源密钥解密，输出可恢复的明文；导入用目标密钥重新加密。路径：`system/shared/export/support.rs:11`、`system/shared/export/providers.rs:91`、`request/system/backup_keys.rs:117`、`request/system/import.rs:305`、`system/shared/configs.rs:125`、`request/system/user_backup.rs:438`。
- 用户备份拒绝携带源 `key_encrypted`；Key 明文存在时必须与 `key_hash` 一致。没有可导出明文的 Key 仍能仅恢复原 hash，但不能恢复完整 Key 的展示能力。
- 当前用户备份要求有效 bcrypt 密码哈希，和 desktop 启动时允许复用没有可用密码的唯一管理员是两个不同契约。
- `overwrite` 改变密码哈希时撤销当前管理员 sessions，并返回 `reauthentication_required`；桌面应通过自身会话端点恢复。资料恢复 SQL 不改变 `role/auth_source/is_active/is_deleted`。
- 管理备份文件包含凭证明文和密码哈希，应只保存在受保护的迁移目录，核验输出只保留计数/布尔结果。

### 当前 desktop API 导入不是整体原子操作

`request/system/import.rs:500` 先验证两个文档，再执行配置，随后执行用户导入。运行期加密、数据库约束或用户恢复失败，先前配置仍可能已写入；用户 Key 又先于汇总和管理员资料写入。汇总写入有自己的事务，不覆盖之前步骤。

配置导入顺序：代理节点 → 外部模型代理设置 → global models → 每个 Provider/endpoint/Key/model → 系统默认路由 → 其他 system configs。响应的 `config.stats.errors` 可以包含已跳过的模型/路由项，不能只检查 HTTP 200。

## CLI 完整逻辑迁移

**CLI 是顶层 `export`、`import`、`copy`，没有 `data` 前缀。** 定义见 `apps/aether-gateway/src/main.rs:1121,1211,2130`。

```sh
aether-gateway export --database-driver sqlite --database-url '<source-snapshot-url>' --output '<private-backup.jsonl>'
aether-gateway --database-driver sqlite --database-url '<isolated-target-url>' --migrate
aether-gateway import --database-driver sqlite --database-url '<isolated-target-url>' --input '<private-backup.jsonl>'
```

也可以在已迁移 schema 的隔离目标执行：

```sh
aether-gateway copy --source-driver sqlite --source-url '<source-snapshot-url>' --target-driver sqlite --target-url '<isolated-target-url>'
```

这些是基于代码核对的命令模板，本调查未执行。真实路径、数据库版本和凭据由主任务确认。

- `--database-driver/--database-url` 对应 `AETHER_DATABASE_DRIVER/AETHER_DATABASE_URL`，另支持 `DATABASE_URL` 回落。当前实际支持 SQLite；旧 PostgreSQL/MySQL 不能交给当前 CLI 直接读取。
- 不指定 `--domains` 时导出全部 14 个支持域；JSONL 格式版本为 2，导入接受 1–2。导出的域排序并去重，导入按 manifest 顺序执行。
- 静态解析当前 SQLite SQL migration 的建表、删表、重命名后，与导出注册表比对：**77 张业务表，77 张均注册**。这不包括 `_sqlx_migrations` 等元数据，也不证明未知旧版本/第三方表会被保留；原 SQLite 快照保真范围更大。
- SQLite 导出在单读事务中读取所有域；导入在单写事务中设置 `PRAGMA defer_foreign_keys = ON`，提交时验证引用。冲突按表主键 `DO UPDATE`，不做 `REPLACE`，不会删除目标中备份未包含的行；其他唯一键冲突会失败。
- `import/copy` 不创建 schema、不自动执行 migrations；`--migrate` 是单独分支，完成后直接退出，不触发 desktop 管理员创建。不要先启动 desktop 生成另一个管理员，再把原用户全量 upsert 进去。
- CLI 原样保留数据库密文，没有跨密钥重加密。即使 export/import 参数里可见 encryption_key，实际数据函数只接收 SQL config；保留原加密密钥是恢复凭证可用性的前提。
- 导入允许忽略未知且值为 null 的列；未知非 null 列会拒绝并回滚。缺少目标要求字段、退役非空域和不兼容 schema 需要先在副本处理，不能假设任意版本互通。
- export 的连接工厂启用 `create_if_missing(true)` 和 WAL，不是严格只读的源连接；只对已取得的一致性快照工作副本调用。`export/import/copy` 会在内存中组装完整 JSONL，大历史迁移优先考虑 SQLite 快照。
- `copy --omit-request-body-details` 会删除 usage/http audit body 列并跳过 `usage_body_blobs`，保留完整历史时不应使用。

主要证据：`crates/aether-data/runtime/src/lifecycle/export.rs:98,724,743,773,798,921`、`export/sqlite.rs:40,94,288`、`crates/aether-data/adapters/sqlite/src/pool.rs:33`。已有源码回归覆盖保留密文、父表 upsert 不级联删除、后期失败回滚等，见 `export/tests.rs:405,470,640`；本轮没有运行这些测试。

## 主任务可执行的最安全顺序

1. 核实 Docker 实际版本、数据库类型和 schema，以及唯一有效本地管理员是否存在；不从当前工作区分支推断容器内容。
2. 获取一致性源快照并保留原备份；在另外的工作副本操作，记录迁移截点。在线源在快照后产生的新请求不会自动出现在快照中。
3. 同版本/兼容 SQLite 优先保留完整快照，再仅在目标副本执行所需 schema 迁移；需要逻辑复制时使用上面的 CLI 路径并先准备 schema。
4. 为迁移目标持久化原数据库加密密钥，和桌面 Keychain 配置对齐；不能用桌面新生成的加密密钥直接读取源密文。数据库与密钥应成对可恢复。
5. 启动前在隔离目标核对管理员数量与原 ID、Provider/Key/model/usage/统计/路由数量、外键一致性；确认凭证可解密但不输出其内容。
6. 原生客户端复用迁入的唯一管理员；核对页面、API Key、渠道、路由和历史明细。若选择 API 聚合导入，明确其覆盖限制并逐项检查导入统计、跳过项和重新认证。
7. 验证通过后才把已确认的目标数据交给日常桌面目录；保持源 Docker 数据和回退备份可恢复。

本子任务唯一新增文件是本记录；未改产品源码、未接触实际用户数据，未执行构建、测试、启动或迁移。剩余事项是主任务核实实际 Docker 状态、选择源格式匹配路径、执行备份/迁移并验收。
