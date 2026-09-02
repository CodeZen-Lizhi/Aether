# 设计：供应商密钥倍率统一为 Key 级默认倍率

## 语义决策

`rate_multipliers`（按 API 格式覆盖）整体降级为遗留字段：DB 列与 API 字段保留（无迁移），但计费、排序、UI 三层全部不再读取；弹窗保存密钥时显式清空（`null`）。单一事实来源为 `default_rate_multiplier`。

## 分层改动

### 后端 billing（crates/aether-billing）

- `pricing.rs::rate_multiplier_for_api_format`：签名保留（调用方无需改），实现改为只读 `provider_api_key_default_rate_multiplier`，忽略 `provider_api_key_rate_multipliers`；无效/缺失回落 1.0 语义不变。
- 单测：`rate_multiplier_format_mapping_overrides_key_default` 改写为「格式映射存在也不影响结果」；`rate_multiplier_falls_back_to_key_default_when_format_mapping_missing`、`rate_multiplier_invalid_key_default_falls_back_to_one` 保留语义。
- `service.rs` 中依赖「覆盖优先生效」的测试（如 `rate_multipliers: {"openai:chat": 0.5}` 断言 result.rate_multiplier == 0.5）改写为走默认倍率。

### 调度排序

- 候选 `rate_multiplier` 填充点：从 ranking/types.rs 注释顺藤摸瓜（grep `rate_multiplier_for_api_format` / `rate_multipliers` 在 gateway / dispatch / provider 各 crate 的引用），改为等价走 Key 默认倍率；注释同步。

### 前端

- `api/endpoints/keys.ts`：类型保留（`rate_multipliers` 仍可传 `null`），注释标注遗留字段。
- `KeyFormDialog.vue`：
  - 删除「按 API 格式覆盖倍率」块（模板 180-202 行附近）与 `setFormatMultiplier`、提交前的 filteredMultipliers 逻辑。
  - 更新 payload：`rate_multipliers: null`（清空存量）；新增 payload：不再携带该字段（或 `null`，与后端 create 语义对齐即可）。
  - 预取不再读 `editingKey.rate_multipliers`；helper 文案改写（如「该密钥所有请求按此倍率计费，1 表示不调整」）。
- `ProviderDetailDrawer.vue`：
  - `getKeyRateMultiplier(key, _format)` → 返回 `key.default_rate_multiplier ?? 1`（或直接去掉 format 维度）。
  - 行内编辑提交：`updateProviderKey(keyId, { default_rate_multiplier: value })`；校验范围沿用 0.01–100。
  - 文案：「Key 默认成本倍率：未单独配置倍率的格式按此计费」→「Key 成本倍率：该密钥所有请求按此计费」。
- `ProviderAuthDialog.vue` 及其他引用「按格式设置倍率」帮助文案的地方：grep `倍率` 逐一核对改写。

### i18n

- 按 frontend spec 流程：先 grep 确认引用归零再删字典条目（「按 API 格式覆盖倍率」「按默认」等）；messages.ts 可能与另一进行中任务冲突，只动本任务相关行。

## 兼容性 / 回滚

- 更新接口语义不变（`null`=清空、`undefined`=保留）；API 继续接受 `rate_multipliers` 字段但后端不再消费，旧客户端传值无副作用。
- 回滚 = revert 提交即可，无 schema 变更、无数据迁移；已清空的 `rate_multipliers` 数据不恢复（可接受，字段已废弃）。

## 风险

- 排序填充点若漏改，成本排序仍可能读到旧映射 —— 用 grep 全仓 `rate_multipliers` 收敛确认。
- messages.ts 与并行任务冲突 —— 提交时按文件/行分开暂存。
