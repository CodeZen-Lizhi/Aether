# Design — 需求清单第 1/2/3 项

> 所有触点均于 2026-09-02 在 `slim-personal` 工作树核实过（行号为当时快照，实现时以符号名为准）。
> 仓库结构注意：admin/gateway 处理器在 `apps/aether-gateway/`，核心逻辑在 `crates/`。

## 实施顺序

第 2 项 → 第 3 项 → 第 1 项 → 交叉验证。（2 是编译器驱动的纯删除，先做收敛面小；1 涉及 DB 迁移最后做。）

---

## 第 2 项：删除全局模型正则映射

### 语义删除点（调度）

`crates/aether-scheduler-core/src/model.rs`：

1. `resolve_global_model_name...` 链中第 4 个 `.or_else` 分支（~L75-88）：按 `global_model_mappings` 正则认领请求模型名 → **删**。
2. `row_supports_requested_model_exact`（~L148-157）中 `|| row.global_model_mappings...matches_model_mapping(...)` 子句 → **删**。
3. Key 白名单放宽回退循环（`resolve_key_allowed_model...` 内 ~L240-248）：`for pattern in global_model_mappings { if matches_model_mapping(pattern, allowed_model) ... }` → **删**。
4. `matches_model_mapping`（pub, ~L417-429）**保留**：`aether-model-fetch/src/association_sync.rs:8` 仍引用（自动关联存量 patterns）。
5. 测试同步：删除/改写依赖正则映射语义的用例（`regex_allowed_model_replaces_selected_provider_model_name`、`responses_model_mapping_scope_covers_search_in_one_direction`、`codex_live_reuses_only_codex_responses_model_mappings`、`operation_scoped_mapping_overrides_generic_mapping_for_compaction`、`windsurf_dashed_gpt55_alias_*`、`endpoint_scoped_default_mapping_limits_exact_global_model_match`）；`matches_model_mapping` 自身的 3 个单测保留；`sample_row` helper 去掉 `global_model_mappings` 字段。

### 行结构与读侧

- `crates/aether-data/contracts/src/repository/candidate_selection/types.rs:37`：删 `global_model_mappings` 字段。
- `crates/aether-data/adapters/sqlite/src/candidate_selection.rs`：删解析（~L656-658、690-693）、初始化（~L528 区域在 apps 侧同名文件，注意区分）、测试 fixture（~L997、L1107）。
- `crates/aether-data/runtime/src/repository/candidate_selection/memory.rs:259`：删初始化。
- `apps/aether-gateway/src/data/candidate_selection.rs:528`、`apps/aether-gateway/src/control/auth/gate.rs:830`（测试 fixture）：删初始化。
- `apps/aether-gateway/src/scheduler/candidate/tests/{model,selection,affinity,support}.rs`：按编译错误与语义更新（别名场景改精确名或供应商映射）。

### 网关路由预览

`apps/aether-gateway/src/handlers/admin/model/routing.rs`：删 `config.model_mappings` 读取（~L89-92）、响应字段 `global_model_mappings`（~L299）及其匹配函数（~L312）；**保留** provider 级 `model_mappings` 响应（~L210-226）。

### 前端

- 删 `frontend/src/features/models/components/ModelMappingsTab.vue` + `components/index.ts:5` 导出。
- `ModelDetailDrawer.vue`：删 Tab 3 挂载（~L514-527）、import（~L560）、`modelMappingsTabRef`（~L598）、`handleMappingsUpdate` 与注释（~L653）、tab 选项里的"模型映射"项；`linkProvider/linkProviders` emit 若仅该 Tab 使用则一并清理。
- `RoutingTab.vue`：删正则展示逻辑（`modelMappingRegexCache` ~L641、`getCompiledModelMappingRegex` 调用 ~L943/964、`MAX_MODEL_NAME_LENGTH` ~L1039 视残留）；`getGlobalModelRoutingPreview` 调用（~L599/939）保留，响应里 `global_model_mappings` 字段消费随之后端删除一并清理。
- `frontend/src/features/models/utils/model-mapping-regex.ts` + `__tests__/model-mapping-regex.spec.ts`：视残留 import 者决定删或瘦身。
- 供应商级映射（`ModelMappingDialog.vue`、`provider-tabs/ModelMappingTab.vue`）**不动**。

---

## 第 3 项：模型管理批量操作重构

`frontend/src/views/admin/ModelManagement.vue`（1932 行）：

### 删除（面板化一套）

- 工具栏按钮（~L30-37，`ListChecks` 图标，`openBatchManageDialog`）。
- 面板对话框模板（~L474-665+）与状态：`batchManageDialogOpen`、`batchPricingProviderId`、`filteredBatchManageModels`（模板 ~L560）、`applyBatchManageShortcut`、`toggleBatchManageModelSelection`、`toggleAllBatchManageModels`、`loadBatchManageModels`、`openBatchManageDialog`、`getBatchPricingState*` 系列、`batchPricingProviderOptions/selectedBatchPricingProvider/batchPricingProviderModels/rememberedBatchPricingProviderIds/batchPricingStateByModelId/batchManageSelectionSummary` 等 computed。
- **注意甄别**：`batchAddProviders` 一族（`handleBatchAddProvidersDialogUpdate` ~L1345、`isBatchProviderSelected`、`toggleBatchProviderSelection`、`saveBatchProviderChanges` 等）若是独立对话框则**保留**，仅移除面板绑定部分。

### 新增（列表化批量）

