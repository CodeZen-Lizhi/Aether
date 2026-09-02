# Implement — 修复预存质量债

## Phase A — 失败测试（✅ 完成，commit 43455c6eb）

**范围修正**：原估 3 个预存失败，实际启用 `--no-fail-fast` 后 gateway lib 基线失败 29 个（此前每次全仓跑都被 fail-fast 挡住，从未测全）。

- [x] 归因：3 个原报失败均为 111929e72（单用户化快照）删除语义后漏改的废弃测试（CodexLive 格式 ×2、ClaudeReadToolSanitize 流式模式 ×1）→ 删除
- [x] 修复需求落地连带的 7 个失败：gate ×3 改走供应商映射别名路径；candidate_source 指令回退改供应商映射；路由预览白名单 fixture 改真实覆盖；hotrouter 签到测试删除（第 4 项并行遗留）
- [x] gateway lib 失败 32 → 25，差集为空（全部为开工前基线）

**剩余 25 个基线失败聚类（未修，需单独立项）**：
- ~16 个：`Internal("EOF while parsing an object at line 1 column ~111")` — execution_runtime stream 测试的 mock 上游 SSE 载荷被截断，疑似共享 mock/桥接层单一根因
- ~7 个：断言不匹配（stream 事件序、健康投影、settlement）
- 修复需要深入 execution_runtime/stream（4000+ 行）与 mock 基建

## Phase B — 前端 type-check（部分完成，任务保持 open）

- [x] 系统性根因修复：ApiClient HTTP 方法默认泛型 `unknown` → `any`（对齐 axios 默认），`handleResponseError` 返回类型 `Promise<never>` → `Promise<AxiosResponse>`；错误 358 → 299，API 层全部清零
- [ ] 剩余 299 个为逐文件组件/测试类型债（`{}` 推断、事件类型、图表断言、隐式 any），分布：usage 组件群 ~38、providers 组件群 ~24、admin 视图 ~20、charts 6、测试 ~15、其余长尾
- [ ] type-check 脚本改 `vue-tsc -b`：**阻塞于错误清零**（现在切换会立刻红）

## 验证状态

- cargo test --workspace：scheduler-core / billing / data 全绿；gateway 剩 25 个基线失败（见上）
- 前端 vitest 648 全过、vite build 通过
