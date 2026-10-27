# 本地限流详情保留普通耗时并收敛重复状态

## 请求与关联

用户反馈供应商本地限流跳过时的请求详情重复表达恢复状态：概览被替换成恢复时间，请求结果也展示恢复时间，Key 卡片又展示截止与倒计时，展开后还有一张“本地调度”错误卡。

本轮确认的展示契约：

- 概览第二项始终是请求级总耗时，本地限流不改变普通请求的时间结构。
- 请求结果仍显示“未发送”，下一行与其他请求一样显示总耗时。
- Key 卡片保留本地限流截止和实时倒计时，但两者需要视觉分隔。
- Key 展开后保留 HTTP、耗时、限流时间、剩余冷却及无上游额度说明。
- 本地限流 Key 展开后不再重复显示“本地调度”错误卡。

关联 Spec：`docs/specs/frontend/index.md` 的“请求详情首屏与尝试分页”。

## 实现

- `RequestDetailDrawer.vue` 移除本地限流对概览第二项的替换，概览固定显示 `总耗时` 与请求级耗时。
- `HorizontalRequestTimeline.vue` 的请求结果节点固定显示 `总耗时`；本地限流只影响结果状态“未发送”，不影响耗时语义。
- 本地限流 Key 卡片底部拆分为截止时刻和倒计时两个节点，使用弹性布局、12px 间距和等宽数字；窄卡片允许左文本省略但保留倒计时。
- 本地限流跳过的 Key 展开详情不渲染 `RequestAttemptErrorPanel`，避免重复“本地调度”结论；其他失败和真实上游 429 保持原逻辑。
- Fixture 的本地限流 trace 使用真实 `4ms` 请求级耗时，不再用 `0` 掩盖请求处理时间。

## 验证

- `npm run test:run -- src/features/usage/components/__tests__/HorizontalRequestTimeline.spec.ts src/features/usage/components/__tests__/RequestDetailDrawer.pricing.spec.ts`：57 项通过。
- `npm run type-check`：通过。
- `node scripts/run-check.cjs scripts/check-request-detail-redesign.cjs ...?status=cooldown`：真实 Chromium 七场景通过；覆盖概览和请求结果的 `4ms 总耗时`、Key 卡片截止/倒计时至少 8px 的可见间距、展开详情四项事实、无“本地调度”错误卡、无上游响应，以及 390/540/800px 布局。
- 截图复核：概览显示 `4ms / 总耗时`，请求结果显示 `未发送 / 总耗时 4ms`，Key 卡片两组冷却信息分开，展开详情保留四项且错误卡已隐藏。

## 状态与边界

状态：实现、自检和浏览器验收完成；未提交、未构建、未发布。

边界：Fixture 与浏览器检查使用模拟接口，不代表真实供应商调用；本轮未修改后端耗时采集和真实 429 语义。
