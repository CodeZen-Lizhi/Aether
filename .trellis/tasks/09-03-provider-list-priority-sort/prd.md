# 供应商列表按调度优先级排序

## Goal

供应商管理列表当前按「启用状态 → 创建时间」排序（`frontend/src/views/admin/ProviderManagement.vue` 的 `sortProvidersByActiveAndPriority`），与调度策略页（按优先级）顺序不一致，用户困惑。改为「启用状态 → 调度优先级 → 创建时间」，使列表顺序与调度策略页对齐。**纯前端改动，后端接口现成。**

## Requirements

1. 供应商管理页加载时，并行额外请求 `listRoutingGroups()`（`frontend/src/api/routing-profiles.ts:115`），用 `findSystemDefaultRoutingGroup` 取系统默认分组，再用 `parseSchedulingStrategy(group?.config_json)` 解析出 `providerPriorities: Record<provider_id, number>`（1 起，越小越靠前）。工具函数都在 `frontend/src/features/routing/utils/schedulingStrategy.ts`，读法范例见 `frontend/src/views/admin/RoutingProfiles.vue:290-308`。
2. `displayedProviders`（`ProviderManagement.vue:454`）排序规则改为三级：
   - `is_active` 优先：启用的在前（保持现状）；
   - 同状态内按调度优先级升序；未配置优先级的供应商缀尾（fallback `Number.MAX_SAFE_INTEGER`，与调度策略页 `RoutingProfiles.vue:304` 的做法一致）；
   - 优先级相同或均未配置时按 `created_at` 升序（保持现有兜底行为）。
3. 容错：`listRoutingGroups()` 请求失败、无系统默认分组、分组无 `ui_provider_priority` 配置时，全部供应商落到 fallback，排序退化为现有行为；**不得阻塞或失败供应商列表本身的加载**。
4. `displayedProviders` 同时驱动桌面表格与移动端卡片，一处改动两边生效，不需要分别处理。

## Acceptance Criteria

- [ ] 在调度策略页配置过优先级的供应商，在供应商列表同启用状态组内按该顺序排列（P1 在 P2 前）。
- [ ] 未配置优先级的供应商排在所有已配置供应商之后，组内按创建时间升序。
- [ ] 未启用（停用）的供应商仍在所有启用供应商之后。
- [ ] routing groups 接口失败/超时/无配置时，列表正常加载且顺序退化为「启用状态 → 创建时间」。
- [ ] 不新增任何 UI 文案（避免触碰 i18n 字典）；不改动调度逻辑本身。
- [ ] `frontend` 目录 lint 与现有测试通过。

## Non-goals

- 不改调度/路由后端逻辑，不改调度策略页。
- 不做分组切换 UI——列表固定展示系统默认分组的优先级口径。
- 不在列表中展示优先级数字列（仅排序，不加新列；如后续要展示另立任务）。

## Notes

- 优先级本质是「每个路由分组一份」的 overlay 配置，列表页取系统默认分组口径，与调度策略页编辑的是同一份数据。
- 若实现时发现排序函数值得抽成纯函数（便于单测），可放 `frontend/src/features/providers/utils/` 并补最小单测；非强制。
