# 供应商列表余额列失败态静默为横杠

## Goal

供应商列表的余额监控列是汇总视图：任何失败（余额查询报错、签到失败、签到 Cookie 失效）一律显示横杠 `-`，不在列表里暴露报错文字。具体报错信息只在「测试」动作和用户认证「验证」弹窗中展示（这两个入口已经能显示完整报错，无需改动）。

## 背景与现状（问题定位）

现状渲染逻辑在 `frontend/src/features/providers/components/ProviderBalanceCell.vue`（唯一渲染点，由 `ProviderTableRow.vue` / `ProviderManagement.vue` 透传 props）：

1. 批量查询返回非 success/pending 状态时（如 `连接失败: xxx`、`认证失败`），第 91-97 行把原始 message 红字全文打在列表里（`useProviderBalance.ts` 的 `getProviderBalanceError` 特意保留了全部状态，注释见该文件 76 行）。
2. 余额查询成功但 `extra.checkin_success === false` 时，第 74-88 行在余额下挂红字「签到失败」（后端在查余额时顺带做了签到探测，见 `query_balance/mod.rs`）。
3. `extra.cookie_expired` 为 true 时显示「签到 Cookie 已失效」。

以上三类报错文字都违反「列表外不展示报错」的原则。

## Requirements

- R1 余额查询失败（`getProviderBalanceError` 非 null 的所有状态：auth_failed、上游连接失败、查询失败等）→ 余额列显示 `-`，不显示报错 message，也不挂 tooltip。
- R2 签到失败（`checkin_success === false`）→ 列表不显示「签到失败」字样。
- R3 签到 Cookie 失效（`cookie_expired === true`）→ 列表不显示「签到 Cookie 已失效」字样（同属报错类信息，按同一原则收掉）。
- R4 以下现状保持不变：
  - 余额查询成功时的金额显示（含余额 + 积分两行的 breakdown）。
  - `total_available` 为 null 时金额显示 `-`。
  - 签到成功（`checkin_success !== false`）时「已签到」灰字保留。
  - `monthly_quota` 计费类型的配额显示。
  - pending 状态的加载 spinner。
  - 「测试」按钮、用户认证「验证」弹窗中的完整报错展示（不在本任务改动范围，仅作为报错详情的唯一出口）。
- R5 按 `.trellis/spec/frontend/index.md` 的 i18n 规范：删除「签到失败」「签到 Cookie 已失效」文案后，grep 确认 `frontend/src/i18n/messages.ts` 中对应字典条目无其他引用，再一并清理（保留仍被引用的条目）。

## Acceptance Criteria

- [ ] 批量余额查询返回错误状态（连接失败/认证失败等）时，余额列渲染为 `-`，页面 DOM 中不出现报错文字。
- [ ] `checkin_success === false` 时列表不出现「签到失败」。
- [ ] `cookie_expired === true` 时列表不出现「签到 Cookie 已失效」。
- [ ] 签到成功时「已签到」仍正常显示；余额成功、积分 breakdown、monthly_quota、loading spinner 展示均不回归。
- [ ] `grep -rn "签到失败\|签到 Cookie 已失效" frontend/src` 中，已删除文案的 i18n 字典条目无残留引用。
- [ ] 前端 lint / typecheck / 相关单测通过。

## Notes

- 改动预计集中在 `ProviderBalanceCell.vue`（模板分支收敛）与 `useProviderBalance.ts`（如需调整 getter 语义），i18n 字典做配套清理。
- 后端不需要改动：`checkin_success` / `cookie_expired` / 错误 message 数据照常返回，只是列表不再渲染报错文字；测试/验证弹窗继续展示完整报错。
