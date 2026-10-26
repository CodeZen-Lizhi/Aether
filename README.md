<p align="center">
  <img src="frontend/public/aether_adaptive.svg" width="120" height="120" alt="Aether Logo">
</p>

<h1 align="center">Aether</h1>

<p align="center">
  <strong>个人自托管 AI API 网关</strong><br>
  统一接入模型供应商 · 三种调度策略 · 有限重试与故障转移 · 本地用量与成本统计
</p>

<p align="center">
  <a href="#项目定位">项目定位</a> •
  <a href="#当前功能">当前功能</a> •
  <a href="#部署">部署</a> •
  <a href="#使用流程">使用流程</a> •
  <a href="#本地开发">本地开发</a> •
  <a href="#环境变量">环境变量</a>
</p>

---

## 项目定位

Aether 是一个面向个人和小团队自托管的 AI API 网关。它把不同供应商的模型、端点和凭据收在同一个本机服务后面，按统一模型名完成路由、协议转换、失败切换和用量记账。

当前仓库是独立维护的个人版本，主线为 `codex/tauri-macos`。项目以 macOS 桌面客户端为主要使用方式，同时保留 Web 管理页、Docker Compose 和源码运行方式。服务默认使用 SQLite 与内存运行时，不依赖外部数据库或 Redis。

核心边界：

- 单管理员、单机优先，数据默认保存在本机。
- 只代理实际配置的供应商；是否支持某种请求取决于对应供应商、端点、认证和模型能力。
- 计费统计按本地配置价格计算，不预测供应商账单；中转服务的最终扣费仍以其账单为准。
- 网关不替代客户端工具，不内置模型账号，也不附带任何第三方 API 额度。

## 当前功能

### 1. 桌面客户端

- macOS 14+、Apple Silicon 为主要交付目标，Tauri 2 宿主内打包 Rust 网关、SQLite 与前端资源。
- 安装后自动初始化并打开后台，桌面模式不需要额外创建管理员或经过 Web 登录页。
- 菜单栏控制启动、停止、重启、打开后台、数据目录、日志目录和退出；支持登录自启。
- 系统设置可修改端口、自启和查看诊断日志；网关仅监听本机回环地址。
- 应用数据位于 `~/Library/Application Support/com.aether.desktop/`，JWT 与加密密钥保存在 macOS 钥匙串。
- 版本升级前生成数据库与 WAL 快照，支持配置导出、完整备份与恢复。

详细安装、数据目录、签名和构建说明见 [macOS 客户端文档](docs/desktop-macos.md)。

### 2. 供应商、端点和上游 Key

- 管理供应商、API 端点与多渠道 Key；同一供应商可配置多个端点和多把 Key。
- 支持渠道 OAuth 账号、API Key、Cookie 等认证方式，以及供应商自定义认证模板。
- Key 可设置接口格式、模型范围、内部优先级、并发限制、过期时间和 RPM 限制。
- 支持上游模型发现、单条/批量模型映射、模型别名、模型能力与图片、视频等计费配置。
- 支持供应商余额查询、签到、额度重置、月配额展示和批量余额刷新。
- 支持从上游同步分组倍率，也可手动设置 Key 的价格倍率；成本优先调度使用该倍率。
- 支持渠道级出口代理、代理节点选择、端点健康与冷却状态查看。
- 支持模型测试与故障转移测试，从真实候选链路观察请求是否可达。

### 3. 全局模型与协议代理

- 以统一模型名管理客户端可见模型，再映射到各供应商的实际模型。
- 支持 OpenAI Chat Completions、Responses、Realtime、Images、Videos、Embeddings、Rerank、Search 等端点。
- 支持 Claude Messages、Gemini Generate Content、Gemini Interactions、Gemini Files 及对应 embedding/video 路由。
- 支持 Codex Live、Responses WebSocket、普通 HTTP 流式与非流式请求。
- 允许在协议能力兼容时做格式转换；同格式请求可透传，转换失败或供应商不支持时按明确错误返回。
- 模型指令支持为请求选择推理强度等参数，具体可用后缀取决于当前 API 格式与供应商能力。
- 对已交付且不可安全重放的文本或工具状态，不在中途静默换 Key 重发整轮请求。

