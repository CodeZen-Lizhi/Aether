<p align="center">
  <img src="frontend/public/aether_adaptive.svg" width="104" height="104" alt="Aether logo">
</p>

<h1 align="center">Aether</h1>

<p align="center">
  <strong>本地优先的 AI API 网关，面向个人与小团队。</strong><br>
  用一个入口管理供应商、模型、协议、调度、额度和用量。
</p>

<p align="center">
  <a href="https://github.com/CodeZen-Lizhi/Aether/actions/workflows/rust-ci.yml">
    <img src="https://github.com/CodeZen-Lizhi/Aether/actions/workflows/rust-ci.yml/badge.svg?branch=main" alt="Rust CI">
  </a>
  <img src="https://img.shields.io/badge/Rust-2021-000000?logo=rust" alt="Rust 2021">
  <img src="https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri" alt="Tauri 2">
  <img src="https://img.shields.io/badge/Vue-3-42B883?logo=vuedotjs" alt="Vue 3">
  <a href="LICENSE">
    <img src="https://img.shields.io/badge/license-Noncommercial-orange" alt="Noncommercial license">
  </a>
</p>

<p align="center">
  <a href="#快速开始">快速开始</a> ·
  <a href="#为什么选择-aether">为什么选择</a> ·
  <a href="#界面">界面</a> ·
  <a href="#兼容性">兼容性</a> ·
  <a href="#安全与数据">安全</a> ·
  <a href="#开发">开发</a> ·
  <a href="#文档">文档</a>
</p>

![Aether 仪表盘](docs/assets/screenshots/overview-light.webp)

<p align="center">
  <sub>界面截图使用合成数据，不含真实供应商、密钥、余额或请求内容。</sub>
</p>

## 概览

Aether 把不同 AI 供应商的端点、凭据和模型收集在一个本机服务后面。客户端只面对一个统一地址和模型名，Aether 负责鉴权、模型映射、协议适配、候选调度、故障转移、用量记账与请求诊断。

它采用单管理员、单节点优先的设计：SQLite 保存业务数据，内存运行时承载调度状态，不依赖外部数据库或 Redis。macOS 桌面客户端是主要交付形态，同时保留 Web、Docker Compose 和源码部署方式。

> [!NOTE]
> Aether 当前聚焦个人开发环境和小团队内部使用。它不提供模型账号、第三方额度、多租户 SaaS，也不承诺所有跨协议字段都能无损转换。

**适合**

- 想在一个客户端里切换多个供应商和模型。
- 需要控制上游 Key、客户端 Key、并发、限速和额度。
- 关注每次请求实际走了哪个供应商、Key 和重试路径。
- 希望数据、凭据和请求记录尽可能留在自己的机器上。

## 为什么选择 Aether

| 特性 | 实际价值 |
| --- | --- |
| 本地优先 | 桌面版默认只监听回环地址，SQLite 保存数据，主密钥进入 macOS 钥匙串 |
| 一个 API 入口 | OpenAI、Claude 和 Gemini 等协议共用统一模型名和访问控制 |
| 调度可解释 | 缓存亲和、固定顺序、成本优先三种策略，并保留真实候选与失败路径 |
| 完整可观测 | 从请求、Token、缓存、首字延迟到成本和上游错误都在同一处核对 |
| 供应商管理 | 多端点、多 Key、模型映射、健康度、冷却、余额与倍率统一维护 |
| 可迁移 | 配置导出与完整备份覆盖业务数据，恢复冲突不会被静默跳过 |

## 快速开始

### macOS 桌面版

需要 macOS 14+、Xcode Command Line Tools、Rust 工具链，以及 Node.js 20.19+ 或 22.12+。

```bash
git clone https://github.com/CodeZen-Lizhi/Aether.git
cd Aether

npm --prefix frontend ci
npm --prefix apps/aether-desktop ci
npm --prefix apps/aether-desktop run build
```

产物位于 `target/release/bundle/macos/Aether.app` 和 `target/release/bundle/dmg/`。默认使用 ad-hoc 签名，不包含 Apple 公证。

需要在本机反复重建并稳定复用钥匙串密钥时，先初始化个人签名身份：

```bash
npm --prefix apps/aether-desktop run signing:setup
```

完整说明见 [macOS 客户端文档](docs/desktop-macos.md)。

### Docker Compose

```bash
cp .env.example .env
./generate_keys.sh
# 在 .env 中设置 ADMIN_PASSWORD 和必要的密钥

docker compose up -d --build
```

打开 `http://127.0.0.1:8084` 并进入管理后台。SQLite 数据保存在 Docker named volume `aether-personal_aether-data`。

> [!IMPORTANT]
> Compose 默认把端口发布到宿主机网络接口。公网访问应配置 HTTPS 反向代理、访问控制和防火墙，不要直接暴露管理端口。

### 源码运行

```bash
npm --prefix frontend ci
npm --prefix frontend run build
cargo run -p aether-gateway --release
```

