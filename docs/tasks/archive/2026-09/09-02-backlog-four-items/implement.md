# Implement — 需求清单第 1/2/3 项

> 分工约束（见 prd.md）：并行会话负责第 4 项；本任务不改 `ops/**`、`RoutingProfiles.vue`、`ProviderAuthDialog.vue`；提交只精确 stage 自己的文件。每次提交前必须 `git status` 复核 staged 清单。

## Phase A — 第 2 项：删除全局模型正则映射

- [x] A1 scheduler-core：删 3 处消费（or_else 认领分支 / row_supports 子句 / 白名单放宽循环），保留 `matches_model_mapping`
- [x] A2 行结构删字段：contracts candidate_selection/types.rs + sqlite candidate_selection.rs + memory.rs + apps data/candidate_selection.rs + gate.rs fixture
- [x] A3 routing.rs 预览响应删 `global_model_mappings` 部分（保留 provider 级）
- [x] A4 scheduler-core 测试更新（删别名语义用例，保留 matches_model_mapping 单测）
- [x] A5 apps gateway scheduler/candidate 测试更新
- [x] A6 前端：删 ModelMappingsTab.vue + index.ts 导出 + ModelDetailDrawer 挂载 + RoutingTab 正则展示 + model-mapping-regex.ts 瘦身/删除
- [x] A7 验证：`cargo test -p aether-scheduler-core`（及受影响 crate）+ 前端 `npm run type-check && npm run test:run`
- [x] A8 提交：`refactor(models): 删除全局模型正则映射 — 调度零消费 + 映射 Tab/正则展示清理`

## Phase B — 第 3 项：模型管理批量操作重构

- [x] B1 删面板入口按钮 + 面板对话框模板与状态（甄别保留独立的 batchAddProviders 对话框）
- [x] B2 行复选框（表格 + 次视图）+ 表头三态全选（范围 = filteredGlobalModels）
- [x] B3 批量操作栏：同步在线价格（REMEMBERED，统计提示）+ 批量删除（二次确认）+ 取消选择
- [x] B4 验证：type-check + test:run + build
- [x] B5 提交：`refactor(models): 模型管理批量操作列表化 — 复选框/三态全选/浮动批量栏替换面板`

## Phase C — 第 1 项：Key 级默认倍率

- [x] C1 迁移 SQL + contracts/provider_catalog 读写 + sqlite key insert/update/select
- [x] C2 billing 链路：MODEL_CONTEXT_COLUMNS + StoredBillingModelContext + pricing snapshot + `rate_multiplier_for_api_format` 默认回退
- [x] C3 apps 处理器：payloads DTO + normalize + create/update/batch + export/import + 展示载荷
- [x] C4 前端：类型 + KeyFormDialog 输入控件 + ProviderDetailDrawer 回落展示
- [x] C5 测试：结算（格式优先/默认回退/free_tier）+ 迁移测试修复 + 前端 spec
- [x] C6 验证：`cargo test --workspace` + 前端三件套
- [x] C7 提交：`feat(billing): Key 级默认倍率 — 结算回退 + Key 表单倍率配置入口`

## Phase D — 交叉验证与收尾

- [x] D1 与用户确认并行会话（第 4 项）是否已完成；未完成则记录待办
- [x] D2 全仓验证：`cargo test --workspace` + `npm run type-check && npm run test:run && npm run build`
- [x] D3 第 4 项改动面遗留问题修复（仅限其改动面）
- [x] D4 spec 更新（如有沉淀）+ 日志收尾 + `task.py archive`

## 验证命令

```bash
# 后端（受影响优先）
cargo test -p aether-scheduler-core
cargo test --workspace          # Phase C 后必跑
# 前端（frontend/ 下）
npm run type-check
npm run test:run
npm run build
```
