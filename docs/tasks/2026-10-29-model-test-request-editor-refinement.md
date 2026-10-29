# 模型测试请求内容与 Key 展示调整

日期：2026-10-29

## 目标

- 只调整模型测试弹窗左侧的“请求内容”，右侧执行与结果工作台保持不变。
- 请求头、请求体不再分别呈现为零散表单框，展开后使用统一的代码编辑表面。
- 测试 Key 只展示 Key 名称，不拼接掩码、认证类型或 `.API`；每次打开测试弹窗重新读取最新名称。

## 实现

- `ModelTestDialog.vue` 将请求头和请求体合并到统一的 `request-composer`，每个折叠行展示名称、用途、JSON 标识与展开状态。
- 展开区增加 `Headers` / `Body` 工具栏、格式化与重置操作、代码编辑表面、JSON 状态、行数和字符数；错误状态使用统一内联提示。
- `ModelsTab.vue` 每次打开模型测试强制刷新 Key 列表；Key 选项文案改为纯名称，缺名时才回退到掩码或标识。
- 抽取 `formatModelTestKeyName`，明确 Key 名称展示规则。

## 验证

- `ModelTestDialog.spec.ts`、`ModelsTab.spec.ts` 与 `model-test-request.spec.ts` 共 77 项通过，覆盖请求内容统一编辑器结构、每次打开重新读取 Key 及 Key 名称纯文本。
- `npm run type-check`、目标 ESLint、`npm run build` 通过。
- 真实组件 fixture 在 1200px 与 390px 验证请求头和请求体折叠/展开、代码编辑表面、JSON 状态及无横向溢出。
- Key 下拉实际只显示 `production-key-a`、`backup-key-b`，不包含掩码、认证类型或 `.API` 后缀。
