# 上游来源与本地证据

基线：`slim-personal@58c445ddc`；上游只读参考 `upstream/main@17d01d7fe`。

| 需求 | 上游来源 | 本地证据 / 初步判断 |
| --- | --- | --- |
| R1 | `88d2b002b` | `crates/aether-ai/formats/src/formats/openai/chat/stream.rs:1843` 只忽略 keepalive；两文件 patch check 通过 |
| R2 | `36e9d21e3`，参考 `579f2c7cc` 对相同错误边界的前置改动 | `transport.rs:476` sanitizer 保留 userinfo；`:554` derive Debug，`:584` 直接格式化 UpstreamRequest |
| R3 | `b08fa3bdb` | `transport.rs:3950` 保留代理 URL；`handlers/proxy/websocket/transport.rs:234` 直接传给 wreq；原设计明确接受两个 SOCKS scheme |
| R4 | `ec95f2ca1` | 修改 17 个文件并引入 SETTINGS/流控协商等前置依赖；先核验必要性，不整体导入 |

代理语义依据：`.trellis/tasks/archive/2026-09/09-02-proxy-ui-polish/design.md:54`、`.trellis/spec/frontend/index.md:79`、`frontend/src/views/admin/system-settings/__tests__/ProxyNodeEditDialog.spec.ts:379`。现有两种 scheme 都能保存，不能把上游策略变化当成分支缺陷。

本次保留错误排障信息的依据：本地 `format_upstream_request_error`/`format_wreq_upstream_request_error` 附带 kind、URL 和底层错误链；任务明确禁止改变用户已有请求错误展示意图。sanitizer 只去除凭据 URL 部分，普通原因、host/path 和状态必须保留。不要恢复 Chat PII 模块或修改请求内容。

常规 specs 中不少是空模板，已读取；以已填充的项目规则、实际调用路径、上述设计和本任务约束为准，不借机填写无关模板。
