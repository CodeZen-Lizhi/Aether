# Journal - lizhi (Part 1)

> AI development session journal
> Started: 2026-08-31

---



## Session 1: 调度升级编译修复 + 需求清单 + 旧库数据迁移上线
<!-- trellis-session: v=2 fp=0186ba983bc95c0d -->

**Date**: 2026-09-02
**Task**: 调度升级编译修复 + 需求清单 + 旧库数据迁移上线
**Branch**: `slim-personal`

### Summary

修复 345 处编译错误（号池调度/供应商优先级/已删 planner 家族残留，lib+全部测试目标全绿，前端 type-check+build 通过）；建立 docs/requirements-backlog.md（Key 级倍率计费、删除全局正则映射、模型管理批量操作重构、中转站预设收敛，均已确认设计待排期）；完成旧 PostgreSQL(aether-app:8084) → 新 SQLite(aether-personal:18084) 数据迁移：走 admin data export/import，10 供应商/24 端点/21 Key/27 供应商模型/4 全局模型全部导入，21 个 Key 逐个 reveal 解密比对与旧库一致，代理节点自动切直连；aether-personal 容器以最新镜像上线 18084（healthy），AETHER-MAIN 8084 停用保留可回滚。

### Git Commits

| Hash | Message |
|------|---------|
| `ac2296834` | docs: 新增需求优化清单 — Key 倍率计费/全局正则映射删除/批量操作重构/中转站预设收敛 |
| `bd36f6e67` | refactor(scheduling): 调度升级与单用户化收尾 — 修复全部编译错误 |
| `c6fe6c61a` | fix(frontend): RoutingProfiles 改从 components/layout 导入 PageContainer — 修复容器内前端构建 |

### Status

[OK] **Completed**

---

## Session 2: 需求清单四项落地（1/2/3 项开发 + 第 4 项交叉验证）
<!-- trellis-session: v=2 fp=0186ba983bc95c0d -->

**Date**: 2026-09-02
**Task**: 09-02-backlog-four-items
**Branch**: `slim-personal`

### Summary

落地 docs/requirements-backlog.md 已确认需求。期间发现并行会话（Codex/@aether-tunnel）正在同仓实施第 4 项（中转站预设收敛），经用户确认分工：本会话承担第 1/2/3 项（文件集不重叠，逐项精确 stage 提交），第 4 项留给并行会话（其完成后 commit 85a3c4b15）。

- **第 2 项（887b3a860）**：删除全局模型正则映射——调度三处消费（入站别名认领/行支持判定/Key 白名单放宽）、candidate selection 行字段与读侧解析、网关路由预览 global_model_mappings、公共模型列表白名单正则放宽、前端映射 Tab 与 RoutingTab 正则展示、model-mapping-regex 工具全部移除；保留供应商级映射全链路、association_sync、matches_model_mapping。
- **第 3 项（a818c8fb1）**：模型管理批量操作列表化——删"快速筛选与批量操作"面板，表格/卡片行复选框 + 表头三态全选（范围=当前筛选结果），浮动批量栏（同步在线价格 REMEMBERED 来源懒加载 + 批量删除二次确认）。
- **第 1 项（981f06676）**：Key 级默认成本倍率——provider_api_keys.default_rate_multiplier（迁移 20260902000000 + 逻辑 schema + 生成产物），结算链路格式映射未命中回落默认倍率（非法值回落 1.0，free_tier 仍为 0），创建/更新/批量/导入导出全链路透传，KeyFormDialog 新增默认倍率与按格式覆盖配置区，Key 行回落展示默认倍率徽标；新增 4 个结算测试。
- **交叉验证**：cargo test --workspace 仅剩 3 个**预存失败**（candidate/model 两个 codex 映射测试 + claude 流式重写器测试，已用 worktree 在本会话开工前的提交 c3ab653bd 上复现，非本次/第 4 项引入）；前端 vitest 648 全过、vite build 通过。
- **发现的基础设施问题**：`npm run type-check`（vue-tsc --noEmit）对 solution-style tsconfig 是空跑；真正的检查 `vue-tsc -b` 当前有 ~358 个预存类型错误（api client/端点层为主），建议后续单独立项修复并把 type-check 脚本改为 `-b`。

### Git Commits

| Hash | Message |
|------|---------|
| `887b3a860` | refactor(models): 删除全局模型正则映射 — 调度零消费 + 映射 Tab 与正则展示清理 |
| `a818c8fb1` | refactor(models): 模型管理批量操作列表化 — 复选框/三态全选/浮动批量栏替换面板 |
| `981f06676` | feat(billing): Key 级默认成本倍率 — 结算回落 + Key 表单倍率配置入口 |

### Status

[OK] **Completed**