### 4. 调度、重试与健康度

- 调度页提供三种策略，首次选择与故障后的候选顺序使用同一套规则：

| 策略 | 选择方式 | 适用场景 |
| --- | --- | --- |
| 缓存亲和 `cache_affinity` | 同一会话优先复用供应商、端点和 Key；备用成功后迁移绑定 | 长对话、Codex 等连续开发任务 |
| 固定顺序 `fixed_order` | 严格按供应商优先级主备，不按会话粘滞 | 渠道少、顺序明确 |
| 成本优先 `cost_based` | 同模型下按 Key 价格倍率升序，失败后继续找下一候选 | 多渠道、关注配置成本 |

- 支持按供应商拖拽调整全局优先级，并保留已有请求修改规则、条件、阶段和停止语义。
- 同一 Key 可进行有限重试；总尝试次数包含首次，设为 `2` 表示首次加一次重试。
- 调度健康度按 Key 和接口格式维护，范围 0–10；错误分类、连续失败加重、冷却、熔断与探测恢复均按当前聊天策略执行。
- 流式请求共享首输出总预算，覆盖调度、排队、连接、刷新凭据、退避与等待有效输出；默认 `90s`，开始有效输出后不截断正常回答。
- 普通重试的额外等待有上限；供应商返回有效 `Retry-After` 时保留完整冷却期限。
- 已禁用、模型/接口不匹配、熔断未到探测时间或本地容量不足的候选会被过滤，不伪造为上游失败。

完整规则见 [聊天调度与故障转移](docs/specs/aether-gateway/backend/chat-failover.md)。

### 5. API Key 与访问控制

- 管理独立余额 API Key，可配置初始余额、无限额度、按余额消费或独立余额模式。
- 支持按分钟限速、并发限制、模型白名单、Provider 白名单、IP/CIDR/通配符规则。
- 支持有效期、到期禁用或自动删除；可单独启用、禁用和删除 Key。
- 网关通过 Bearer API Key 鉴权；独立 Key 的创建结果需按页面提示及时保存，列表只展示脱敏值。
- 全局配置提供默认限速、到期 Key 自动删除和 API 格式转换开关。

### 6. 用量、成本与诊断

- 仪表盘展示今日请求、Token、费用、缓存和平均响应时间，以及历史累计和按小时/天/周/月的趋势。
- 使用记录支持时间、模型、供应商、接口格式、状态和客户端类型筛选，并可自动刷新。
- 请求详情显示模型、费用、Token、缓存、首字耗时、总耗时和吞吐，并区分调用方 Key 与实际上游执行 Key。
- 请求链路按真实调度顺序展示入口、供应商、Key、重试和结果；未尝试或仅跳过的候选不会伪装成实际调用。
- 支持查看请求头、请求体、响应体、原始上游错误、curl 回放计划和相关诊断信息。
- 用量按模型、供应商和记录维度汇总；金额根据模型定价与 Key 倍率本地计算。
- 系统设置提供记录级别、敏感请求头、日志保留、自动清理、手动清理和数据维护。

### 7. 备份、导入与运行维护

- 支持配置导出/导入和完整备份恢复，覆盖供应商、端点、Key、模型、代理节点、调度策略、系统配置、管理员资料、API Key 与用量。
- 完整备份默认使用 `skip / overwrite / error` 合并模式；遇到冲突或无法关联的数据会明确中止，不静默丢弃。
- SQLite 使用增量迁移、WAL 与启动前自动准备数据库；桌面升级前保存数据库快照。
- 支持通过配置或系统设置控制日志格式、日志落点、轮转、保留期限和请求正文记录级别。

### 8. Aether Tunnel

