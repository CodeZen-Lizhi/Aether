# 需求优化清单

记录待优化需求，暂不开发。每条保持简短：需求描述 → 现状 → 缺口。

---

## 1. 供应商 Key 级倍率计费

**需求描述**：可为每个供应商的每个 Key（endpoint key）单独设置成本倍率。计费时统一按以下公式：

```
最终费用 = 用量 × 模型价格（计费公式结果）× Key 倍率（Key 未配置倍率则视为 1）
```

该规则对 token 计价与"每请求价格"（price_per_request）两类模型同样适用——倍率乘在公式算出的总费用上。

**现状**（2026-09-01，branch `slim-personal`）：大部分已实现。

- Key 模型已有 `rate_multipliers` 字段：按 API 格式（如 `openai:chat`）→ 倍率 的映射，值须为 ≥0 的有限数（`handlers/admin/provider/write/normalize.rs:41`）。
- 前端配置入口在**供应商详情抽屉 → 密钥列表**：每个 Key 第二行 API 格式名旁的内联 `1x` 小字，点击可按格式编辑（0.01–100，`ProviderDetailDrawer.vue:186`）。Key 新建/编辑弹窗（`KeyFormDialog.vue`）只透传数据、没有输入控件。入口过于隐蔽（实际使用中找不到），需优化。
- 结算链路已应用：`crates/aether-billing/src/service.rs:263-269`，`actual_total_cost = total_cost × rate_multiplier`；倍率按本次请求实际使用的 Key、按请求 api_format 查映射，未命中回落 1.0；`free_tier` 仍固定为 0。
- 用量记录结算快照中已带 `rate_multiplier` 字段，usage 页有展示。

**缺口（2026-09-01 已确认结论）**：

1. **Key 级默认倍率 — 要做**。新增 Key 级默认倍率字段，默认值固定为 1；未配置格式映射时按默认倍率算，格式级映射保留且优先于默认值。同时把倍率配置入口从"内联 `1x` 小字"优化到更显眼的位置（如 Key 表单/Key 详情）。
2. **展示口径 — 已定**。用量/账单展示**折后价 + 倍率标注**（如 `¥0.24 ×1.5`）；折前价单独展示不需要。
3. **回溯性 — 按现状默认**：修改倍率只影响之后的新请求（按结算当时配置），历史账单不回溯。
4. **free_tier — 按现状默认**：free_tier 固定计 0，倍率不参与。

---

## 2. 删除全局模型正则映射规则（保留供应商级映射）

**结论**（2026-09-01 已确认）：两套映射只保留一套——**供应商级"模型映射"**（精确匹配、可改名 gpt-5→gpt-6-sol、可按端点/格式/请求类型限定，每个供应商单独设置）保留不动；**全局模型详情里的"映射规则"（正则）** 整体从代码删除。

删除理由：客户端始终用精确全局模型名请求、Key 白名单未启用或名字精确时，正则规则零作用；正则只做白名单放宽和入站别名，不能改名，且配置入口易产生困惑（"无匹配"）。

**删除范围**：

- 前端：`frontend/src/features/models/components/ModelMappingsTab.vue`（全局模型详情的映射规则 Tab，挂在 `ModelDetailDrawer.vue`），以及 `components/index.ts` 的导出。
- 前端注意：`features/models/utils/model-mapping-regex.ts` 同时被 `RoutingTab.vue` 引用（路由预览也用正则展示映射匹配效果），删除时 RoutingTab 的相关展示要一并处理，工具函数视残留引用决定去留；`getGlobalModelRoutingPreview` 接口为 RoutingTab 共用，只清 `ModelMappingsTab` 的调用，接口本身保留。
- 后端：`crates/aether-scheduler-core/src/model.rs` 中 `global_model_mappings` 的两个匹配分支（入站别名认领、Key 白名单放宽回退，见 `resolve_global_model_name_by` 第四分支与白名单回退循环）；模型 config 的 `model_mappings` 字段读侧停止解析，存量数据无需迁移。
- 不动：供应商级映射整条链路（`model_provider_model_mappings`、`ModelMappingDialog.vue`、`provider-tabs/ModelMappingTab.vue`）。

