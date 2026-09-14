# Chat Retry Configuration Contract

Config owner implements the shared resolver in `aether_contracts::chat_retry`.

- `resolve_chat_max_attempts(provider_config: Option<&Value>, endpoint_config: Option<&Value>, endpoint_legacy: Option<i32>, provider_legacy: Option<i32>) -> EffectiveChatAttempts` exposes `max_attempts: u32` and `source: &'static str`.
- Precedence: provider `config.failover_rules.max_attempts`, legacy provider `config.failover_rules.max_retries`, endpoint `config.failover_rules.max_attempts`, endpoint legacy value other than 2, provider `config.failover_rules.provider_max_attempts`, provider legacy value other than 2, saved endpoint/provider 2, default 1.
- Canonical values are 1..99 total attempts. Legacy 0 is 1; legacy values above 99 retain raw storage and resolve to 99. Missing stored values remain missing. No read writes a default 2 into storage.
- `provider_max_attempts` records explicit writes to the provider-level legacy API field without promoting that field above endpoint overrides. Endpoint legacy API writes record endpoint `failover_rules.max_attempts` in its config.
- New failover rule writes use `max_attempts` and `chat_policy_version: 1`; unknown object fields and stop rules survive. New and old fields supplied in the same patch/scope must agree after legacy 0 normalization; conflicting writes are rejected. A canonical-only chat edit preserves stored legacy fields for non-chat compatibility; an old API write synchronizes the canonical chat field. Do not resubmit a preserved legacy alias alongside a newer canonical override.
- Retry runtime must call this helper only for chat; existing non-chat resolver remains unchanged.
- Budget is provider `config.failover_rules.stream_failover_budget_ms`, integer 1..1200000, default 90000. `aether-provider-transport::resolve_transport_execution_timeouts` projects it into `ExecutionTimeouts.stream_failover_budget_ms`. Existing gateway plan paths already consume this helper. Runtime applies it only to streaming chat / Responses WS and preserves the first logical request deadline across candidates.
- Read APIs return `effective_max_attempts`, `effective_max_attempts_source`, and provider `stream_failover_budget_ms`; endpoint reads resolve against the owning provider. Provider-level values describe its default before endpoint overrides. `chat_policy_version` and `legacy_effective_max_attempts` provide a read-only migration preview alongside the unchanged raw fields. Non-chat endpoint reads use `resolve_legacy_max_attempts` and do not advertise a chat version.

## Anonymous Migration Samples

| Rules | Endpoint raw | Provider raw | Chat effective | Legacy effective |
| --- | --- | --- | --- | --- |
| absent | null | null | 1 | 1 |
| absent | 2 | null | 2 | 1 |
| absent | null | 2 | 2 | 1 |
| absent | 2 | 5 | 5 | 5 |
| absent | 2 | 0 | 1 | 1 |
| absent | 7 | 2 | 7 | 7 |
| max_retries=2 | 7 | 5 | 2 | 2 |
| absent | 999 | 2 | 99 | 99 |

New endpoint `config.failover_rules.max_attempts=2` beats provider raw 5. Null endpoint `max_retries` PATCH removes its canonical override and restores inheritance. Reads are deterministic and do not mutate saved originals; there is no production migration operation in this change.

## Integration Status

Config implementation is written. Follow-up inspection confirms retry_runtime has wired `resolve_chat_max_attempts` in `orchestration/attempt.rs` with both config objects. Config owner did not edit that file.

The budget projection is in `crates/aether-provider/transport/src/network.rs`, consumed by existing gateway plan construction. One complete `ExecutionTimeouts` literal in video test fixtures was mechanically extended with None; active runtime owner files were untouched.

Validation obtained:

- `cargo test -p aether-contracts chat_retry --lib`: passed (3 tests, including final preview-helper assertion).
- `cargo test -p aether-admin provider:: --lib`: passed (137 tests), including endpoint save/clear inheritance.
- After preserving legacy non-chat values: `cargo test -p aether-admin provider::failover --lib`: passed (3 tests).
- `cargo test -p aether-provider-transport network::tests::transport_execution_timeouts --lib`: passed (5 tests); existing unused-import warnings in conversion.rs and request_url/mod.rs.
- Provider form, failover rules and key action tests: passed (15 tests); the final failover dialog alias-preservation edit was rerun (3 passed).
- Frontend type-check: passed. Targeted ESLint passed for form, failover dialog, API types, i18n and new test scope. ProviderDetailDrawer has 8 pre-existing unused-variable errors; its diff only adds recognition of the two new settings. Existing key-action test has one pre-existing `any` warning.
- rustfmt on owned Rust files and `git diff --check`: passed.

