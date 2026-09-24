Active task: docs/tasks/09-13-review-scheduler-failover

你是 Trellis check 子 agent，不要再 spawn。用户已批准实施。先读 check.jsonl 全部引用、prd/design/implement 和 research/implementation-contracts.md，再对本任务跨层实现做只读先行审查。当前 health_policy 和 retry_runtime 两个原生实现者，以及 channel storage/config/websocket/ranking 仍活跃；此轮不要改活跃文件，不跑 cargo（main 正在统一编译），写 research/check-findings.md 并报告具体问题。

优先实际正确性：健康事实按错误来源一次分类、每真实 attempt 只结算一次、启动前捕获旧凭据与 circuit epoch、探测续租/取消、Retry-After 完整与累计2秒、provider配置90秒跨K总预算真正生效、首输出门与重放门分离、所有 sync/SSE/WS 健康及 retry调用链、状态引用固定K、三种模式真实读写亲和、配置规范化保留旧显式覆盖。不要把规划未实现处误报成最终已确认bug，指出当前缺口并注明 owner 正在接入即可。不要扩张为无关全仓问题。

主会话已运行 cargo check -p aether-gateway --tests 通过；orchestration filter 159通过1失败 (recovery::anthropic_failure_disposition_controls_candidate_retry，可能编译取到owner中间版本)。主会话正在真实网关集成filter。你不需要重复执行这些。

完成初审后等待 main 要求最终全范围复核；届时根据稳定版本做必要机械修复与针对校验。不能以编译通过替代行为验收；报告只列可重现影响与对应位置。禁止提交/发布/生产数据/真实模型调用。
