# 技术设计

## 边界与职责

| 交付项 | 所有者 | 复用边界 |
| --- | --- | --- |
| 桌面应用折叠 | `frontend/src/desktop/DesktopSettings.vue` | 现有桌面 IPC emit 和端口校验不变 |
| 版本信息 | `useSystemConfig.ts` | `desktopApi.status()` 的受限 Tauri IPC；Web 回退 `adminApi.getSystemVersion()` |
| 代理测试 | `ProxyConfigSection.vue`、`api/proxy-nodes.ts` | 既有 `POST /api/admin/proxy-nodes/:id/test`，不修改服务端 |
| 弹窗尺寸 | `components/ui/dialog/Dialog.vue`、`GlobalModelFormDialog.vue` | 所有使用共享 `Dialog` 的组件自动继承 |

## 交互与数据流

### 桌面应用折叠

正常模式将外层桌面设置面板改为语义化、可访问的展开容器：标题行承担开关，初始展开；详情区包含现有表单、目录和诊断日志。恢复模式保持原先直接展示的结构，确保网关故障时无需额外点击即可修复。

### 版本信息

```
桌面会话 -> desktopApi.status() -> Status.version (桌面发行版本)
     失败或非桌面 -> adminApi.getSystemVersion() -> 网关版本
```

版本读取保持异步和非阻塞。只在 `hasDesktopSession()` 为真时调用受限 IPC；因此 Web/Docker 仍使用当前 HTTP API，且不引入桌面能力。

### 代理节点测试

```
节点行「测试」 -> proxyNodesApi.testProxyNode(id)
                 -> POST /api/admin/proxy-nodes/:id/test
                 -> success / latency_ms / exit_ip / error
                 -> 单节点 loading + toast 反馈
```

列表测试不先读取节点详情，后端使用已存储凭据。编辑弹窗继续调用 `testProxyUrl`，以便用户在保存前测试新地址；两处共享成功结果的格式化规则，避免中英文或字段缺省时出现不同含义。

### 弹窗尺寸

共享 `Dialog` 引入按尺寸档计算的 CSS 自定义最大宽度，并使用 `min(尺寸档, 可视宽度减安全边距)` 作为桌面端上限；高度同样以 `min(合理上限, 可视高度减上下边距)` 约束。既有 flex 结构保持 header/footer 不收缩，只有 body 滚动。移动端现有底部弹层布局不变。

模型创建/编辑将选用收敛后的尺寸档，避免编辑模式保留当前 `4xl` 宽度。高密度/宽表类弹窗仍通过自身尺寸档表达相对优先级，但不会越过当前窗口安全边界。

## 兼容性与风险

- Tauri 状态 IPC 的版本字段已受前端解码校验；无须新增 native command 或扩大 capability。
- 代理测试端点已在权限和控制路由中登记；仅补前端 API 方法和调用，避免改变探针/超时策略。
- 宽度收敛可能让复杂弹窗更早进入纵向滚动；以 840×620、1024×768 和常规宽度验证保存、取消、关闭与关键字段可达。
- 新增/修改中文测试文案须确认 `messages.ts` 的精确和动态映射；当前主要格式已存在，仅补实际新增且未覆盖的分支。

## 回滚

所有改动局限于前端展示和既有 API 调用。回滚时恢复相关 Vue/TypeScript 文件即可；无数据库迁移、版本清单变更或服务端契约变更。
