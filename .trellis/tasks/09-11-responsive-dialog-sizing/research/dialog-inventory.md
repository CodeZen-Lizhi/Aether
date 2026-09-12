# 弹窗静态清单（2026-09-12）

扫描范围：frontend/src 及 apps/aether-desktop。`<Dialog` 共 22 个调用文件、25 个实例；另有一个独立 ReplayDialog。实例清单如下，路径均相对 frontend/src。

| 调用文件 | 实例/档位 | 检查重点 |
| --- | --- | --- |
| components/common/AlertDialog.vue | 默认 md | 全局确认，z-index 120 |
| features/api-keys/components/StandaloneKeyFormDialog.vue | xl | 自定义 header/footer |
| features/models/components/GlobalModelFormDialog.vue | 3xl | 创建/编辑，嵌套视口高度，多列价格字段 |
| features/providers/components/ProviderFormDialog.vue | xl | 添加/编辑，多字段、验证反馈 |
| features/providers/components/ProviderAuthDialog.vue | md | 认证结果双栏 |
| features/providers/components/ProviderModelFormDialog.vue | xl | 多标签、多列字段 |
| features/providers/components/KeyFormDialog.vue | xl | 固定双栏及嵌套网格 |
| features/providers/components/EndpointFormDialog.vue | 2xl | 多列表单、内含 AlertDialog |
| features/providers/components/ModelAliasDialog.vue | lg | 别名表单 |
| features/providers/components/ModelMappingDialog.vue | lg | 模型选择与长标签 |
| features/providers/components/BatchModelMappingDialog.vue | 5xl | lg:grid-cols 两栏，第二栏 minmax(20rem,...) |
| features/providers/components/BatchAssignModelsDialog.vue | 2xl | 批量模型列表 |
| features/providers/components/KeyAllowedModelsDialog.vue | 2xl | 独立列表滚动 |
| features/providers/components/KeyAllowedModelsEditDialog.vue | 2xl | 独立列表滚动、操作按钮 |
| features/providers/components/FailoverRulesDialog.vue | lg | 根节点 max-h-[60vh] 及滚动 |
| features/providers/components/provider-tabs/ModelTestDialog.vue | 主 3xl；预览 6xl | 设置/运行/结果、错误详情、图片高 72vh |
| views/admin/ApiKeys.vue | lg | 新 Key 结果，自定义 header |
| views/admin/ModelManagement.vue | 2xl | 模型选择 |
| views/admin/system-settings/ProxyNodeEditDialog.vue | 默认 md | 表单与连通测试反馈 |
| views/admin/system-settings/ManualCleanupConfirmDialog.vue | lg | 清理确认 |
| views/admin/system-settings/ConfigImportDialog.vue | 两个默认 md | 导入确认和结果 |
| views/admin/system-settings/AggregateImportDialog.vue | 两个默认 md | 完整备份导入确认和结果 |
| features/usage/components/ReplayDialog.vue | 独立，拟复用 6xl 尺寸 | max-w-6xl、60-85vh、左右各半 |

间接确认入口：components/ConfirmContainer.vue、ProviderDetailDrawer.vue、EndpointFormDialog.vue、provider-tabs/ModelAliasesTab.vue、provider-tabs/ModelMappingTab.vue，经 AlertDialog 继承共享尺寸。

排除：ModelDetailDrawer、ProviderDetailDrawer、RequestDetailDrawer 是侧边抽屉；MultiSelect、ActivityHeatmap、sortable-table-head、RoutingProfiles 的 Teleport 为选择/提示/拖动等浮层，不能按居中模态框改造。

现阶段证据为源码静态检查，未打开运行页面，未测量截图；实际最小可操作宽度与测试结果动态高度留到实施验证。
