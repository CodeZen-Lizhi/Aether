# 实施清单与验收

状态：completed，实施与必要验证完成。最终行为证据和限制见 verification.md；源码未提交、未安装或发布。

## 顺序与依赖

1. 已按 Trellis 流程 start 并加载 trellis-before-dev。保护既有 docs/specs 与 AGENTS 改动；native context 注入 implement.jsonl/check.jsonl，子代理缺失上下文时自行加载。
2. 先建立两条独立的可观察回归：R2 的早期 headers+迟到 body/结果、R1 的候选失败与重试间隙。复用 output/compact-timeout-diagnosis/reproduce.py 和临时 SQLite/合成供应商，不调用真实付费上游。
3. 配置/契约准备：共享默认900秒与独立字段、API校验/patch/readback、provider.config JSON、ExecutionTimeouts 与必要 tunnel 传播、表单/i18n。前后端以 design.md 的字段语义为共同依赖；接口契约稳定后才并行修改表单与运行时。
4. 超时实现由 trellis-implement 主工作单元处理 gateway runtime/transport/candidate loop 与配置契约：成功最终 headers 解除首次计时；移除成功 headers 后首 body 限制；旧首有效输出预算退出目标HTTP链路；完整deadline覆盖预读和响应pump。明确错误预读/commit与计时上下文解耦。
5. R1状态修复可作为独立有界工作单元，与步骤4并行，但文件范围只限 usage API/前端usage状态及测试，不修改运行时或配置文件。两部分共用最终HTTP/SQLite验收，保留真终态保护与历史/图片兼容。
6. 定向验证与修复所有本次引入的问题后，按 auto 模式调用 trellis-check，核对默认值/空值/旧字段兼容、body 生命周期、错误/终态race。高风险并发/公共契约的审查和限定验证按项目 reviewer/verifier 规则执行，避免重复全仓检查。
7. 最小真实页面流程：保存/清空/重读超时设置；列表→详情→切换尝试→关闭重开→刷新，核对进行中和终态一致。使用隔离本地服务或fixture，不修改生产供应商。
8. 主会话整合、更新直接相关 spec 与任务证据。仅报告实际通过的验证和限制；不执行提交、push、安装、发布。

不拆父子任务：R1与R2可独立编辑，但共同构成这次用户报告的修复，最终请求状态验收相互关联；采用明确的文件所有权和依赖即可，避免多个活动任务重复管理。

## 关键验收场景

| 场景 | 可观察结果 |
| --- | --- |
| 100ms HTTP200，首次1s，3s才有body/普通文本或压缩结果，总5s，旧预算1.8s | 成功，首次限制与旧预算均不误杀 |
| 100ms HTTP200+启动/心跳，后续静默超过首次期限，最终总期限内完成 | 成功；不要求持续心跳 |
| 首次期限内完全没有headers | 首次超时；符合策略时重试 |
| HTTP200后永无body | 到总期限失败，错误明确为完整响应超时 |
| 持续有body但超过总期限 | 总超时，心跳/正文不延长截止点 |
| 首候选失败、retry gap、第二轮成功/失败 | records/detail/trace的请求状态一致，单轮失败不锁住列表 |
| 重试、供应商切换、Retry-After | 不增加尝试次数，不延长请求总deadline |
| HTTP200后早期error、已提交内容后error/EOF、客户端取消 | 对应真实终态；不违规重放、不伪造成功 |
| 总期限到达/成功终态/持久化竞态 | 单一终态事实，permit/probe释放，后续请求可用 |
| 配置保存、null清除、旧config与未知keys | 有效值一致；旧90秒不变为完整回答期限 |
| 非流式/WS/图片路径 | 保持既有独立超时与协议语义 |

## 验证命令入口

下列命令仅在实现后运行，使用过滤项选择实际受影响用例；不得当成已执行结果：

- `cargo test -p aether-contracts --lib chat_retry`；公共新字段补serde往返用例。
- `cargo test -p aether-provider-transport --lib execution_timeouts`（按实际用例名筛选）；`cargo test -p aether-admin --lib provider`。
- `cargo test -p aether-gateway --lib stream_response_timeout`（新增公共行为用例统一前缀）；现有 first_output/watchdog/transport/admin usage 受影响用例与 scheduler_failover 中重试/EOF相关场景定向运行。
- 若修改隧道接收端：`cargo test -p aether-tunnel --lib` 加实际timeout用例过滤；若该包没有lib target，按 Cargo target 改为对应二进制测试，不扩大为全仓测试。
- 前端 `npm run test:run -- src/features/usage/utils/__tests__/status.spec.ts src/features/usage/utils/__tests__/recordSync.spec.ts src/features/usage/composables/__tests__/useUsageData.spec.ts src/features/usage/components/__tests__/HorizontalRequestTimeline.spec.ts`，另加ProviderFormDialog实际新增的timeout配置用例。
- 变更Rust包fmt与必要check；前端受影响文件eslint及必要type-check，不运行带`--fix`的全仓lint。
- 构建单一gateway目标用于本地HTTP/SQLite合成验收；浏览器流程证据与真实供应商验证分开记录。

## 实施前检查

- 用户已明确批准包含900秒默认与兼容方式的最终方案，无需重复确认。
- implement.jsonl/check.jsonl 中所有路径存在，包含实际spec/research和本次设计；已读chat-failover中的旧首有效预算段落与本次用户要求冲突，实施时以本次已确认PRD为准并同步更新该规范。
- 实施中保留用户既有spec整理；完成后给出具体变更、测试结果及未验证范围。
