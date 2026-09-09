# macOS 客户端

Aether 桌面版采用 Tauri 2 和现有 Vue 后台，内置 Rust 网关、SQLite 和页面资源。安装后无需 Rust、Node.js 或 Docker。目标系统为 macOS 14 及以上，以保证不同数据目录的 WebView 会话隔离。首版交付 Apple Silicon；Intel 可按下文指定目标构建，实际验证范围以验收记录为准。

## 安装与使用

打开 `.dmg`，把 **Aether.app** 拖入“应用程序”，再从应用程序启动。首次会自动初始化并进入管理界面，无需创建账号或登录。默认网关地址为 `http://127.0.0.1:8084`；其他工具使用在后台创建的 API Key 调用，例如 OpenAI 兼容客户端的 Base URL 为 `http://127.0.0.1:8084/v1`。

菜单栏图标和应用菜单提供打开后台、客户端设置、启动、停止、重启、数据目录、日志目录与退出。关闭窗口会隐藏窗口并保持网关运行；菜单“退出 Aether”或 `⌘Q` 才会停止网关并退出。`⌘,` 打开客户端设置，可修改停止状态下的端口、选择登录自启、查看诊断日志。更换应用安装路径后，重新关闭再开启登录自启以更新路径。

网关仅监听本机 IPv4 回环地址。客户端自动建立与恢复本机会话，保留接口鉴权和 HttpOnly Cookie 续期；普通网页或程序仅知道本机地址不能取得管理权限。桌面界面移除创建账号、登录、账号密码与退出登录入口，保留管理业务和偏好设置。原 Web/Docker 模式的账号登录与默认监听行为保持不变。

外部 HTTP(S) 链接在系统默认浏览器打开。配置导出保存到系统“下载”目录，文件名带唯一后缀，完成后在 Finder 中显示；导入使用原有文件选择器。原生“编辑”菜单支持复制、粘贴、全选等快捷键。

## 数据与故障恢复

默认数据目录：`~/Library/Application Support/com.aether.desktop/`。

| 内容 | 位置 |
| --- | --- |
| 数据库及 SQLite WAL | `aether.db`、`aether.db-wal`、`aether.db-shm` |
| 普通设置 | `desktop.json`，仅版本、端口、初始化状态 |
| 网关日志 | `logs/` |
| 版本升级前快照 | `backups/before-<version>-<id>/` |
| JWT 与数据库加密密钥 | macOS 登录钥匙串，账户 `gateway-secrets` |

钥匙串服务名为 `com.aether.desktop.<数据目录哈希>`，不在普通配置中保存密钥或管理员密码。钥匙串锁定或访问被拒绝会显示错误；已有数据库缺失原密钥时会拒绝启动，避免替换密钥后使渠道凭据无法解密。解锁登录钥匙串、允许 Aether 访问后重试。本地临时签名版本在重新构建后可能需要重新允许钥匙串访问。

从早期需要登录的桌面版本升级时，会复用唯一有效本地管理员的数据身份，原密码哈希、API Key、渠道与用量继续保留。多个管理员或没有有效管理员的非空数据库会明确报错；这种数据应通过逻辑备份迁入新的个人数据目录。客户端不会自动修改原账号权限。

更换 `.app` 会保留数据。应用版本变化时，网关启动和数据库迁移前会在停止状态复制数据库、WAL 和普通配置。恢复物理快照时，先退出 Aether，把当前数据另行备份，再恢复同一快照的整个数据库文件组和 `desktop.json`，并保留**同一数据目录对应的原钥匙串条目**；降级应用应配套恢复升级前快照。

跨机器或从 Docker/源码部署迁入时，优先使用后台“系统设置”的配置/数据导出导入。导出可能包含渠道凭据，须保管好文件。物理数据库单独复制并不包含钥匙串中的密钥，不能据此恢复所有加密数据。桌面版不会自动读取现有 `.env`、Docker 卷或源码部署数据库。

端口被占用时，客户端会报错并允许修改端口，不会终止占用进程。网关意外退出时会显示客户端设置及日志入口，修复后手动重启。正常退出先停止接收新请求，再等待在途请求与用量落库；总预算为 20 秒，超长请求可能在连接等待期限结束后被取消，日志会记录超时。宿主异常退出会通过 stdin EOF 通知受管网关退出。

## 开发与构建

需要 macOS、Xcode Command Line Tools、Rust 工具链，以及满足 Vite 7 要求的 Node.js（20.19+ 或 22.12+）。运行 `xcode-select --install` 安装编译工具；项目 Rust 依赖以根 `Cargo.lock` 为准。

从仓库根目录安装依赖：

```bash
npm --prefix frontend ci
npm --prefix apps/aether-desktop ci
```

开发启动：

```bash
npm --prefix apps/aether-desktop run dev
```

