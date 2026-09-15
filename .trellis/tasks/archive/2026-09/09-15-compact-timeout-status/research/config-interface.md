# 配置接口协调

已加入 `aether_contracts::chat_retry::{DEFAULT_STREAM_TOTAL_TIMEOUT_MS = 900_000, resolve_stream_total_timeout_ms(config: Option<&Value>) -> u64}`。
`ExecutionTimeouts.stream_total_ms: Option<u64>` 由 transport resolver 填入，不复用旧 `total_ms` / 首输出预算。
API 秒字段：`stream_total_timeout`（原始覆盖值/null），`effective_stream_total_timeout`（生效秒数），`effective_stream_total_timeout_source`（`config.stream_total_timeout_ms` 或 `default`）。

静态扫描发现唯一缺少 `..Default` 的外部 ExecutionTimeouts literal：`apps/aether-gateway/src/video_tasks/tests/fixtures.rs:104`。需运行时/主代理加入 `stream_total_ms: None`，配置代理不改此所有权范围。

2026-09-15 12:35：配置/表单已完成，gateway handler 新增 `stream_total_tests` 两项创建/PATCH/readback/无效值/旧配置保留测试，待统一 gateway test 编译。前端 10 项组件测试和定向 eslint 已通过。`cargo test -p aether-contracts --lib stream_total` 3 项通过。transport 定向测试遇共享 artifact lock，已立即中止（退出130），请主代理统一跑 `cargo test -p aether-provider-transport --lib execution_timeouts` 与 `cargo test -p aether-admin --lib provider::timeouts`。
