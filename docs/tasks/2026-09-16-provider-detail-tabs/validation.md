# 供应商详情 Tab 内容隔离

状态：修复完成；本地真实组件回归通过。用户已授权仅提交并推送本次修复。

## 范围与流程

采用 delivery 的「一般功能或行为修复」分支，由主 agent 完成一个交付单元：定位与复现 → 聚焦修复 → 自检与浏览器验证 → 记录交付。没有拆分代理，没有调用 Trellis。

验收：首次打开默认密钥管理；三个分区内容互斥；公共头部保留；配额只在密钥管理；同供应商切换保留分页与展开；切换供应商、关闭重开清理旧数据和临时状态；代表性管理操作及重新读取可用。

开始时业务源码没有已有修改。本次仅修改 `frontend/src/features/providers/components/ProviderDetailDrawer.vue`，新增浏览器 fixture、回归脚本和本记录。原有 Trellis 卸载、AGENTS/spec/tasks/worklog 迁移、`docs/desktop-macos.md` 和既有 output 内容均保留。

## 实际读取的规范

共享根目录：`/Users/zhenglizhi/我的云端硬盘 (zhenglizhi1128@gmail.com)/selfinfor/engineering-handbook/`。

- `index.md`
- `standards/common/comments.md`
- `standards/common/naming-and-readability.md`
- `standards/common/reuse-and-design.md`
- `standards/common/verification.md`
- `standards/common/contracts-and-safety.md`（切换供应商的请求生命周期）
- `standards/frontend/typescript-react.md`（通用前端条目，未使用 React 专属条目）

项目根目录：`/Users/zhenglizhi/otherProjects/Aether/`。

- 会话提供的 `AGENTS.md` 全局与项目约定。
- `docs/specs/index.md`
- `docs/specs/frontend/index.md`
- `docs/specs/frontend/quality-guidelines.md`

实际使用技能入口：`/Users/zhenglizhi/.codex/skills/delivery/SKILL.md`；`/Users/zhenglizhi/.agents/skills/` 下的 `diagnosing-bugs/SKILL.md`、`ui-ux-pro-max/SKILL.md`、`find-docs/SKILL.md`、`playwright/SKILL.md`、`code-review-and-quality/SKILL.md`。诊断按局部缺陷简化，使用可失败的真实组件浏览器检查，不为明确根因机械增加多组假设或日志。UI 技能只用于局部导航检查，没有重新设计页面。

## 根因与修复

1. `ModelsTab`、`ModelMappingTab` 同时包含列表卡片和弹窗，是多根节点组件；父级直接使用 `v-show` 时 Vue 忽略指令。浏览器修复前首屏同时显示「订阅配额、密钥管理、模型列表、模型映射」，控制台输出 `Runtime directive used on component with non-element root node`。Context7 检索的 [Vue 官方说明](https://vuejs.org/guide/reusability/custom-directives#usage-on-components) 与实际运行一致。
2. 使用父级原生容器控制模型、映射的显隐，首次访问才挂载、访问后保持挂载，保留 provider key、事件和弹窗引用。仅改为隐藏挂载时，浏览器复验发现智能分页无法测量首次隐藏的高度；改为按需挂载后模型分页与跨 Tab 页码保留均通过。父级使用 flex gap，隐藏分区不再留下额外间距。订阅配额限定在密钥管理；余额仍属原供应商汇总列表，未增加重复展示。
3. 可控延迟复现出「新供应商头部下仍显示旧密钥」。复用关闭时的状态清理逻辑，在切换供应商时清空实体数据并使旧请求失效；补齐模型编辑、故障转移弹窗和临时编辑状态清理。无列表快照的详情请求完成后核对当前供应商及打开状态。

## 验证证据

浏览器使用真实 `ProviderDetailDrawer`、ModelsTab、ModelMappingTab 和编辑弹窗，Vue/Vite 实际编译。fixture 中所有 API 读写被拦截，合成 alpha/beta 数据仅在内存中保存，没有调用真实管理写入或上游。

可重复执行：

```sh
cd frontend
npm run dev -- --host 127.0.0.1
```

另一个终端中执行：

```sh
cd /Users/zhenglizhi/otherProjects/Aether
/Users/zhenglizhi/.agents/skills/playwright/scripts/playwright_cli.sh -s=provider-tabs open http://127.0.0.1:5173/scripts/fixtures/provider-detail-tabs.html
/Users/zhenglizhi/.agents/skills/playwright/scripts/playwright_cli.sh -s=provider-tabs run-code --filename frontend/scripts/check-provider-detail-tabs.cjs
```

实际结果：

- 修复前首屏断言失败，信息为 `密钥管理混入模型分区`；慢加载断言修复前失败，信息为 `加载新供应商时仍显示旧供应商数据`。
- 修复后首屏只有「订阅配额、密钥管理」；三个 Tab 来回切换内容互斥，公共供应商头部保持可见。
- 密钥第二页、模型第二页和映射展开跨 Tab 保留。切换到按量计费供应商时回到密钥管理，不显示订阅配额；加载期间没有旧供应商内容。
- beta 请求暂停时返回 alpha，再释放 beta 响应，三个分区均未被旧数据覆盖。
- 关闭重开回到密钥首页；真实密钥启停、模型启停、编辑映射新增名称并保存，重新打开和重新读取后结果仍一致。
- 840、1024、1280、1920、390 px 宽度下逐个切换三个 Tab，抽屉均满足 `scrollWidth <= clientWidth`。浏览器检查没有页面异常或多根指令警告。
- 补验 `?snapshot=0`：无列表快照首次打开、详情加载中关闭重开、旧详情响应返回后仍保持当前供应商，均通过。
- 截图：`output/playwright/provider-detail-tabs/keys.png`、`models.png`、`mapping.png`，真实组件、合成数据，1280×900。
- `cd frontend && npm run test:run -- src/features/providers/components/__tests__/ProviderDetailDrawer.loading.spec.ts`：5/5 通过。
- `cd frontend && ./node_modules/.bin/eslint scripts/check-provider-detail-tabs.cjs`：通过。
- `git diff --check -- frontend/src/features/providers/components/ProviderDetailDrawer.vue`：通过。
- 对修改组件执行定向 ESLint：8 个未使用声明错误；用 `git show HEAD:frontend/src/features/providers/components/ProviderDetailDrawer.vue | ./node_modules/.bin/eslint --stdin --stdin-filename src/features/providers/components/ProviderDetailDrawer.vue` 确认 HEAD 同样存在这 8 项。本次未引入新的 lint 发现，未扩张到历史死代码清理。

## 审查与边界

按 code-review-and-quality 主 agent 自检完整修复差异、子组件多根结构、头部、分页、映射弹窗引用、事件回调与实际调用方。确认原业务操作连接未变；显隐由原生容器承担；同供应商切换不卸载；跨供应商复用既有清理路径；新增函数有准确中文说明；无新增依赖、API/数据库或权限契约变化。没有待修复的本次新增问题。

未验证：真实后端持久化、真实供应商模型测试/余额查询、原生桌面 WebView；未逐项执行所有新增/删除/权限/代理管理操作。上述接口和事件保持原有实现，浏览器管理验证是隔离 API 的代表性调用链，不能替代真实联调。未跑全仓测试、全量构建或全前端类型检查，模板编译与本次行为由定向测试和浏览器覆盖；组件既有 8 项 lint 问题仍在。
