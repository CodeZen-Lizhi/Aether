# Chat Scheduling and Failover

## 1. Scope and trigger

This contract covers chat generation over HTTP, SSE and Responses WebSocket.
Image, video and embedding policies retain their existing behavior. Read this
before changing candidate ordering, retries, provider health, recovery probes,
terminal effects or the provider retry settings.

## 2. Signatures and ownership

- `aether_contracts::chat_retry::resolve_chat_max_attempts(...)` is the shared
  resolver for actual attempts and management effective-value readback.
- `ExecutionTimeouts.stream_total_ms` carries the independent HTTP chat-stream
  total deadline to `execution_runtime::chat_retry`. The legacy
  `stream_failover_budget_ms` field remains compatible for other paths.
- `capture_chat_health_attempt(state, plan)` captures a new identity before
  each actual model attempt. Reports carry `chat_health_attempt` and the planned
  credential fingerprint. Never create a replacement identity at settlement.
- `try_claim_managed_local_{circuit,rate_limit}_probe` returns an owned guard.
  Attach each guard report under `probe_leases`, hold it through the body/turn,
  observe `lost()`, settle the terminal outcome, then finish the guard.
- `ProviderCatalogKeyHealthPendingFact` stores the immutable, redacted terminal
  DTO. Enqueue returns the first stored fact for the exact identity; duplicate
  delivery must consume that authoritative payload and observation time.
  `enqueue/list/remove_*health*fact` and receipt lookup support retry after
  database failure. `settle_provider_catalog_key_health_attempt` atomically
  inserts a receipt and updates health with credential, circuit, snapshot and
  probe-expiry conditions. Memory and SQLite must implement the same contract.
- Operational tables: `provider_key_health_settlements` and
  `provider_key_health_pending_facts`. Keep logical schema, audit SQL, additive
  migrations and backup coverage synchronized. Portable exports retain committed
  receipts but exclude pending work; a full SQLite file retains both tables.

## 3. Runtime and configuration contracts

`max_attempts` includes the first request: 2 means initial + one retry. Existing
saved 2 must not be discarded as an inherited default. Preserve explicit
overrides, unknown failover rules and request/response transformation rules.
Management must show the effective value and its source, not just a raw field.

For HTTP chat streams (including Responses V2 `compaction_trigger`), successful
final upstream response headers end the candidate first-response wait. Do not
wait for a body byte or useful model output; informational 1xx and the tunnel
relay's outer HTTP 200 are not successful upstream headers. Preserve the saved
`stream_first_byte_timeout` field and default while labeling it first-response
timeout in management (default 30 seconds). Nonstream requests, including
nonstream compact, plus WS, image and video retain their existing limits.

An independent logical-request deadline caps the full HTTP stream, including
selection, admission, retries, generation and body forwarding. Its default is
900000 ms. Store the override at provider `config.stream_total_timeout_ms`;
management `stream_total_timeout` uses seconds (1–1200, at most 3 decimals).
Omission preserves the current override; explicit null clears it. Readback
includes raw/effective values and `effective_stream_total_timeout_source`
(`config.stream_total_timeout_ms` or `default`). Preserve unknown config keys and
legacy failover rules, and never reinterpret an old failover budget as this
new total limit. The first selected provider fixes the total deadline relative
to the original accepted time; later headers, output and retries do not reset it.
Do not add a post-headers idle or semantic-output timeout.

Semantic pre-read/commit decisions remain independent of timeout. Responses
text, reasoning, refusal, tool and compaction snapshots still participate in
useful-output detection; compaction requires nonempty `encrypted_content`.
Headers do not prove success, item completion is not response completion, and
empty/unknown items do not qualify. Record headers, first body, first effective
output and logical end as distinct milestones; timeout preserves already
observed milestones. Keep `stream_timing` and `timeout_trigger` in both usage
metadata merge allowlists; use existing `end_to_end_time_ms` and
`end_to_end_first_byte_time_ms` for request-wide display. UI explanations must
distinguish these clocks. Log event
types and timings, never output text or encrypted payloads. Early errors may retry only before the existing
protocol/tool/model commitment boundary; errors or premature EOF afterward
remain failed and must not replay the generation.

