# 供应商密钥倍率统一为 Key 级默认倍率

## Goal

密钥的成本倍率按「密钥」一个维度管理：密钥管理列表的展示、列表行内编辑、密钥编辑/新建弹窗三处共用同一个字段 `default_rate_multiplier`；后端计费只按 Key 级默认倍率计算，按 API 格式的覆盖映射（`rate_multipliers`）不再参与计费与排序。

## 背景与现状（问题定位）

一个 Key 目前有两个倍率字段：

- `default_rate_multiplier`：Key 级默认成本倍率。编辑弹窗「默认成本倍率」读它（KeyFormDialog.vue 预取 `editingKey.default_rate_multiplier ?? 1`）。
- `rate_multipliers`：`Record<api_format, number>` 按 API 格式覆盖。

关键调用点：

- 列表展示：ProviderDetailDrawer.vue `getKeyRateMultiplier`，取 `rate_multipliers[format] ?? default_rate_multiplier ?? 1`。
- 列表行内编辑：ProviderDetailDrawer.vue `startEditMultiplier` → `updateProviderKey(keyId, { rate_multipliers })`，只写格式覆盖映射。
- 弹窗：KeyFormDialog.vue「默认成本倍率」+「按 API 格式覆盖倍率」块（每个已选格式一个小输入框）。
- 计费：crates/aether-billing/src/pricing.rs `rate_multiplier_for_api_format`，优先级为 格式覆盖 > Key 默认 > 1.0。
- 成本排序：aether-scheduler-core/src/ranking/types.rs 候选 `rate_multiplier` 注释表明取自 Key 的 `rate_multipliers` 映射（填充点需顺藤摸瓜）。
- 前端更新语义：update payload 中 `null` 表示清空、`undefined` 表示保持原值（KeyFormDialog.vue 注释与 allowed_models 先例）。

用户实际发生的现象：在列表把某格式倍率改成 0.15（写进了 `rate_multipliers`），打开编辑弹窗「默认成本倍率」仍显示 1 —— 两处展示/修改的不是同一字段，但计费实际走了 0.15。

## Requirements

- R1 KeyFormDialog.vue：删除「按 API 格式覆盖倍率」整块 UI 及 `setFormatMultiplier` 等相关逻辑；提交 payload 不再携带格式覆盖映射，更新时显式 `rate_multipliers: null` 清掉存量覆盖值；「默认成本倍率」helper 文案（「未单独配置倍率的请求按默认倍率计费…」）改为不再以格式覆盖为前提的表述。
- R2 ProviderDetailDrawer.vue：每个 API 格式后展示的倍率改为 Key 级默认倍率（`default_rate_multiplier ?? 1`）；行内编辑提交 `default_rate_multiplier`（与弹窗同一字段）；相关 tooltip / 标题文案同步。
- R3 billing：`rate_multiplier_for_api_format` 忽略格式覆盖映射，直接返回 Key 级默认倍率；无效值（负数/NaN/缺失）回落 1.0 的现有语义保留；pricing.rs / service.rs 相关单测同步改写。
- R4 存量数据：不做迁移脚本。计费不再读取 `rate_multipliers`（R3 生效后存量值天然失效），弹窗下次保存时显式清空（R1）。用户需在「默认成本倍率」重新设置一次目标值（如 0.15），属预期行为。
- R5 成本排序：填充候选 `rate_multiplier` 的调用点同步改为取 Key 级默认倍率，相关注释与单测同步。
- R6 i18n（按 .trellis/spec/frontend/index.md 规范）：删除/改写文案后 grep `frontend/src/i18n/messages.ts`，清理无引用条目（如「按 API 格式覆盖倍率」「按默认」等），保留仍被引用条目。

## Acceptance Criteria

- [ ] 对同一个 Key，列表展示的倍率与编辑弹窗「默认成本倍率」始终一致（同一字段 `default_rate_multiplier`）。
- [ ] 弹窗中不再有「按 API 格式覆盖倍率」块；在弹窗保存后该 Key 的 `rate_multipliers` 为空。
- [ ] 列表行内编辑倍率后，再打开编辑弹窗显示的是刚改的值（不再出现「列表 0.15、弹窗 1」的分裂）。
- [ ] 计费单测：`provider_api_key_rate_multipliers` 即使有值也不影响计费倍率；`default_rate_multiplier = 0.15` 的 Key 计费倍率为 0.15；无效默认值回落 1.0。
- [ ] 成本排序使用的倍率与 Key 默认倍率一致。
- [ ] 被删除文案的 i18n 字典条目无残留引用（grep 验证）。
- [ ] 前端 lint / typecheck / 相关单测通过；后端 aether-billing 及受影响 crate 测试通过。

## Notes

- 与进行中的 09-03-balance-cell-fail-silent 任务文件基本不重叠，但 `frontend/src/i18n/messages.ts` 双方都可能触碰，提交时注意分开暂存。
- DB schema 不动：`rate_multipliers` 列保留但降级为遗留字段，避免迁移成本。
