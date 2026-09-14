# Real Gateway Failover Fixture Contract

Owner: integration. Scope: new test files only; production behavior remains owned by the other implementers.

## Entry and Registration

- Add `mod scheduler_failover;` to `apps/aether-gateway/src/tests/mod.rs` (main owns this one-line registration).
- New root: `apps/aether-gateway/src/tests/scheduler_failover.rs`; support: `tests/scheduler_failover/support.rs`.
- Run serially under main's gateway cargo ownership: `cargo test -p aether-gateway --lib tests::scheduler_failover:: -- --test-threads=1`.
- Reuse `tests::start_server`, `AppState::new`, `build_router_with_state`, and the existing memory auth/candidate/catalog/request-candidate injection. Actual requests enter `/v1/chat/completions` over loopback HTTP, then use the production transport to separately listening scripted upstreams. No execution-runtime override or fake serving port is installed.

## Observable Contracts

- Upstream handlers record receipt time, target and parsed request body. Assertions count these actual receipts, not planner slots or synthetic reports.
- `provider.max_retries = 2` exercises legacy stored-2 normalization. Explicit `provider.config.failover_rules.max_attempts` exercises normalized total attempts. First-output budget is set only at `provider.config.failover_rules.stream_failover_budget_ms`, following the config owner's canonical interface. There is no test-only timeout injection.
- Routing mode and provider priorities come from an isolated system-default `StoredRoutingGroup` repository. Key prices come from `default_rate_multiplier`.
- K1 health is read from the shared catalog repository after completion, using normalized `health_by_format["openai:chat"].health_score`; expected 0.6 means user score 6.
- Scripted SSE sends a valid text delta before an explicitly released stream failure; client observation releases failure so the test proves delivery precedes disconnect.
- All listeners use existing loopback binding and are aborted on fixture drop, including assertion failure. Client deadlines bound each scenario.

## Evidence Boundaries

The reusable pressure testkit currently seeds fixed configuration and does not expose catalog/routing mutation, so extending it would cross another file owner. Existing chat failover tests intercept `/v1/execute/sync`; this suite deliberately uses the actual provider HTTP transport.

SQLite can start through `AppState::with_data_config` plus `prepare_database_startup` (see `src/data/tests.rs`), but requires SQL auth/catalog seeding and migrations. This suite uses isolated memory repositories; storage owner remains responsible for memory/SQLite settlement parity. Management API save/refresh, actual Codex client, and WS are separate acceptance evidence.

## Delivery and Budget Recheck

Six tests cover explicit/legacy two attempts and score 6, the three modes after backup success, lower price over priority/affinity, short/long Retry-After including a request after two seconds, shared provider-configured deadline across two real upstreams, and client-observed SSE delivery before disconnection.

The initial system-config mismatch has been corrected in the retry owner's current source. Static recheck confirms this production chain:

1. `provider.config.failover_rules.stream_failover_budget_ms` is read by `aether_contracts::chat_retry::resolve_stream_failover_budget_ms`.
2. `aether-provider-transport::resolve_transport_execution_timeouts` projects that value into `ExecutionTimeouts.stream_failover_budget_ms`; the OpenAI chat decision payload consumes the helper.
3. `executor/stream_path.rs` calls `with_stream_first_output_budget`, which records the logical request start before candidate planning. It no longer reads a system-config budget.
4. `ChatRetryTracker::prepare_attempt` configures the deadline once from the first plan, relative to that original request start. Subsequent candidate plans do not reset it.

The fixture sets only the provider field and installs no system-config value or task-local timeout override. This closes the previously reported static wiring discrepancy; runtime behavior still requires the real-route test run by main.

The latest config and storage contracts have also been read. The fixture's memory catalog implements the atomic settlement repository contract, and health assertions continue to read the resulting authoritative key state. It does not synthesize settlement receipts or bypass production health effects.

The Retry-After test checks at least 900 ms elapsed for a one-second upstream delay. Whole-second storage must not silently turn that into an immediate retry. Long Retry-After is tested again after 2100 ms, when a wrongly clipped two-second cooldown would have expired.

Validation: targeted `rustfmt --check` and patch whitespace checks; gateway compilation/tests deferred to main's serial cargo ownership. Tests are not marked ignored, and no runtime pass is claimed before main registers/runs the module.

Channel delivery attempt failed with EPERM opening `~/.trellis/channels/.../scheduler-failover-impl.lock`, outside this worker's writable roots. This file and final response provide the report; no permission bypass attempted.

## Fixture Diagnosis After Registration

