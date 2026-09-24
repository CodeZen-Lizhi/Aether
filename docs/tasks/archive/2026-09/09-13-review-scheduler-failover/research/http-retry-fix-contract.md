# HTTP retry follow-up

Owner: retry_fix. Source implemented; verification is coordinated with integration's exclusive gateway Cargo process. This is not full A1-A11 acceptance.

## Changed behavior

- Sync chat HTTP 200 error envelopes and invalid terminal shapes are promoted to a failure before analysis, health, and successful affinity effects. Existing Responses `completed` and `incomplete` terminal statuses remain accepted, including length/content-filter termination.
  Repository evidence: `execution_runtime/attempt_lifecycle.rs::classify_attempt_provider_effect` and `settlement_table_row_legitimate_incomplete_is_billed_without_provider_failure` explicitly separate a billing-layer failed status from provider failure for legal `response.incomplete`; the latter asserts billed usage and `ProviderSuccess`. `formats/shared/stream_core/format_matrix.rs::a_legitimate_incomplete_agrees_across_both_entries` asserts an observed terminal with no parser error for `max_output_tokens`. The HTTP validator follows this existing contract; legal partial output must not become an automatic retry or health penalty merely because its status is `incomplete`.
- `ExecutionRuntimeTransportError::{UpstreamTimeout,LocalConfiguration}` preserve typed provenance. Real reqwest/wreq send errors are classified before string conversion; local builder/proxy/body/configuration errors stay neutral, upstream timeout scores one, connection/TLS/protocol failures score two. Sync stores the source; SSE consumes it directly through `ClassifiedHealthFailure`. Compatibility non-chat effect dispatch remains existing behavior.
- HTTP Responses with a non-null `previous_response_id` fail with 409 before redaction, conversion, or history hydration. Existing HTTP history does not prove the original physical upstream binding. The runtime also rejects references still present in a plan. WS has its separate verified ownership path. Independent complete-context requests continue ordinary failover.
- Attempt capture compares `planned_chat_credential_fingerprint` with captured `transport_fingerprint` and refuses stale plans before sending. No settlement-time recapture or fabricated identity is introduced.
- The five chat ownerless planner/runtime probe claims are removed. Sync acquires after execution gate; SSE acquires after execution gate and target admission. Both capture the credential fence after managed claim, so claim storage waiting cannot hide an intervening rotation, and attach the exported `PROBE_LEASES_REPORT_FIELD` array to the exact report used by runtime and watchdog.
- `ChatProbeSession` owns the original managed guards in a bounded monitor; scoped execution and the SSE pump retain the session until terminal effects finish. There is no re-claim or reconstructed guard. Terminal success/failure marks the known model outcome before effects, so owner consumption by the effect cannot replace that outcome with a lease error. Explicit finish releases after effects; dropping a cancelled scope releases through the existing guard worker.
- Lease loss cancels the live upstream future, records a neutral Cancelled candidate diagnostic, and ends an already-started body with an IO error. It cannot trigger replay. Probe streams use the existing spawned pump instead of inline passthrough so the owner covers the body terminal.
- OpenAI chat streaming previously performed a second randomized pressure-based selection over two already-ranked candidates. The three complete modes and absent-policy fallback now preserve the ranked linear order. Only explicitly retained legacy `LoadBalance` uses that preselection.

## Verification

Integration reported the first real-router run after fixture `/v1` repair: six of seven passed, including `http_200_error_envelope_retries_and_never_restores_health`; the failing budget case exposed the second streaming selection above. After the correction, a 12-test snapshot passed nine tests: all original six and F1/F2. The two probe fixtures incorrectly seeded runtime health via the configuration-only `update_key`; these now use `update_key_health_state` and assert the persisted zero score before requesting. The remaining Responses fixture required client authorization for that format; it has been updated.

At 2026-09-14 00:06 Asia/Shanghai, integration reported **all 12 HTTP router tests passed in 19.52 seconds**, including F1-F3, shared budget, due probe .1 recovery, and SSE owner/cancellation lifetime. At 00:07, the **latest capture-after-claim snapshot compiled and all 12 HTTP tests passed again in 17.70 seconds**. That same binary also passed the separate WS gateway smoke owned by websocket.

New real-router tests in `tests/scheduler_failover/http_failures.rs`:

- HTTP 200 error envelope: K1/K1/K2, K1 health .6, subsequent affinity K2.
- Sync client with forced upstream streaming: real typed first-byte timeout, K1 .9, one failure.
- Connection refusal .8 versus malformed local URL 1.0.
- HTTP reference rejected before cross-format history expansion, zero upstream requests.
- Due circuit probe makes one real request and recovers .1.
- SSE body holds owner after useful text, competing request uses backup, client cancellation releases without restoring health.

Specified-file rustfmt and `git diff --check` pass. Latest test build compiles the changed production and test scope. Long (>60s) HTTP renewal and lease-loss-during-output runtime scenarios remain unverified; ordinary SSE cancellation/owner exclusion is verified. Gateway clippy was not run by this worker because integration owns the Cargo slot. No paid upstream, commit, push, or production mutation.
