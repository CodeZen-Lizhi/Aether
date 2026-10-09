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

## SUB2API 密钥倍率同步

本地密钥的倍率来源与同步状态保存在 `upstream_metadata.multiplier`，缺省为手动。操作 `sync_multiplier` 支持供应商级同步、按 `config.key_id` 单密钥同步，以及通过 `config.mode` 切换 `manual` / `upstream`。模式切换立即持久化；跟随上游时随后同步，失败保留原生效倍率。

仅精确匹配同站点、同账号下已配置的本地密钥，不导入上游密钥。用户专属分组倍率优先于默认倍率，零值有效；缺分组或非法值不得推断或回退为 `1`。成功更新唯一生效字段 `default_rate_multiplier`，计费契约见 [倍率契约](../../aether-billing/backend/quality-guidelines.md)。

SUB2API 的手动、批量和后台余额查询共用动作入口，附带独立 `multiplier_sync` 结果；不新增定时任务。余额查询续期后重读保存的凭据，倍率失败不改变余额结果。无跟随上游密钥时不查询倍率接口。

倍率写入以凭据、当前倍率和命名空间版本为条件，失败返回冲突；每次模式或同步结果改变版本。普通编辑同样核对该命名空间，不能恢复旧同步倍率。手动模式不接受自动更新，跟随上游模式不接受直接修改倍率。

汇总列表沿用静默失败展示规则；具体错误只在密钥详情/编辑入口展示。密钥和令牌不进入同步摘要或日志。真实站点可能不返回某些分组，需保留现值并展示原因。

**Extensibility**: 新增第三种方式 = 再加一个 `ProviderOpsAuthSpec` 条目 + 对应 schema，前端自动出现新选项，无需前端改动。
