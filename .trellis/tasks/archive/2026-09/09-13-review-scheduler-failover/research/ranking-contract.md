# Ranking Implementation Handoff

Owner: ranking. Scope: scheduler-core ranking, gateway candidate_ranking,
candidate_source pagination, scheduler affinity mode predicate, logical-request
order metadata, local affinity cache/AppState writes, and the serving ranking
port call. No edits to effects, health, or routing rules. No gateway cargo
command was started by this worker.

## Current Delivery Status

Concurrent completion protection is implemented in the shared worktree, not
only proposed: request-scoped metadata, atomic cache ordering, AppState write
API, and regression tests are present. HTTP reuses `Arc<RequestDiagnostics>`;
WS reuses each logical turn's UUIDv7. There is no long-lived request-ID registry
and no order minting at completion.

On the latest read, the health owner has also wired CostBased and the ordered
chat-success API in `effects.rs`; this worker did not edit that file. Remaining
integration is the WS planning epoch described below. Gateway compilation and
the new behavioral tests remain unrun by this worker under the main session's
centralized-build instruction. Implementation status is therefore ready for
integration/verification, not end-to-end HTTP/SSE/WS acceptance.

## Implemented

- `compare_cost_based`: capability tier, cross-format demotion and format
  preference precede price. Within that tier, `default_rate_multiplier` precedes
  cached affinity, then existing manual priority and stable fallback ordering.
- `compare_fixed_order` is unchanged: it does not consume health, inflight,
  latency, affinity, or price. A public ranking-entry regression supplies all
  adverse dynamic signals and asserts provider/key manual order.
- `run_ai_candidate_ranking` reads the existing affinity port once in both
  CacheAffinity and CostBased modes. It forwards the resulting exact target
  match to each candidate. FixedOrder and legacy LoadBalance do not read it.
- Gateway uses its existing authenticated session/model/client-format/group
  scope key and TTL; missing explicit session identity still yields no affinity.
- CostBased loads unique key IDs in one batch through the existing catalog
  reader for each ranked candidate set and reuses the map for duplicate K
  candidates. Invalid legacy multipliers retain the neutral 1.0 fallback and
  emit a diagnostic. Read failure retains the previous fallback with a warning.
- The existing ranking reason now reports cached affinity for CostBased too.
  This is candidate-level match metadata, not a claim that it beat cheaper keys.

## Required Effects and Storage Integration

1. `effects.rs::scheduler_cache_affinity_enabled` now accepts CostBased in the
   system-config branch (health-owner integration observed in the worktree).
   The report-context branch delegates to
   `scheduler/affinity.rs::SchedulerAffinityPolicyContext::cache_affinity_enabled`,
   which also accepts both CacheAffinity and CostBased.
2. `remember_successful_local_scheduler_affinity` must run only after complete
   model success for HTTP/SSE/WS. Backups migrate the same session-scoped target;
   transient failure/cooldown does not eagerly erase the binding. The current
   worker-observed effects changes already retain chat affinity on failure.
3. Exact invalidation must compare the full `SchedulerAffinityTarget`
   (provider_id, endpoint_id, key_id), under the same cache scope. The current
   effects owner has already changed `local_scheduler_affinity_matches_failed_target`
   to equality. Preserve that; do not fall back to endpoint-only matching.
4. The authorized ordering follow-up below provides request metadata and
   `AppState::remember_scheduler_affinity_target_for_request`. The health owner
   now consumes that API on completed chat success, rejecting missing order or
   epoch without an unordered fallback. Keep epoch protection and do not mint
   either the epoch or order at terminal completion.
5. Current read port requires no new signature for CostBased. Its auth/session,
   client format, requested model, routing group/version scope and existing
   epoch invalidation remain the integration contract. No global per-user
   fallback binding was introduced.

## Verification

- PASS: `cargo test -p aether-scheduler-core --lib ranking::` (29 tests).
  Includes cheaper-over-affinity, equal-price affinity, compatibility tiers,
  fixed manual order with adverse signals, and existing cache-affinity cases.
- Earlier clippy run was BLOCKED by a warning outside owned files (not rerun):
  `cargo clippy -p aether-scheduler-core --lib -- -D warnings` reports
  `health.rs:655` manual_range_patterns for `401 | 402 | 403`.