- Tunnel 是独立代理节点程序，适合无法直连 Aether 或需要固定出口的场景。
- 节点通过 WebSocket 回连 Aether，无需对外监听端口；支持多条服务地址、自动重连、连接池与限流。
- 支持出口代理、DNS 策略、IPv4/IPv6 选择、TLS/PSK 通道安全、心跳指标和多平台服务安装。
- 管理页支持添加节点、查看指标、测试连通性和维护远程配置；升级能力以当前节点和管理页状态为准。

安装和参数见 [Tunnel 文档](apps/aether-tunnel/README.md)。

## 部署

### macOS 客户端（推荐）

桌面应用是当前主要交付形态。打开安装包后将 **Aether.app** 拖入“应用程序”并启动；首次运行会自动创建本地数据库和管理员身份。

默认网关地址：`http://127.0.0.1:8084`

OpenAI 兼容客户端 Base URL：`http://127.0.0.1:8084/v1`

Claude 兼容客户端 Base URL：`http://127.0.0.1:8084`

构建当前架构安装包：

```bash
npm --prefix frontend ci
npm --prefix apps/aether-desktop ci
npm --prefix apps/aether-desktop run build
```

产物位于 `target/release/bundle/macos/Aether.app` 和 `target/release/bundle/dmg/`。构建会递增版本；默认使用 ad-hoc 签名，不含 Apple 公证。

### Docker Compose

```bash
git clone https://github.com/CodeZen-Lizhi/Aether.git aether
cd aether

cp .env.example .env
./generate_keys.sh
# 在 .env 中设置 ADMIN_PASSWORD，首次启动时创建管理员

docker compose up -d --build
```

默认监听 `0.0.0.0:${APP_PORT:-8084}`，SQLite 数据保存在 Docker named volume `aether-personal_aether-data`。`docker compose down` 保留数据卷；只有确认不再需要数据时才使用 `docker compose down -v`。

### 源码运行

需要 Rust 工具链和 Node.js。SQLite 与 memory 运行时不需要额外数据库服务。

```bash
npm --prefix frontend ci
npm --prefix frontend run build
cargo run -p aether-gateway --release
```

生产环境至少设置 `JWT_SECRET_KEY`、`ENCRYPTION_KEY` 和 `ADMIN_PASSWORD`。监听在非回环地址时，应自行配置 HTTPS 反向代理、访问控制与防火墙。

## 使用流程

1. 启动网关或桌面应用并打开管理后台。
2. 在“供应商管理”中添加供应商、端点和上游 Key；完成模型发现或手动建立模型。
3. 在“模型管理”中创建统一模型，关联一个或多个供应商模型；必要时配置计费。
4. 在“调度策略”中选择缓存亲和、固定顺序或成本优先，并调整供应商优先级。
5. 在“独立余额 API Keys”中创建客户端 Key，配置额度、模型范围、IP 和并发限制。
6. 将客户端 Base URL 指向网关，在 Authorization 头使用刚创建的 Key。
7. 在“用量记录”中核对请求结果、实际供应商、Token、费用和失败原因。

常用 API 端点：

| 能力 | 路径 |
| --- | --- |
| OpenAI Chat Completions | `POST /v1/chat/completions` |
| OpenAI Responses | `POST /v1/responses` |
| OpenAI Responses WebSocket | `GET /v1/responses`（升级） |
| OpenAI Realtime | `GET /v1/realtime`（升级） |
| OpenAI Embeddings | `POST /v1/embeddings` |
| OpenAI Images | `POST /v1/images/generations` |
| OpenAI Videos | `POST /v1/videos` |
| Claude Messages | `POST /v1/messages` |
| Gemini Generate Content | `POST /v1beta/models/{model}:{action}` |
| 模型列表 | `GET /v1/models` |
| 健康检查 | `GET /v1/health` |

具体请求格式以对应供应商和格式文档为准；Aether 不保证跨格式的所有字段都能无损转换。

## 本地开发

```bash
make dev           # 后端 + 前端开发服务
make dev-backend   # 仅后端
make dev-frontend  # 仅前端
```

前端检查：

```bash
npm --prefix frontend run type-check
npm --prefix frontend run test:run
```

Rust 检查：

