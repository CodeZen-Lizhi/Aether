# 使用记录筛选维度拆分

## 目标与范围

- 将使用记录“类型”拆成请求类型、生命周期、客户端传输和调度四个独立筛选。
- 类型列以 `request_type` 为主标签，以失败/活跃/取消和 HTTP 流式/标准/WS 为次标签。
- 类型下拉从当前时间范围的 `request_type` 聚合获取，并补充当前页历史值和已选值。
- 服务端分页、总数与前端轮询后的本地结果使用同一组筛选语义。

## 关联 Spec

- [Frontend Spec：Usage filters](../specs/frontend/index.md)

## 验收

- 四个下拉可以同时组合，切换任一条件都会回到第一页并重新读取列表与总数。
- 选择请求类型后只返回相同 `request_type`，大小写和首尾空白归一。
- HTTP 流式/标准按客户端可见传输方式判断，转换后的上游模式不会改变命中分组。
- 调度筛选保留发生重试和发生转移，且不混入请求类型下拉。
- 未知历史请求类型可在下拉中选择，类型列保留未知类型文案。
- 前端类型检查、目标 Vitest、网关 HTTP 测试和 SQLite/Memory 查询测试通过。

## 实际结果

- 已实现：类型列以 `request_type` 为主标签，失败/活跃/取消和 HTTP 流式/标准/WS 为次标签。
- 已实现：类型、状态、传输、调度四个快捷筛选独立组合；请求类型选项只来自时间范围聚合、当前页实际值和当前选择。
- 已实现：管理端列表、计数、关键词搜索和请求类型聚合均支持 `request_type`；非法 `request_status`、`transport`、`attempt` 显式值返回 400。
- 已验证：`cargo check -p aether-data-contracts -p aether-data -p aether-data-sqlite -p aether-admin -p aether-gateway` 通过。
- 已验证：SQLite/Memory 的请求类型列表、计数、关键词和聚合测试通过；管理端客户端传输语义测试、网关四维组合、请求类型聚合和重试/转移测试通过。
- 已验证：`npm run type-check`、`npm run build` 通过；目标 Vitest 6 个文件、83 项全部通过。同步修正了性能提示加入首字说明后仍要求标题完全相等的旧断言。
- 已验证：使用真实网关、隔离 SQLite、合成上游和实际 `/admin/usage` 页面完成验收。类型下拉只显示当前数据中的对话、会话压缩、视频、实时会话和未知类型；四维组合请求实际包含 `request_type=compact&request_status=failed&transport=stream&attempt=fallback`，并只返回对应记录。
- 已验证：实际页面分别验证 HTTP 标准、未知类型、Realtime WebSocket、视频重试四组筛选；1024 和 390 宽度均无横向溢出，窄屏四个快捷筛选保持可达。
- 未验证：未连接现有业务数据库。HTTP 流式/标准与重试/转移筛选为保持既有客户端语义采用内存过滤，尚未针对超大时间范围做性能实测。
