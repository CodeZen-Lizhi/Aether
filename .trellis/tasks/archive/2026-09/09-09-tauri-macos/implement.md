# 实施与验证

## 变更边界

差距是现有 Web/CLI 网关缺少桌面宿主与受控本机生命周期。新增桌面包、内置启动页和打包脚本，复用现有管理业务。2026-09-09 用户进一步确认个人客户端打开即用，新增桌面专用会话初始化与恢复，替换首启账号表单。该行为跨宿主 `gateway/process/windows`、网关身份初始化/会话路由与前端 auth/router/设置入口；不修改代理 API Key 鉴权、计费、路由规则，也不免除 Web/Docker 登录。

## 顺序

- [x] 检查基线；从 `0942819e8` 创建 `codex/tauri-macos` 和独立 worktree。
- [x] 完成 PRD、设计、实现计划与资料清单；本轮授权已明确包括实施直至完成。
- [x] 网关：回环地址参数、信号/父进程退出、连接及用量排空与必要回归测试。
- [x] 前端：桌面启动/设置页、类型化 IPC、首启表单/运行状态/错误恢复/日志与自启动设置。
- [x] 宿主：Tauri 配置、sidecar 路径、进程状态机、Keychain、菜单/窗口/单实例/自启动。
- [x] 集成前端资源、外部导航、导出下载、构建脚本和安装包。
- [x] 运行受影响自动检查，修复本次问题；代码审查。全仓格式检查的基线问题单列在下方。
- [x] 隔离数据与模拟上游执行 CLI/API 生命周期及流式验证。
- [x] 启动实际 `.app` 验证原首启/登录/主要路由/导入导出/菜单/持久化/退出。
- [ ] 按用户新确认的打开即用体验完成身份初始化、会话恢复与界面调整。
- [x] 移除独立“客户端设置”入口/页面，将配置并入系统设置、生命周期控制并入后台顶栏。
- [ ] 重验受影响鉴权边界、旧数据兼容、原生首启/会话恢复并重新打包。
- [ ] 完成安装包、使用文档、规范与验收记录；按需求逐项审计。

## 并行边界

遵循项目 Phase 2.1 的 implement 子 agent 流程：gateway agent 负责网关/用量退出，后续负责桌面模式身份初始化与会话端点；frontend agent 只改 `frontend/` 的桌面入口、会话 adapter、auth/router/UI、测试与必要 i18n；主 agent 负责 `apps/aether-desktop`、根 Cargo/lockfile、集成、产物与最终检查。子 agent 不递归委派，所有命令使用独立 worktree；Rust 构建协调执行，避免多份重型编译挤占内存。

## 检查

- Rust：`cargo fmt --all -- --check`；受影响 crate 的 targeted tests；`cargo check -p aether-desktop --locked`；必要 clippy。
- Vue：`npm run type-check`；受影响路径 `npx eslint ...`；桌面与必要现有登录/API/路由测试；`npm run build`。
- 打包：桌面脚本构建本机 sidecar、Tauri `.app`/`.dmg`；检查 bundle 内资源和二进制，验证签名、启动。
- 行为：回环绑定、冲突恢复、单实例、首次设置和会话续期、模拟渠道/API Key代理（JSON与SSE）、统计持久化、重启、普通退出、宿主死亡退出、前端8个管理页和文件交互。

## 恢复与操作边界

最初在独立 worktree 实施，不访问生产数据。测试记录只在测试目录/测试 Keychain service。不得删除、终止无关进程。2026-09-09 用户已追加授权同步共享功能，并明确要求提交全部现有客户端代码、push 到 `origin/codex/tauri-macos`，再将主项目目录 `/Users/zhenglizhi/otherProjects/Aether` 切换到该分支。原独立 worktree 保留在已提交版本，后续分支开发使用主项目目录；安装包重建、原生验收和公开发布分别按原任务及对应授权处理。任何无法实测的内容明确标记，不能用窄范围测试代替验收。

## 2026-09-09 检查证据

环境：Apple Silicon、macOS 27.0、CLT SDK 27、Rust/Cargo 1.95.0、Node 25.2.0。最低支持 macOS 14；旧版 macOS 和 Intel 尚未实测。

