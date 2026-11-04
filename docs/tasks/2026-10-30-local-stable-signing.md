# macOS 本地稳定签名

日期：2026-10-30

## 问题

桌面版每次重新构建使用新的 ad-hoc 签名。macOS 登录钥匙串中的通用密码项保存的是代码指定要求；ad-hoc 指定要求由代码哈希组成，重建后不再匹配，应用读取网关密钥时再次要求输入登录密码。

## 处理

- 新增 `signing:setup`，在登录钥匙串创建并复用 `Aether Local Signing` 自签名代码签名身份。
- 初始化成功后写入被忽略的 `apps/aether-desktop/signing.local.json`，桌面构建自动加载。
- 显式 `--config` 继续覆盖本地签名配置，Developer ID 分发流程不变。
- 不修改现有钥匙串密钥、数据目录、网关密钥格式或已安装应用。

## 验收

- 新脚本单元测试通过。
- 初始化脚本能在当前用户登录钥匙串创建证书，私钥仅授权 `codesign` 使用。
- 两次 Tauri 打包使用同一身份时，应用指定要求为固定证书根而非变化代码哈希。
- 首次切换签名后的启动需要允许一次钥匙串访问；选择“始终允许”后，同一身份的后续重装不再提示。

## 验证结果

- `npm --prefix apps/aether-desktop test`：20 项通过。
- `build:verified`：0.1.79 arm64 APP 与 DMG 构建成功，桌面脚本、随包生命周期和产物签名检查共 3 项通过。
- 已安装 0.1.77 的指定要求为 ad-hoc `cdhash`；候选包 0.1.79 为固定证书根 `d56142b9ad7f726f9f8bc007ddb84d8520b20905`，签发身份为 `Aether Local Signing`。
- DMG：`target/release/bundle/dmg/Aether_0.1.79_aarch64.dmg`，SHA-256 `28a2aeca68848d50229b30b0d746620deed52474932723a4761b7980817bfe1d`。
- `spctl` 按预期拒绝该自签名、未公证包；它仅用于本机，不用于网络分发。
- 未替换正在运行的 `/Applications/Aether.app`，原生窗口启动未复验；随包网关生命周期检查通过。
