# 设置页重规划设计

## 页面结构

保留 `/admin/system` 和 AppShell。页面采用 160-176px 分类导航和单一内容区，正文上限约 760px，按实际内容宽度重排；可用宽度不足则分类改顶部 Select，不横向滚动。

| 分类 | 常用层 | 次级层 |
| --- | --- | --- |
| 连接与启动 | 默认代理、节点、自启动 | 网关端口 |
| 记录与存储 | 记录级别、自动清理、内容/记录保留 | 详细保留策略、敏感请求头 |
| 备份与恢复 | 完整备份创建和恢复 | 配置迁移 |
| 高级与诊断 | 请求兼容、Key 默认规则、版本 | 目录/日志、数据维护与历史 |

设置行宽窗口左名称右控件，窄窗口上下重排。沿用项目字体、颜色、控件；设置分组无浮动卡片，不更改通用 CardSection 影响其他页面。保留帮助判断的说明，如端口重启、心跳影响 HTTP 状态和数据操作范围，不展示实现过程说明。

有改动时该保存分组出现取消和带图标的保存按钮。自启动、自动清理保持即时生效，请求时禁用自身、失败后恢复确认状态。不建立跨全部配置的保存按钮。

## 路由与草稿

- 分类使用 `?tab=connection|records|backup|advanced`，未知值回落 connection，保留无关 query。
- 旧 hash 映射：desktop-gateway/proxy -> connection；data-mgmt -> backup；basic/sysinfo -> advanced；request-log/cleanup -> records；完整旧 ID 均带 `section-` 前缀。
- hash 定位时优先打开目标分类及所需折叠内容。主动切换分类移除旧 hash，避免刷新跳回。
- 系统草稿继续由父级 useSystemConfig 持有；本地输入区域首次访问后保持实例，或显式提升草稿。不能因分类卸载丢失端口输入或反馈。
- 节点、清理历史在相关内容首次访问时加载；诊断日志按需读取。
- 保留桌面旧偏好路由重定向；Web ProfileSettings 删除偏好区后继续提供账号功能。

## 组件边界

| 原实现 | 处理 |
| --- | --- |
| SystemSettings.vue | 替换目录和编排，复用配置/导入 composable 与弹窗 |
| DesktopSettings / DesktopGatewaySection | 正常展示拆分连接与诊断，共用 useDesktopGateway；恢复场景保留关键控件 |
| ProxyConfigSection | 行式默认代理和紧凑节点列表，保持独立 CRUD/测试 |
| RequestLogSection / CleanupPolicySection | 同分类但保留各自保存边界；维护行为与表单展示分开 |
| DataManagementSection | 主备份、次级迁移，清空行为移入高级维护组并复用 handler |
| BasicConfigSection | 请求兼容和 Key 规则同属高级，沿用原基本配置保存组 |
| SystemInfoSection | 精简为版本行 |
| ProfileSettings / UserPreferenceFields | 删除偏好表单专有状态和事件；引用清零后删字段组件 |

记录内容保留使用 compressed_log_retention_days；detail_log_retention_days 在次级层标为「开始压缩内容」。保持既有校验，期限冲突时指出字段并打开相关次级项，不静默调整其他值。

移除编辑键为 cleanup_batch_size、request_candidates_cleanup_batch_size、proxy_node_metrics_cleanup_batch_size。可以继续读取，但不进入普通保存 payload；数据库及备份中的值原样保留。

保存仅覆盖对应可编辑键，开始时捕获提交值，完成后只更新成功字段的基线。单字段 API 并行调用无事务保证，不将部分失败报成全组成功，不无差别刷新覆盖其他草稿。取消仅恢复对应组的确认值，不为当前拆分建立通用表单框架。

## 行为兼容

- 完整备份/迁移沿用既有 composable 及两个导入弹窗，不引入格式识别/转换。
- 数据维护为折叠列表，点击具体操作复用原危险确认及执行机制；取消不发送写请求。
- 按策略清理沿用当前确认及历史流程，不因导航触发。
- 顶部 LanguageSwitcher/ThemeModeButton 不依赖偏好字段组件，保持其 store/composable。
- 偏好 API、备份中 preferences 和时间处理保留；本次删除前端表单编辑入口。
- Web 同步采用公共布局，不获得桌面 IPC 权限，第一分类称网络连接。

## 风险与回退

清理组件同时包含表单、即时开关、手工任务和历史，拆分需明确状态归属，避免重复加载或误触发维护。保存仍需表达局部失败。真实清理/恢复/端口重启测试使用隔离数据，不使用用户当前数据。

草图只展示分类、密度和局部交互，演示值不是用户实际配置，未连接 API。实施不迁移配置/数据库，可独立回退前端恢复旧入口。
