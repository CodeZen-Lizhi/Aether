# 实施清单：供应商密钥倍率统一为 Key 级默认倍率

按依赖顺序执行（后端语义先行，避免 UI 统一后计费仍旧语义）。

## 1. 后端 billing

- [ ] `crates/aether-billing/src/pricing.rs`：`rate_multiplier_for_api_format` 忽略 `provider_api_key_rate_multipliers`，只读 `provider_api_key_default_rate_multiplier`；缺失/负数/NaN 回落 1.0。
- [ ] 改写 pricing.rs 单测：`rate_multiplier_format_mapping_overrides_key_default` → 「格式映射存在也不影响结果」；补一条 `default = 0.15` 生效用例。
- [ ] `crates/aether-billing/src/service.rs`：依赖覆盖优先的测试（`rate_multipliers: {"openai:chat": 0.5}` 断言 0.5）改写为默认倍率语义。

## 2. 调度排序

- [ ] 从 `crates/aether-scheduler-core/src/ranking/types.rs` 候选字段注释出发，grep 定位 `rate_multiplier` 填充点（全仓 `rg -n "rate_multiplier" crates --type rust`），改为走 Key 默认倍率，注释与测试同步。

## 3. 前端弹窗 KeyFormDialog.vue

- [ ] 删除「按 API 格式覆盖倍率」块（模板 ~180-202 行）与 `setFormatMultiplier`、提交前 filteredMultipliers 组装逻辑。
- [ ] 更新 payload：`rate_multipliers: null`（显式清空存量）；新增 payload：不再携带该字段。
- [ ] 预取逻辑去掉 `editingKey.rate_multipliers`；helper 文案改写为不依赖格式覆盖的表述（如「该密钥所有请求按此倍率计费，1 表示不调整」）。

## 4. 前端列表 ProviderDetailDrawer.vue

- [ ] `getKeyRateMultiplier` 改为返回 `key.default_rate_multiplier ?? 1`（format 维度可移除）。
- [ ] 行内编辑提交 `updateProviderKey(keyId, { default_rate_multiplier })`，校验沿用 0.01–100 与浮点容差比较逻辑。
- [ ] tooltip/标题文案同步（「Key 默认成本倍率：未单独配置倍率的格式按此计费」→ 按 Key 维度表述）。

## 5. 全仓文案与其他引用收敛

- [ ] `rg -n "倍率" frontend/src --glob '!**/__tests__/**'` 核对 ProviderAuthDialog.vue 帮助文案、keys.ts 注释等残留的「按格式设置倍率」表述，逐一改写。
- [ ] 按 frontend i18n spec：grep `frontend/src/i18n/messages.ts` 确认被删文案无引用后清理条目（如「按 API 格式覆盖倍率」「按默认」）。

## 6. 验证

- [ ] 前端：`frontend/package.json` 中的 typecheck / lint / vitest（重点 providers 相关 spec，含引用被删 UI 的用例改写或删除）。
- [ ] 后端：`cargo test -p aether-billing`；受影响的其他 crate 测试（排序填充点所在 crate）。
- [ ] 手工路径核对（代码层面）：新建 Key → 弹窗设默认倍率 → 列表展示一致；列表行内编辑 → 弹窗回显一致。

## 回滚点

- 每个大步骤后可独立 revert；最终单次提交。
