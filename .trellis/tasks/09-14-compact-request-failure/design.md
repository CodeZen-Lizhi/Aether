# 技术方案

## 输出语义
在现有 Responses SSE 有效结果判定处补齐明确含内容的快照事件。text/reasoning done 必须有非空文本；content_part/output_item done 根据已支持的 message、reasoning、tool、compaction 内容判断，不能任意 done 或空 item 都视为输出。优先复用已有格式解析契约，保持 SSE framing 与转换行为。

有效结果只解除“首次有效输出等待”，不将单项完成当作整个响应 completed。最终成功仍由合法流终态和现有结算链路决定。

## 取消与终态
从已有 lifecycle seed / active attempt 上下文保存可用于失败结算的最小信息，不复制整个敏感请求。总预算到期由单一所有者完成 candidate 与 usage 终态，沿用幂等结算/取消边界，避免 drop future 后只能依赖十分钟清理。重试耗尽时保留已发生的 timeout 原因，不覆盖成无法构建计划。

## 可观察性
在现有结构化 tracing/metadata 中记录首响应头、首 SSE 帧、首有效事件、取消触发器及配置阈值。只记时刻、事件类型、字节数/计数和必要 trace/candidate 标识；不记录事件内容、encrypted_content、鉴权或订阅信息。输出后 TLS EOF 继续作为 transport/protocol error，不能升级为 completed。

## 边界与兼容
保持供应商 30 秒 / 90 秒默认和用户显式值；不引入强制完整回答总时限。60 秒 idle 日志不是中断动作。未知事件不静默当成功。普通 chat 与 compact 共享正确内容语义，旧 /responses/compact 非流式超时不改变。

本机代理自动关闭连接建议单独调整，不属于仓库实施。历史失败缺少 SSE 原文，修复已复现缺陷不等于证明所有历史 timeout 同因。

## 风险与回滚
风险在过早解除预算、重复 usage/健康结算、异步取消遗漏。用真实 Router+本地上游和临时 SQLite 覆盖。无需数据迁移；回滚聚焦本次分类/取消/诊断变更，保留现有配置。
