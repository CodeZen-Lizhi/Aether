# 直接进入仪表盘验证

## 原因与修改

设置窗口原来以 `visible(!background || !configured)` 创建，普通启动必然先显示；仪表盘打开后才隐藏。改为初始隐藏且不聚焦，网关在后台初始化后直接打开仪表盘。

初始启动用 `StartupState` 记录前台打开意图，Dock、菜单和第二实例的早期激活通过相同 `open_dashboard` 入口延后处理，完成初始化时原子交接。没有将锁持有到网关或原生窗口操作中。后台自启不主动打开窗口，显式设置入口和失败恢复仍然保留。

独立审查另外修复退出竞态：初始化线程检查退出标志后仍可能被 Quit 抢占，随后出现迟到的设置窗口回退。`show_launcher` 统一检查退出状态，避免该回退重新显示或聚焦设置。

## 检查

- `cargo test -p aether-desktop --locked`：最终 0.1.2 源码 21 passed、0 failed、2 ignored；包括前台启动、后台启动、早期重复激活与激活/初始化完成并发的 4 项新回归。两项显式依赖 Keychain 的旧集成测试未运行。
- `cargo fmt -p aether-desktop -- --check` 与 `git diff --check` 通过。全工作区格式检查发现未修改的网关/计费等文件存在既有格式差异，未在本任务扩展修改。
- 独立源码审查通过。前端与网关源码未变；生产前端、网关和 Tauri 构建通过。
- 包内 `CFBundleShortVersionString`、`CFBundleVersion` 均为 `0.1.2`。打包使用干净源码提交 `9c3e53ceef5de326b8f86bb19cf5506a4e88ab2a`，构建锁正常释放。
- 包内 web 与本次 dist 完全一致，深度严格签名和 DMG 校验通过；只读挂载后的 117 个包内文件与原 app 一致。
- 网关二进制 SHA256 仍为 `7ed710f0005130cc1050c632958811e77493643c5b412577f3eaf5055e306ac7`，复用此前完整生命周期证据。
- 原生启动实测通过：用户确认退出当前客户端后，将实际 release app 复制到仓库外临时目录，使用隔离数据运行，并连续采样该测试进程的可见窗口。未操作真实客户端数据。
- 普通启动及早期重复激活：最初无可见窗口，约 2.38 秒后首个窗口为 `Aether` 仪表盘窗口；整个采样期间没有出现客户端设置窗口。
- 后台启动及重开：网关就绪后继续检查 2 秒，没有可见窗口；向测试实例自己的激活 socket 发出请求后打开仪表盘，未显示设置窗口。
- 失败恢复：让临时端口被本测试占用，网关启动失败后按预期显示 `Aether · 客户端设置`，验证隐藏初始窗口未掩盖故障。
- 每轮测试后结束自己创建的宿主，确认其网关释放端口；临时 profile 的专用 Keychain 条目已删除。测试事件保存在日志目录的 `native-startup-verification.json`。

## 安装包

- 代码提交：`9c3e53ceef5de326b8f86bb19cf5506a4e88ab2a`，已仅 push 到 `codex/tauri-macos`。
- 安装包：`/Users/zhenglizhi/Downloads/Aether_0.1.2_aarch64_20260910_9c3e53cee.dmg`。
- 版本 `0.1.2`；arm64，macOS 14+；本地 ad-hoc 签名，未 Apple 公证。
- 大小：31,529,729 字节。
- SHA256：`3b74b85baf4a9b7553bb1a2e79c6a2dfb14fbf833283fda29b34bbda11bcc0d1`，下载目录副本再次校验一致。
- 本次日志及隔离验证脚本目录：`/var/folders/fg/bzpd9ft96g976xqf_w4lwbrr0000gn/T/aether-direct-dashboard-rzow1ehf/`。