Main's first serial run failed all six tests with candidate exhaustion and no scripted handler receipts. The fixture configured endpoint `base_url` as the listener root, while the production `request_url` resolver uses `build_openai_chat_url`, which appends `/chat/completions` without adding `/v1`. The fixture only serves `/v1/chat/completions`; the original requests therefore missed that route.

The endpoint now uses the listener's `/v1` API base. The fixture additionally checks the actual upstream Authorization header against the target's synthetic plaintext credential, covering the catalog-to-Fernet-decryption-to-transport chain. Each gateway request has a unique trace ID and failed requests print their stored candidate records. Existing attempt order, health, cooldown, budget and delivery assertions remain intact.

Integration received exclusive permission for the serial gateway filter. The first compilation encountered concurrently edited production/test-helper errors in `orchestration/effects.rs`, `execution_runtime/chat_retry.rs`, and `execution_runtime/server.rs`; these were reported to main and their owners. Runtime acceptance remains pending a successful build and test execution. Targeted rustfmt checks pass.

## Runtime Result After Fixture Fix

Command: `cargo test -p aether-gateway --lib tests::scheduler_failover:: -- --test-threads=1`.

After the shared compile fixes, compilation completed in 1m38s and the serial run completed in 16.47s: **6 passed, 1 failed** (exit 101). Passing tests prove actual upstream requests and Authorization decoding for explicit/legacy two attempts with health 0.6, routing modes after backup success, lower multiplier over affinity, short/long Retry-After, delivered SSE disconnection without replay, and the retry owner's new HTTP 200 error-envelope test.

The remaining shared-output-budget test received `[1]`, not `[0, 1]`. Its sole stored candidate was `candidate_index=1`, `key-1`, `Cancelled`, status 504, error type `stream_failover_budget_exhausted`, and latency 758ms under the configured 1100ms logical budget. No K0 upstream request occurred. This does not prove cross-candidate budget behavior.

Static diagnosis identifies a production ordering violation in `apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/plans/stream.rs`: `next_raw_attempt_with_target_select` (around line 201) unconditionally prefetches two attempts by default (constant at line 35), then `select_target_index` (around line 367) compares target load, selection pressure and a clock/counter-derived randomized tie break. It does not preserve the configured scheduler mode's candidate order. A one-attempt configuration puts K0 and K1 into the window and can select K1 first. Two attempts put K0/K0 into the first window, masking the violation in the other test setup. This is an R1 SSE production issue, reported to main and retry_fix for production ownership; the fixture must not disable target selection through an environment override or weaken order assertions.

The remaining compiler warning is the pre-existing unreachable pattern at `apps/aether-gateway/src/dispatch/refs.rs:71`. Fixture rustfmt and patch whitespace checks pass. No production source was changed by integration in this diagnosis turn.

## Latest Production-Capture Verification

After main removed unfenced health fallback, integration verified the real HTTP fixture never supplies `chat_health_attempt`, a fence or a synthetic settlement identity. Sync/SSE capture is performed by `executor/candidate_loop.rs` through `capture_attempt_report_context`; the latest runtime owner revision captures after managed probe claim and passes the same report to execution/terminal handling.

The retry owner's first probe fixtures used the admin `update_key` operation, which intentionally preserves live health/circuit fields (`memory.rs::merge_admin_key_update`). They did not actually create zero-score/open-circuit state. The owner corrected this to `update_key_health_state` and added an authoritative pre-request zero-score assertion. This changes initial state setup only; recovery remains expected 0.1, owner retention/cancellation assertions remain unchanged, and no fence is fabricated. The owner also enabled the fixture's Responses permission/conversion and fixed the production stream second-sort issue described above.

Latest serial commands and observed results:

- `cargo test -p aether-gateway --lib tests::scheduler_failover:: -- --test-threads=1`: **12 passed**, first 19.52s and then 17.70s on the latest capture-after-claim compiled snapshot. Covers the original six plus abnormal 200, typed first-byte timeout, upstream connection refusal versus neutral local transport configuration, HTTP state-reference rejection, due probe recovery 0.1, and SSE probe ownership through delivery and cancellation.
- `cargo test -p aether-gateway --lib handlers::proxy::websocket::responses::route_smoke:: -- --test-threads=1`: **1 passed**, 3.11s. The authenticated gateway WS route uses production planner/execution/capture and verifies K1/K1/K2 plus persisted K1 0.6 and K2 0.7.
- `target/debug/aether-schema check --require-tables-from crates/aether-data/adapters/sqlite/migrations/20260913001000_add_provider_health_pending_facts.sql`: **passed**.

