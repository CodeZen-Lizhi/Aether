

## 本地 Spec 入口

开发前从 [docs/specs/index.md](docs/specs/index.md) 找受影响包和对应业务契约；共享审查与验证规则按链接读取。初始化后补齐或维护规范时使用 [Spec 维护指南](docs/specs/guides/spec-maintenance.md)。

## 本地规范与交付

开发与审查前读取 [项目规范索引](docs/specs/index.md)，再按实际范围读取正文，并应用全局指令中的共享工程规范。开发流程按全局 `delivery` 及任务复杂度执行；独立审查使用对应 Review Skill。

任务计划与结果使用 `docs/tasks/`；历史日志在 `docs/worklog/`。旧任务中的 JSON/JSONL、状态与命令只作历史参考，不再驱动框架脚本。保留当前项目约定与用户已有改动。

行为验收按全局共享门禁执行；私有清单和机器证据放在 `.cache/delivery/` 或临时目录，不属于需归档的任务报告。桌面候选包使用 `verify:delivery`，构建并验收使用 `build:verified`，参数见 [桌面验证入口](docs/desktop-macos.md#交付门禁)。普通 `build` 只证明构建成功，不能替代本次功能验收。
