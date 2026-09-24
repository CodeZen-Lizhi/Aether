# 实施与验收记录

日期：2026-09-12。前端实现与独立审查完成；用户追加授权后已提交并构建 0.1.19 安装包，未公开发布或替换正在运行的 APP。

## 自动检查

- 实现阶段 6 个相关组件/composable 测试文件 79 项通过，覆盖导航、代理选择与管理、桌面端口、自启动、分组保存、导入恢复及展示模式隔离。
- 独立审查发现外部 query 对象索引会把 `constructor`、`toString`、`__proto__` 识别成视图，导致空白页；改用 Map。3 项新增回归先失败后通过，SystemSettings 测试文件最终 10 项通过。
- frontend type-check、变更文件定向 ESLint、git diff --check 通过。最终前端生产构建通过，未执行原生打包或全仓构建。

## 真实浏览器与隔离 HTTP

运行真实 AppShell、组件、字体及独立 SQLite gateway；仅桌面 IPC 使用 fixture。预览前端 127.0.0.1:5175，隔离 gateway 127.0.0.1:18089。未改动用户的 8084 网关或既有 5173 服务。

| 检查 | 结果 |
| --- | --- |
| 900x640、1024x768、1200x820、1440x900、1920x1080 | 无横向溢出；设置行始终两列，标题 24px、输入文字 13px；正文最大 960px |
| 900x640 常用首页 | 正文 740px 宽、约 490px 高；备份按钮下沿约 502px，五项入口及高级入口首屏可达 |
| 连续宽度 1180/1080/1030/1024/1000/960/940/920/900/920/1000/1200 | 无导航模式或列关系切换 |
| 明暗主题与中英文 | 最小及大窗口检查无裁切；英文备份按钮调整控件列后保持单行 |
| 长代理名称 | 最小窗口完整换行，无重叠或横向溢出 |
| 代理流程 | 新建隔离节点、选中并保存默认代理、刷新读回；返回保留草稿；测试本地不可用地址收到明确失败；编辑名称后刷新读回一致；最后恢复直连 |
| 备份 | 真实下载 2888 字节 JSON，包含 exported_at/config_data/user_data；恢复预览识别隔离管理员，取消后无导入 POST |
| 分组保存 | 记录级别和保留期限分别编辑；切换及浏览器后退保留草稿；保存一组不消除另一组草稿，分别保存后刷新读回一致 |
| 旧导航 | records/backup query 及 cleanup/basic/sysinfo hash 打开对应视图与折叠层 |
| 390px Web | 独立无桌面注入上下文经真实登录；文档宽 390px，无端口或自启动控件，备份可达 |

## 桌面命令 fixture

- 非法端口 80 不触发命令，错误聚焦；8085 保存模拟失败时保留草稿，重试成功并刷新读回。
- 自启动失败保留实际开关值，重试成功；最终恢复 fixture 的 8084 和开启状态。
- 这些结果验证前端调用与失败处理，不证明原生端口绑定、进程重启或系统登录项已正确执行。

## 剩余边界

- 当前已安装 Aether 正在运行，原生单实例锁按用户占用，无法在不中断用户 APP 的情况下启动第二个独立宿主。本次未中断用户 APP，原生最小/最大化、WebKit 字体渲染、键盘焦点及实际 IPC 未验收。
- 当前显示器逻辑分辨率由 system_profiler 核实为 1920x1080，浏览器同尺寸检查不等于真实原生最大化，也不代表所有显示器上限。
- 完整恢复提交与部分保存失败沿用既有隔离回归测试；未恢复或清空真实个人数据。

## 截图与复现

截图位于 `output/playwright/`：`settings-stable-900.png`、`settings-stable-1920.png`、`settings-stable-dark-en-900.png`、`settings-stable-advanced-dark-en-900.png`、`settings-stable-long-proxy-900.png`、`settings-stable-restore-preview-900.png`、`settings-stable-web-390.png`。

布局脚本：`output/playwright/settings-stable-layout-check.cjs`。IPC fixture：`output/playwright/settings-stable-desktop.cjs`。独立服务启动器：`output/playwright/settings-stable-server.mjs`，进程与运行目录记录于相邻 `settings-stable-state.json`。

普通浏览器打开预览 URL 是 Web 模式，使用隔离账号 `settings-layout-qa@example.test` / `Settings-Layout-QA-Only-2026!`。Playwright 的 `settings-stable` 浏览器会话带桌面 fixture，可查看完整首页；截图来自该真实前端会话。

## 提交与打包

- 用户追加授权：提交代码、push，然后打包。设置改造提交 `a28a8da98` 已推送到 `origin/codex/tauri-macos`，一并推送该分支之前已提交的 `1917a8ffe` 桌面布局修复；未同步其他分支。
- 执行 `npm --prefix apps/aether-desktop run build`，脚本自动分配 0.1.19。版本文件提交 `55e7e38ba`，连同本交付记录推送到同一远端分支。
- 产物：`target/release/bundle/dmg/Aether_0.1.19_aarch64.dmg`，32,138,010 bytes；另有 `target/release/bundle/macos/Aether.app`。
- DMG SHA256：`fc46733722e09610f2f3290dee8b9702adf1fcc7ca3b8f23253342831c8a6ef7`。
- 独立包装核验：APP 与 DMG 内 APP 的深度严格签名校验通过；DMG 验证通过；只读挂载确认 APP 与 Applications 链接，挂载已清理；DMG 内 APP 与构建 APP 逐文件一致，随包 Web 与 frontend/dist 一致。
- APP 短版本与构建版本均为 0.1.19；宿主和网关均为 arm64，最低 macOS 14.0；动态依赖仅系统 Framework 与 /usr/lib，无工作区依赖。签名为本地 ad-hoc，未公证。
- 使用本次随包 release 网关和 Web 资源执行 `qa_lifecycle.py`，桌面模式和 Web 模式均通过：身份/登录、Cookie 续期、配置导入导出、JSON/SSE 代理、8 个后台路由、用量落库、stdin EOF、SIGTERM 与父进程退出后清理。
- 隔离测试诊断目录：临时目录中的 `aether-cli-lifecycle-4ul2vc8f`（桌面模式）、`aether-cli-lifecycle-mb7cqzac`（Web 模式）。测试子进程已自动清理，未操作现有网关数据。
- 原生宿主 UI/真实 IPC 验收限制仍有效，不因打包通过而标为已验证；因此保留任务为 in_progress，不自动归档。
