# Implement — 修复预存质量债

## Phase A — 失败测试（✅ 完成，commit 43455c6eb）

**范围修正**：原估 3 个预存失败，实际启用 `--no-fail-fast` 后 gateway lib 基线失败 29 个（此前每次全仓跑都被 fail-fast 挡住）。

- [x] 3 个原报失败均为 111929e72（单用户化快照）删除语义后漏改的废弃测试（CodexLive 格式 ×2、ClaudeReadToolSanitize ×1）→ 删除
- [x] 修复需求落地连带失败 7 个（gate ×3 / candidate_source ×2 / 路由预览 / hotrouter 签到遗留）
- [x] gateway lib 失败 32 → 25，差集为空

## Phase B — 前端 type-check（✅ 完成，commits e45ea6d47 / 9ce8221f2 / ae57b8ab2 / b0a6e4e68）

- [x] 系统性根因：ApiClient 默认泛型 unknown → any；handleResponseError 返回类型修正
- [x] 逐文件清理 358 → 0（vue-tsc -b 全绿）
- [x] type-check 脚本从空跑的 `vue-tsc --noEmit` 切换为 `vue-tsc -b`
- [x] vitest 全过、vite build 通过

## 交接给并行会话/后续（非本任务范围）

- gateway lib 预存基线失败 ~25 个（聚类：execution_runtime stream 的 mock SSE JSON 截断 EOF ×16、断言组 ×7、其余）；并行会话另有 "fix(baseline) 全绿" 提交 75f7dd1b2，但其 slim 阶段 3/4 又引入 6 个新失败（auth resolution/data startup/redirect 等）
- 注意：`cargo test` 默认 fail-fast，全仓验证必须加 `--no-fail-fast` 才能看到完整失败面
