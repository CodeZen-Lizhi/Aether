# 实施顺序

## 阶段边界

用户于 2026-09-12 批准本方案实施，后续追加提交、push 和打包授权。代码、独立审查、定向检查、浏览器验收与 0.1.19 本机打包已完成；原生宿主界面检查未执行，具体证据与限制见 `validation.md`。

## 顺序

- [x] 重新核对 git status 和当前设置代码，保留用户改动；加载 frontend 与 desktop 相关 spec。
- [x] 在 SystemSettings 编排首页、代理管理和高级二级页，完成旧 query/hash 与前进后退映射，保留草稿和保存状态。
- [x] 拆清代理选择/管理展示、桌面普通端口行与 recovery 分支、备份操作/配置迁移展示边界；复用原调用与弹窗。
- [x] 基于 900x640 完成 settings.css 固定左右行布局，统一标题字体层级、控件、对齐和弹性留白；增加中英文文案。
- [x] 更新直接相关组件测试，聚焦入口可达、路由兼容、返回草稿、端口校验和模式隔离；复用原保存与导入测试，不新建全仓门禁。
- [x] 运行下述窄范围检查，修复本次引入问题。实现检查：6 个相关测试文件 79 项通过、type-check、定向 ESLint 和 diff --check 通过。
- [x] 开本地前端服务，使用隔离 API 状态验证真实页面流程、全部指定浏览器尺寸及动态缩放。原生宿主因正在运行的用户实例占用单实例锁而未验证，未中断用户 APP。
- [x] 按当前 Trellis 工作流派发 `trellis-check`，聚焦计划范围和复用证据，主 agent 整合；更新 frontend spec 中旧四分类约定。独立审查修复外部 query 的继承属性问题并新增 3 项回归测试。
- [x] 给出改动、截图、行为证据及未验证项；按后续授权提交、push 并打包 0.1.19，未公开发布或安装替换。原生宿主验收待后续具备独立运行条件时补充，任务不自动归档。

## 验证命令

在 frontend 工作目录执行，命令按实际涉及文件缩减：

```bash
npm run test:run -- src/views/admin/system-settings/__tests__/SystemSettings.spec.ts src/views/admin/system-settings/__tests__/ProxyConfigSection.spec.ts src/desktop/__tests__/DesktopSettings.spec.ts src/views/admin/system-settings/__tests__/useSystemConfig.spec.ts src/views/admin/system-settings/__tests__/useConfigExportImport.spec.ts
npm run type-check
npx eslint src/views/admin/SystemSettings.vue src/views/admin/system-settings src/desktop/DesktopSettings.vue src/i18n/messages.ts
```

eslint 不用全仓 `npm run lint`（带 --fix）。若有既存类型错误，分辨来源并报告。默认不跑 Rust 全量测试或打包，因为本方案不变后端及 IPC。

## 实际页面验收

1. 用 900x640 最小窗口，从代理选择进入管理、测试一个隔离节点并返回；检查草稿及当前选中节点未丢失。
2. 在隔离状态下测试端口非法值、有效值的保存/重启反馈，自启动成功和失败；刷新确认读回值一致。
3. 创建测试备份，检查结果文件非空且符合现有结构；打开恢复预览后取消，确认未触发导入提交。恢复提交回归沿用既有隔离测试，不导入真实个人数据。
4. 进入高级设置并编辑两个不同保存组，前进后退和切换后分别保存；确认另一组草稿、失败字段及隐藏批次值保持。
5. 在 900x640、1024x768、1200x820、1440x900、1920x1080 和真实显示器最大化下测量溢出、入口可见性和控件位置；连续拖动穿过旧 760px 内容断点，确认结构不切换。
6. 最小/最大窗口补明暗主题、中英文、长代理名称、节点为空和请求失败；正常中文首页首屏五项入口可达，异常和长文本允许纵向增长。
7. Web 无桌面命令；390px Web 验证无裁切和操作可达，兼容规则不影响桌面范围。

所有尺寸均为 CSS/逻辑像素。最大化尺寸取当前显示器实际值，不将 1920x1080 声称为所有设备最大尺寸。

## 回滚与交付

仅回滚本次展示、导航和文案文件；不回退用户并行改动、运行数据或既有配置。文档和示意不计为功能已通过，AC 在实现检查后逐项更新。
