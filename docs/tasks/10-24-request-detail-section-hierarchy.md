# 请求详情执行路径纵向瀑布重构

## 请求与关联

用户连续否定了卡片、轨道、状态符号、主从检查器和左右分栏，最终明确选择样式参考中的方案 B：直行瀑布。目标是让多供应商、多 Key 的实际调度顺序与相对耗时一眼可见，展开某个 Key 后再看该次调用的真实错误与上游响应。

关联长期 Spec：[前端规范](../specs/frontend/index.md) 的“请求详情首屏与尝试分页”。本次将“尝试分页/主从检查器”更新为纵向执行瀑布；计费明细、请求上下文、更多数据及请求上下文中的调用密钥/上游执行 Key 语义保持不变。

## 本次结果

- `HorizontalRequestTimeline.vue` 已改为 B 方案的纵向执行瀑布：左侧按实际调度顺序分组供应商和 Key，右侧以共享时间轴、起点和条宽表达单次调用耗时；不再存在旧左右导航/检查器、轨道、竖线或状态符号方案。
- 时间条只有成功、失败、冷却和进行中的语义色，Key 行不写“失败/成功/HTTP”或符号。冷却 Key 不绘制耗时条，以中性文字“冷却跳过”或“未发送”表达未发送事实。
- Key 详情默认收起；点击在所选行下方展开，再次点击收起。刷新或重排按记录 ID 保持展开身份，新请求使旧展开状态失效。首层只保留 HTTP、耗时、时间、Key 标识、可选 usage/图片进度及一条真实上游错误；原始上游响应是唯一技术展开入口。
- 摘要使用“供应商数 · 上游请求数 · 冷却跳过数”，冷却跳过不计入上游请求。供应商行显示 Key 数及实际发送数，不再使用“尝试轮次”或操作提示。
- 执行瀑布外壳与计费明细、请求上下文、更多数据使用同一圆角卡片语言；深色主题通过组件主题类显示深红、深绿时间条，避免状态条发白。390px 仅收窄标签列，仍保持纵向瀑布且无横向溢出。

## 实际验证

- `npm --prefix frontend run test:run -- src/features/usage/components/__tests__/HorizontalRequestTimeline.spec.ts src/features/usage/components/__tests__/RequestDetailDrawer.pricing.spec.ts`：43 项通过，覆盖默认收起、点击展开/收起、按记录 ID 保持展开、重排、快速切换请求、真实错误与冷却语义。
- `npm --prefix frontend run type-check`：通过。
- 真实 Chromium 使用 `frontend/scripts/fixtures/request-detail-layout.html` 的六场景通过：成功、上游 500、冷却、多供应商、单 Key 429、真实上游 400。独立断言单一瀑布、无旧检查器、供应商与 Key 顺序、时间条起点递增、成败色差异、冷却无耗时条、Key 行无状态文字/HTTP/符号、详情默认收起及点击展开/收起、真实错误仅一次、原始响应可展开、390/540/800px 无横向溢出、深色状态条可辨、Console 无未解释错误。
- 可视截图：`/tmp/aether-request-detail-multi-provider.png`、`/tmp/aether-request-detail-multi-provider-390.png`、`/tmp/aether-request-detail-real-upstream-error.png`、`/tmp/aether-request-detail-dark.png`。

## 状态

方案 B 纵向瀑布已实现，组件、类型与真实浏览器检查通过；已以 `重构请求详情执行瀑布` 提交，并随 `codex/tauri-macos` 推送。Aether 0.1.62 安装包使用该提交构建并通过 6 项交付门禁；最终视觉仍待用户肉眼主观确认。