Not verified: gateway compilation/real management API, real HTTP/SSE/WS scheduling, and actual browser geometry. These remain main's integration work. Browser CLI was blocked writing its daemon cache outside sandbox; the available Chrome adapter then timed out. A UI-only fixture (mock adapter, no gateway/upstream calls) is available at http://127.0.0.1:5186/scripts/fixtures/chat-retry-config.html while the local Vite server runs.

Channel delivery itself failed with EPERM on `~/.trellis/channels/.../scheduler-failover-impl.lock`; this document is the durable handoff. No commits, production writes or paid upstream calls were made.

## Follow-up: Save and Refresh

- Added four management-route tests in `apps/aether-gateway/src/tests/control/admin/providers/chat_retry.rs`: repeat migration-preview reads preserve raw storage; endpoint explicit 2 survives PATCH/GET and null restores inheritance; provider budget PATCH/GET preserves unknown/stop rules and invalidates a prewarmed transport snapshot; rejected writes do not mutate stored config.
- Updated the existing provider update regression's three exact JSON assertions for the newly persisted scope field/version; existing rule preservation is still asserted.
- Extended the actual Vue form test host to receive emitted provider updates and open/close state. The new test edits 2/90000 to 3/45000, saves, reopens using the API result, verifies the authoritative source label, then saves again without creating another override.
- Latest frontend form run: 7 passed; type-check and targeted test-file ESLint passed. Rust tests formatted; `git diff --check` passed.
- Final combined frontend rerun (23:08): ProviderFormDialog 7, FailoverRulesDialog 3, ProviderKeyActionCluster 6, total **16 passed**. Frontend type-check and scoped ESLint for the changed form/dialog/key-action/types/i18n/test files passed. Drawer baseline findings remain separately documented above.
- Management-route tests have not been run by config, to keep gateway cargo execution owned by main. Run `cargo test -p aether-gateway --lib chat_retry_admin` and `cargo test -p aether-gateway --lib gateway_updates_admin_provider_locally_with_trusted_admin_principal` in main's serial gateway validation.
- While main/integration owned gateway compilation, config checked the newest available test binary (`aether_gateway-6a36c77018a5bdeb`, mtime 22:57:05); `--list chat_retry_admin` returned 0 tests because it predates these additions. This is **not** a passing smoke run. No second gateway cargo was started.
- Actual production projection chain verified in source: provider `failover_rules` -> `network::resolve_transport_execution_timeouts` -> OpenAI chat/Responses and family decision payload `timeouts` -> `aether-ai-serving/src/attempt_plan.rs` moves `payload.timeouts` into `ExecutionPlan` -> HTTP `execution_runtime/chat_retry.rs` and WS `responses/turn_state.rs` read `stream_failover_budget_ms`. Runtime behavior remains covered by main's scheduler failover suite, not inferred solely from this source trace.
- Added two further management-route regressions (six `chat_retry_admin` tests total): clearing a configured budget twice restores 90000 in GET and a previously warmed transport snapshot, removes the stored field and preserves unrelated rules; creating an embedding endpoint with legacy 2 and then editing canonical chat attempts keeps embedding at its legacy effective 5 while chat reads 3.
- These six tests exercise real local management handlers backed by the in-memory repository. They do not establish SQLite restart durability. The 5186 fixture's localStorage is UI-only and is not management API persistence evidence.
- Per main's explicit cargo ownership instruction, no cargo command was run for these additions. Scoped rustfmt and `git diff --check` passed; integration must execute the `chat_retry_admin` filter after compiling the current sources. Main is performing browser verification against the unchanged running 5186 fixture.
- Added `chat_retry_admin_create_refresh_preserves_absence_and_explicit_values` (seven smoke tests total). It creates providers through POST with omitted values, legacy explicit 2, and canonical 3/45000; repeated summary GETs assert raw/effective values and sources. Creating a chat endpoint without attempts must retain raw null and inherit the same effective source. Repository assertions check raw provider absence and canonical/unknown rule persistence. This addition is formatted but unexecuted under the same no-cargo instruction; the fixture was not edited.
