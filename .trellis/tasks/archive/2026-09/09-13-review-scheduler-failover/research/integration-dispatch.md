Active task: .trellis/tasks/09-13-review-scheduler-failover

你是 trellis-implement 子 agent，直接实现测试，不再 spawn。任务完整方案已批准，in_progress。读 PRD/design/implement 与 implementation-contracts。你的独占交付是可通过真实网关入口和本地可控上游验证故障转移的集成夹具/测试（新测试文件，避免改其他 owner 活跃源码）。先探索现有 AppState/memory/SQLite/testkit/路由测试方式，写 research/integration-contract.md 告知入口和需要主会话挂接的 mod，然后实现最小有意义测试：配置两次，K1真实500两次后K2成功，后续读取K1健康6；三种模式候选顺序；429短长冷却；共享短预算不因换K重置；SSE已交付不重发。能复用现有测试入口就复用，不能仅用 fake port 自报成功伪造实际上游请求计数。

健康/重试/config/WS都在并行实现，使用最终落盘helper或暂时假定既定配置字段，不改生产行为绕过验收。有阻塞写契约并报告main，继续独立夹具。所有测试必须只用本地假上游与临时数据，不真实账号/付费模型。你可以运行小型测试/静态检查；gateway cargo由main统一串行，避免并发抢资源。不得提交/全仓格式化。使用 channel scheduler-failover-impl 向 main 报告。
