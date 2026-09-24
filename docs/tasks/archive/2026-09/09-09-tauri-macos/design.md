# Tauri macOS 客户端设计

## 边界与复用

新增 `apps/aether-desktop/src-tauri` 作为 Cargo workspace 成员，内置 `aether-gateway` sidecar 和编译后的 Vue 资源。桌面宿主管理进程、Keychain、菜单、窗口、登录自启、日志和固定目录。Vue 管理端仍由网关以本机 HTTP 同源提供，保留既有认证 Cookie、API、history 路由和 SSE 行为。

另增加 `frontend/desktop.html` 与 `frontend/src/desktop/` 作为应用内的故障恢复页面。Tauri 的 `main` 窗口加载该内置页面，拥有有限桌面命令权限；管理窗口（label 为 `dashboard-<uuid>`）加载受管网关的 `http://127.0.0.1:<port>/`，仅获得网关生命周期与桌面配置命令。其他 HTTP 页面无 IPC 权限，Rust 命令层还会校验动态窗口标签与当前受管回环 origin。外部 HTTP(S) 页面经系统浏览器打开，不允许在受信任窗口导航。业务界面不直接读 SQLite。

桌面主题、语言、时区偏好以及端口、自启动、目录统一放入现有系统设置页面；桌面侧栏和移动菜单移除底部身份入口，旧个人设置路径导向合并后的入口。后台顶栏提供紧凑的网关状态菜单和启停/重启操作，应用菜单与菜单栏保留同类生命周期操作，但不再存在“客户端设置”入口或独立设置页。内置页面仅用于网关尚未启动或故障时恢复。Web/Docker 保留原个人设置和账号区域。

## 进程生命周期

状态：`setup → starting → running → stopping → stopped`，可恢复故障进入 `failed`。开始/停止/设置操作互斥，重复启动幂等；只管理自己创建的 Child。默认关闭窗口隐藏，菜单栏持续运行。退出先停网关，再退出应用。

网关新增可选 `--app-host`（原默认 `0.0.0.0`）、`--shutdown-timeout-seconds` 与 `--exit-on-stdin-close`。桌面显式传入回环地址、单监听分片、生产环境、绝对数据库路径、独立日志路径、持久密钥。stdin 管道由宿主持有，宿主死亡导致 EOF，触发网关正常退出流程。SIGINT/SIGTERM 与 stdin EOF 共用停止链；停止接入、等待连接与用量持久化，然后关闭后台任务；超时明确记录。

宿主不根据端口杀进程；启动前检查端口并等待自己子进程的 ready 日志及健康响应，避免连接到无关服务。输出持续消费，内存日志有界；最终 stop 时等待并回收 Child，超时才终止自己的进程。

管理窗口绑定子进程代次；打开窗口、读取临时会话凭据及窗口创建与启停使用同一个生命周期锁。代次变化后重建窗口，使用独立 label 避免异步 destroy 尚未移除旧名称时重建冲突。故障 watcher 在该锁内重新确认 Failed 后才关闭窗口，避免关闭手动恢复后的新窗口。macOS Reopen 派发到后台执行，UI 主线程不等待生命周期锁。

单实例使用同步 `flock`，在 Tauri、数据目录和 Keychain 初始化前选出唯一宿主；其他进程仅通过当前用户私有 Unix socket 激活它。保留锁文件 inode，只有持锁宿主创建和清理 socket。没有使用先检测 socket、稍后异步绑定的插件流程，避免并发冷启动都进入初始化。

## 持久化与首启

默认使用系统 Application Support 下的应用数据目录和独立日志目录。普通配置只包含版本、端口和初始化状态，原子写入。JWT/数据库加密密钥通过 macOS Keychain API 保存。

个人版首次打开自动初始化并进入后台，不提供创建账号或登录表单。网关为业务数据维护内部本机身份；旧桌面数据只有一个有效本地管理员时复用相同身份与密码哈希，多个或无有效管理员的非空数据明确报错，不猜测或修改权限。已有 DB 而 Keychain 密钥缺失时报错，禁止生成新密钥掩盖问题。测试使用独立数据目录与派生 Keychain service，不接触用户现有记录。逻辑备份在管理页面导出/导入，原数据库迁移的密钥与 WAL 要求写入文档。

宿主每次启动受管网关生成新的高熵临时会话凭据，仅在宿主/网关内存中保存，通过清理后的子进程环境 `AETHER_DESKTOP_SESSION_SECRET` 传入。显式 `--desktop-mode` 且 `127.0.0.1`、宿主 stdin EOF 管理才可启用。`POST /_gateway/desktop/session` 必须校验临时凭据、准确 Host/Origin 和设备标识，复用现有 Bearer/HttpOnly Cookie 会话，不把所有本机请求视为可信。

宿主通过 Tauri 2.11.5 的主 frame `initialization_script`，只在准确的受管 HTTP origin 注入 `window.__AETHER_DESKTOP__.authenticate(clientDeviceId)`。此函数在闭包内持有临时凭据，用固定同源地址换取普通会话；JWT/加密根密钥不进入页面。前端启动及 Cookie 续期失效时调用该函数并去重，故障呈现可重试连接页。后台窗口仍无桌面 IPC；Web/Docker 无注入且不开启该端点，保留原有登录。

## 前后端命令契约

`DesktopStatus` 字段均为 snake_case：

```ts
type GatewayPhase = 'setup' | 'starting' | 'running' | 'stopping' | 'stopped' | 'failed'
interface DesktopStatus {
  phase: GatewayPhase
  configured: boolean
  port: number
  gateway_url: string
  data_dir: string
  log_dir: string
  autostart: boolean
  pid: number | null
  error: string | null
  version: string
}
```

命令：`desktop_status`、`desktop_start`、`desktop_stop`、`desktop_restart`、`desktop_set_port({ port })`、`desktop_set_autostart({ enabled })` 均返回状态；`desktop_open_dashboard`、`desktop_open_data_dir`、`desktop_open_log_dir`、`desktop_quit` 返回 void；`desktop_logs` 返回最多 100 行已过滤的日志。`desktop_start` 同时负责首启初始化，不再保留 `desktop_setup`。端口只能在停止状态修改。命令错误给出可操作中文信息，前端保留端口输入并解除等待。

## 打包与兼容

Tauri 2 使用系统 WKWebView，菜单和登录自启复用官方 API 或插件，单实例由同步文件锁实现。最低 macOS 14，以使用独立 `data_store_identifier` 隔离不同数据目录的 WebView 会话。Cargo 默认成员保持网关，Web build 可独立执行；Linux CI 的 workspace clippy/nextest 排除 macOS 桌面包。

`apps/aether-desktop` 提供固定版本 npm CLI、开发/构建脚本；sidecar 按 target triple 命名，资源从构建产物读取。先交付 Apple Silicon 本地签名应用和安装包，签名公证使用可选外部配置，不放入仓库凭据。升级替换程序，数据库迁移前先做可恢复备份。macOS 27 构建时保留构建期过程宏符号，避免系统加载器拒绝裁剪后的宏动态库；应用与网关仍采用 release 优化和符号裁剪。

## 需由验证收敛的技术点

- WKWebView 的 blob 下载、文件选择和外部新窗口回调使用实际 `.app` 验证，必要时在该边界增加适配，不迁移业务 API。
- 网关用量队列需结合现有 UsageRuntime 的实际状态建立有界等待，不能仅调用后台 supervisor.shutdown 就宣称已落库。
- 全部代码检查与 GUI 行为分开取证；`.app` 启动成功不等于全部业务流程已通过。