首次启动前至少设置 `JWT_SECRET_KEY`、`ENCRYPTION_KEY` 和 `ADMIN_PASSWORD`。完整变量见 [`.env.example`](.env.example)。

### 完成第一次调用

1. 添加供应商、端点和上游 Key。
2. 发现上游模型，或手动创建供应商模型。
3. 创建统一模型，并关联一个或多个供应商模型。
4. 选择调度策略，创建供客户端使用的 API Key。
5. 让客户端请求 Aether。

```bash
export AETHER_API_KEY="sk-your-aether-key"

curl http://127.0.0.1:8084/v1/chat/completions \
  -H "Authorization: Bearer $AETHER_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "your-model-name",
    "messages": [{"role": "user", "content": "Hello from Aether"}]
  }'
```

## 界面

<table>
  <tr>
    <td width="50%"><strong>使用记录</strong><br><img src="docs/assets/screenshots/usage.webp" alt="使用记录与筛选"></td>
    <td width="50%"><strong>调度策略</strong><br><img src="docs/assets/screenshots/routing.webp" alt="缓存亲和、固定顺序和成本优先策略"></td>
  </tr>
  <tr>
    <td width="50%"><strong>请求链路</strong><br><img src="docs/assets/screenshots/request-trace.webp" alt="请求在多个供应商和 Key 之间的真实执行路径"></td>
    <td width="50%"><strong>供应商工作台</strong><br><img src="docs/assets/screenshots/provider-detail.webp" alt="供应商配额、Key 和模型管理"></td>
  </tr>
</table>

<details>
  <summary>查看深色主题</summary>
  <br>
  <img src="docs/assets/screenshots/overview-dark.webp" alt="Aether 深色主题仪表盘">
</details>

## 核心能力

### 统一模型与协议入口

- 使用稳定模型名屏蔽上游模型差异，再为一个入口关联多个供应商模型。
- 支持 OpenAI Chat Completions、Responses、Realtime、Images、Videos、Embeddings、Rerank 和 Search。
- 支持 Claude Messages、Gemini Generate Content、Gemini Interactions 与 Gemini Files。
- 能力兼容时进行协议转换；同格式请求可原生透传，减少不必要的中间转换。
- 支持流式、非流式、WebSocket 和 Codex Responses WebSocket。

### 供应商与上游资源

- 一个供应商可配置多个端点、多把 Key、多组模型映射和自定义认证模板。
- Key 可设置模型范围、并发、RPM、过期时间、内部优先级和价格倍率。
- 支持余额、签到、月配额、分组倍率、端点健康和冷却状态。
- 模型测试和故障转移测试从真实候选链路发起，帮助定位认证、模型名或兼容性问题。
- 支持批量模型映射、手动刷新上游模型，以及按优先级调整映射顺序。

### 调度、重试与健康度

| 策略 | 调度依据 | 适合场景 |
| --- | --- | --- |
| 缓存亲和 `cache_affinity` | 同一会话优先复用供应商、端点和 Key | 长对话、编码助手等连续任务 |
| 固定顺序 `fixed_order` | 严格按供应商优先级主备 | 渠道少、顺序明确 |
| 成本优先 `cost_based` | 同模型下按 Key 价格倍率升序 | 渠道多、关注实际成本 |

调度会过滤已禁用、模型或接口不匹配、熔断未到探测时间以及本地容量不足的候选。流式请求共享首输出预算，收到有效输出后不会因该预算截断正常回答。

### 客户端访问控制

- 独立 API Key 可设置余额、有效期、并发和分钟级限速。
- 支持模型、供应商、IP/CIDR 与通配符白名单。
- 支持到期禁用或自动删除。
- 列表只显示脱敏 Key；完整值仅在创建结果中提供。

### 用量、成本与诊断

- 仪表盘展示今日请求、Token、费用、缓存、RPM/TPM 和平均响应时间。
- 使用记录可按时间、模型、供应商、协议、状态和传输类型筛选。
- 请求详情展示计费、Token、首字耗时、总耗时、吞吐和真实调度链路。
- 可查看请求头、请求体、响应体、原始上游错误、curl 回放和诊断信息。
- 本地费用按模型价格和 Key 倍率估算，最终账单仍以供应商结算为准。

### 桌面、备份与 Tunnel

- macOS 桌面版内置 Rust 网关、SQLite 和前端资源，无需单独安装 Docker 或数据库。
- 菜单栏支持启动、停止、重启、打开后台、数据目录、日志目录和退出。
- 升级前生成数据库与 WAL 快照，支持配置导入导出和完整备份恢复。
- Aether Tunnel 通过 WebSocket 回连网关，为无法直连或需要固定出口的网络提供代理节点。

## 兼容性

