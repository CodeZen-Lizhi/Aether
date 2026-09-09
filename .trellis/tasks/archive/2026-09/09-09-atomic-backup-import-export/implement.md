# 执行计划

1. 完成现有读写入口及缓存副作用调查，明确同连接事务接入。
2. 修改前端备份结构检查、DTO 和文案：去除版本限制和部分成功流程，补文件选择、失败和下载回归测试。
3. 修改后端内容规范化与 DTO，兼容实际旧字段，补非版本限制解析测试。
4. 接入 SQLite 原子备份事务，覆盖配置、用户、统计、会话和提交后的缓存失效。
5. 增加并运行真实临时 SQLite 路由测试：跨实例往返、后段失败回滚、版本无关、旧格式和导出失败。
6. 运行受影响 Rust tests、前端 Vitest、vue-tsc、定向 ESLint，审查所有调用方和失败分支。
7. 记录最终契约和验证结果；保留可审阅工作区改动，不提交、不 push、不部署。

## 预期命令

- cargo test -p aether-admin system
- cargo test -p aether-data-sqlite <transaction tests>
- cargo test -p aether-gateway --test admin_unsigned_identity_headers <backup tests>
- frontend 的定向 Vitest / npm run type-check / 定向 ESLint（以 package.json 为准）

## 回退

只回退本任务改动；不修改已发布迁移及用户数据库，不恢复或重新创建已授权删除的旧目录。
