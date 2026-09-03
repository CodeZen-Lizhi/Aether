# Sub2API 入站 gzip 请求体支持调查

调查日期：2026-09-03

## 结论

上游开源项目 [Wei-Shaw/sub2api](https://github.com/Wei-Shaw/sub2api) 的 `main` 分支提交
[`b1748c4ea99ce2120401a269142aa071e18a84da`](https://github.com/Wei-Shaw/sub2api/tree/b1748c4ea99ce2120401a269142aa071e18a84da)
支持客户端对 API 请求体使用 `Content-Encoding: gzip`（也支持 `x-gzip`）。这不是仅支持压缩响应：网关读取入站请求体时会解压 gzip，再交给 API handler 解析。

该能力由提交 [`798fd673e95deeacac079458686b19999006cd2b`](https://github.com/Wei-Shaw/sub2api/commit/798fd673e95deeacac079458686b19999006cd2b) 引入，首个包含它的发布标签为 `v0.1.136`。

因此，对运行该代码或包含该逻辑的后续版本的 Sub2API 实例，Aether 可以发送 gzip 压缩的 JSON 请求体，并附带 `Content-Encoding: gzip`。

## 源码证据

1. [`ReadRequestBodyWithPrealloc`](https://github.com/Wei-Shaw/sub2api/blob/b1748c4ea99ce2120401a269142aa071e18a84da/backend/internal/pkg/httputil/body.go#L25-L65) 读取 HTTP 请求体后检查 `Content-Encoding`，并在解压成功后删除该请求头和旧的 `Content-Length`，将长度更新为解压后的值。
2. [`decompressRequestBody`](https://github.com/Wei-Shaw/sub2api/blob/b1748c4ea99ce2120401a269142aa071e18a84da/backend/internal/pkg/httputil/body.go#L78-L103) 对 `gzip`、`x-gzip` 调用 Go 标准库 `gzip.NewReader`。该函数还支持 `zstd` 与 `deflate`；解压后的请求体上限为 64 MiB。
3. Codex 使用的 [`/backend-api/codex/responses`](https://github.com/Wei-Shaw/sub2api/blob/b1748c4ea99ce2120401a269142aa071e18a84da/backend/internal/server/routes/gateway.go#L369-L376) 路由转发至 `responsesHandler`。OpenAI Responses handler 随后通过 [`readLenientJSONRequestBodyWithPrealloc`](https://github.com/Wei-Shaw/sub2api/blob/b1748c4ea99ce2120401a269142aa071e18a84da/backend/internal/handler/openai_gateway_handler.go#L383-L390) 读取请求体；该 helper 最终调用上述请求体解压函数（见 [`request_body_limit.go`](https://github.com/Wei-Shaw/sub2api/blob/b1748c4ea99ce2120401a269142aa071e18a84da/backend/internal/handler/request_body_limit.go#L32-L33)）。
4. 仓库包含直接构造 gzip 请求并断言解压结果的测试 [`TestReadRequestBodyWithPrealloc_DecodesGzip`](https://github.com/Wei-Shaw/sub2api/blob/b1748c4ea99ce2120401a269142aa071e18a84da/backend/internal/pkg/httputil/body_test.go#L61-L79)。在该提交的源码副本上执行：

   ```text
   go test ./internal/pkg/httputil -run '^TestReadRequestBodyWithPrealloc_DecodesGzip$' -count=1
   # ok   github.com/Wei-Shaw/sub2api/internal/pkg/httputil
   ```

## 与响应压缩的区别

部署目录中的 [`Caddyfile`](https://github.com/Wei-Shaw/sub2api/blob/b1748c4ea99ce2120401a269142aa071e18a84da/deploy/Caddyfile#L63-L88) 配置的是服务器向客户端返回内容时的 `gzip`/`zstd` 压缩，并明确避免压缩 SSE。这一配置不能单独证明入站请求支持 gzip；本调查的结论以请求体解压代码和其测试为准。

## 实施边界

- 上述结论证明的是公开源码在该提交中的行为。公共 Sub2API 站点可能使用旧版本、二次开发版本，或在应用前加了不透明的 CDN/WAF/反向代理；不应仅凭仓库源码断言目标实例已部署该版本。
- 实施时必须同时发送压缩后的字节和 `Content-Encoding: gzip`，由 HTTP 客户端重新计算 `Content-Length`。不可保留原始长度。
- 不应对同一个生成请求在 gzip 失败后自动以原始请求重试，除非有明确的幂等保护；否则可能重复消耗上游额度或产生两个执行任务。
- gzip 只能减少 Aether 到 Sub2API 的请求上传部分，不能缩短 Sub2API 已开始计时后的上游首字时间；实际收益须用同模型、同会话的 A/B 请求测量。
