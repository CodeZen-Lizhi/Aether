# 实施与验证计划

## 开始前

- [ ] 用户明确同意最新方案；维持 planning，批准前不运行 task.py start。
- [ ] 检查最新 git 状态，保留既有桌面配置/依赖修改。
- [ ] 验证 implement.jsonl / check.jsonl 引用真实规范与清单；由主 agent 激活正确的 responsive-dialog-sizing 任务，不激活已归档 dashboard-compact-layout。

## 实施顺序

1. trellis-implement 按共享 Dialog 和 ReplayDialog 建立唯一尺寸规则，保留公共接口。
2. 根据 research/dialog-inventory.md 逐项审核调用方，修复比例收紧导致的内层宽高和列布局问题。
3. 重点验证供应商新增/编辑与模型测试所有展示阶段，再覆盖图片、全局模型、批量映射、回放及确认/导入。
4. 针对直接修改的业务组件运行已有相关测试；新增测试仅用于可观察行为回归。真实尺寸使用浏览器测量，不以 jsdom 或 CSS 字符串快照证明。
5. trellis-check 复用已有证据核对完整任务范围；主 agent 汇总实际验证结果和未验证项。

## 检查命令与证据

- 清单复核：rg -n '<Dialog|<AlertDialog|<Teleport|fixed inset-0' frontend/src。
- 定向 lint：在 frontend 内运行 npx eslint <本任务改动的源文件>，不用会修复全仓的 npm run lint。
- 类型：在 frontend 内运行 npm run type-check。
- 按改动选取现有测试，例如 npm run test:run -- src/features/providers/components/__tests__/ProviderFormDialog.transfer-limits.spec.ts src/features/providers/components/__tests__/BatchModelMappingDialog.spec.ts；不要默认跑全仓测试。
- 浏览器：在可用服务上打开真实页面；如需启动前端服务，使用空闲端口。优先使用隔离数据或受控接口数据，避免触发真实付费模型请求或修改用户供应商配置。
- 每个主要尺寸保存截图和弹窗 boundingClientRect / 视口宽高 / 各内容容器 scrollWidth、clientWidth；核对留白、上限和最后一个操作按钮。
- 打开后动态缩放，检查输入、选择、结果保留；测试成功/失败视图与嵌套预览关闭返回父弹窗。
- 在 640-839px 兼容范围抽测 740px 宽；在 390×844 检查原移动端模式。在中英文及明暗主题中抽查最复杂布局，不重复全套业务测试。

## 完成边界

- 按 PRD AC1-AC6 给出证据；无法获得运行条件的项目明确记为未验证。
- 产品代码验证后再同步已确认的弹窗规范；不在本规划阶段提前写入全局开发规范。
- 无明确授权不提交、push 或发布。回滚只针对本任务实际修改，不动原有修改。