### 已通过

- Vue：类型检查、受影响路径 ESLint、生产构建；桌面 27 项测试以及既有 i18n/API/auth 17 项测试。浅色、深色、窄窗口浏览器截图已检查；这些结果不替代 WKWebView 验收。
- 最新 Rust 二进制测试：desktop 16、gateway 70（含 HTTP/2 超长请求退出）、已有 WebSocket probe 4，全部通过。日志 `/tmp/aether-desktop-final-tests.log`。
- 用量 drain 5、终态 handoff 2、WebSocket 生命周期 7、同步取消 1、SSE 取消 1 的定向回归通过。桌面 clippy `--all-targets --locked -- -D warnings` 通过；构建脚本 2 项测试通过。
- 本次 23 个 Rust 文件的 `rustfmt --check --config skip_children=true` 通过。全仓 `cargo fmt --all -- --check` 发现 5 个未修改的基线文件有格式差异；已逐文件与 `HEAD` 比对一致：`model_test.rs`、`provider_query.rs`、`aether-billing/src/pricing.rs`、`aether-data/contracts/src/repository/billing/types.rs`、`aether-scheduler-core/src/health.rs`。未混入无关格式修复。
- 正式 `.app/.dmg` 构建通过。`codesign --verify --deep --strict`、DMG checksum 通过；只读挂载镜像后，应用内 121 个文件逐一 SHA-256 比对一致，包含 Applications 链接，已卸载。两个 Mach-O 均为 arm64，动态库依赖仅系统库；最低系统版本元数据为 14.0。
- 把正式应用复制到工作区外，同时启动 4 个宿主，仅 1 个存活、3 个退出码为 0。测试目录 `/private/var/folders/fg/bzpd9ft96g976xqf_w4lwbrr0000gn/T/aether-desktop-native-dxe_o3e0/`，应用 `release/Aether.app`，数据 `data/`。
- 直接使用上述正式应用内的网关和 Web 资源执行 `qa_lifecycle.py`：隔离 SQLite 路径包含空格、中文、`#`；创建模拟渠道、加密上游 Key、模型与客户端 Key；JSON/SSE 代理、HttpOnly Cookie 续期、配置导出及 skip 导入均通过；8 个管理路由均返回随包 SPA。
- 同一次正式二进制测试：stdin EOF 保留 1 秒在途 SSE 并退出（1.09 秒）；SIGTERM 在 3 秒测试预算内取消 30 秒流（2.77 秒）；杀死持有 stdin 的测试父进程后网关退出（0.06 秒）。数据库保留 4 条 usage、4 条 settlement snapshot、4 条 HTTP audit。测试没有访问外部上游。日志 `/tmp/aether-desktop-release-lifecycle.log`，原始数据 `/private/var/folders/fg/bzpd9ft96g976xqf_w4lwbrr0000gn/T/aether-cli-lifecycle-4jl2dca7/`。

### 原生界面与最终宿主已验证（免登录调整前）

- 从工作区外正式应用完成首次设置、登录、8 个管理页面、模拟渠道普通/流式调用及用量可见、完整 API Key 复制粘贴、配置导出下载及系统文件选择器导入。外链在默认 Chrome 打开，测试标签已关闭；Finder 正确打开数据/日志目录。
- 菜单重启与再次打开保持既有登录；发现根路由允许停留登录页，宿主改为 `/admin/dashboard`。已有设置窗口重新激活时保持原窗口，`Reopen` 只在无可见窗口时打开后台。
- 开启/关闭独立 QA 自启动项，确认参数包含 `--background --data-dir` 且关闭后 plist 删除。关闭窗口后 1 秒与 12 秒流请求正常完成。
- 正常退出期间：停止信号到连接截止 18.004 秒，符合 20 秒总预算；释放端口、退出宿主并删除 activation socket。请求总时长包含退出前等待，不能用作退出耗时。正式宿主被 SIGKILL 后 0.06 秒内网关退出，没有孤儿。
- 真实 Keychain roundtrip 与完整宿主集成两个 ignored 测试均按需执行通过；同一 access token 在新 Gateway 实例及重启后仍能使用。日志 `/tmp/aether-desktop-keychain-test.log`、`/tmp/aether-desktop-host-integration.log`。
- 缺失密钥 fixture 的原生窗口显示“检测到已有数据库，但钥匙串中缺少原加密密钥…不会替换原密钥”。数据库 SHA-256 未变、Keychain 条目仍不存在、51006 未监听；测试宿主已退出。
- 最终原生实测副本为 QA 根目录 `final/Aether.app`。最新桌面 clippy、深度签名与 DMG checksum 均通过；日志 `/tmp/aether-desktop-final-clippy.log`。