| 能力 | 默认路径 |
| --- | --- |
| OpenAI Chat Completions | `POST /v1/chat/completions` |
| OpenAI Responses | `POST /v1/responses` |
| OpenAI Responses WebSocket | `GET /v1/responses`（协议升级） |
| OpenAI Realtime | `GET /v1/realtime`（协议升级） |
| OpenAI Embeddings | `POST /v1/embeddings` |
| OpenAI Images | `POST /v1/images/generations` |
| OpenAI Videos | `POST /v1/videos` |
| Claude Messages | `POST /v1/messages` |
| Gemini Generate Content | `POST /v1beta/models/{model}:{action}` |
| 模型列表 | `GET /v1/models` |
| 健康检查 | `GET /v1/health` |

具体字段取决于供应商、端点和当前协议能力。跨格式转换无法安全表达请求时，Aether 会明确拒绝，而不是静默丢弃字段。

## 架构

<p align="center">
  <img src="docs/architecture/architecture-light.svg" alt="Aether 架构图" width="760">
</p>

```text
客户端
  │  OpenAI / Claude / Gemini / WebSocket
  ▼
Aether Gateway
  ├─ API Key 鉴权、限速与模型权限
  ├─ 全局模型解析与供应商模型映射
  ├─ 缓存亲和 / 固定顺序 / 成本优先调度
  ├─ 协议转换、原生透传、流式转发与重试
  └─ 用量、计费、健康度与请求诊断
  │
  ▼
已配置的上游供应商与 Key
```

核心 workspace 按职责拆分：

```text
apps/
  aether-desktop/      Tauri macOS 宿主、打包与交付脚本
  aether-gateway/      HTTP/WebSocket 网关入口
  aether-tunnel/       独立代理节点程序
crates/
  aether-ai/           协议格式、转换、规划与 serving
  aether-provider/     供应商、端点、Key 与传输
  aether-routing-core/ 调度规则
  aether-gateway/      前门、控制、执行与 worker
  aether-data/         SQLite、查询、Schema 与迁移
  aether-usage/        用量、统计与结算
frontend/              Vue 3 管理界面
docs/                  规范、架构、任务与运维文档
```

## 安全与数据

> [!IMPORTANT]
> 配置和完整备份可能包含上游凭据、管理员资料和请求数据。更换 `ENCRYPTION_KEY` 前先导出并验证恢复方案；丢失原密钥后，已有密文无法解密。

- 桌面模式固定监听本机 IPv4 回环地址，并自动建立本机会话。
- 桌面主密钥保存在 macOS 钥匙串；Web/Docker 主密钥来自环境变量。
- 桌面升级前会复制数据库、WAL 和普通配置，但不会复制钥匙串密钥。
- 不要把 `.env`、数据库、日志、导出文件或真实凭据截图提交到仓库。
- 公网部署前必须配置 HTTPS、访问控制、防火墙和强随机密钥。
- 安全问题请按 [SECURITY.md](SECURITY.md) 私下报告。

## 开发

```bash
make dev           # 后端 + 前端开发服务
make dev-backend   # 仅后端
make dev-frontend  # 仅前端
```

常用检查：

```bash
npm --prefix frontend run type-check
npm --prefix frontend run test:run

cargo fmt --all -- --check
cargo check -p aether-gateway
cargo test -p aether-gateway --lib

npm --prefix apps/aether-desktop test
```

桌面交付涉及构建产物验证时，优先使用项目提供的 `verify:delivery` 或 `build:verified`。普通 `build` 只证明安装包构建成功，不替代功能验收。

## 文档

| 需求 | 文档 |
| --- | --- |
| 了解业务与代码契约 | [Spec 索引](docs/specs/index.md) |
| 安装、签名与故障恢复 | [macOS 客户端](docs/desktop-macos.md) |
| 调度与故障转移 | [聊天调度规则](docs/specs/aether-gateway/backend/chat-failover.md) |
| 修改三种调度策略 | [调度策略配置](docs/specs/aether-routing-core/backend/simplified-routing.md) |
| 备份、导入与恢复 | [配置备份与导入](docs/specs/aether-gateway/backend/admin-json-backup.md) |
| 协议透传和转换边界 | [格式透传契约](docs/api/format-passthrough-contract.md) |
| Responses WebSocket | [WebSocket 模式](docs/WebSocket-Mode.md) |
| 部署独立代理节点 | [Aether Tunnel](apps/aether-tunnel/README.md) |

## 贡献

- 提交 Issue 或 Pull Request 前阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。
- 行为变化、公共接口、数据模型、权限和依赖调整请先说明目标与影响。
- 安全问题不要公开提交，按 [SECURITY.md](SECURITY.md) 私下报告。

## 许可证

本项目使用 [Aether 非商业开源许可证](LICENSE)。允许个人学习、研究、教育、非盈利组织和企业内部非盈利性质的使用；售卖本软件、使用本软件提供付费服务或其他盈利用途需要单独授权。

当前许可证并非 OSI 定义的开源许可证，因此 Aether 应表述为“源码开放、非商业许可”项目。
