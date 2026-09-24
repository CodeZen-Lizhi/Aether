# Implement — 系统设置-网络代理 UI 优化

## 执行顺序

### Step 1: 新增 ProxyNodeEditDialog.vue
- [ ] 新建 `frontend/src/views/admin/system-settings/ProxyNodeEditDialog.vue`，按 design.md 契约实现 props/emits。
- [ ] 表单：名称*、代理地址*（前缀校验 `/^(https?|socks5h?):\/\//i` + 内联错误）、用户名、密码（编辑态 placeholder「留空保持不变」）、区域。
- [ ] 底部按钮：左「删除」（仅编辑态 + confirmDanger）、右「测试连接」+「保存」。
- [ ] 内联测试结果展示（成功：延迟/出口 IP；失败：错误信息），覆盖式更新。
- [ ] CRUD：`store.createManualNode` / `proxyNodesApi.updateManualNode` / `deleteProxyNode` / `testProxyUrl`；`toRequestPayload` 密码空 → undefined。
- [ ] 成功/失败 toast 与现状文案一致（「代理节点已添加」「代理节点已更新」「添加失败」「更新失败」「删除失败」）。

### Step 2: 重构 ProxyConfigSection.vue
- [ ] 头部按钮文案 →「保存默认代理」+ 禁用 title「暂无改动」。
- [ ] 内容区改为：默认代理 Select（逻辑保留）→ 分隔 → 代理节点区块。
- [ ] 节点列表渲染（名称/区域、状态 Badge、地址、编辑按钮）+ 空状态。
- [ ] 「添加节点」入口（标题行右侧 + 空状态）；打开弹窗（node=null / 对应节点）。
- [ ] 处理 `saved` / `deleted` 事件；`deleted` 副作用按 design.md（clearModelsDevCache、update:proxyNodeId）。
- [ ] 移除：内联节点表单、「选择要管理的节点」下拉、表单按钮行、`applyManagedNode`/`managedNodeId` 等相关状态。

### Step 3: i18n 字典
- [ ] `frontend/src/i18n/messages.ts` 英文映射补充新增文案；清理不再使用的条目（如「选择要管理的节点」相关，grep 确认无其他引用后删）。

### Step 4: 测试
- [ ] 新增 `frontend/src/views/admin/system-settings/__tests__/ProxyNodeEditDialog.spec.ts`：
  - 添加/编辑模式初始化回填；密码留空 → payload 无 password 字段。
  - 代理地址非法前缀 → 保存禁用 + 错误提示；合法 → 可保存。
  - 测试连接成功/失败 → 内联结果渲染。
  - 删除确认取消 → 不调用 deleteProxyNode；确认 → 触发 deleted 事件。
- [ ] 新增/更新 `ProxyConfigSection.spec.ts`（参考 `SiteInfoSection.spec.ts` 写法）：
  - 列表渲染节点行；空状态；头部按钮文案与禁用 title；`deleted` 副作用联动。

### Step 5: 全量校验（最后一轮 2.2）
- [ ] `pnpm --dir frontend type-check`
- [ ] `pnpm --dir frontend lint`
- [ ] `pnpm --dir frontend test:run`
- [ ] `pnpm --dir frontend build`
- [ ] 手动冒烟：dev 起前端，走查 AC1–AC8（默认代理保存、列表、弹窗 CRUD、校验、测试结果、删除联动、空状态、英文翻译）。

## 审查门 / 回滚点

- Step 2 完成后为功能回滚点：此时弹窗与列表已接通，若方向有误可整块 revert。
- 每步保持可独立提交的绿色状态（type-check + 相关 spec 通过）。

## 风险

- `Dialog` 组件的 `open` 双向绑定与表单初始化时序：用 `watch(() => props.open)` 在打开时重置表单，避免上一次编辑残留。
- 删除当前被选为默认代理的节点：联动逻辑已有测试覆盖点，迁移时保持 `cleared_system_proxy || proxyNodeId === node.id` 双条件。
