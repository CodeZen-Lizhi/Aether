# Design — 系统设置-网络代理 UI 优化

## 组件结构

```
ProxyConfigSection.vue (重构，仍是唯一入口)
├── CardSection #actions: 「保存默认代理」按钮 (R1)
├── 区块一：默认代理 Select + 说明文字 (R2)
├── 区块二：代理节点
│   ├── 标题行: 「代理节点」Label + 数量 + 「添加节点」按钮(右侧)
│   ├── 节点列表行: 名称(+区域) | 状态 Badge | 地址 | 「编辑」
│   └── 空状态: 提示文案 + 「添加节点」按钮 (R3)
└── ProxyNodeEditDialog.vue (新增) (R4/R5)
    └── Dialog: 表单 + 内联测试结果 + 底部按钮组
```

删除 `nodes.length > 1` 才出现的「选择要管理的节点」下拉与内联表单按钮行。

## 新组件契约：ProxyNodeEditDialog.vue

```ts
props: {
  open: boolean
  node: ProxyNode | null        // null = 添加模式
}
emits: {
  'update:open': [value: boolean]
  saved: []                      // 创建/更新成功（store 已刷新列表）
  deleted: [payload: {
    nodeId: string
    clearedSystemProxy: boolean
    clearedExternalModelsProxy: boolean
  }]
}
```

- 表单状态为组件内部 `ref`，`open` 变 true 时按 `node` 初始化（添加模式全空；编辑模式回填，密码用详情接口的明文，与现状 `proxyNodesApi.getNode` 一致）。
- CRUD 沿用现有通道：`store.createManualNode` / `proxyNodesApi.updateManualNode` / `proxyNodesApi.deleteProxyNode` / `proxyNodesApi.testProxyUrl`。
- `deleted` 只上报结果，副作用（清 models.dev 缓存、清 `proxyNodeId`）由父组件处理，避免弹窗了解父级状态。

## ProxyConfigSection 变化

- 头部按钮：文案「保存默认代理」，`:title="hasChanges ? undefined : '暂无改动'"`，逻辑不变。
- 节点列表数据：`store.nodes`（`onMounted ensureLoaded` 保持）。地址复用现规则 `node.tunnel_mode ? node.ip : \`${node.ip}:${node.port}\``。
- 事件处理：
  - `saved` → 依赖 store 内部 `fetchNodes` 刷新，toast 由弹窗内完成（现状一致）。
  - `deleted` → `if (payload.clearedExternalModelsProxy) clearModelsDevCache()`；`if (payload.clearedSystemProxy || props.proxyNodeId === payload.nodeId) emit('update:proxyNodeId', null)`。
- 默认代理 Select 及 `selectableNodes`（离线/排空节点过滤、当前选中回插）逻辑原样保留。

## 关键决策与取舍

1. **弹窗 vs 内联展开**：列表 + 弹窗解决「无总览」「下拉布局跳动」「删除误触」三个问题，且与项目既有 Dialog 模式一致（`ConfigImportDialog.vue` 等）。代价是多一次组件拆分，可接受。
2. **删除按钮位置**：仅放弹窗 footer 左侧（红色 outline），配合现有 `confirmDanger`；列表行不放删除，降低误触面。
3. **代理地址校验**：前端前缀校验 `/^(https?|socks5h?):\/\//i`（后端 `apps/aether-gateway/src/state/proxy.rs:232-236` 用 `url::Url::parse` + scheme `socks` 前缀判断，口径一致）；校验失败禁用保存 + 字段下方内联错误。不做更深的 URL 语法校验（避免与后端口径漂移）。
4. **密码语义**：编辑态 placeholder「留空保持不变」，提交 payload `password: form.password || undefined`（现状行为，后端空=不改）。删除原说明文案。
5. **测试结果状态**：`ref<{ kind: 'success' | 'error'; text: string } | null>`，存于弹窗内、随弹窗生命周期，不持久化。

## i18n（R7）

`frontend/src/i18n/messages.ts` 的英文映射字典补充新增文案，如：
`保存默认代理`、`暂无改动`、`添加节点`、`编辑节点`、`暂无代理节点`、`代理地址必须以 http://、https:// 或 socks5:// 开头`、`留空保持不变`、`测试通过：延迟 {x}ms · 出口 IP {ip}` 等（按实际文案为准）。

## 兼容性 / 回滚

- 纯前端展示层重构，不改 API 与数据结构；`saveProxyConfig`（默认代理保存）与 `hasChanges` 计算不动。
- 回滚 = revert 前端两个组件 + i18n 字典条目，无数据迁移。

## 可访问性

- 列表行「编辑」用真实 `<Button>`；Badge 沿用现有组件。
- Dialog 组件自带焦点管理；表单 Label 与 Input 保持现有写法（Label 包裹或 for 关联与现状一致，不额外扩大范围）。
