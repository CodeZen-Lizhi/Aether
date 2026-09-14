# Chat Scheduling and Failover

## 1. Scope and trigger

This contract covers chat generation over HTTP, SSE and Responses WebSocket.
Image, video and embedding policies retain their existing behavior. Read this
before changing candidate ordering, retries, provider health, recovery probes,
terminal effects or the provider retry settings.

## 2. Signatures and ownership

- `aether_contracts::chat_retry::resolve_chat_max_attempts(...)` is the shared
  resolver for actual attempts and management effective-value readback.
- `ExecutionTimeouts.stream_failover_budget_ms` carries provider configuration
  to `execution_runtime::chat_retry`; the deadline is logical-request scoped.
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

The default streaming first-effective-output budget is 90000 ms, configurable
on provider `failover_rules.stream_failover_budget_ms`. Selection, admission,
connection, refresh, backoff, retries and fallback share the original deadline.
Headers, keepalives and empty/protocol startup frames are not effective output.
Effective output ends this budget; it does not cap the complete answer. Existing
nonstream/compact/WS turn and connection limits remain separate.

Responses streams include ordinary chat and V2 `compaction_trigger` requests.
Nonempty `response.output_text.done`, reasoning/refusal/tool argument snapshots,
supported content parts, and `response.output_item.done` with message, reasoning
or compaction content release both first-output waits, just like deltas.
Compaction requires nonempty `encrypted_content`; empty/unknown items do not
qualify. Item completion never substitutes for the response's protocol terminal.
Log the first effective event type and timings, never its text/encrypted payload.

First-output budget cancellation must hand off a failed usage terminal with 504,
the original request identity/type, and actual logical elapsed time. Retain only
identity/routing/body references for this handoff, without copying conversations
or credentials. Terminal persistence ends first-output waiting so it cannot be
cancelled by that same deadline. Candidate watchdog exhaustion also returns 504;
retain its outcome in the request loop, since candidate audit writes are queued
and an immediate SQLite read can still show the previous status. Do not replace
that known timeout with `no_local_stream_plans` or wait for stale-request cleanup.

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
| Caller cancellation, local configuration/admission/capacity rejection | neutral |
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

For first-output changes, cover early text/compaction snapshots followed by a
delayed valid completion beyond both deadlines, empty startup frames that still
time out, total-budget cancellation, and output followed by premature EOF.
Read back usage/candidate state using SQLite as well as memory: queued candidate
writes must not change the immediate timeout response; later reads must retain
failed/504 and the original compact/chat type. EOF after output stays failed and
must not replay the generation.

## 7. Incorrect and correct patterns

Incorrect: classify a generic 502 after losing the transport error type, create
a new attempt ID when retrying the health write, and reset cooldown relative to
the retry time. Correct: preserve the typed terminal fact and original identity
and observation time, then retry only the fenced health transaction.

Incorrect: claim a probe in the planner and claim again in the executor, or
release its guard after returning HTTP headers. Correct: one owned claim after
admission, kept through the full body/turn and consumed only by its terminal.
