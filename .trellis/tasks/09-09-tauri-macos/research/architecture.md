# 代码与官方资料

- `frontend/src/router/routes/admin.ts`：8 个管理路由；独立 desktop.html 入口保留原入口和业务路由。
- `frontend/src/api/client.ts`：相对同源 API、Bearer access token、HttpOnly refresh Cookie；本机 HTTP 管理 WebView 与静态文件同源可保留该契约。
- 改造前的 `apps/aether-gateway/src/main.rs`：固定 IPv4 all-interfaces、单独 hyper listener 任务、未注册退出信号；`--static-dir` 已支持 SPA 静态资源。本次新增可配置监听与统一退出链。
- `crates/aether-runtime/base/src/shutdown.rs`：已有 SIGINT/SIGTERM wait helper 可复用。
- `crates/aether-data/adapters/sqlite/src/pool.rs`：文件数据库使用 WAL，备份不能只复制活跃主文件。
- Tauri 2 sidecar：https://v2.tauri.app/develop/sidecar/ ，`bundle.externalBin` 及目标架构后缀。
- Tauri process model：https://v2.tauri.app/concept/process-model/ ，macOS 使用系统 WKWebView。
- Tauri menus：https://v2.tauri.app/learn/window-menu/ ，支持原生应用菜单和系统托盘菜单。
- Tauri autostart：https://v2.tauri.app/plugin/autostart/ ，桌面插件提供 LaunchAgent 注册和状态查询。
- Apple Keychain：https://developer.apple.com/documentation/security/keychain-services ，用于小型持久化密钥。
- Apple notarization：https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution ，对外 Developer ID 分发需要签名、公证及相应开发者凭据；本地验收不伪造公证状态。

## 实施中收敛的结论

- Tauri `data_store_identifier` 在 macOS 14+ 才支持独立持久存储，因此最低版本从初稿的 13 调整为 14；后台窗口按 canonical 数据目录派生标识。
- `tauri-plugin-single-instance 2.4.4` 的 macOS socket 检测与异步 bind 之间存在冷启动窗口，改用初始化前同步 `flock`。实际同时启动 4 个正式宿主，1 个存活、3 个正常退出。
- 首启后的真实账号验证必须带 `X-Client-Device-Id`，与现有前端登录契约一致。`POST /api/auth/refresh` 必须无请求体；集成脚本不能发送 `{}`。
- 个人版在用户确认后取消账号创建/登录表单；复用现有会话响应，桌面临时 capability 仅换取普通会话。Keychain JWT/加密密钥仍留在 Rust 侧。Tauri 2.11.5 官方 crate 的 `WebviewWindowBuilder::initialization_script` 在页面脚本前、仅主 frame 执行；仍按官方建议准确检查 origin，另显式排除子 frame。该 API 的版本源码为 `tauri-2.11.5/src/webview/webview_window.rs`；Context7 查询的官方文档也确认远程 IPC 依赖显式 capabilities，本实现不给后台窗口任何远程 IPC。
- macOS 27 / CLT SDK 27 / Rust 1.95.0 环境中，release `strip=true` 的 `serde_derive` 宏动态库发生 `mis-aligned LINKEDIT string pool`。同一 `aether-contracts` 构建在默认宏裁剪下失败，设置 `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=none` 后通过；完整 `.app/.dmg` 构建也通过。配置仅作用于构建期依赖，不把过程宏打进安装包。Cargo 参考：https://doc.rust-lang.org/cargo/reference/profiles.html#build-dependencies
- 新增 Tauri 跨平台 lockfile 依赖时，`proc-macro-crate 2.0.2` 精确依赖旧 `toml_edit`，使原有 `toml 0.8.23` 降级。通过 `cargo update -p proc-macro-crate@2.0.2 --precise 2.0.0` 和 `cargo update -p toml@0.8.2 --precise 0.8.23` 恢复原版本；没有手改 lockfile。该宏依赖来自 Linux GTK 链，macOS 桌面不编译此链。`serde_json 1.0.151` 满足新增 `serde_with` 的最低约束，保留。
