# Verified source evidence and product constraints

基线 7692fde7d32ff359ed5bbd38f2ae4cdbe3d9da4b。

- 导出默认域：crates/aether-data/runtime/src/lifecycle/export.rs:739，SQLite 直接 SELECT：export/sqlite.rs:67，辅助清单 LDAP：export.rs:119。DROP：adapters/sqlite/migrations/20260902010000_drop_single_user_tables.sql:6。当前最后迁移恢复 internal_priority，不能覆盖/删除。
- runtime/Cargo.toml:10 只有 sqlite 实际依赖，postgres/mysql 是空 feature。backend/mod.rs 只接 SQLite，但仍有条件引用旧 backend；启用旧 feature 会暴露残缺分支。
- 本地 auth_session.rs:142 固定 unlimited，:326 丢弃真实钱包读取；实际钱包 settlement.rs:457/590 继续读写。旧单用户任务明确不改 wallet 双挂结算，故修摘要而不是删除结算。
- routing core policy.rs:108 简化入口仍执行 :209 RestrictModels、:242 SetKeyPriority，且 GlobalKey 可由 default/actions 流入；这些和已确认 R11 验收不一致。旧 R11 见 .trellis/tasks/archive/2026-09/09-01-scheduling-upgrade/prd.md:79–110、148–149。
- frontend schedulingStrategy.ts:75 builder 重建 rules:[priorityRule]，RoutingProfiles.vue:319 更新并发布，导致 header/body 等未退役配置丢失。现状会保存 key overlay，但旧 R11 要求忽略它并从实体 Key 取优先级。
- build.rs 无条件拼 PG bootstrap，但 lifecycle/mod.rs 已不接 bootstrap。compose_schema.sh:36 仍引用不存在 PG adapter baseline。静态 schema 生成与历史 SQL fixture 应和运行能力分开。
- auth_modules.rs 的“local enabled 仍读取表”注释不实，auth_helpers.rs:6 固定返回 local=true。不要改已发布 DROP 迁移的 checksum。
- StripePaymentDialog 仅 common/index.ts re-export，无业务使用；announcement.ts helper 无外部使用。完整跟踪文件搜索已验证；每次删除前再复核。
- payment_gateway_configs/referral_rewards 是存储候选而非确认安全删除：不执行 DROP，不连接实际数据库。users/session、usage/stats、provider auth_config/Fernet、management_tokens 可信转发和 body refs 回填仍有当前或兼容依赖。
- 34 SQLite migrations 静态最终 77 张业务表、1,356 列、154 显式索引。只做计数并不足以证明 export round trip；需要真实现有 SQLite 回归。

检索注意：普通 rg ignore 会漏掉已经跟踪的 apps/aether-gateway/src/data/** 文件；引用核对使用 git grep/git ls-files 或 rg --no-ignore。绝不能在第一个 #[cfg(test)] 处截断文件，前面可能只是 test import。
