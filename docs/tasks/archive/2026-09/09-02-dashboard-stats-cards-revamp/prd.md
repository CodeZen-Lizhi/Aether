# 仪表盘统计卡片改版：今日请求/费用/RPM 重排

## Goal

仪表盘顶部 4 张统计卡片改版：第 1 张改为今日请求数（含成功/失败与成功率），第 2 张今日 Token 不变，第 3 张只展示费用，第 4 张为全站 RPM/TPM；移除用户数卡片及后端无用查询，并同步更新前后端测试。

## Background

系统已完成单用户化改造，仪表盘第 4 张卡片「在线 / 启用用户」在单用户场景下没有意义。用户希望把这张卡片换成今日接口访问次数，并把原来第 1 张「今日请求 / 费用」拆开：费用单独一张卡，请求数单独一张卡并补充成功/失败明细。

## Requirements

### 1. 卡片内容与顺序（后端 `/api/dashboard/stats`）

新顺序与内容为：

1. **今日请求**：主值 = 今日请求总数；副行（subValue）= `成功 X / 失败 Y`（成功 = 请求数 - 错误数，失败 = 错误数）；徽标（change）= `成功率 Z%`（请求为 0 时成功率显示 0.0%）。
2. **今日 Token**：保持现有主值与副行不变。
3. **今日费用**：主值 = 今日费用（USD 格式）；副行仅在有节省（cost savings > 0）时显示 `节省 $X`，无节省时不显示副行。
4. **全站 RPM / TPM**：保持现有内容不变。

### 2. 移除用户数统计（后端）

- 删除「在线 / 启用用户」卡片的组装逻辑。
- 删除随之无用的查询与常量：`dashboard_load_user_counts`、`dashboard_load_online_user_count`、`DASHBOARD_ONLINE_USER_WINDOW_SECS`、`DASHBOARD_ONLINE_USER_AGGREGATION_LIMIT`。
- 删除响应 payload 中的 `users` 字段（前端已无消费方）。
- 不动 `summarize_export_users` 状态方法本身（admin 监控端点仍在使用）；不动 `api_keys`、`tokens`、`system_health`、`cost_stats` 等其他 payload 字段。

### 3. 前端同步（`frontend/src/views/shared/Dashboard.vue`）

- 卡片模板已支持 subValue 与徽标，不需要改模板结构。
- 空状态占位卡片（`emptyStatPlaceholders`）文案与新卡片保持一致：今日请求 / 今日 Tokens / 今日费用 / 全站 RPM。
- 移除已废弃的 `activeUsers` ref 及对 `statsData.users` 的赋值。

### 4. 测试同步

- 更新 `apps/aether-gateway/src/tests/frontdoor/public_support/dashboard.rs` 中对卡片名、value/subValue 格式以及 `payload["users"]` 的断言，覆盖新卡片顺序与新内容。

## Constraints

- 卡片文案保持中文，与现有实现一致（后端直接输出中文卡片名，无 i18n 改造）。
- 不改变 `/api/dashboard/stats` 的其余行为（缓存策略、鉴权、today/api_keys/tokens/system_health/cost_stats/cache_stats/token_breakdown 字段均保持不变）。

## Acceptance Criteria

- [ ] `/api/dashboard/stats` 返回的 4 张卡片按新顺序排列：今日请求、今日 Token、今日费用、全站 RPM / TPM。
- [ ] 「今日请求」卡片包含请求总数、成功数、失败数与成功率；请求为 0 时成功率显示 0.0%。
- [ ] 「今日费用」卡片只展示费用，节省 > 0 时副行显示节省金额，节省为 0 时无副行。
- [ ] 响应 payload 不再包含 `users` 字段；后端不再查询在线用户/总用户数。
- [ ] `apps/aether-gateway` 相关测试（frontdoor public_support dashboard）全部通过。
- [ ] 前端构建/检查通过（lint + build 或项目等效命令），空状态占位文案与新卡片一致。

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` for technical design and `implement.md` for execution planning before `task.py start`.