该命令先构建 Vue 和调试版网关，再运行 Tauri。启动页始终使用内置来源，未向 HTTP 开发服务器开放桌面权限。修改前端或网关后重新运行此命令；Tauri 自身的 Rust 文件由开发进程监听。

构建当前机器架构的正式优化包：

```bash
npm --prefix apps/aether-desktop run build
```

默认产物：

```text
target/release/bundle/macos/Aether.app
target/release/bundle/dmg/Aether_0.1.0_aarch64.dmg
```

指定 Intel 目标（需对应 Rust target 和可用的原生交叉编译环境）：

```bash
rustup target add x86_64-apple-darwin
npm --prefix apps/aether-desktop run build -- --target x86_64-apple-darwin
```

显式指定目标后，产物位于 `target/<triple>/release/bundle/`。支持 `CARGO_TARGET_DIR` 自定义输出目录，默认编译并发为 2，可用 `CARGO_BUILD_JOBS` 调整。当前脚本只接受单架构 macOS 目标。

桌面构建保留构建期过程宏的符号，以兼容 macOS 27 的动态库加载检查；这些构建工具不会进入安装包。应用和网关仍采用 release 优化配置。

单独准备资源或构建调试包：

```bash
npm --prefix apps/aether-desktop run prepare:desktop -- --debug
npm --prefix apps/aether-desktop run build -- --debug --bundles app
```

调试隔离目录可用环境变量 `AETHER_DESKTOP_DATA_DIR` 或应用参数 `--data-dir <绝对路径>` 指定；不同目录使用不同钥匙串服务。该诊断实例启用登录自启时也会使用独立名称并携带相同数据目录。应用为单实例，测试前退出其他 Aether 桌面实例。

## 签名与发布

默认配置使用本地 ad-hoc 签名（`signingIdentity: "-"`），可用于本机安装和验证，**没有 Apple 公证**。通过网络分发后的 Gatekeeper 行为与已公证应用不同。对外分发需要用户自己的 Developer ID 证书和 Apple 公证凭据。

可在本机另建配置文件覆盖签名身份，再传给构建脚本：

```json
{ "bundle": { "macOS": { "signingIdentity": "Developer ID Application: YOUR NAME (TEAMID)" } } }
```

```bash
npm --prefix apps/aether-desktop run build -- --config /absolute/path/signing.json
```

公证环境变量按 [Tauri 官方 macOS 签名文档](https://v2.tauri.app/distribute/sign/macos/) 配置，凭据不写入仓库。本次不包含公开发布、自动更新服务或 Mac App Store 上架。

## 验证

前后端契约及本次验收证据记录在 `.trellis/tasks/09-09-tauri-macos/implement.md`。核心自动检查：

```bash
cargo fmt --all -- --check
cargo test -p aether-desktop --locked
cargo test -p aether-gateway --bin aether-gateway --locked
cargo test -p aether-usage-runtime --lib shutdown_tests --locked
npm --prefix frontend run type-check
npm --prefix apps/aether-desktop test
```

`aether-desktop` 编译前需要已构建的前端和对应 target 后缀的 sidecar，首次先运行 `prepare:desktop -- --debug`。真实 WKWebView 的窗口、菜单、上传、下载、外部浏览器和会话行为需要从 `.app` 验证，不能只用普通浏览器页面测试替代。

可用正式应用内的资源运行隔离的 API/退出集成检查：

```bash
python3 apps/aether-desktop/scripts/qa_lifecycle.py \
  target/release/bundle/macos/Aether.app/Contents/MacOS/aether-gateway \
  target/release/bundle/macos/Aether.app/Contents/Resources/web
```

脚本只使用临时 SQLite、合成测试账号和回环地址的模拟上游，检查登录续期、配置导入导出、普通/流式代理、用量、stdin EOF、SIGTERM 和父进程死亡；不会导入现有部署数据。完成后输出诊断目录，测试网关和上游自动停止。

真实钥匙串集成测试需单独执行，会创建并清理一个临时服务条目；若钥匙串要求交互授权，测试会失败并报告原因：

```bash
cargo test -p aether-desktop --locked keychain_roundtrip -- --ignored --test-threads=1
```

另一个按需集成测试直接运行宿主的网关管理代码，验证自动初始化、端口冲突、本机会话与续期、重启更换临时凭据、重启持久化和丢失密钥后保留原数据库。设置 `AETHER_DESKTOP_QA_GATEWAY` 与 `AETHER_DESKTOP_QA_WEB` 为上述随包程序/资源的绝对路径，再运行：

```bash
cargo test -p aether-desktop --locked native_gateway_passwordless -- --ignored --test-threads=1
```

给 `qa_lifecycle.py` 加上首个参数 `--desktop` 可验证桌面模式的自动身份与完整代理链路；默认不加参数继续验证 Web 登录兼容性。测试临时会话凭据只在进程内存和测试子进程环境中传递，不写入诊断文件。
