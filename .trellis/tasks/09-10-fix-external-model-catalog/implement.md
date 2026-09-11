# 实施计划

- [x] 1. 在后端外部目录拉取边界补失败分类和有限重试，保留现有缓存/代理隔离契约。
- [x] 2. 先添加后端回归测试：瞬时失败后成功、不可重试错误、重试耗尽。
- [x] 3. 在 `models-dev.ts` 增加带来源状态的目录读取，普通加载允许兼容旧缓存兜底，强制刷新保持严格。
- [x] 4. 先添加前端 API 测试：旧缓存兜底、严格刷新不回退、成功刷新覆盖缓存。
- [x] 5. 更新创建模型弹窗的 loading/stale/error/empty 状态与重试操作，并补组件测试。
- [x] 6. 运行受影响的 Vitest、`npm run type-check`、`npm run build`、`git diff --check`。
- [x] 7. 通过 Docker Rust 工具链运行格式检查和聚焦测试。
- [x] 8. 使用真实浏览器在 `/admin/models` 验证正常、失败、旧缓存三种创建弹窗状态；严格同步行为由组件回归测试验证。

## 风险文件与回滚点

- `apps/aether-gateway/src/handlers/admin/model/external_cache.rs`：外部请求时序；可独立回滚重试包装。
- `frontend/src/api/models-dev.ts`：共享目录缓存语义；必须用 API 单测锁定严格刷新与普通读取的差异。
- `frontend/src/features/models/components/GlobalModelFormDialog.vue`：创建弹窗状态；不得影响编辑模型表单和提交。

## 验证命令

```bash
cd frontend && npm run test:run -- src/api/__tests__/models-dev.spec.ts src/features/models/components/__tests__/GlobalModelFormDialog.prefill.spec.ts
cd frontend && npm run type-check
cd frontend && npm run build
cargo fmt --all --check
cargo test -p aether-gateway external_models -- --nocapture
git diff --check
```
