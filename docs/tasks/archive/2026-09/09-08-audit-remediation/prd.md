# 审查问题修复：单用户与 SQLite 一致性

## Goal

把当前会话审查中已经确认的问题落实为可验证的代码修复，并清理已证明孤立的前端代码。用户已明确要求“直接改代码，把这个改好为止，开启 trellis”，已授权建立任务及直接实施，不再重复请求规划或实施确认。

## Background

审查基线 7692fde7d，完整报告位于当前会话输出目录 aether-readonly-audit.md。代码仓库初始干净。审查归纳 F01–F05，下面按真实产品约束修复，不能把候选表当作可直接删除的数据。

已复核的产品边界：归档 09-01-single-user-cleanup/prd.md 明确保留 wallet 结算、用户/session、management_tokens 和核心表；09-01-scheduling-upgrade/prd.md 的 R11 明确要求旧模型策略、RestrictModels、GlobalKey、SetKeyPriority 在简化配置读取时退役，Key 实体优先级继续生效。

## Requirements

- R1 / F01：在最新 SQLite 迁移后的数据库上，默认 `aether-gateway export` 和 `aether-gateway copy`（含 omit-request-body）成功，不访问已删除 OAuth/用户组/LDAP 表；当前业务表数据完整保留。历史导出文件的兼容必须有明确行为，不能默默丢失记录或假成功。
- R2 / F02：实际可用的 SQL 驱动和 CLI/构建能力统一为 SQLite；旧 Postgres/MySQL 空 feature 不得激活不完整代码；不继续对外宣称可用的 multi-node SQL 部署。历史 JSON 中 driver 标识可保留用于读取元数据，不能让其成为已实现的数据库连接能力。
- R3 / F03：用户钱包摘要反映当前钱包及真实 limit_mode，查询失败必须走既有错误链路；保留实际钱包/套餐结算与统计，不再无条件声称所有 Key unlimited。
- R4 / F04：简化路由按已确认 R11 约束忽略旧模型限制、GlobalKey、Key overlay，继续使用 Key 实体优先级；保存页面时不丢失未退役的请求头/请求体等规则及条件语义。旧 load_balance 继续归一为 cache_affinity。公开完整 resolver 保留其原本语义。
- R5 / F05：schema 维护命令、构建输入和 runtime 文档以实际 SQLite-only 实现为准，检查不引用已经不存在的 adapter 文件；修正 auth_modules 的不实注释。不得通过改写已发布迁移修正说明。
- R6 / 孤立代码：删除已验证没有消费者的 StripePaymentDialog 及公告前端 helper，清理相应导出与仅由其使用的依赖/资源；真正仍被构建、鉴权、后台或历史兼容使用的对象保留。
- R7 / 验证：执行受影响包的类型/编译检查、直接相关的现有回归测试、SQL/代码审查与 git diff --check；记录实际结果、未覆盖的现场数据和回滚边界，不以文本搜索替代运行验证。

## Acceptance Criteria

- [x] R1：SQLite 最新迁移后的默认导出/导入/复制回归通过，覆盖当前表清单及 omit-body 分支；旧域不触发缺表查询。
- [x] R2：实际 runtime 的受支持 feature 组合可编译；CLI 不再接受/展示未实现的 SQL 驱动或多节点运行配置；旧元数据可解析且错误明确。
- [x] R3：现有有限/无限钱包摘要和无钱包情况正确，错误路径不吞异常；结算相关现有回归没有被移除或失效。
- [x] R4：旧 RestrictModels/GlobalKey/SetKeyPriority 不再影响简化决策，实体 Key 优先级保留；保留规则 round trip 和旧 load_balance 回归通过。
- [x] R5：schema check 可针对当前仓库结构执行；不再产生无用 PostgreSQL runtime 构建输入；相关说明准确。
- [x] R6：孤立前端组件/helper 和无用导出不再存在，前端类型与相关测试通过。
- [x] R7：范围内明确缺陷已修复，最终 diff 审查、校验记录及 Trellis spec/session 收尾完成。

## Constraints

不连接或改动用户实际数据库，不新增 DROP 迁移，不改写历史迁移，不撤销用户既有改动，不提交或 push。payment_gateway_configs/referral_rewards 等仅存量候选表保留；其实际数据和仓库外消费者仍待现场核验，不能用代码审查替代。不得新增测试文件或临时测试代码；优先现有测试文件/用例及编译校验，必要时调整已有回归以覆盖本次契约。后端测试单次设置 60 秒超时；单次校验上限 120 秒，不跑全仓测试、全量构建或启动浏览器。

## Completion evidence

已完成代码修复和定向验收；完整命令、结果、审查与验证盲区见 [verification.md](verification.md)。后续已按用户明确授权提交修复：`ad7ac23ab199c736b9250e7895efa39bf10a17c9`；未操作真实数据库。