These results supersede the earlier 6/7 and 9/12 runtime snapshots. They prove local simulated HTTP/SSE/WS paths only, not real Codex client behavior or paid-provider behavior. Data pending/lease-expiry and migration Rust filters are being run separately at main's request.

## Expanded Checkpoint Requested by Main

All commands remained serial. Passing results at this checkpoint:

| Scope/filter | Result |
| --- | --- |
| `aether-data` + `aether-data-sqlite`, `health_` | 5 + 5 passed; pending persistence/reopen, duplicate facts, independent connections, transaction-time lease expiry |
| `aether-data`, `sqlite_core_export_covers_every_portable_table` | 1 passed |
| `aether-scheduler-core`, `health::` / `ranking::` | 20 / 29 passed |
| `aether-ai-serving`, `attempt_loop::` / `candidate_ranking::` | 4 / 4 passed |
| Gateway `handlers::proxy::websocket::responses::` | 183 passed, including the actual authenticated WS route smoke |
| Gateway `scheduler::` / `cache::` / `affinity` | 49 / 85 / 89 passed; overlapping filters are not distinct total test counts |
| Gateway `request_diagnostics` / `task_runtime` | 7 / 8 passed |

Gateway's last five filters were run directly against the Cargo-produced test executable to avoid rebuilding while owners were fixing tests. The first direct diagnostics run aborted from stack overflow because it omitted `.cargo/config.toml`'s `RUST_MIN_STACK=33554432`; re-running with that exact repository setting passed all seven. It does not indicate a product source change. Logs for direct filters are under `output/scheduler-failover-verification/`; the first diagnostics log records the aborted environment-mismatched attempt.

Open checkpoint failures, reported to owners for correction and rerun:

- Migration 11/12: expected enabled-version list omits the actual new `20260913001000` pending migration. Logical schema equality and actual migration execution passed.
- Orchestration 167/168: old eight-concurrent-light-failure test expects streak 8 but observes 6 after zero-score opens a new circuit epoch. Main is updating the no-lost-update test without removing old-epoch fencing; missing-fence, pending reprojection, probe owner and old-success tests already passed.
- Admin chat retry 6/7: endpoint save/refresh smoke fails local route resolution. Main owns the fixture correction.
- Candidate loop 21/22: legacy dynamic mock omits the managed chat claim now responsible for zero-score recheck. Retry owner is aligning the test with the actual gate; real HTTP due-probe/SSE exclusion tests passed.

Main added a thirteenth HTTP scenario for 503 with Retry-After; its final snapshot run is still pending here.

## Corrected-Checkpoint Results

Main corrected the fixtures while preserving their assertions: four concurrent same-epoch failures prove no lost updates without crossing a circuit generation; admin endpoint update uses the production PUT method; dynamic mock execution now claims managed admission and captures a real health fence before execution, with a complete provider and zero-transition seed.

After recompilation:

- Gateway `orchestration::`: **168 passed**, 13.67s, including missing fence, pending reprojection and corrected concurrent test.
- Gateway `chat_retry_admin`: **7 passed**, 2.26s.
- Gateway `executor::candidate_loop::`: **22 passed**, 1.46s.
- Gateway `tests::scheduler_failover::`: **13 passed**, 20.56s. The new 503 Retry-After scenario proves the full 30-second shared cooldown persists across the later request. Earlier 13-test artifact also passed in 20.43s.

The last three used the freshly compiled test binary with the same `RUST_MIN_STACK=33554432` setting as Cargo. Complete output is in `output/scheduler-failover-verification/final_*.log`.

Final reviewer subsequently identified the gateway consuming a local duplicate pending payload despite immutable enqueue retaining the first stored fact. Main is correcting enqueue to return the authoritative original fact and adding a regression. These checkpoint results precede that correction; the affected settlement/API regression and final lint are pending the owner's stable snapshot. Previously passing unrelated filters will not be repeatedly rerun.

## Final Reviewer Target-Permit Follow-up

Main delegated the target-admission lifetime fix to integration. `executor/candidate_loop.rs` now transfers the existing chat target permit into the full-response body guard rather than dropping it when the watchdog returns a streaming response. The global execution permit still follows its original configurable hold mode. There is no second target claim. Target queue saturation is handled as a candidate-level local admission timeout, so an available backup can execute without scoring the occupied target.