- Gateway regression added:
  `ai_serving::planner::candidate_ranking::tests::configured_modes_rank_catalog_prices_and_real_session_affinity`.
  Seeds the real in-memory catalog prices and session affinity cache, then calls
  `resolve_and_rank_local_execution_candidates` through real transport
  resolution and ranking. Covers FixedOrder, CacheAffinity, expensive affinity
  losing to cheaper K, same-price affinity winning, and absent session identity.
- Serving regression added:
  `candidate_ranking::tests::cost_ranking_reads_affinity_once_and_reuses_it_for_equal_price_candidates`.
- Gateway and serving tests await the main session's coordinated cargo run.
  No claim of HTTP/SSE/WS route acceptance, completed-success migration,
  or concurrent binding protection is made by these planner/core tests.

## Authorized Pagination Follow-up

- `candidate_source.rs::next_page` now collects candidate pages before final
  ranking when a routing policy exists OR the configured mode is CostBased.
  This fixes the no-policy path emitting expensive first-page candidates before
  seeing cheaper later-page candidates. Existing per-format scan caps, bounded
  repository page queries, and filters remain in effect.
- Added gateway regression
  `candidate_source::tests::cost_mode_without_routing_policy_ranks_cheaper_key_from_later_page_first`.
  The paged repository contains PAGE_SIZE + 1 rows. Only the final row has the
  0.1 multiplier; first-page keys have 1.0. It asserts actual repository offsets
  `[0, PAGE_SIZE]`, then runs production transport resolution/ranking and asserts
  the later-page key ranks first. It also verifies subsequent cursor exhaustion.
- `scheduler/affinity.rs::cache_affinity_enabled` now accepts CostBased.
  `policy_scoped_cost_mode_reads_affinity_but_fixed_order_does_not` exercises the
  real scoped cache reader for CacheAffinity/CostBased/FixedOrder and checks
  caller isolation.
- PASS: rustfmt check and git diff --check on both changed files.
- NOT RUN: gateway compilation/tests, per coordinated-build ownership. Main
  should run the two above tests with the other gateway regressions. No new core
  code changed in this follow-up, so its earlier 29-test result was not rerun.
- Channel CLI delivery previously failed because sandbox access to
  `~/.trellis/channels/...lock` is denied; the final response carries this handoff.

## Authorized Concurrent Completion Follow-up

### Logical Request Identity

- HTTP: `RequestDiagnostics::default` captures a UUIDv7 once when frontdoor
  creates the request scope. `RequestDiagnostics::request_order` exposes that
  immutable value. `build_local_execution_report_context` reads it through
  `current_request_diagnostics`, writing `scheduler_affinity_request_order`.
  HTTP initial attempts, retries, and backups share the existing task-local
  `Arc<RequestDiagnostics>`. The heartbeat background task in
  `executor/orchestration.rs` already carries this same Arc with
  `scope_request_diagnostics_with`; no new request-ID registry or manual planner
  field propagation is needed. Missing scope leaves the order absent instead
  of minting an attempt-local replacement. The earlier input/payload fields were
  removed; non-chat write policy remains unchanged.
- WS: reuse `websocket_logical_turn_id`; its three production mint sites in
  `client.rs` and `session.rs` now use `Uuid::now_v7()` at logical-turn start.
  Existing `LogicalTurn` and `quota.rs` retry paths retain this same ID even when
  they allocate a different attempt/request ID. No order is minted on success.
- `scheduler_affinity_request_order_from_report_context(Option<&Value>) ->
  Option<uuid::Uuid>` in `scheduler/affinity.rs` is the consumption API. It
  prioritizes the WS logical turn field over an inherited HTTP/template order.
  Missing, malformed, or non-v7 IDs fail closed; an invalid WS logical ID cannot
  fall back to the prior template's order. Update WS test fixtures that use
  arbitrary strings if they exercise completed-success affinity writes.
- The locally locked `uuid` implementation documents same-process creation
  ordering for `Uuid::now_v7` in `src/v7.rs`; this does not depend on random v4
  IDs, HTTP trace IDs supplied by callers, or per-attempt provider order IDs.

### Atomic Write API

