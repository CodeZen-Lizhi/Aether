# 验证与交付记录

2026-09-15，源码修复完成，修复提交 af09ccdf3 已推送 origin/codex/tauri-macos。未修改生产设置、未调用真实供应商。桌面打包验收见文末。

## 最终行为

- HTTP普通聊天流与流式compact在真实上游成功响应头到达时解除首次超时；无需等待body或有效正文。没有响应头仍按首次期限结束。
- 流式总期限默认900秒，独立配置1–1200秒，包含调度、重试与整个输出过程。已响应后无新的静默/有效内容限制；已输出也不能越过总期限。旧预算不会被当作新总期限，非流式/WS保持既有独立语义。
- 请求lifecycle统一控制列表、详情与trace；候选错误留在本轮节点。请求总耗时优先metadata.end_to_end_time_ms，保留response_time_ms的单轮语义。真终态防陈旧更新、legacy和图片显式失败仍受保护。
- HTTP200后的错误或缺失协议终态不会冒充完成，已提交输出不透明重放。保留Responses事件别名和合法JSON→SSE桥接。
- request/pump共享总期限与取消归属；正常结束释放发送者，让后台持久化继续而不拖住HTTP EOF。总超时保留首body及headers/body/effective/结束阶段信息，并记录独立失败类别。

## 实际验证

- 最终真实HTTP/临时SQLite矩阵9/9通过：100ms headers、首次1秒、3秒body/压缩结果、总5秒、旧预算1.8秒仍完成；启动事件后静默仍完成；无headers首次超时；headers-only/心跳-only总超时；普通文本/compact item输出后的总超时；已输出后连接断开。
- HTTP配置create→GET→PATCH→GET→null清空，5类非法写入拒绝；关闭网关后重开SQLite核对新值已清除，旧预算/未知配置/非流式设置保留。见config-results.json。
- gateway stream_response_timeout最终11项通过（含内部Responses矩阵11个case、真实HTTP路由、owner交接、慢持久化时EOF、JSON桥接、非对象终态反例）。其余13组定向gateway回归通过，包含HTTP失败/探针/取消、capacity、watchdog、direct transport、SQLite lifecycle、历史/图片fallback、trace、配置、local relay与真实WS路由。
- root最后修正trace总时钟后重新运行SQLite lifecycle 2项与trace13项，通过；两轮耗时119008ms/180003ms，与全程300008ms分别验证。
- contracts配置/serde、admin timeout 2项、provider transport timeout 6项均通过；usage metadata新增1项同时覆盖copy/move/sanitize路径并保留未知字段过滤。
- ProviderForm 11项、usage相关136项前端测试通过；type-check、修改文件ESLint通过。浏览器真实组件验证状态关闭重开/刷新和配置保存600→重开→清空900，详见research/browser-validation.md。
- 最终gateway二进制构建通过；29个修改Rust文件格式检查、范围diff-check、任务manifest校验通过。

## 证据边界

本地合成供应商证明网关此次行为，不能保证真实供应商后续总能成功。真实远端隧道节点的整条慢压缩/reset链路和真实KV延迟未注入；使用本地relay、取消回归与受控慢持久化owner测试验证相关边界。合法response.incomplete沿用已有failed/200展示与原因，保留tokens，不在本任务重定义其状态或计费。没有执行全仓测试。

最终HTTP矩阵：/Users/zhenglizhi/otherProjects/Aether/output/compact-timeout-diagnosis/acceptance-1789449165056530000/results.json

配置HTTP/SQLite：/Users/zhenglizhi/otherProjects/Aether/output/compact-timeout-diagnosis/run-1789448917683879000-da3392b5/config-results.json

首批数据后总超时的阶段记录：
```json
[
  {
    "response_headers_elapsed_ms": 459,
    "first_body_elapsed_ms": 459,
    "first_effective_output_elapsed_ms": null,
    "first_byte_ms": 107
  }
]
```

最终测试二进制SHA-256：`72a7bd564630979a2f605d8aaa560078425459bff23a735576447e04237679b7`。该二进制仅用于本地验收。

## 桌面 0.1.25 打包验收

- 修复提交：`af09ccdf3`；版本提交：`18f69254d`，分支 `codex/tauri-macos`。
- `npm --prefix apps/aether-desktop run build` 成功，五处版本元数据统一为 0.1.25。
- 安装包：`/Users/zhenglizhi/otherProjects/Aether/target/release/bundle/dmg/Aether_0.1.25_aarch64.dmg`，32374023 bytes。
- SHA-256：`31fcc345b4422da751debda1b05ce8a768cabcc132505ff17b1cda8f80e055a2`。
- 只读挂载 DMG 验证版本、应用/网关二进制、网页入口与已验收包一致，含 Applications 链接；校验后正常卸载。应用 deep/strict 代码签名、DMG 完整性检查通过；两个可执行文件均为 arm64，最低 macOS 14.0，动态库均来自系统路径。签名为 ad-hoc，未做公证。
- 直接运行包内 gateway 与 web 的 `qa_lifecycle.py --desktop`：免密码会话、管理/API/JSON/SSE、八个 SPA 路由、备份及 usage 持久化、stdin EOF/SIGTERM/父进程退出与端口释放均通过。临时数据：`/private/var/folders/fg/bzpd9ft96g976xqf_w4lwbrr0000gn/T/aether-cli-lifecycle-z_9_el8l`。
- 包内 Release 网关超时矩阵 9/9 通过，包含快速 headers 后延迟 compact/chat body、启动事件后静默成功、首次响应超时、整体超时及输出后断流，且最终状态写入临时 SQLite。结果：`output/compact-timeout-diagnosis/acceptance-1789450074866983000/results.json`。
- 构建日志：`/tmp/aether-desktop-0.1.25-build.log`；包内生命周期日志：`/tmp/aether-desktop-0.1.25-lifecycle.log`；超时矩阵日志：`/tmp/aether-desktop-0.1.25-timeout.log`。
- 本次未安装/启动原生 GUI，未验证真实 Keychain、WKWebView 和供应商；沿用前述源码/浏览器验收，不将打包成功等同于真实供应商必定成功。未改动生产数据。
