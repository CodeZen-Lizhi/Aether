# 批量模型映射错误展示验收

## 根因与修改

- 底部保留了已移除的单条编辑入口说明，现已删除，并同步清理英文映射。
- `parseUpstreamModelError` 原先截断 80 / 100 字符、只保留分号前第一条错误、以通用提示覆盖 HTTP / 网络详情。现在友好提示和完整原文同时保留。
- 查询结果同时包含 error / warning 时原先只取一项，现在合并并去重。
- 批量保存原先只显示第一个失败，现在逐个列出客户端模型及完整错误；全部成功自动关闭和失败重试逻辑保持。
- 原始错误用纯文本 `pre` 展示，保留换行、长 URL 自动换行，并避免英文翻译改写原始错误。

## 自动验证

- 新增 12 项复现先在旧实现失败，修复后全部通过。
- `npm --prefix frontend run test:run -- src/features/providers src/utils/__tests__/errorParser.spec.ts src/i18n/__tests__/i18n.spec.ts`：27 个测试文件、179 项通过。
- `npm --prefix frontend run type-check`：通过。
- 所有变更 TS / Vue 文件定向 ESLint：通过。
- 既有 provider-key 测试的 InputStub size 属性警告、Browserslist 数据过期提示不影响测试结果，本次未修改对应依赖和测试。

## 隔离页面验收

临时验收目录：`/var/folders/fg/bzpd9ft96g976xqf_w4lwbrr0000gn/T/aether-batch-mapping-qa-7ov7xjgm`。

使用真实 Vue 组件、错误解析和模型查询 composable，API 替换为合成错误夹具；阻止未知 API 请求，未访问真实数据库。

- 1280 px 视口：完整多 Key 错误含末尾中文行，详情 clientWidth / scrollWidth 均为 935；white-space 为 pre-wrap，overflow-wrap 为 anywhere。
- 390 px 视口：详情 clientWidth / scrollWidth 均为 319；页面宽 390，无水平溢出，最后一条详情可见。
- 英文界面：反馈标题为 Provider model notice；中文原始错误、换行及第二个 warning 原文保留。
- 两个模型同时保存失败：分别显示 Client 1 / Client 2 及多行完整原因；再次保存成功，弹窗自动关闭。
- 保存验收页面无浏览器 error 日志。

## 分支和安装包

待提交、同步及打包完成后记录。
