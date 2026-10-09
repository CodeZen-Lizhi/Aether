# Provider Ops 认证方式契约

通用要求见 [共享规范](../../backend/quality-guidelines.md)；以下保留项目已确认的专门规则。

### Convention: provider-ops 架构预设的多认证方式拆分（2026-09，任务 09-02-newapi-auth-split-balance-errors）

**What**: `crates/aether-admin/src/provider/ops/architectures/` 下新增/调整架构预设时，凡凭据存在「二选一」方式（如 访问令牌 vs Cookie），必须在 `supported_auth_types` 中拆成多个 `ProviderOpsAuthSpec` 条目（一个方式一个 schema），而不是把所有字段塞进单一 schema 靠 `any_required` / `conditional_required` 兜底。

**Why**: 前端 `ProviderAuthDialog.vue` 的「认证方式」下拉框只在 `supported_auth_types.length > 1` 时渲染，schema 驱动渲染各方式字段。单一扁平 schema 会让用户看到全部输入框，保存时才报校验错误；拆分后按方式只显示对应字段。

**Example**（参照 `sub2api.rs` 与 `new_api.rs`）:
- 每个条目：`auth_type` 必须取前端 `ConnectorAuthType` 联合（`'api_key' | 'session_login' | 'oauth' | 'cookie' | 'none'`）中的值；display_name 用中文方式名
- 校验用显式 `required`（JSON schema `required` 数组 + `x-validation` type `"required"`），不用 any/conditional 组合
- `credentials_schema`（顶层）指向 `default_connector` 对应方式的 schema
- 存量兼容：沿用旧的 `auth_type` 值给默认方式，避免已保存配置解析不到 schema
- 方式间共享的字段（如 new_api 的 user_id）在每个 schema 中都要出现且必填

## 密钥倍率同步

本地密钥的倍率来源与同步状态保存在 `upstream_metadata.multiplier`，缺省为手动。操作 `sync_multiplier` 支持供应商级同步、按 `config.key_id` 单密钥同步，以及通过 `config.mode` 切换 `manual` / `upstream`。切回手动立即持久化且不依赖用户认证，可携带 `config.multiplier` 保存手动值；开启跟随上游须先查询到有效倍率，再原子保存来源和值，失败保留原模式和倍率。

仅精确匹配同站点、同账号下已配置的本地密钥，不导入上游密钥。用户专属分组倍率优先于默认倍率，零值有效；缺分组或非法值不得推断或回退为 `1`。成功更新唯一生效字段 `default_rate_multiplier`，计费契约见 [倍率契约](../../aether-billing/backend/quality-guidelines.md)。

三种认证模板的手动、批量和后台余额查询共用动作入口，附带独立 `multiplier_sync` 结果；不新增定时任务。SUB2API 在同轮查询前共用续期凭据，余额与倍率并发查询并分别处理超时、失败，余额结果先独立写入缓存；倍率失败不改变余额结果。无跟随上游密钥时不查询倍率接口。

New API 模板使用账号访问令牌或 Cookie 查询 `/api/token/`、必要的完整密钥读取接口及 `/api/user/self/groups`（旧版兼容 `/api/user/groups`），按完整密钥匹配后使用当前用户的分组 `ratio`。`auto` 分组无法表示为固定成本倍率，拒绝开启跟随上游。

API Key 用量模板在配置用户认证后，使用当前本地密钥查询兼容 SUB2API `/v1/sub2api/billing` 的 `effective_rate_multiplier`。切换跟随前必须取得有效倍率，否则保留手动来源及当前倍率并返回具体原因；已有跟随密钥的后续失败只记录失败状态，不修改当前倍率。New API 和 API Key 模板复用余额刷新任务。所有供应商密钥显示来源选择和当前倍率，界面仅保留逐密钥立即同步入口。未填写的初始倍率是 `1`，用户创建时填写的值在同步成功前保留。

清除用户认证仅移除供应商操作配置，不修改密钥的倍率及来源。认证过期、缺失、无权限或网络请求失败时跳过倍率更新，保留当前生效值及上次成功时间；`1` 仅是创建时未指定倍率的默认值，不能作为同步失败的回退值。

倍率写入以用户认证配置快照、密钥凭据、当前倍率和命名空间版本为条件，失败返回冲突；每次模式或同步结果改变版本。普通编辑同样核对该命名空间，不能恢复旧同步倍率。手动模式不接受自动更新，跟随上游模式不接受直接修改倍率。

汇总列表沿用静默失败展示规则；具体错误只在密钥详情/编辑入口展示。密钥和令牌不进入同步摘要或日志。真实站点可能不返回某些分组，需保留现值并展示原因。

**Extensibility**: 新增第三种方式 = 再加一个 `ProviderOpsAuthSpec` 条目 + 对应 schema，前端自动出现新选项，无需前端改动。

运行时账号续期仅按旧认证配置快照条件更新 `provider_ops`，不得整行覆盖供应商。清除或更换认证与续期/倍率同步并发时，旧请求的凭据与倍率结果均丢弃。
