# 代码复用

新增逻辑前，在相关包及直接调用方查找现有语义相同的实现；用 `rg` 限定范围，先确认输入、错误与生命周期是否兼容。

Aether 的具体复用入口：

- [provider/transport](../../../crates/aether-provider/transport/src/lib.rs)：认证 header、URL、转换规则与诊断。
- [chat_retry](../../../crates/aether-contracts/src/chat_retry.rs)：聊天重试与有效配置解析，避免管理回显和执行次数各算一套。
- [ApiClient](../../../frontend/src/api/client.ts)：管理 API 请求与认证恢复。
- [运行边界](../backend/architecture.md)：决定共享逻辑归哪个模块。

只有真实相同的契约才合并。不要因出现相似代码就强制引入抽象，也不要把单一功能细节放进通用库。以调用方需要理解的概念是否减少、改动是否集中来判断收益，测试通过公共行为验证。
