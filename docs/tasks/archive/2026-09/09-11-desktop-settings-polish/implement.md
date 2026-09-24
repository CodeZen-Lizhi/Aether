# 实施计划

## 执行顺序

1. 读取 `frontend` 和 `aether-desktop` 的实施规范及相关测试约定；确认工作区已有改动未与目标文件冲突。
2. 实现桌面设置标题折叠，并扩展 `DesktopSettings.spec.ts` 覆盖默认、切换、键盘/语义以及恢复模式。
3. 在系统配置版本加载路径中添加桌面发行版本优先与 HTTP 回退；扩展 `useSystemConfig.spec.ts` 覆盖桌面、回退和 Web 行为。
4. 为 `proxyNodesApi` 增加已保存节点测试方法，在 `ProxyConfigSection` 添加单节点测试状态和操作；补列表测试的调用、并发隔离和结果反馈断言。
5. 将代理测试结果格式化收敛为可复用逻辑，确认编辑弹窗的保存前测试仍复用既有 URL 接口和内联状态。
6. 在共享 `Dialog` 建立桌面宽高的窗口相对上限，在 `GlobalModelFormDialog` 采用较小编辑尺寸；新增/更新 Dialog 单测，验证尺寸变量、scroll body 和 footer 结构。
7. 运行聚焦前端单测和类型检查；启动本地前端，以 840×620、1024×768、常规桌面窗口检查模型弹窗、代理节点、桌面设置和另一处共享弹窗的无横向溢出及关键操作可达。
8. 在供应商窄窗口卡片中复用桌面余额单元的展示参数，补充余额渲染回归测试，并以 840×620 真实页面确认余额与卡片统计同时可见。
9. 使用 `trellis-check` 完成范围检查，记录未能执行的原生 Tauri 验证及其原因；仅在用户另行授权时提交。

## 验证命令

```bash
cd frontend
npm run test:run -- src/desktop/__tests__/DesktopSettings.spec.ts src/views/admin/system-settings/__tests__/useSystemConfig.spec.ts src/views/admin/system-settings/__tests__/ProxyConfigSection.spec.ts src/views/admin/system-settings/__tests__/ProxyNodeEditDialog.spec.ts
npm run type-check
```

将为共享 Dialog 增加或定位对应聚焦测试，并一并运行。界面变更完成后，在可运行条件具备时用真实浏览器验证，不以 CSS 字符串或单元测试替代。

## 风险文件与检查点

- `frontend/src/components/ui/dialog/Dialog.vue`：公共影响面最大；检查尺寸档、移动端底部布局、嵌套弹窗 z-index 和 header/footer 滚动边界。
- `frontend/src/views/admin/system-settings/composables/useSystemConfig.ts`：桌面 IPC 只能在桌面会话调用，失败必须回落 HTTP 版本。
- `frontend/src/views/admin/system-settings/ProxyConfigSection.vue`：每行测试状态必须按节点 ID 隔离，不能复用 store 的全局 loading 状态。
- `frontend/src/desktop/DesktopSettings.vue`：恢复模式不应被折叠，端口错误聚焦和诊断日志刷新仍需工作。

## Rollback Points

- 共享 Dialog 验证出现跨页面回归时，先保留窗口安全边界并将特定高密度使用方改为显式档位，不能撤回全局可滚动和可达性修复。
- 桌面 IPC 无法可靠取得版本时，回退到现有 HTTP 版本显示，不新增静态硬编码版本。