- 表格（`hidden xl:table`，表头 ~L45）加复选框列：表头三态 checkbox（以 `filteredGlobalModels` 为全选范围），行 checkbox `@click.stop`（防触发行点击进详情）。另一处行渲染（~L224 的卡片/次视图）同样加 checkbox。
- 选择状态：`selectedModelIds: Set<string>` + `isAllSelected/isIndeterminate` computed + `toggleSelectAll/toggleSelectOne`。
- 批量操作栏：`selectedModelIds.size > 0` 时浮出（列表上方 sticky 条）：`已选 N` + `同步在线价格` + `删除` + `取消选择`。
- **同步在线价格**：复用 `getModelsDevList`（~L737）、`buildGlobalModelPriceSyncPlan`（import ~L739，REMEMBERED 来源）、`runBatchTasksWithConcurrency`（~L1598 定义，保留）；统计提示 `可更新 N · 已一致 N · 不兼容 N · 无在线价格 N`。
- **批量删除**：二次确认（项目内确认对话框模式）→ 并发调用现有删除接口 → 刷新 + toast。
- 后端无改动。

---

## 第 1 项：Key 级默认倍率

字段：`default_rate_multiplier`（REAL，NOT NULL DEFAULT 1，≥0 有限数校验同格式映射值）。

### 数据层（自底向上）

1. 迁移：新增 `crates/aether-data/adapters/sqlite/migrations/20260902000000_add_provider_api_key_default_rate_multiplier.sql`：`ALTER TABLE provider_api_keys ADD COLUMN default_rate_multiplier REAL NOT NULL DEFAULT 1;`（风格对齐 20260901 slim 迁移）。仅 sqlite adapter 存在（`crates/aether-data/adapters/` 下只有 sqlite）。
2. `crates/aether-data/contracts/src/repository/provider_catalog/types.rs:419`：Key record 加字段（默认 501 区域、setter 549/568 区域同步）。
3. `crates/aether-data/adapters/sqlite/src/provider_catalog.rs`：SELECT 列清单（~L116/173/233）、行读取（~L951）、`key_insert_sql`（~L2720，占位符数量同步）、`key_update_sql`（~L2745+）。

### 计费链路

4. `crates/aether-data/adapters/sqlite/src/billing.rs`：`MODEL_CONTEXT_COLUMNS`（~L14-16）加 `COALESCE(pak.default_rate_multiplier, 1.0) AS provider_api_key_default_rate_multiplier`；行读取（~L1018）。
5. `crates/aether-data/contracts/src/repository/billing/types.rs`：`StoredBillingModelContext` 加字段 + `new()` 参数（所有调用点同步，含 memory 实现）。
6. `crates/aether-billing/src/pricing.rs`：`BillingModelPricingSnapshot` 加字段；两个 `From<StoredBillingModelContext>` impl（~L462-500）同步；`rate_multiplier_for_api_format`（~L443-460）回退改为：格式映射未命中（含空 api_format）→ `provider_api_key_default_rate_multiplier`（None/非法回落 1.0）。`service.rs:263` 调用点无需改动。

### Admin 处理器（apps/aether-gateway）

7. `handlers/admin/provider/shared/payloads.rs`：create（~L22）/update（~L66）DTO 加 `default_rate_multiplier: Option<f64>`；key 响应 DTO 加字段。
8. `handlers/admin/provider/write/normalize.rs`：`normalize_rate_multipliers`（~L41-62）旁新增 `normalize_default_rate_multiplier`（≥0 有限，None→1）。
9. `write/keys/{create,update,batch}.rs` 透传；`handlers/admin/system/shared/export/providers.rs` 与 `handlers/admin/request/system/import.rs` 的 key 载荷同步（旧数据无字段→1）。
10. 展示类载荷补字段（低成本保持一致性）：`handlers/public/system_modules_helpers/keys_grouped.rs:163`、`handlers/shared/catalog.rs:2299`、`handlers/admin/observability/monitoring/cache_affinity_reads.rs:203`、`crates/aether-admin/src/system.rs:416` 区域 DTO；`control/auth/gate.rs:988` 结构体视用途决定。
11. transport snapshot（`crates/aether-provider/transport/src/snapshot.rs:67`）**不改**：结算走 DB 上下文，快照非计费路径。

### 前端

12. 类型：`api/endpoints/keys.ts`（195/227/259）、`api/admin.ts:312`、`api/endpoints/types/provider.ts`（236/329）加 `default_rate_multiplier`。
13. `KeyFormDialog.vue`：表单加"成本倍率"区——默认倍率 number 输入（默认 1，范围 0.01–100）+ 按 API 格式倍率输入列表（当前表单无控件，仅透传）；提交时构造 `rate_multipliers`（替换现有 789-845 的透传过滤）并带 `default_rate_multiplier`。
14. `ProviderDetailDrawer.vue`：`getKeyRateMultiplier` 未命中格式映射时回落显示 Key 默认倍率；默认倍率 ≠1 时在 Key 第二行显示 `默认 {n}x` 徽标（编辑走 KeyFormDialog；内联格式编辑保留）。
15. 测试：`provider-key-concurrent_limit.spec.ts` 同目录按需补；结算测试在 `aether-billing`（格式优先/默认回退/free_tier）。

### 风险

- `migrates_cross_driver_schema_parity_contract` 等迁移测试可能要求同步 parity fixture —— 实现时以测试输出为准修复。
- 并行会话未提交改动庞大：任何时刻发现工作树出现新文件变更，先 `git status` 核对再继续；**绝不** stage 非本任务文件。