**删除后影响 / 使用前提**：

- 客户端必须用精确全局模型名请求，别名不再支持。
- Key 若启用模型白名单，白名单名字须与实际模型名精确一致；上游出新版本号时需手动更新白名单或补供应商映射。
- "一键关联供应商"入口随 Tab 移除，改去模型管理页关联。

**时机**：与第 1 条（Key 级倍率）一起排期，现在不开发。

---

## 3. 模型管理列表：批量操作重构（复选框 + 全选，去面板化）

**需求描述**（2026-09-01 已确认）：去掉"快速筛选与批量操作"面板，把批量能力直接放到模型列表上。

- 模型管理页每个模型行前加**复选框**，表头加**全选**（三态：全选/半选/未选；全选范围为当前筛选/搜索结果）。
- **删除右上角"快速筛选与批量操作"按钮**及其整个面板（`ModelManagement.vue` 的 `openBatchManageDialog` / `batchManageDialog` 一套）。
- 选中 >0 时列表上方浮出**批量操作栏**，提供两个动作：
  1. **同步在线价格**：沿用现有 `buildGlobalModelPriceSyncPlan` 对比逻辑，按每个模型已记录的在线价格来源（原面板默认的 REMEMBERED 来源）执行，跳过规则不变（价格一致/计价不兼容/无在线价格），完成后提示统计（可更新 N · 已一致 N · 不兼容 N · 无在线价格 N）。
  2. **批量删除**：需二次确认。
- **"价格是否一致"列表字段：先不做**。已核实该状态不是定时任务判定、也没有落库字段，而是打开原面板时前端实时拉取 models.dev 在线价格（`getModelsDevList`，`ModelManagement.vue:1589`）并用 `buildGlobalModelPriceSyncPlan` 现场对比得出的。要在列表常显需后端定时同步价格并落库，届时再单独立项。随面板删除，"价格一致"筛选视图暂无入口，接受。

**涉及范围**：

- `frontend/src/views/admin/ModelManagement.vue`：行复选框列 + 表头全选 + 批量操作栏；删除面板相关状态与模板（`batchManageDialogOpen`、`batchPricingProviderId`、筛选 chips 等）。
- 复用保留：`buildGlobalModelPriceSyncPlan`、`getModelsDevList`、批量执行并发工具 `runBatchTasksWithConcurrency`。
- 后端无需改动（同步价格、删除均走现有接口）。

**时机**：现在不开发，与第 1、2 条一起排期。

---

## 4. 供应商运维认证：中转站架构预设收敛（只保留 3 种）

**需求描述**（2026-09-01 已确认）：供应商"用户认证/运维"里可选的中转站架构预设目前共 9 种，收敛为 3 种。

- **保留**：`new_api`（New API）、`sub2api`（Sub2API）、`usage_api`（API Key 用量查询）。
- **删除**：`yescode`（YesCode）、`nekocode`（NekoCode）、`cubence`（Cubence）、`done_hub`（Done-hub）、`anyrouter`（Anyrouter）共 5 个预设。
- **保留但继续隐藏**：`generic_api`（通用 API，当前已 `hidden`）——它是注册表的兜底归一化目标：未知/空 `architecture_id` 一律回落到它（`crates/aether-admin/src/provider/ops/architectures/mod.rs:125-135`），被删预设的存量供应商读取时自动落到它，等于免费迁移，**不能删**。

**删除范围**：

- `crates/aether-admin/src/provider/ops/architectures/{yescode,nekocode,cubence,done_hub,anyrouter}.rs` 及 `mod.rs` 注册项。
- 后端余额查询的 `ProviderOpsBalanceMode::YescodeCombined` 特殊分支及 `handlers/admin/provider/ops/providers/actions/query_balance/yescode.rs`（`Sub2ApiDualRequest` 分支保留）；若签到等动作中有被删预设独有逻辑，一并清理。
- 前端无需改动（预设列表由后端下发），存量数据无需迁移（自动回落 `generic_api`）。

**影响**：原来选被删预设的供应商将失去专属的余额查询/签到动作，已填凭据保留。

**时机**：现在不开发，与第 1–3 条一起排期。