```rust
state.remember_scheduler_affinity_target_for_request(
    &cache_key,
    target,
    SCHEDULER_AFFINITY_TTL,
    LOCAL_EXECUTION_SCHEDULER_AFFINITY_MAX_ENTRIES,
    expected_epoch, // u64 captured before execution
    request_order,  // uuid::Uuid from the report decoder above
)
```

- `SchedulerAffinityCache` serializes compare/insert and epoch clearing under
  one mutex. For the same scope and epoch, an older or equal order cannot
  replace a fresh ordered binding. Legacy unordered writes cannot replace it
  either. Other scopes remain independent.
- `clear_for_epoch` retains an epoch floor. A stale writer cannot repopulate an
  emptied cache after invalidation, and a delayed older invalidation cannot
  erase a newer epoch's binding.
- `remember_scheduler_affinity_target_for_epoch` remains for compatibility and
  now returns the cache's acceptance result. Rejected writes do not queue an
  external diagnostic mirror update.
- Async runtime KV mirror writers serialize and read the latest accepted local
  target under the write gate instead of publishing their captured old target.
  The mirror includes the order string for diagnostics. Selection continues to
  use the existing local cache; this change does not introduce a distributed
  affinity store or a cross-process total-order guarantee. Ordering comparisons
  share the existing cache TTL/capacity lifetime.

### Exact Effects Wiring for Main

Status: the following chat-success wiring is now present in the shared
`effects.rs`, added by its owner. Keep this contract during integration.

In `effects.rs::remember_successful_local_scheduler_affinity`, keep the current
mode/session/target checks. For the chat policy, replace the unordered call with:

```rust
let Some(expected_epoch) = expected_epoch else { return; };
let Some(request_order) =
    crate::scheduler::affinity::scheduler_affinity_request_order_from_report_context(
        context.report_context,
    )
else { return; };
let _ = state.remember_scheduler_affinity_target_for_request(
    &cache_key, target, SCHEDULER_AFFINITY_TTL,
    LOCAL_EXECUTION_SCHEDULER_AFFINITY_MAX_ENTRIES,
    expected_epoch, request_order,
);
```

Keep non-chat behavior on the compatibility method. Do not fall back to the
unordered writer when chat metadata is missing. A false return is a stale or
duplicate affinity write, not an upstream failure and not a reason to retry.

WS integration detail: `turn.rs::prepare_websocket_report_context` currently
removes `scheduler_affinity_epoch` when `reuse_selected_candidate` is false.
The main/WS owner must preserve or inject the logical turn's planning epoch
before completed-success consumption. Never substitute the current epoch at
completion. The request-order identity itself is already carried by the
existing logical-turn field across WS attempts.

### Added Regressions and Checks

- `cache::scheduler_affinity::tests::late_older_request_and_duplicate_cannot_overwrite_newer_binding`
- `cache::scheduler_affinity::tests::concurrent_request_completions_keep_the_latest_request`
  starts 16 completion writers together and asserts the largest issued order
  wins regardless of lock acquisition order.
- `cache::scheduler_affinity::tests::invalidation_rejects_stale_epoch_even_when_cache_is_empty`
- `state::core::tests::scheduler_affinity_request_order_blocks_late_success_in_the_same_epoch`
  also verifies the public cache read and list after the rejected write.
- `scheduler::affinity::tests::logical_ws_order_overrides_template_and_survives_changed_attempt_ids`
- Extended `planner::report_context` regression builds initial, retry and backup
  reports with the same diagnostics Arc after a newer request scope has been
  created, asserting all three retain their original logical-request order.
  It also checks that an unscoped builder does not create an order.
- `request_diagnostics::tests::request_order_survives_background_handoff_and_nested_request_scopes`
  exercises real task-local scopes, a spawned background task carrying the same
  Arc, independent later-request ordering, and scope restoration/cleanup.
- PASS: rustfmt checks on the modified cache/state/planner/scheduler files and
  `git diff --check` on this scope. The WS edits only change the three logical ID
  mint sites; other owners' WS changes are preserved.
- NOT RUN: cargo/type-check or these new behavioral tests, per main-session
  centralized gateway compilation. The now-wired effects consumption, actual
  HTTP/SSE/WS completed-success behavior, and runtime KV mirror behavior still
  require that coordinated verification; WS planning-epoch propagation also
  remains an integration action for main/WS owner.