```bash
cargo fmt --all -- --check
cargo check -p aether-gateway
cargo test -p aether-gateway --lib
```

桌面开发与交付验收：

```bash
npm --prefix apps/aether-desktop run dev
npm --prefix apps/aether-desktop run verify:delivery -- --help
npm --prefix apps/aether-desktop run build:verified -- --help
```

`build` 只证明安装包构建成功；涉及功能验收时使用 `verify:delivery` 或 `build:verified`，参数与证据目录规则见 [桌面验证入口](docs/desktop-macos.md#交付门禁)。

## 环境变量

| 变量 | 说明 |
| --- | --- |
| `APP_PORT` | 网关监听端口，默认 `8084` |
| `APP_HOST` | 监听地址；桌面模式固定回环，Web/Docker 默认 `0.0.0.0` |
| `AETHER_DATABASE_DRIVER` | 数据库驱动，当前使用 `sqlite` |
| `AETHER_DATABASE_URL` | SQLite 地址，例如 `sqlite:///opt/aether/data/aether.db` |
| `AETHER_RUNTIME_BACKEND` | 运行时状态后端，单机使用 `memory` |
| `AETHER_GATEWAY_AUTO_PREPARE_DATABASE` | 启动前自动应用 SQLite migration |
| `JWT_SECRET_KEY` | 管理会话签名密钥；更换后已有登录态失效 |
| `ENCRYPTION_KEY` | 上游 Key 等敏感数据加密密钥；丢失后原密文无法解密 |
| `ADMIN_USERNAME` / `ADMIN_PASSWORD` / `ADMIN_EMAIL` | 首次启动自举管理员 |
| `API_KEY_PREFIX` | 客户端 API Key 前缀，默认 `sk` |
| `CORS_ORIGINS` / `CORS_ALLOW_CREDENTIALS` | Web 模式跨域来源与 Cookie 配置 |
| `RUST_LOG` | Rust 日志过滤，例如 `aether_gateway=info` |
| `AETHER_LOG_FORMAT` / `AETHER_LOG_DESTINATION` / `AETHER_LOG_DIR` | 日志格式、落点与目录 |
| `AETHER_GATEWAY_MAX_IN_FLIGHT_REQUESTS` | 单实例请求并发上限 |
| `AETHER_MAX_REQUEST_BODY_MB` | 解压后请求体上限，`0` 表示不限制 |

完整示例见 [.env.example](.env.example)。凭据不要提交到仓库；更换 `ENCRYPTION_KEY` 前先导出并核对恢复方案。

## 数据与备份

- Web/Docker 模式默认数据库为 SQLite；桌面模式数据库和快照位于应用数据目录。
- 导出配置与完整备份可能包含上游凭据、管理资料和用量，应视为敏感文件保管。
- 物理复制数据库不包含 macOS 钥匙串中的 JWT/加密密钥，不能单独作为桌面数据恢复依据。
- 恢复完整备份前应停止写入并保留当前数据副本；合并冲突不会被静默跳过。

## 项目结构

```text
apps/
  aether-desktop/      Tauri macOS 宿主
  aether-gateway/      HTTP/WebSocket 网关入口
  aether-tunnel/       隧道节点程序
crates/
  aether-ai/           协议格式、转换与 serving
  aether-provider/     供应商、端点、Key 与传输
  aether-routing-core/ 调度规则
  aether-gateway/      前门、控制、执行与 worker
  aether-data/         SQLite、查询与 schema
  aether-usage/        用量与统计
frontend/              Vue 管理界面
docs/                  规格、任务与运维文档
```

## 文档

- [Spec 索引](docs/specs/index.md)
- [macOS 客户端](docs/desktop-macos.md)
- [Aether Tunnel](apps/aether-tunnel/README.md)
- [Responses WebSocket 模式](docs/WebSocket-Mode.md)
- [供应商接口定义](docs/api/provider-interface-definitions.md)

## 许可证

本项目沿用 [Aether 非商业开源许可证](LICENSE)。许可边界以仓库中的 `LICENSE` 正文为准。
