# WebSocket Implementation Handoff

Owner: websocket. Scope: `apps/aether-gateway/src/handlers/proxy/websocket/responses/`.
No commits, paid upstream calls, or gateway cargo invocations were made.

## Connected Behavior

- `LogicalTurn` owns the monotonic first-output deadline, attempts per key,
  cumulative retry wait per key, and request-local excluded keys. Bootstrap,
  pinned continuations, independently replanned turns, and retry replacements
  retain this state. The selected `ExecutionTimeouts.stream_failover_budget_ms`
  is measured from before planning; before a candidate provides configuration,
  planning uses the shared 90000 ms default and its existing owner timeout.
- Planning, turn startup/admission waiting, initial connection/write, repeated
  writes, retry backoff/replanning/rebinding, and the active relay are bounded.
  The existing first-event, terminal, and connection limits remain separate.
- Effective text/reasoning/tool progress or a successful terminal removes only
  the first-output deadline. `response.created`, empty deltas, private events,
  and heartbeat frames do not. Deadline exhaustion uses a neutral cancellation
  outcome; already-settled upstream failures are not settled again.
- A pure terminal `error`/`response.failed` frame received before public response
  state can enter the existing settle-then-replan branch. Mixed batches or prior
  public response/tool events block transparent retry. Unknown send/receive
  failures retain the conservative no-replay path. `previous_response_id` is
  never removed and never sent to another key by retry.
- Existing Codex quota retry uses the same deadline and per-turn exclusions.
  The old one-retry boolean was removed; rejected keys remain excluded for the
  logical turn, while definitive quota exclusions retain their existing scope.
- Ordinary failures use the shared classifier and user termination decision.
  Same-key retry is explicitly pinned, re-enters the real planner after prior
  settlement, and consumes the effective total attempt limit. A rejected pinned
  candidate falls back to normal planning with that key excluded. Required
  waiting is never shortened: the two-second limit rejects the retry instead.
- Retry delay reads authoritative persisted cooldown first, then a parsed
  Retry-After, then the approved local backoff. Error-event `headers.retry-after`
  is carried into terminal provider headers for shared health settlement.
- Nested `response.error` is normalized into the internal error envelope consumed
  by shared classification/settlement. Original captured and relayed frames are
  preserved.

## Shared APIs Consumed

- `orchestration::classify_chat_failure_for_plan` and `ChatFailureFact`.
- `orchestration::resolve_local_failover_decision_for_attempt` to preserve stops.
- `orchestration::local_attempt_slot_count` over the selected transport snapshot.
- `orchestration::parse_chat_retry_after_secs` and scheduler-core cooldown reader.
- `execution_runtime::chat_retry::capture_attempt_report_context` before each WS
  attempt to attach attempt ID, credential fingerprint, and circuit epoch.
- `aether_contracts::chat_retry::DEFAULT_STREAM_FAILOVER_BUDGET_MS`.

The HTTP tracker itself is not reused: it owns task-local deadlines, while WS
retains a logical turn across multiple connection operations
and re-enters the pinned planner. Its backoff formula is currently duplicated
locally; a future shared pure backoff function could remove that small duplicate.

## Required Health Fence

`begin_unowned_responses_websocket_turn` in `turn.rs` calls the shared capture
helper for every new provider attempt, after local admission and managed probe
claim, before lifecycle startup, handshake or socket write. The helper calls
`orchestration::capture_chat_health_attempt(state, plan)` and writes its value
under `report_context[CHAT_HEALTH_ATTEMPT_REPORT_FIELD]`.

Bootstrap, pinned continuations, independent replans and transparent retries all
enter this same turn-begin function. Retry decision preparation clears an inherited
fence before capturing a fresh one. Capture errors propagate and prevent the
upstream call. If capture returns `None`, the context stays unfenced; there is no
WS-generated candidate-ID or CAS fallback and no previous-attempt fence is reused.

`AttemptLifecycleSeed` owns that context through all terminal paths. Initial
handshake failures use `finalize_unbound_turn`; rebind handshake failures use
`queue_turn_finalization`. Both settle the attempt that already captured its own
identity. Normal/error terminals, provider headers, adapter metadata, client
cancellation, deadline cancellation and drop-guard finalization preserve the
same context. `ExecutionAttemptLifecycle::settle` forwards it to the shared stream
failure/success effects. Main has removed health fallback for missing fences, so
missing capture deliberately produces no health deduction or recovery.

## Ordered Affinity

`prepare_websocket_report_context` preserves the planner's captured
`scheduler_affinity_epoch` even when `reuse_selected_candidate` is false. That
branch only removes the previous candidate/pool identity; it does not erase the
planning generation. The standard Responses planner supplies the epoch before
upstream execution. This worker does not substitute a current epoch at terminal
time or mint one when the planner omitted it.

Ranking's three UUIDv7 logical-turn creation sites remain unchanged. The same
`websocket_logical_turn_id` survives retry/replacement and takes precedence over
an inherited HTTP order in `scheduler_affinity_request_order_from_report_context`.
`ws_attempt_replacement_preserves_planning_epoch_and_logical_order` covers both
reuse modes across three attempts, preserving the planning epoch and original
logical order while changing request IDs. Main's ordered affinity effects reject
missing epoch/order without an unordered fallback.

