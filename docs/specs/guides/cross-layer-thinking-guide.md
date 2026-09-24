# 跨层契约

针对本次变化追踪输入 → 验证/转换 → 执行或写入 → 再次读取 → 展示。只扩展相关调用链，不以涉及多层为由全仓检查。

- 配置字段：核对缺省、继承、显式 null、单位、保存范围与读后回显。Aether 范例见 [useSystemConfig](../../../frontend/src/views/admin/system-settings/composables/useSystemConfig.ts) 和 [路由契约](../aether-routing-core/backend/simplified-routing.md)。
- 请求状态：区分未发送、上游失败、客户端取消、合法终态和持久化失败。调度/健康变化读取 [聊天故障转移](../aether-gateway/backend/chat-failover.md)，不要从单个 HTTP 状态推断完整调用结果。
- 数据变化：检查实际 migration、repository、memory 对应实现、导入导出与旧数据兼容；参见 [数据规范](../backend/database-guidelines.md)。
- 安全边界：根据真实输入来源决定校验和权限，前端提示不能替代服务端权限检查；复用解析逻辑不等于省略跨信任边界的验证。
- 验收：让断言覆盖最终行为及后续读取、刷新或重复操作。局部函数通过不代表跨层契约成立，具体选择见 [验证规范](../backend/quality-guidelines.md)。
