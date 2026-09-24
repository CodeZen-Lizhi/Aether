# macOS 打包版本验证

## 实现与审查

- 每次 `desktop.mjs build` 在构建锁内分配下一个数字版本，写入五个文件中的六处版本值；patch 和 minor 达到 99 后进位。`dev` 和单独 prepare 保持当前版本。
- 先校验版本一致性，再修改文件；写入失败尝试恢复原值。已经分配的构建失败时保留该版本，正常成功与失败均释放锁。强制杀死进程后的残留锁由人工核实 pid 后处理。
- Tauri 版本覆盖放在调用方配置之后，避免签名配置带入旧版本。脚本自身不执行 Git 操作。
- 独立审查未发现实现缺陷，补充入口隔离测试验证参数、完整构建持锁和失败重试。六个相关脚本 ESLint 与语法检查通过。

## 验证

- `npm --prefix apps/aether-desktop test`：14/14 通过，覆盖连续分配、数字进位、五文件一致、并发排斥、入口配置顺序、dev/prepare 不递增及 prepare/Tauri 失败后保号释锁。
- `cargo test -p aether-desktop --locked`：17 通过、0 失败；两项显式依赖 Keychain/网关路径的集成测试保持 ignored。此轮没有操作真实客户端或数据。
- 前端生产构建与完整 Tauri release 构建成功；`git diff --check` 通过。前端源码未再次改动，沿用前一仪表盘任务的 36 项回归和真实浏览器响应式验收证据。
- 从干净的源码提交构建并再次核对 HEAD，构建期间没有源码变动；实际构建锁已释放。
- Info.plist 的 `CFBundleShortVersionString`、`CFBundleVersion` 均为 `0.1.1`。两个可执行文件均为 arm64，动态库均来自系统目录，最低 macOS 14。
- 包内 web 与本次 frontend/dist 逐文件一致，累计、今日响应、排行、小时趋势及此前修复均存在。
- 深度严格签名、DMG 校验成功；只读挂载后的 117 个包内文件与原 app 一致，Applications 快捷方式正常，挂载已卸载。
- 包内网关 SHA256 与此前通过生命周期验收的二进制完全相同：`7ed710f0005130cc1050c632958811e77493643c5b412577f3eaf5055e306ac7`，复用该证据，没有重新操作真实客户端。

## 交付

- 版本：`0.1.1`；下次正常 build 分配 `0.1.2`。
- 源码提交：`3bcb54b59f1dfc80a20a8b6d11141533d5a268f4`，已 push `origin/codex/tauri-macos`。本任务没有再同步 SLIM。
- 安装包：`/Users/zhenglizhi/Downloads/Aether_0.1.1_aarch64_20260910_3bcb54b59.dmg`。
- 大小：31,536,167 字节。
- SHA256：`89c5eef8650685b687a6faac26cf33681fcdf1edbc2416249b33786bf96114e6`，复制至下载目录后再次匹配。
- 本地 ad-hoc 签名，未进行 Apple 公证。
- 构建、验包日志和逐项结果：`/var/folders/fg/bzpd9ft96g976xqf_w4lwbrr0000gn/T/aether-usage-trend-3t7i6geg/` 中的 `build-version.log`、`version-release-context.json`、`bundle-0.1.1/bundle-verification.json`、`installer-delivery.json`。
