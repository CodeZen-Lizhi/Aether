# SOCKS 与 Tunnel 适用性核验

## SOCKS

本地锁定 `reqwest 0.12.28`、`wreq 6.0.0-rc.28`。已核对下载的对应版本官方 crate 源码：

- reqwest `src/connect.rs::connect_socks`：`socks5` 使用 `DnsResolve::Local`，`socks5h` 使用 `DnsResolve::Proxy`。
- wreq `src/client/conn/connector.rs`：`socks5` 使用 `DnsResolve::Local`，`socks5h` 使用 `DnsResolve::Remote`；`src/client/conn/proxy/socks.rs` 分别发送解析 IP 与原始域名。
- 本地网关 `resolve_proxy_url` 和 WS 客户端把已配置的 URL 交给库，没有上游大型安全重构引入的预解析/DNS guard。
- 本地既有前端设计与测试接受两种 scheme，因此它们的不同含义是需要保留的配置契约。

上游 `b08fa3bdb` 为其 DNS guard 将 `socks5` 自动转换为 `socks5h`。本分支不采用这项策略变更。只要现有实测符合各自 scheme，就不存在需要通过自动转换修复的缺陷。

新增 `execution_runtime/transport/proxy_dns_tests.rs` 使用真正的现有客户端构建/WS 握手入口和本机 SOCKS5 服务端，直接检查 CONNECT 的地址及 DNS 语义：

1. 普通 reqwest HTTP。
2. browser_wreq HTTP。
3. 普通 WebSocket。
4. browser_wreq WebSocket。

每条路径覆盖 `socks5h` 的 `.invalid` 域名和 `socks5` 的 localhost。远程域名不需要外网解析，fixture 不向实际目标建立连接；同时检查响应成功、Host/path 保留。最终 **4 passed、0 failed**，覆盖全部 8 种配置组合；命令和整合检查结论见 `verification.md`。

### IPv6 实测与测试边界

第一轮有 3 项通过，普通 reqwest HTTP 的连接本身成功，但断言收到 `Domain("[::1]")` 而不是二进制 IP 编码。对照锁定版本源码确认：

- reqwest 0.12.28 `src/connect.rs::socks::connect` 先本地解析 localhost，将 IPv6 包上 `[]` 组装目标 URI。
- hyper-util 0.1.20 `src/client/legacy/connect/proxy/socks/v5/mod.rs` 从 URI 取出 `[::1]`，`parse::<IpAddr>()` 不接受方括号，因此以 domain address 编码这个已经解析的数字地址。
- 这仍证明原始 `localhost` 没有交给代理解析。回归断言据此接受可以解析的数字 loopback 地址（包括该版本的带括号表示），仍拒绝未经本地解析的原始域名。没有用改 scheme、禁用 IPv6 或更换依赖来使测试通过。

该 fixture 只验证 DNS 选择、握手与请求的透传，不能证明所有外部 SOCKS 服务都接受该 IPv6 编码。这是锁定依赖的互操作性边界；本次没有当前代理因此连接失败的证据，未扩大为依赖升级。它也不构成采用上游自动改写 `socks5` 策略的理由。

## Tunnel

Tunnel 是部署在另一台机器、通过 WebSocket 连接本网关的配套 `aether-tunnel` 代理程序，区别于设置中填写 HTTP/SOCKS URL 的普通手动代理。

可确认的证据：

- 仓库仍保留 Tunnel 的代码与 README；它不是已删除模块。
- 仓库 `data/aether.db` 以 SQLite URI `mode=ro` 和 `query_only` 只读查询，节点合计 1、手动节点 1、Tunnel 节点 0。只输出计数，没有输出地址或凭据。
- 当前个人版容器使用 Compose 的命名卷 `aether-data:/opt/aether/data`，不是上述仓库数据库。不能把旧文件的计数等同于运行环境；运行镜像没有 shell，本次没有为查询引入调试容器、复制运行库或改动服务。
- Docker 运行列表未看到单独的本地 Tunnel 容器；这不证明其它机器没有 Tunnel。
- 上游 `ec95f2ca1` 不只是单点修复：它修改 17 个文件，增加 SETTINGS 校验/窗口协商并改变会话清理；上游 README 明确要求匹配网关与客户端的协议升级顺序。

结论：本次未取得当前使用 Tunnel 或发生相应卡流问题的证据。用户允许仅做第 1、4 项，并禁止引入新功能/改变既有协议，因此不移植此条件性升级；保持已有 Tunnel 代码和配置不动。此结论不是“Tunnel 无 bug”或“生产环境一定未使用”。
