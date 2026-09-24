Active task: docs/tasks/09-13-review-scheduler-failover

你是后续 implement worker，原生 retry_runtime 已交付并结束。用户已批准完整方案，直接实施，勿再次spawn。读 implement.jsonl、最新 check-findings.md、retry-runtime-contract.md、implementation-contracts.md。你独占 HTTP execution_runtime sync/stream/transport failure/chat_retry、executor candidate_loop/stream_path 及必要 HTTP Responses planner request 状态绑定路径；其他owner活跃，不改effects.rs/policy.rs（main）、WS（websocket）、probe_lease/circuit_probe/rate_limit_probe（storage）、affinity排序/上下文（ranking）。

优先修check F1/F2/F3：同步异常HTTP200 error envelope不能回血，应归一失败有限重试；typed transport first byte timeout=1、可归因上游连接/TLS=2、本地配置/取消=0且不能重复settle；HTTP previous_response_id必须旧响应原K/物理绑定而不是本次首候选，不能跨K或静默删除ID/重建历史/转换绕过。有现成可验证ownership存储则复用，否则明确失败，保留无状态完整上下文的正常failover。测试从真实入口验证，避免只测新helper。

storage正在实现 managed probe owner guard，契约research/probe-lease-contract.md。API稳定后请负责HTTP guard真实接线：移除聊天planner旧预占和runtime重复claim（非聊天保留兼容），在执行gate/admission后最后时刻原子检查并claim；持有到sync完成/SSEbody终态；lease.lost中性停止，不重放已交付输出；report包含owner上下文供main effects校验；终态先effect后finish。你可与storage/channel同步API，不需要main批准常规接线。

main effects现强制chat_health_attempt，不再有无身份CAS；capture现在有transport_fingerprint，policy追加planned_chat_credential_fingerprint，拒绝旧计划给新凭据计分。请保留并可在capture_attempt_report_context时比对这两个指纹，凭据已变化时不中转旧计划、不扣分。不要新造自动重置身份。

integration worker当前独占gateway Cargo，六项真实HTTP fixture首轮失败receipts=[]，正诊断。请通过channel协调该worker验证，不能启动第二个gatewaycargo。只改针对范围和有意义测试，先fmt指定文件。报告API、修复和剩余验证，不能报告计划即完成。禁止提交/付费调用/生产变更。
