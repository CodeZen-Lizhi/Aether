# 模型映射优先级调整移至列表

日期：2026-10-29

## 目标

- 管理映射弹窗恢复之前的紧凑映射标签样式，不再提供行内拖拽或上下箭头。
- 弹窗中的优先级只由选择顺序产生：已有项保持原顺序，新选择项依次追加。
- 模型映射列表的精确映射展开区支持拖动排序，交互效果与调度策略列表一致，不提供箭头按钮。

## 实现

- `ModelMappingBatchEditor.vue` 恢复紧凑映射标签，删除拖拽状态、拖动事件、上下移动按钮和重排事件。
- `BatchModelMappingDialog.vue` 删除批量工作台草稿重排逻辑；保存时继续按映射数组顺序生成连续的 `1..N` priority。
- `ModelMappingTab.vue` 在精确映射展开区增加拖动排名、拖动手柄、虚线占位和跟随指针的浮动预览。
- 拖动只在松手后提交；保存时只重排当前映射分组并生成连续 priority，成功后刷新供应商详情，失败时恢复原顺序。

## 验证

- `BatchModelMappingDialog.spec.ts` 15 项验证弹窗不存在重排控件，新增项按选择顺序追加。
- `ModelMappingTab.mapping-order.spec.ts` 与 `ModelMappingTab.contract.spec.ts` 共 3 项验证拖动手柄、无上下箭头及连续 priority 保存。
- 真实组件 fixture 完成实际拖动：`gpt-5.6-sol` 从第 1 位移动到第 2 位，写请求 priority 为连续的 `1、2、3`。
- 管理映射弹窗在 1200px 与 390px 下恢复紧凑标签样式，无行内排序控件，页面无横向溢出。