Transfer the same deadline owner explicitly from the request task to the body
pump before returning Response. Recheck ownership under lock before claiming
cancellation. Total expiry cancels upstream/body work, releases execution permits
and probes, and records the still-active candidate failed/504 plus usage
failed/504 with `stream_total_timeout`, original identity/type and logical
elapsed time. During retry gaps, settle the request without overwriting a
previously finished attempt.
Terminal persistence must survive that same deadline. Do not retain copied
conversation bodies or credentials just to persist a timeout. Candidate first
response watchdog exhaustion likewise stays 504; queued candidate writes must
not replace a known timeout with `no_local_stream_plans`.

Usage lifecycle is authoritative in records, detail and trace. An individual
failed candidate during retry cannot change pending/streaming into a request
failure. Keep per-attempt errors and durations on their nodes; the trace and
request use whole-request elapsed time: prefer request metadata
`end_to_end_time_ms`; `response_time_ms` can describe only the final attempt.
Preserve explicit image failure and
legacy records that lack a recognized lifecycle, plus frontend protection
against stale updates reverting a true terminal. Drawer trace events must not
write an individual attempt's error, status code or latency back as the whole
request result.

For one logical request and K+format, additional retry waiting totals at most
2 seconds. Preserve the full valid Retry-After deadline across later requests,
including non-429 failures. A long deadline skips this K for the current request.
Do not hold an execution permit or probe while waiting to retry.

The three modes own both the initial order and fallback order:

- Fixed order: manual provider/key priorities; positive health cannot reorder.
- Cache affinity: exact provider/endpoint/K binding; complete backup success
  migrates it. Recovery of the original K does not force migration back.
- Cost based: compatibility/capability tier, configured price multiplier, then
  affinity only among equal prices. Collect bounded pages before ranking so a
  cheaper later-page K can win. This is configured price, not bill prediction.

Do not apply pressure/random selection a second time to these ordered streams.
Affinity scope requires existing authenticated session/model/format/group
identity. Capture epoch and logical request UUIDv7 before execution, preserve
across attempts, and use the ordered atomic write API. Old/duplicate completion
cannot overwrite a newer accepted binding in the same cache epoch and lifetime.

## 4. Validation and error matrix

Health is displayed as 0..10 and stored normalized as 0..1 with fixed-point
calculation. Policy version 2 preserves legacy scores while resetting the old
non-strict failure counter on first projection.

| Fact | Base effect |
|---|---|
| Upstream busy/429, temporary 503/504, upstream first-output timeout | -1 |
| 500/502, attributable connection/TLS/protocol failure, ambiguous relay auth | -2 |
| Proven current credential invalid or unfunded from a trusted provider error | zero, recoverable circuit |
| Caller cancellation, local configuration/admission/capacity rejection, local stream total deadline | neutral |
| Complete valid model terminal | +1 up to 10; reset failure streak |

Consecutive failures 1–2 add 0, 3–4 add 1, 5 onward add 2 to the base penalty.
Mixed failure types share the streak. Neutral outcomes neither reset nor score.
HTTP 200 with an error envelope, HTML/plain/empty body or absent required terminal
is not success. A sync caller's forced upstream SSE must first pass the existing
terminal aggregation; same-family Responses must reuse its authoritative finalizer
so valid extended terminal output is preserved. Cross-format unknown output and
unknown intermediate events remain rejected.
Legitimate Responses `incomplete` length/filter terminals follow the existing
protocol terminal contract and must not trigger transparent retry.

