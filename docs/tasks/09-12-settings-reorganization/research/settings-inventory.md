# 设置盘点

2026-09-12，只读基线 HEAD 8743f734a。已观察实际界面，未操作运行时配置。

| 事实 | 源码证据 |
| --- | --- |
| 七类长页面、右侧目录、折叠区附操作 | frontend/src/views/admin/SystemSettings.vue:1、:137、:230 |
| 桌面含端口、自启动、目录和日志 | frontend/src/desktop/DesktopSettings.vue:107 |
| 记录级别及敏感头一组保存 | frontend/src/views/admin/system-settings/RequestLogSection.vue:1 |
| 压缩时点、内容删除、请求头、整条记录为不同期限 | frontend/src/views/admin/system-settings/CleanupPolicySection.vue:48、:68、:88、:108 |
| 清理保存 11 键，单项并行写；自动开关独立即时保存 | frontend/src/views/admin/system-settings/composables/useSystemConfig.ts:359、:455 |
| 基础配置含限速、过期 Key、四个兼容项 | frontend/src/views/admin/system-settings/BasicConfigSection.vue:1 |
| 两种导入导出，五种清空 | frontend/src/views/admin/system-settings/DataManagementSection.vue:182、:205 |
| 版本独占卡片 | frontend/src/views/admin/system-settings/SystemInfoSection.vue:1 |
| 偏好字段只由 ProfileSettings 使用，有专有服务端同步 | frontend/src/views/admin/ProfileSettings.vue:215、:368 |
| 顶部主题/语言独立组件 | frontend/src/components/common/ThemeModeButton.vue:1；LanguageSwitcher.vue:1 |
| 桌面旧偏好路由已重定向 | frontend/src/router/index.ts:23 |

## 运行能力消费点

- 全局格式转换：apps/aether-gateway/src/state/transport_snapshot.rs:377。
- Cyber 转移：apps/aether-gateway/src/orchestration/policy.rs:62。
- 生图/文本心跳：apps/aether-gateway/src/executor/orchestration.rs:751、:769。
- 过期 Key：apps/aether-gateway/src/maintenance/runtime/config.rs:106。

以上只证明运行代码有消费点，不能证明使用频率。本次收纳到高级入口，不判定为可删除的后端能力。

## 边界

未读取用户数据库或执行导入、清空、重启。时区按用户要求保持现状。UI/UX 本地查询只返回表单标签/反馈等通用条目；分类方案以仓库及用户要求为依据。
