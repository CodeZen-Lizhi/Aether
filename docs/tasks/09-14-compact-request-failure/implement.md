# 实施与验证计划

1. 加载网关、usage 和共享错误/取消规范，基于现有复现添加最小回归；确认原实现确实失败。
2. 修复 Responses 非空快照结果识别，验证 text.done、compaction item.done 与正常 delta 的延迟 completed 场景。
3. 在现有请求终态所有权边界修复总预算取消；验证失败后读回 usage、candidate 和资源释放，不新增独立状态机。
4. 保留超时真实分类，补首部/首帧/有效输出的脱敏诊断。
5. 执行对应 aether-gateway 测试过滤：真实 stream public Router 场景、chat_retry 总预算取消场景；编译/格式只检查修改包。
6. 用 output/compact-timeout-diagnosis/reproduce.py 选择新构建二进制做隔离对照：早 text.done、早 compaction.done、正常 delta 后长完成、纯心跳超时、总预算504终态、EOF失败。生产请求不重放。
7. 对取消/持久化变更做专门 review 与限定 verifier，更新相关契约，提交可审阅 diff。未授权提交/发布，不执行。

命令基线：cargo test -p aether-gateway --lib <已确认实际测试过滤词>；cargo fmt -p aether-gateway --check。具体测试名由所新增用例落定，必须确认执行非零目标测试。不跑全仓测试。

启动前：用户审阅最新方案；实现/check manifest 有真实条目；task.py start 后再派发实现。需求或方案实质变化时回到审阅。
