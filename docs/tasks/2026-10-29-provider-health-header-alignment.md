# 供应商端点健康表头对齐

日期：2026-10-29

## 目标

- 修正供应商管理页面“端点健康”表头与下方健康条不在同一条垂直线上的问题。

## 实现

- `ProviderManagement.vue` 的端点健康表头改用 `text-center` 与 `align="center"`，与资源统计、状态和操作列采用相同的表头/内容居中规则。
- `provider-spacing-preview.html` fixture 同步资源统计、端点健康、状态和操作列的居中表头，便于真实组件视觉验收。

## 验证

- `ProviderManagement.lazy-detail-drawer.spec.ts` 7 项通过；`npm run type-check` 与目标 ESLint 通过。
- 真实组件 fixture 在 1920 宽下测量：资源统计、端点健康、状态三列的表头文字中心与数据内容中心一致，端点健康偏差为 0px，页面和列表无横向溢出。