At zero, open the circuit. First probe is due after one minute, failed probes
double the interval subject to the existing 1–32 minute configuration bound.
A successful zero-score probe restores one point. Do not also trigger the old
eight-failure, success-rate or fast-full-recovery rules for this policy.

Disabled, known unavailable, incompatible, local-full or still-cooling targets
are filtered without scoring. Recheck at actual admission; planning must not
reserve an anonymous probe. The managed owner renews every 20 seconds with a
60-second TTL and bounded storage operations. Lost/expired ownership is neutral
and cannot restore health. Validate owner on every CAS reread and expiry again
inside the repository write; terminal projection consumes owner metadata.

Once protocol/tool/model state has been delivered, do not transparently replay
or concatenate a second generation. HTTP `previous_response_id` currently
returns 409 when the original physical binding cannot be verified, before
history expansion or conversion. WS continuation uses its original ownership
and physical binding; never remove the reference to manufacture a fallback.

## 5. Good, base and bad cases

- Base: K1 returns two 500 responses, then K2 succeeds. Upstream order is
  K1/K1/K2; K1 ends at 6 and K2 success cannot heal K1.
- Good: all light failures yield 9,8,6,4,1,0; all heavy failures yield 8,6,3,0.
  Complete success adds one and resets the next failure to its base penalty.
- Bad: waiting until zero inside one request, replacing health after each retry,
  accepting HTTP headers as successful output, or passing old state references
  to a newly selected K.
- Persistence: keep the original attempt/time/fence in pending facts; reproject
  current health after conflicts. Receipt lookup precedes owner checks so an
  uncertain committed result can be recognized after its owner was consumed.
  Expired/replaced/released probe facts become diagnostics, never revived health.
  If SQLite itself cannot retain a fact, report retention failure honestly;
  an in-memory task or log is not durable acceptance.

## 6. Required regression evidence

Use local scripted upstreams through real authenticated gateway routes, and
read authoritative health/bindings afterward. Keep these suites meaningful:

- `tests::scheduler_failover`: actual counts/order, all three modes, full
  Retry-After, shared deadline, output then disconnect, typed transport errors,
  continuation rejection, due probe and cancellation.
- `responses::route_smoke`: real WS K1/K1/K2 and separate health settlement.
- `orchestration::{classifier,chat_health,effects,probe_lease}`: formulas,
  no double settlement, late credential/epoch/owner, renewal and cancellation.
- `cache::scheduler_affinity`, `scheduler::affinity`, planner pagination/ranking:
  late completion, epoch invalidation, exact scope and cheaper later pages.
- Data `health_*`: memory/SQLite parity, reopen persistence, uncertain receipt,
  CAS conflict/rollback, and lease expiry after waiting for the DB connection.
- `chat_retry_admin` plus provider form tests: old 2, overrides, save/GET,
  clear-to-inherit, invalid-write rejection, non-chat compatibility.

Module tests and compile success do not replace real route evidence. Keep live
Codex/relay verification separate from these isolated fixture results.

For first-response/total changes, cover fast successful headers followed by a
late first body, heartbeat then silence, no headers until first timeout, headers
without a body until total expiry, continuous output beyond total expiry and
output followed by premature EOF. Include retry-shared deadlines, request/pump
ownership transfer, permits retained by an unread body, and terminal persistence
crossing the generation deadline. Read back usage/candidate state through SQLite
and management APIs. Verify retry gaps, terminal errors and whole elapsed time
agree in records, detail and trace; reopen/refresh must preserve those results.

## 7. Incorrect and correct patterns

Incorrect: classify a generic 502 after losing the transport error type, create
a new attempt ID when retrying the health write, and reset cooldown relative to
the retry time. Correct: preserve the typed terminal fact and original identity
and observation time, then retry only the fenced health transaction.

Incorrect: claim a probe in the planner and claim again in the executor, or
release its guard after returning HTTP headers. Correct: one owned claim after
admission, kept through the full body/turn and consumed only by its terminal.
