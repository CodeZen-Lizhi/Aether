# 参与贡献

感谢你愿意改进 Aether。项目当前以个人单机和小团队自托管为核心，提交改动时请优先保持部署边界、数据安全和现有交互语义。

## 开始之前

- 先搜索现有 Issue 和 Pull Request，避免重复工作。
- 行为变化、公共接口、数据模型、权限、并发或依赖调整，请先开 Issue 说明目标、影响和验证计划。
- 开发前阅读 [项目 Spec 索引](docs/specs/index.md)，再按实际范围读取对应正文。
- 安全问题不要公开提交；请按 [SECURITY.md](SECURITY.md) 私下报告。

## 开发环境

基础要求：

- Rust 工具链，版本以 `rust-toolchain.toml` 为准。
- Node.js 20.19+ 或 22.12+。
- macOS 桌面构建需要 macOS 14+ 和 Xcode Command Line Tools。

安装依赖并启动开发服务：

```bash
npm --prefix frontend ci
npm --prefix apps/aether-desktop ci
make dev
```

## 改动原则

- 保持变更聚焦，沿用现有架构、命名、样式和错误语义。
- 新增或修改具名函数、类型、字段和业务常量时，补充准确的中文说明。
- 优先复用已有组件、API、查询和工具，不复制一套平行实现。
- 外部输入、权限、凭据、持久化和跨协议行为按实际风险验证。
- 不提交真实上游供应商、密钥、余额、请求内容、数据库、日志或导出的备份。
- 界面截图必须使用合成数据；确需展示真实环境时，先完成脱敏。

## 验证

按改动范围选择最小但有效的检查：

```bash
npm --prefix frontend run type-check
npm --prefix frontend run test:run

cargo fmt --all -- --check
cargo check -p aether-gateway
cargo test -p aether-gateway --lib

npm --prefix apps/aether-desktop test
```

涉及页面、键盘操作、布局、下载、持久化或跨层链路时，仅运行编译或单元测试不足以证明功能完成。请给出实际运行入口、操作、结果和未覆盖边界。

## 提交与 Pull Request

- 提交信息应说明实际行为变化，推荐使用 `feat:`、`fix:`、`docs:`、`refactor:`、`test:`、`chore:` 前缀。
- 一个 Pull Request 尽量只解决一个问题，避免混入无关格式化或重构。
- 描述中写明背景、改动、验证方式和风险。
- 修改界面时附合成数据截图，并覆盖相关窄屏、长文本、空态和失败态。
- 提交前确认未带入密钥、个人数据、构建产物或临时调试文件。

## 许可

向本仓库提交贡献即表示你同意该贡献按仓库当前的 [Aether 非商业开源许可证](LICENSE) 分发。
