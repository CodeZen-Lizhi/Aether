# SUB2API 同步分组名

用户要求：同步倍率时，把对应 SUB2API 密钥所属分组名写入本地密钥名称，不使用上游密钥名称。

关联：`docs/specs/aether-admin/backend/quality-guidelines.md`「密钥倍率同步」。原规则仅同步倍率及元数据；本次增加成功同步时使用有效分组名覆盖本地名称，缺失或空白分组名保留原名，失败不改名，其他认证模板不改名。确认依据为本次用户请求。

范围：复用现有解析、同步和条件写入；名称与倍率同次写入，名称变化拒绝在途旧结果。无新增接口、数据库迁移或依赖。

风险：High Risk，扩展既有原子写入及名称基线；认证、计费数值规则、对外接口及原生生命周期不变。安排独立审查与验证。

验收：现有本地 HTTP 同步旅程覆盖分组名与上游密钥名区分、手动/失败保留；SQLite 写后重读覆盖同次保存、旧结果拒绝；解析测试覆盖空白及分组列表回退。验证预算 20 分钟，必要编译包含在内。

状态：代码和隔离业务验收完成。收敛结论：Spec、实现及本次验收证据一致；无需归档移动的单文件记录原位完成。

实际验证：门禁 3 项共 5 个测试通过，耗时 158.649 秒。真实本地 HTTP 验证分组名而非上游密钥名、手动/未匹配保留；SQLite 验证名称与倍率写后读取、过期名称基线拒绝及失败保留；解析验证空白名称和同分组列表回退。证据：`.cache/delivery/sub2api-name/evidence/result.json`。独立 reviewer 零发现，verifier 确认所有进程结束、无零测试。已执行 cargo fmt 和 git diff --check。

限制：未连接真实 SUB2API 账号，未做浏览器/桌面页面或安装包验证；当前结果证明后端隔离 HTTP 与 SQLite 行为，不代表已安装桌面应用已更新。既有 dispatch/refs.rs unreachable pattern 编译警告与本次无关。

后续交付：用户授权提交、push 和打包。功能提交 `8170656f2` 已推送 `codex/tauri-macos`；构建 `0.1.58` Apple Silicon APP/DMG，build:verified 四项门禁通过，实际包内网关验证桌面会话、管理页面资源、代理、持久化及退出。证据 `.cache/delivery/sub2api-package/evidence/result.json`。版本递增导致外层构建前指纹失效，已按构建后快照重新关联有效门禁，未重复构建。未安装替换、未操作真实账号或原生窗口；使用本地 ad-hoc 签名，无 Apple 公证。

## 10-22 重新打开：API Key 模板未覆盖

用户确认聪明、Input-国际线路点击同步后名称未变。已安装和运行版本均为 0.1.58，原生界面复现 Input 的 6a 点击后保留名称；只读 SQLite 显示成功倍率及空 group_name。真实聪明 billing 响应无分组名，两个站点 usage.planName 均为「钱包余额」，不能用作分组名。当前两站配置均保存 api_key 和 refresh_token；此前只实现账号模板，遗漏 API Key 模板。

新规则：API Key 模板可选 Refresh Token，从同站账号接口精确匹配密钥并同步分组名；仍使用 billing 生效倍率而非账号默认倍率。无账号凭据沿用仅倍率行为；已配置账号凭据但账号查询失败，保留名称及倍率并显示失败，避免声称完整同步成功。续期仍按配置快照持久化，复用缓存；不改 New API 行为。用户本轮要求涵盖实际使用的两个站点，是对原需求范围的澄清。

状态：代码修复及构建完成，真实账号验收受账号认证阻塞。旧完成结论仅适用于账号模板。

新增 `usage_api` 支持：有可用 Refresh Token 时从 SUB2API 账号 API 读取密钥分组名，倍率仍取 `/v1/sub2api/billing`；没有 Refresh Token 保持旧行为。配置编辑不保留账号运行时缓存。复现测试先红（倍率成功但旧名保留），修复后 HTTP 回归通过，包含默认分组率与 billing 生效倍率不同、token 续期及账号查询失败保留。

真实验收：运行实际包内 0.1.59 网关处理聪明及 Input 的 4 条本地跟随密钥，全部返回账号认证失败；名称、倍率均安全保留。以 API Key 请求 `/api/v1/groups/available` 和 `/api/v1/keys` 对两站均返回 401。现有 Refresh Token 是 opaque token；未作试探性刷新以避免可能轮换凭据。故 0.1.59 是候选构建，真实站点验收未通过，用户需在两个 Provider Ops 配置中更新有效 Refresh Token 后才能验证改名。构建 4m18s，门禁前三项源码回归及包生命周期通过；real-sites 项失败，整个 build:verified 状态 failed。证据：`.cache/delivery/usage-group-name/evidence/result.json` 与 `real-sites-detail.log`。

交付：功能代码提交和版本记录虽可推送，但真实业务目标尚未验收，不将候选包标记为已解决。桌面主程序仍运行 0.1.58，未替换安装。
