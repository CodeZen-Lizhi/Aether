# 前端实现与验证

适用于 `frontend/`。既有视觉和业务展示规则保留在 [前端索引](index.md)；下列内容补充代码和状态验证边界。

## 请求与状态

- 管理 API 复用 [ApiClient](../../../frontend/src/api/client.ts) 及已有 `src/api` 封装，保留 token、跨标签刷新、桌面会话恢复和取消处理。新增调用不另建绕过这些行为的 HTTP 客户端。
- 配置读写以 [useSystemConfig](../../../frontend/src/views/admin/system-settings/composables/useSystemConfig.ts) 为现有范例：加载成功后才建立可编辑基线，只提交所属分组的修改；局部失败保留未保存输入，成功字段更新基线。验证保存后刷新，不以 toast 出现代替持久化成功。
- 异步结果应属于当前请求/实体，快速切换或重复操作不能让旧结果覆盖新状态。按实体隔离 pending、error 和重试状态；缓存无效化同时检查后续读取。
- 错误呈现服从具体页面契约：汇总列表与交互弹窗处理不同，参见索引。长错误用文本呈现并保留必要详情，不使用 HTML 解释上游错误，不绕过既有敏感信息保护。
- 桌面 IPC 使用 [desktop contract](../aether-desktop/backend/desktop-contract.md)，普通后台 HTTP 页面不因此获得原生命令权限。

## 最小验证

在 `frontend/` 中运行：

- `npm run test:run -- <实际测试文件路径>`：只选受影响用例。示例 `src/views/admin/system-settings/__tests__/useSystemConfig.spec.ts` 已覆盖加载基线和分组保存。
- `npm run type-check`：需要检查类型传播时使用。
- 需要只检查选中文件的 lint 时，使用项目安装的 `./node_modules/.bin/eslint <修改文件>`。`npm run lint` 实际带 `--fix` 并扫描整个前端，不把它当成只读检查。
- `npm run build` 只执行 Vite 构建；`build:with-typecheck` 才组合类型检查与构建。脚本以 [package.json](../../../frontend/package.json) 为准。

交互或用户可观察的外观变更使用能证明结果的最短有效路径。已有截图、日志、明确描述与当前代码足以定位故障时直接复用并修复，不默认启动运行实例复现；只有根因不明、证据冲突，或结果依赖真实服务、数据、持久化、特定浏览器/WKWebView 或原生行为时，才补对应入口。优先复用现有页面、预览或项目已有检查入口；无法稳定触达当前实现时才建最小 harness 或 fixture，不把所有界面问题限定为同一验证形态。视觉修复后用相同关键条件检查结果；真实交互仍按受影响路径检查成功、失败、重复操作和必要的刷新/后续读取；证据要说明实际运行环境和未覆盖边界。无法取得对应证据时明确记录阻塞原因和未验证范围。文档更新无需启动前后端或执行构建。