## Managed Probe Ownership

`responses/probe.rs::ResponsesProbeLeases` consumes storage's managed circuit
and rate-limit guard APIs. `begin_unowned_responses_websocket_turn` claims once
after `ResponsesWebSocketTurnAdmission::acquire`, re-reading authoritative
eligibility even if the original planner candidate did not require a probe.
Failure to acquire either required guard prevents the upstream call; partially
acquired guards signal their existing cleanup worker on drop.

All acquired guards publish their owner reports in the shared
`report_context[PROBE_LEASES_REPORT_FIELD]` (`probe_leases`) array before health
capture and lifecycle startup. Replanning clears inherited owner reports along
with the previous health attempt fence. The attempt owns the guards through
handshake, send, relay, detached finalization and complete terminal settlement.

Bootstrap/rebind handshakes and both continuation/reused-socket response.create
writes race the actual upstream operation against guard loss. The relay selects
the same loss notification alongside receive and turn deadlines. Loss closes
the upstream, marks replay unsafe, settles a neutral cancellation and closes the
client connection. It cannot transparently replay an independent or pinned turn,
including after public response/tool output.

`ResponsesProviderAttempt::settle` awaits `ExecutionAttemptLifecycle::settle`
(including health effects) BEFORE `ResponsesProbeLeases::finish`. Finishing only
releases ownership and does not replace an already observed provider terminal
with a renewal error caused by that terminal's health/cooldown update. Main must
validate every owner report against each authoritative settlement reread, using
`probe_lease_report_is_current`; this worker does not modify effects.rs.

The shared planner's `circuit_probe_unavailable` and `rate_limit_probe_unavailable`
now skip ownerless reservations when `chat_health_policy_applies` is true
(observed on disk during this follow-up). WS has no legacy try_claim calls and
does not acquire a second guard at a later transport stage. The shared planner
change belongs to retry_fix, not this worker.

## Validation Status

- `rustfmt --edition 2021 --config skip_children=true` on changed WS Rust files:
  completed successfully; final `--check` passed as well.
- `git diff --check -- apps/aether-gateway/src/handlers/proxy/websocket/responses`:
  passed.
- Added `failover_tests.rs`: loopback WS binding with scripted rejection, backup,
  deadline, long-output and tool-state scenarios, plus cumulative-wait checks.
  These exercise real WS transport and production logical-turn policy. They do
  not instantiate the authenticated gateway router or the health repository.
- `turn.rs` regressions assert that preparing a replacement clears the previous
  health fence and probe owners, and that header/adapter/client-delivery context
  updates preserve both current reports. A lease-loss regression asserts neutral
  cancellation with no provider health effect.
- `probe.rs` regressions use the memory repository and production managed API to
  assert both guards remain exclusively owned through provider completion until
  finish, owner reports remain valid before finish and are invalid afterward,
  score remains unchanged, and ordinary attempts do not wait for absent guards.
  These tests are added but not run by this worker.
- `route_smoke.rs::authenticated_ws_gateway_retries_then_backup_and_settles_each_key`
  starts a real authenticated gateway router plus a loopback WS upstream, seeds
  routing/auth/candidate/provider/usage repositories, and sends response.create
  through `/v1/responses`. It asserts K1/K1/K2 requests, no rejected frame leaking
  to the client, completed backup output, persisted K1 health 1.0 -> 0.6 with two
  failures, and K2 health 0.6 -> 0.7. This exercises the production planner,
  admission, retry, lifecycle and health effects rather than invoking transport
  helpers. A 15-second outer deadline bounds the smoke test; it is written but
  NOT RUN by this worker because integration owns Cargo.
- Rust compilation and all new tests are NOT RUN, per main's cargo ownership.
  Main should run:
  `cargo test -p aether-gateway --lib handlers::proxy::websocket::responses::`.

## Integration Checks Still Required

- Main must validate the full authenticated route/config/health-store chain,
  including the new K1/K1/K2 route smoke above, one health settlement per WS
  attempt, pinned rejection, and no client-visible tool event mixing. A written
  smoke test is not pass evidence; the transport/policy fixtures alone do not
  prove the remaining full-route acceptance conditions.
- Integration must run a due-probe request through the full WS planner/admission
  path, hold it past 60 seconds, and force ownership loss while transport is
  active. The connected production guard and repository regressions do not by
  themselves verify real-time renewal/loss cancellation through the full route.
- The legacy quota adapter's provider-account breaker remains in place; verify
  its intended interaction with the new per-key health policy in A4/A6.
- Actual Codex client recovery, provider error-template coverage and real upstream
  latency remain unverified.

`trellis channel send` failed with EPERM opening the channel lock under
`/Users/zhenglizhi/.trellis/channels/`, outside this worker's writable roots.
The final worker reply and this file are the handoff; no permission override was
attempted.
