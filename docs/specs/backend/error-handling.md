# 错误、日志与资源生命周期

## 类型和对外映射

- 数据层复用 [DataLayerError](../../../crates/aether-data/contracts/src/error.rs)。输入错误、配置错误、数据库失败和超时有不同语义；不要为了接口返回方便，把失败变成空列表或成功默认值。
- 网关 HTTP 映射以 [GatewayError::into_response](../../../apps/aether-gateway/src/error.rs) 为入口。当前 upstream/control 不可用映射 502，本地 planning 超时映射 504，准入排队超时映射 429 并带 `Retry-After: 1`。修改时同步 status、body 与 trace header 的直接调用方和已有测试。
- 局部路由可有自己的业务错误封装，不能把上述映射当成全部 API 的统一响应 Schema。[provider-ops 错误](../aether-admin/backend/error-handling.md) 保留特定状态前缀和有效上游业务 message。
- 传输错误在归因前保留其类型与来源。HTTP 200、收到 headers、开始输出都不足以证明模型完成；协议终态与健康结算读取 [聊天契约](../aether-gateway/backend/chat-failover.md)。

## 日志边界

采用当前 `tracing` 结构化事件，关联 trace、phase、耗时和必要实体标识。排障日志使用脱敏样例，不打印 Authorization、Cookie、密钥或完整敏感请求体。需要定位同一段文本而不展示内容时，可复用 [summarize_text_payload](../../../crates/aether-runtime/base/src/redaction.rs) 的字节数和哈希摘要；它不是内容脱敏器。

已有业务记录策略与普通诊断日志不同，变更需沿用具体契约；不要声称当前所有错误输出都天然安全。新增调试记录应在任务结束前移除，必要的长期诊断才保留。

## 取消、permit 与流式结果

[admission.rs](../../../crates/aether-runtime/base/src/admission.rs) 提供响应体/异步任务生命周期的 permit 持有封装。流式请求返回 headers 时仍可能占用执行资源，释放时机必须与真实结束、取消或租约丢失一致。

聊天 probe、重试等待、共享首输出预算、终态结算使用 [chat-failover](../aether-gateway/backend/chat-failover.md) 的专门规则。不要把客户端取消记为上游失败；已交付不可安全重放的状态后，不透明重发整轮请求。

## 验证

错误改动验证可观察的 status/body/header 与相关状态变化；取消和异步改动验证 permit/lease 释放及后续请求。已有 `error.rs` 测试是 HTTP 映射的起点，真实路由与持久化证据见 [验证规范](quality-guidelines.md)。
