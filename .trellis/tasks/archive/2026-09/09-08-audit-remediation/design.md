# Design

## Change boundary

最小行为缺口是：SQLite 最终 schema 与导出域/旧 feature/维护脚本分叉，实际钱包与响应分叉，简化策略与已退役维度/保存数据分叉。分别在数据 lifecycle、真实 DTO 组装入口和共享路由 resolver/前端配置 builder 修根因；不在 HTTP 最外层吞异常、不把有效数据强制清空。

## Data lifecycle and driver contract

主 agent 负责 lifecycle/export.rs、export/sqlite.rs 和现有 export/tests.rs，按当前 SQLite schema 修复默认域和辅助表。现有旧域仅作为历史格式标识：若显式请求不再存在的域应返回清晰输入错误；历史导入必须显式校验，不能因删除 enum 丢失可识别性。保留未退役业务表的完整导出（包括没有生产入口但当前仍存在的表），导入走既有事务与标识符白名单。

仅保留 SQLite runtime driver feature；移除激活旧实现的 cfg 分支及未接入文件。历史 DatabaseDriver 标识作为序列化元数据和输入诊断仍可保留，但 SQL 配置不能实例化未实现 backend。CLI 移除未实现 driver/topology 选项。schema 静态生成器的历史 dialect 与运行驱动分离，不能把历史 SQL fixture 等同于 runtime 支持。

## Wallet payload

保留既定双挂钱包和套餐结算。用户资料 handler 用已有用户钱包读取能力取得钱包；payload 从 StoredWalletSnapshot 组装余额、累计值、币种、状态与 limit_mode；无钱包维持既有兼容外形，不能断言所有 Key 的钱包都无限。数据读取失败返回既有错误响应。只修契约失真，不复活支付页面或删除账本。

## Routing semantics

以旧任务 R11 的已确认验收为准：简化 resolver 始终以 Provider 优先模式解析，忽略模型白名单/模型差异/旧 Key overlay，load_balance 归一。完整公共 resolver 的功能不变。必要日志只在 gateway 层输出，不给 routing-core 引入 tracing 依赖。

前端仅编辑自己拥有的调度模式和供应商优先级规则：保留基配置中未退役的规则、条件、阶段、stop_processing 与 header/body mutation；清除明确退役的维度。不把有条件的外部优先级 action 盲目展平成全局优先级。编辑 Key 优先级继续通过实体字段，不保存旧 overlay。对应改造共享 parse/build 边界和页面调用，验证行为往返。

## Ownership and independent work

- 主 agent：数据导出/导入/复制、CLI/数据库配置、钱包摘要/不实注释、整合和最终验证。
- trellis-implement routing worker：frontend（路由保存 + 孤立组件/helper/依赖）及 crates/aether-routing-core、gateway scheduler/config.rs 和 routing 层的 R11 归一化。不得改 main.rs 或数据层。
- trellis-implement data-runtime worker：runtime Cargo.toml、backend、lifecycle/backfill/migrate、build/schema 脚本与相关文档的 SQLite-only 收敛。不得改 lifecycle/export*、gateway main.rs 或钱包摘要；涉及这些文件的必要协作先向主 agent 给出位置。

不引入新基础设施或替代技术栈。保持共享 API 的历史序列化可识别性、实际不支持的操作显式错误。回滚为按功能组撤回本次代码 diff；没有数据迁移或实际数据变更。