### 打开即用调整的追加验收

待完成：新装不创建账号、不出现登录页；既有 desktopqa 数据身份与密码哈希保持不变；启动与 Cookie 过期/撤销后自动建立普通会话；连接失败可重试且不循环。随机桌面凭据不落盘、不进入 URL/日志；没有凭据的浏览器、伪造 Origin/Host 与普通 API 请求不能获得管理权限。网关重启轮换临时凭据，原 Keychain 密钥和代理 API Key 保持不变。最后重新构建并用新产物验证。

追加自动检查已通过：桌面模式后端 16 项、既有 Web login/refresh/logout 3 项、gateway bin 71 项、desktop 17 项（2 项按需集成测试待新产物）、desktop all-targets clippy、Node 构建/会话脚本 6 项。日志前缀 `/tmp/aether-passwordless-`。

Gateway 严格 Clippy 发现已核对为基线的 6 处告警：`aether-scheduler-core/src/health.rs:656` 的 `manual_range_patterns`；`ai_serving/planner/standard/openai/chat/decision/request.rs:645,646` 两个 `duplicated_attributes`；`ai_serving/planner/candidate_preparation.rs:31` 的 `manual_inspect`；`execution_runtime/sync/execution.rs:2135` 的 `never_loop`；`handlers/admin/observability/monitoring/cache_affinity_reads.rs:204` 的 `bind_instead_of_map`。前四个涉及的未改文件已与 HEAD 整文件比对相同；sync 文件仅新增另两处 handoff，告警 loop 整段与 HEAD 一致。没有修改这些基线，也不能宣称完整严格 Clippy 通过。进一步以 `--no-deps` 且仅对上述四类 gateway 基线 lint 作命令行豁免，其余规则和 bin 检查通过；没有改项目 lint 配置。全部本次 37 个 Rust 文件定向 rustfmt、`git diff --check` 通过。

追加审查修复：窗口创建/代次凭据与启停串行，异步 destroy 后新窗口使用唯一 label，watcher 复核失败状态，Reopen 不在 UI 线程等待生命周期锁；前端接受真实响应 `email:null`，桌面固定同源且不继承 Web API 地址，并处理失效请求与显式 retry 的竞态。

### 产物与验收状态

本机构建复用 `CARGO_TARGET_DIR=/Users/zhenglizhi/otherProjects/Aether/target`：

- `/Users/zhenglizhi/otherProjects/Aether/target/release/bundle/macos/Aether.app`
- `/Users/zhenglizhi/otherProjects/Aether/target/release/bundle/dmg/Aether_0.1.0_aarch64.dmg`

产物使用 ad-hoc 签名，没有 Apple 公证；不包含公开发布。安装和开发说明见 `docs/desktop-macos.md`。

| 验收项 | 当前证据 | 状态 |
| --- | --- | --- |
| AC1 | 原正式安装包、完整资源、工作区外宿主及页面已验证 | 追加免登录产物待构建 |
| AC2 | 端口恢复、原首启、真实并发单实例已通过 | 新首启与会话恢复待验证 |
| AC3 | WKWebView 8 页、API/JSON/SSE、用量及文件/浏览器交互通过 | 按新界面重验受影响路径 |
| AC4 | 真实关窗、菜单重启/退出、长流预算、宿主死亡均通过 | 已通过，追加版本做必要回归 |
| AC5 | 真实 Keychain roundtrip、保留会话、缺密钥原生故障均通过 | 追加旧数据无损切换待验证 |
| AC6 | 已有受影响检查通过；5 个全仓格式差异确认为原有 | 追加鉴权检查待执行 |
| AC7 | 安装/恢复文档与原生证据已补全 | 待新行为与产物同步 |
