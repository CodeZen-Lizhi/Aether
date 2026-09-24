# 配置/公共契约实现交接

已完成独立字段：API `stream_total_timeout` 秒，1–1200，最多三位小数；缺省/缺失继承900秒，显式null清除。存储 `config.stream_total_timeout_ms`；直接config写入限制1000–1200000整数，null删除。通过共享normalize helper覆盖create/PATCH；新字段的缺失/null区分在create同样保留。显式API字段优先于已通过校验的config字段。保留旧failover_rules、request_timeout和未知JSON键。

contracts接口：`chat_retry::DEFAULT_STREAM_TOTAL_TIMEOUT_MS = 900_000`、`configured_stream_total_timeout_ms(Option<&Value>) -> Option<u64>`、`resolve_stream_total_timeout_ms(Option<&Value>) -> u64`。`ExecutionTimeouts.stream_total_ms: Option<u64>`，旧JSON缺省兼容；transport resolver填充新字段。Summary返回 `stream_total_timeout`、`effective_stream_total_timeout`、`effective_stream_total_timeout_source`（`config.stream_total_timeout_ms` / `default`）。

表单替换旧预算编辑为新总超时；首次设置改名首次响应超时并说明成功headers解除。新总时限说明普通流式/压缩、全程计时和清空900秒。保留旧预算数据不自动填入新控件；未修改的新字段不写回。输入、helper和英文翻译已同步。

修改文件：
- crates/aether-contracts/src/{chat_retry,plan}.rs
- crates/aether-provider/transport/src/network.rs
- crates/aether-admin/src/provider/{mod,timeouts}.rs（timeouts新增）
- apps/aether-gateway/src/handlers/admin/provider/shared/payloads.rs
- apps/aether-gateway/src/handlers/admin/provider/summary/value.rs
- apps/aether-gateway/src/handlers/admin/provider/write/provider.rs 与 provider/{create,update,stream_total_tests}.rs（stream_total_tests新增）
- frontend/src/api/endpoints/{providers.ts,types/provider.ts}
- frontend/src/features/providers/components/ProviderFormDialog.vue 与 __tests__/ProviderFormDialog.transfer-limits.spec.ts
- frontend/src/i18n/messages.ts

已通过：
- `cargo test -p aether-contracts --lib stream_total`：3通过（2项新增契约测试 + 1项匹配既有隧道测试）。
- `npm run test:run -- src/features/providers/components/__tests__/ProviderFormDialog.transfer-limits.spec.ts`：11通过，覆盖保存/重开、清空生效值/来源、旧字段保留、保存失败草稿、精度与范围。
- `npm run type-check`：通过。
- 受影响5个前端文件eslint：通过；新增测试后再次检查测试文件通过。
- 受影响Rust文件rustfmt --check及限定git diff --check：通过。

待主代理统一执行（避免与正在执行的gateway cargo check争用artifact lock，transport测试已中止等待，未宣称通过）：
- `cargo test -p aether-provider-transport --lib execution_timeouts`（含新增独立总时限验证）。
- `cargo test -p aether-admin --lib provider::timeouts`（2项新增校验/清空测试）。
- `cargo test -p aether-gateway --lib stream_total_timeout`（handler下2项新增create/PATCH/readback/invalid测试，与运行时过滤范围可整合）。
- 真实HTTP/SQLite及浏览器保存→清空→重读流程由主代理统一验收。这里的gateway handler测试走实际builder和summary，尚未执行，也不替代HTTP/SQLite证据。

唯一外部缺少Default的ExecutionTimeouts literal：apps/aether-gateway/src/video_tasks/tests/fixtures.rs:104，主代理应补 `stream_total_ms: None`（非本代理所有权范围）。未改runtime、usage、tunnel或spec；未提交/安装/发布。
