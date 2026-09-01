# 供应商模块裁剪（需求 1）：类型只保留「自定义」

**目标**：添加/编辑供应商时类型只有「自定义(custom)」；其余 9 个 OAuth 账号型反代类型（vertex_ai、claude_code、codex、chatgpt_web、gemini_cli、grok、kiro、windsurf、antigravity）前后端代码彻底删除；存量非 custom 供应商数据一并清理。

**执行时机**：按你的要求，本次只出计划不改代码；等几点需求都提完后统一实施。

⚠️ 不可逆提示：删除后 Claude Code / Codex 等登录接入能力全部丧失，且不可恢复（已确认）。

---

## A. 后端（Rust workspace）

**A1 入口白名单**
- `apps/aether-gateway/src/handlers/admin/provider/write/normalize.rs:3-13`：白名单收窄为仅 `custom`，同步改错误文案
- `create.rs:39-40`（默认 custom，保留）、`:223`（按类型开格式转换 → 固定为 custom 行为）
- `update.rs:51-63`：不再允许修改类型；`:253-304` codex/claude_code 专属高级配置校验删除

**A2 类型模板与运行时策略**（`crates/aether-provider/transport/src/provider_types.rs`，983 行）
- 删 9 个 fixed 供应商模板（:279-504，含硬编码 base_url/endpoint/policy）
- `provider_runtime_policy`（:475-489）只留 custom 分支，遗留别名（openai/gemini/jina/doubao/aliyun 等）一并删
- OAuth 模板类型（:545-634）与 `ADMIN_PROVIDER_OAUTH_TEMPLATE_TYPES` 删除
- 对应单元测试（:636-983）重写为 custom-only

**A3 OAuth 登录子系统（前后端整条链路）**
- 删 `apps/aether-gateway/src/handlers/admin/provider/oauth/` 整个目录（dispatch、quota、provisioning、runtime、state 等），`admin_api.rs` 路由摘除
- 收敛 `apps/aether-gateway/src/oauth/` 模块与 `state/oauth.rs` 接线（`state/core.rs`、`state/app.rs`）。实施时先确认 `ai_serving/transport.rs` 对 oauth 的引用是否仅为 OAuth 类型上游注入——若是则随类型一起删；若含 custom 也用的上游代理能力则保留代理部分
- `delete_task.rs:174-177,358` 删除保护分支简化

**A4 池适配器**（`crates/aether-provider/pool/src/`）
- 删 `providers/{codex,kiro,grok,gemini_cli,antigravity,windsurf,chatgpt_web}.rs` 与 `unsupported.rs`，注册表 `mod.rs` 只剩 default
- `service.rs:67-129` `adapter()` 分发简化

**A5 请求链路类型特判**
- `aether-ai/formats` `shared/request.rs:42-55,177-189`（codex 特判）、`provider_compat/surfaces.rs:22-113`（按类型适配面）收敛
- `aether-ai/serving/report_context.rs:222-241`（codex→openai:responses）删
- `aether-model-fetch/logic.rs:86-87`（codex URL 特判）、`:546-551` 及预设模型列表（claude_code/gemini_cli/grok）删
- `candidate_selection/types.rs:124-132` codex 特判删

**A6 测试同步**：provider_types 单测重写；`tests/architecture/admin_provider.rs:300-394` 路由结构测试更新；`responses_websocket_e2e.rs` codex 夹具、`aether-admin/provider/pool.rs:741-1085`、gateway 各处 codex 夹具替换为 custom。

## B. 前端（Vue）

**B1 表单**：`ProviderFormDialog.vue` 新建/编辑两份 `<SelectItem>`（:40-105）收敛为仅「自定义」（保持默认值 custom）；删反代提示（:109-113）、kiro（:321）/codex（:337）专属字段、payload 类型分支（:599-602,:652-657）

**B2 类型与工具**：`provider.ts:753` 联合类型收窄为 `'custom'`；`providerTypeUtils.ts` 删 OAuth 集合与判断（调用点 ProviderTableRow:288、ProviderMobileCard:242/310、ProviderBatchActionDialog:114 统一按「密钥」处理），spec 同步清理

**B3 类型专属组件删除**：`OAuthAccountDialog.vue`(+spec)、`ProviderAuthDialog.vue`、`AntigravityQuotaDialog.vue`、`auth-templates/` 目录；`ProviderManagement.vue` 摘除 auth/余额挂载；`useProviderBalance.ts`、`ProviderBalanceCell.vue`、`ProviderTableRow.vue` 的 auth-templates 依赖清理；`KeyFormDialog.vue` vertex_ai 认证分支（:387,:435,:464,:538）删；`ProviderDetailDrawer.vue`（4046 行）约 20 处类型分支清理；`endpoint-protocol-policy.ts`、`endpoint-default-paths.ts` 特例删；`poolStatsDisplay.ts`、`ccswitchImport.ts` 字面量清理

**B4 OAuth API 与孤儿代码**：删 `api/endpoints/provider_oauth.ts`；`keys.ts:386` batch-import 函数及调用方删（`:266` clear-oauth-invalid 与 `auth_type:'oauth'` 联动后端确认后删）；`PoolAccountBatchDialog.vue` 已无引用（pool 功能此前已裁剪）→ 删

## C. 存量数据清理

- 本地 `data/aether.db`：一次性 SQL 删除非 custom 供应商及其关联行（provider_api_keys、provider_endpoints、provider_usage_tracking、request_candidates、pool_member_scores、api_key_provider_mappings、models 等，按实际外键关系生成）；OAuth 账号凭据（推测为 provider_api_keys 中 `auth_type='oauth'` 行，实施时确认）一并清理
- 不改表结构（`provider_type` 为普通 TEXT/VARCHAR，无约束），不新增迁移
- 生产库（docker 部署）提供同样 SQL，或升级后在界面手动删

## D. 验证

1. `cargo build && cargo test`（workspace 全量）
2. 前端 type-check + build + vitest
3. 手动验证：添加供应商弹窗仅「自定义」且默认选中；旧 codex 数据清理后列表/详情/请求链路正常

## 明确不动

- `003_auth_config` 的 `oauth_providers` 表（用户登录 OAuth，与供应商无关）
- 数据库表结构、api_format 体系（那是协议概念，非供应商类型）