The new `target_admission.rs` real-route test configures the production `UpstreamTargetAdmission` with limit 1 in the isolated AppState. It observes useful first-SSE content, requires target in-flight to remain 1, starts a second stream which must use K2, verifies K1 has no extra upstream receipt or health penalty, then checks both normal terminal and client cancellation release capacity and let the next request use K1. Fixture changes add a controlled terminal SSE reply and target occupancy reads; no health fence or settlement report is injected. Main's non-JSON-200 tests use the added `RawSuccess(content_type, body)` reply.

Scoped rustfmt and diff whitespace checks pass. The combined HTTP16 runtime filter is compiling at this checkpoint; final pass/failure evidence will follow.

## Final Reviewer Fix Verification

- `cargo test -p aether-gateway --lib tests::scheduler_failover:: -- --test-threads=1`: **16 passed**, 25.03s, after all three reviewer fixes. Includes target occupancy/backup/terminal/cancel/reuse, abnormal HTTP 200 HTML/plain/empty inputs and valid forced-upstream-SSE compatibility.
- `cargo test -p aether-gateway --lib duplicate_pending_delivery_settles_the_original_fact_and_timestamp -- --test-threads=1`: **1 passed**, 1.19s. Gateway duplicate delivery settles the authoritative first queued payload/time.
- `cargo test -p aether-data -p aether-data-sqlite --lib health_pending`: **1 memory + 1 SQLite passed**, including enqueue's returned original fact and SQLite reopen preservation.
- Migration inventory fix: `cargo test -p aether-data --lib lifecycle::migrate::tests`: **12 passed**, 2.28s.
- Scoped `rustfmt --check` for integration's production/helper/new test files and `git diff --check`: **passed**.

Final scoped clippy runs `cargo clippy -p aether-gateway -p aether-data -p aether-data-sqlite -p aether-scheduler-core -p aether-ai-serving --lib --tests --no-deps`, with output at `output/scheduler-failover-verification/clippy.log`; outcome is pending at this entry.

The first scoped clippy exited 101 with one blocking lint: `clippy::never_loop` in `execution_runtime/sync/execution.rs:2179` (the normalization loop always breaks once). Main owns its expression-block correction. Remaining findings were warnings; none targeted integration's target-permit/helper/new regression files. Reviewer confirmed the target-permit fix and runtime evidence closes N1. Main is adding the separate real >60-second probe-lifetime test and correcting the reviewer's Responses authoritative-terminal compatibility finding before the final targeted checks.

Actual long-duration result: `cargo test -p aether-gateway --lib tests::scheduler_failover::probe_lifetime:: -- --test-threads=1`: **1 passed in 83.18s**. It waits a real 62 seconds past the initial 60-second lease, confirms the same owner with a renewed expiry, verifies an independent request uses the backup, replaces the circuit epoch/owner and observes a body I/O error within the 30-second bound. Health remains zero, no replay occurs, and old cleanup preserves the replacement owner. Useful model output also keeps the stream alive beyond the configured 2-second first-output budget. This command used the already-started pre-N4 snapshot; the independent Responses terminal-compatibility fix and final clippy remain pending main's stability signal.

## Final Stable Snapshot — N4 and Lint

Main's N4 test initially failed to compile because its test module omitted `base64::Engine`; main added the trait import without changing assertions. The corrected stable snapshot passed:

- `cargo test -p aether-gateway --lib responses_sync_success -- --test-threads=1`: **2 passed**, preserving authoritative same-family terminal extensions while rejecting cross-format extensions and unknown intermediate events.
- The same freshly compiled executable, with repository `RUST_MIN_STACK=33554432`, ran `tests::scheduler_failover::http_failures:: --test-threads=1`: **9 passed**, 11.81s, including non-JSON 200 failures and valid forced-upstream SSE normalization.
- Final scoped clippy for `aether-gateway`, `aether-data`, `aether-data-sqlite`, `aether-scheduler-core`, and `aether-ai-serving`, `--lib --tests --no-deps`: **exit 0**, 1m13s. Warnings remain in the wider checked scope; this is not a warning-free claim. No blocking lint or warning targets integration's target-permit/helper/new regression files. Final log: `output/scheduler-failover-verification/clippy.log`; the prior failing log is retained as `clippy.first.log`.
- Integration-owned files' `rustfmt --edition 2021 --check` and `git diff --check`: **passed**.

Main confirmed the N4 sync-only correction and mechanical warning cleanup do not require repeating the already-passing 83.18-second probe-lifetime test. Reviewer's N1–N4 findings are closed; all integration Cargo processes have exited and exclusive Cargo ownership is returned to main. These tests use local synthetic upstreams and production capture/transport paths, not paid providers or real Codex-client sessions.
