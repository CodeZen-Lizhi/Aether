# 需求清单四项落地（本任务承担第 1/2/3 项）

## Goal

落地 `docs/requirements-backlog.md` 中已确认的需求。2026-09-02 与用户确认分工：

- **第 4 项（中转站架构预设收敛）由并行会话（Codex/@aether-tunnel）实施**，本任务不做、只做最终交叉验证。
- **本任务承担第 1、2、3 项**的开发，每项独立提交。
- 并行会话正在编辑的文件（`crates/aether-admin/src/provider/ops/**`、`apps/aether-gateway/**/ops/**`、`frontend/src/views/admin/RoutingProfiles.vue`、`frontend/src/features/providers/components/ProviderAuthDialog.vue`）本任务**一律不改**；提交时只 stage 本任务自己的文件，禁用 `git add -A` / `git commit -a`。

## Requirements

### 第 1 项：供应商 Key 级默认倍率 + 配置入口优化

口径（清单已确认，2026-09-01）：

- 新增 **Key 级默认倍率** 字段，默认值固定为 1；结算时未命中格式级映射则按默认倍率计算；格式级映射保留且优先于默认值。
- 计费公式不变：`最终费用 = 用量 × 模型价格（公式结果）× Key 倍率`；对 token 计价与 price_per_request 两类模型通用；`free_tier` 仍固定计 0；不回溯历史账单。
- 倍率配置入口优化：Key 新建/编辑表单（KeyFormDialog）提供 **默认倍率** 与 **按 API 格式倍率** 的显式输入控件（现仅内联 `1x` 小字，过于隐蔽）；内联快捷编辑保留。
- 展示：Key 行上未配置格式映射的格式，其倍率显示回落为 Key 默认倍率。

### 第 2 项：删除全局模型正则映射（保留供应商级映射）

口径（清单已确认）：

- 全局模型 `config.model_mappings`（正则数组）在**调度/路由链路**的消费全部删除：入站别名认领、行支持判定（row_supports）、Key 白名单放宽回退。
- candidate selection 读侧停止解析并删除行结构字段；存量数据不迁移（config JSON 原样保留在库中）。
- 前端删除全局模型详情的"模型映射"Tab（ModelMappingsTab）及其挂载；RoutingTab 中依赖正则映射的展示一并清理；`getGlobalModelRoutingPreview` 接口本身保留。
- "一键关联供应商"入口随 Tab 消失（模型管理页已有关联能力）。
- **明确保留**：供应商级映射整条链路（`model_provider_model_mappings` / ModelMappingDialog / provider-tabs）；`aether-model-fetch` 的 `association_sync`（模型抓取自动关联仍消费存量 patterns，属清单外保留项，`matches_model_mapping` 函数因此保留）。

### 第 3 项：模型管理列表批量操作重构（复选框 + 全选，去面板化）

口径（清单已确认）：

- 删除右上角"快速筛选与批量操作"按钮及整个面板（batchManageDialog 一套）。
- 模型列表每行前加复选框，表头加全选（三态：全选/半选/未选；全选范围为当前筛选/搜索结果）。
- 选中 >0 时列表上方浮出批量操作栏，两个动作：
  1. **同步在线价格**：沿用 `buildGlobalModelPriceSyncPlan`，按 REMEMBERED 来源执行（原面板默认值），跳过规则不变（价格一致/计价不兼容/无在线价格），完成提示统计（可更新 N · 已一致 N · 不兼容 N · 无在线价格 N）。
  2. **批量删除**：需二次确认。
- "价格是否一致"列表字段**不做**（需后端定时落库，另行立项）。

### 交叉验证（第 4 项，由并行会话实施）

- 待并行会话完成后：`cargo` 编译与测试、前端 type-check/test/build 全绿。
- 若第 4 项改动面存在编译错误或测试缺口，只在该改动面内修复。

## Acceptance Criteria

- [ ] Key 表单可配置默认倍率与按格式倍率；未配置格式的请求按 Key 默认倍率结算（结算测试覆盖：格式映射优先、默认回退、free_tier 仍为 0）
- [ ] Key 行倍率展示回落逻辑正确（未配置格式显示默认倍率）
- [ ] 全局正则映射在调度链路零消费（别名请求不再匹配、白名单不再放宽），相关调度测试同步更新
- [ ] 全局模型详情无"模型映射"Tab；`model-mapping-regex.ts` 视残留引用处理
- [ ] 模型管理：复选框/三态全选/批量操作栏（同步价格 + 批量删除二次确认）可用，面板删除
- [ ] `cargo test --workspace` 全绿；`frontend` type-check / test:run / build 通过
- [ ] 第 4 项并行改动交叉验证通过（或在其完成前记录为待办）
- [ ] 每 item 独立提交且不含并行会话文件

## Notes

- 需求口径唯一来源：`docs/requirements-backlog.md`（4 条结论均为 2026-09-01 用户确认）。
- 本任务在 `slim-personal` 分支开发，与并行会话共享工作区，git 操作必须精确 stage。
