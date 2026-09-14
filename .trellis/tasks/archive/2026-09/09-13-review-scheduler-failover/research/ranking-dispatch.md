Active task: .trellis/tasks/09-13-review-scheduler-failover

你已是 trellis-implement 子 agent，直接实现不要再 spawn。用户已整体批准，in_progress。读产物和 implement.jsonl，独占 crates/aether-scheduler-core/src/ranking/ 与 apps/aether-gateway/src/ai_serving/planner/candidate_ranking.rs 及其必要 serving ranking 调用。实现 R1/R2/R22：人工模式不因低分动态重排；成本同模型/同等兼容等级先比价格倍率再同价亲和；实际亲和读数据接入成本模式，不能只移动比较器字段。不要改 effects.rs（health_policy 独占），写 research/ranking-contract.md 告知健康 owner 成功记亲和和精准解绑需要的接口变化；主会话接线。

优先现有会话身份和目录缓存，避免每K重复查询；仅真实行为所需改动。保留路由分组条件/阶段/请求变更，读 simplified-routing spec。写 targeted core ranking 和 gateway planner 行为测试，证明人工、缓存、成本模式真实候选顺序，而非 tautology。先跑 core 小crate，gateway cargo 由主会话统一，避免资源争抢。不要提交、全仓格式化或付费调用。channel报告结果与效果接线需求。
