<p align="center">
  <img src="frontend/public/aether_adaptive.svg" width="120" height="120" alt="Aether Logo">
</p>

<h1 align="center">Aether · 个人自用版</h1>

<p align="center">
  <strong>自托管 AI API 网关（个人单机裁剪版）</strong><br>
  Claude / OpenAI / Gemini 统一接入与格式转换 · 渠道管理 · API Key · 用量统计 · 代理节点
</p>

<p align="center">
  <a href="#功能">功能</a> •
  <a href="#部署">部署</a> •
  <a href="#本地开发">本地开发</a> •
  <a href="#环境变量">环境变量</a>
</p>

---

## 简介

这是 [Aether](https://github.com/fawney19/Aether) 的个人自用二开分支（`slim-personal`）：在上游基础上**物理删除**了商业化（支付/套餐/用户钱包/返利/注册）、运营周边（公告/邮件/推送通知/审计日志/PII 脱敏/S3 备份/自更新/管理令牌/一键安装/模块开关系统/LDAP/OAuth 登录/全局 IP 黑白名单）与多数据库（Postgres/MySQL）支持，只保留个人单机所需的代理核心。

> 上游同步会大量冲突，本分支视作单向 fork，不再跟随上游合并。

## 功能

- **AI 代理核心**：Claude / OpenAI / Gemini 协议互转、流式透传、Responses WebSocket 模式、模型指令解析
- **渠道管理**：Provider / Endpoint / Key 管理，渠道 OAuth（Claude 订阅账号等）、渠道上游代理、渠道配额重置
- **路由管理**：Routing Profiles、最小候选选择、Key 熔断恢复
- **API Key**：用户 Key / 独立 Key、限流、并发限制、模型与 Provider 白名单、Key 级 IP 规则、有效期与自动删除
- **用量统计**：请求/Tokens/成本统计、模型与 Provider 维度、活跃热图、用量明细
- **代理功能**：Aether Tunnel 正/反向代理节点（保留），渠道级上游代理（保留）
- **单用户**：管理员账号密码登录，账号/密码在系统设置中自助修改

**已删除**：支付、套餐、用户钱包、邀请返利、用户注册、公告、邮件、推送通知、审计日志、PII 脱敏、S3 备份、在线自更新、管理令牌（CC Switch）、一键安装脚本、模块开关系统、LDAP、OAuth 登录、全局 IP 黑白名单、Gemini Files 管理页、视频任务管理页、号池管理页、用户管理页、健康监控、Postgres/MySQL/Redis。多用户体系裁剪为单管理员。

## 部署

### Docker Compose

```bash
git clone <你的 fork 仓库地址> aether
cd aether

# 配置环境变量
cp .env.example .env
./generate_keys.sh   # 生成 JWT_SECRET_KEY / ENCRYPTION_KEY 并写入 .env
# 编辑 .env 设置 ADMIN_PASSWORD（首次启动自举管理员）

# 本地构建并启动（SQLite + memory，单容器）
docker compose up -d --build
```

应用监听 `0.0.0.0:${APP_PORT:-8084}`，SQLite 数据落在 `./data/aether.db`。登录后台后先配置渠道（Provider），再创建 API Key 即可通过 `http://<host>:8084/v1/messages`（Claude 格式）或 `/v1/chat/completions`（OpenAI 格式）代理请求。

### 源码直跑

```bash
# 前端（首次）
cd frontend && npm install && npm run build && cd ..

# 后端（SQLite 默认）
cargo run -p aether-gateway --release
```

## 本地开发

依赖 Rust toolchain 和 Node.js（无需 Docker，SQLite + memory 运行时零外部依赖）。

```bash
make dev          # 同时启动后端 + 前端 Vite dev server
make dev-backend  # 仅后端
make dev-frontend # 仅前端
```

## Aether Tunnel（可选）

Aether Tunnel 是配套的代理节点程序，为无法直连上游的网络环境中转 API 流量。

- Docker Compose 部署或下载预编译二进制直接运行
- 通过 `aether-tunnel setup` 完成交互式配置
- 详细文档见 [apps/aether-tunnel/README.md](apps/aether-tunnel/README.md)

## 环境变量

- `APP_PORT`：`aether-gateway` 唯一监听端口，固定绑定 `0.0.0.0:${APP_PORT}`
- `AETHER_DATABASE_DRIVER` / `AETHER_DATABASE_URL`：SQLite（默认）连接配置
- `AETHER_RUNTIME_BACKEND=memory`：运行时状态后端，单机固定 memory
- `AETHER_GATEWAY_AUTO_PREPARE_DATABASE`：启动前自动执行挂起的 SQLite migration
- `JWT_SECRET_KEY` / `ENCRYPTION_KEY`：认证与敏感数据加密密钥（`generate_keys.sh` 生成）
- `API_KEY_PREFIX`：API Key 前缀，默认 `sk`
- `ADMIN_USERNAME` / `ADMIN_PASSWORD` / `ADMIN_EMAIL`：首次启动自举管理员
- `CORS_ORIGINS` / `CORS_ALLOW_CREDENTIALS`：前端跨域来源控制
- `RUST_LOG`：Rust 日志过滤，例如 `aether_gateway=info`
- `AETHER_GATEWAY_MAX_IN_FLIGHT_REQUESTS`：单实例请求并发上限（按 CPU 自动推导）
- `AETHER_MAX_REQUEST_BODY_MB`：单请求解压后请求体上限（0 为不限制）

## 与上游的差异维护

删除以阶段化 commit 落在 `slim-personal` 分支，按功能可读回滚：

1. `feat(frontend)` 前端瘦身
2. `feat(backend)` 后端周边模块删除（通知/邮件/OAuth 登录/LDAP/审计/备份/管理令牌/自更新/模块系统等）
3. `feat(backend)` 商业化核心删除（支付/套餐/钱包/返利 + 请求链路钱包闸门摘除）
4. `feat(data)` 数据层 SQLite 化 + 部署裁剪

保留例外：`aether-billing` 定价引擎（用量成本计算依赖）、`aether-oauth/provider`（渠道订阅账号 OAuth）、`aether-cache`（数据层依赖）、`aether-task`（后台 worker 运行时底座）、per-key IP 规则、隐私还原库壳（`privacy/`，脱敏入口已短路）。

## 许可证

沿用上游 [Aether 非商业开源许可证](LICENSE)。
