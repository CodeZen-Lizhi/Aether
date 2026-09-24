# Managed Probe Lease Contract

The module and tests are implemented; compilation and test execution are pending main, as explicitly requested. This document is the integration handoff for main.

## Current Integration Owners

- `retry_fix` (channel) owns HTTP F1/F2/F3 and managed probe wiring; the native retry_runtime owner has finished. Read the Duplicate Claim Audit below before wiring so the managed API does not reject an earlier anonymous reservation from the same request.
- `main` owns effects.rs terminal validation using `probe_lease_report_is_current` for every captured owner report on every CAS re-read.
- `websocket` owns WS guard wiring. Its `responses/probe.rs::ResponsesProbeLeases` demonstrates composing the circuit/rate-limit guards; HTTP must give them SSE body lifetime rather than Response-construction lifetime.
- `storage` owns the published guard API. No API renaming or guard/token handoff is required: move the existing non-Clone guard and preserve its renewal worker. Claim after waits/local admission, publish the `probe_leases` array before copying watchdog/terminal report contexts, select `lost()`, and settle the known terminal model outcome before `finish()`.

The direct notification to `retry_fix` was attempted with `trellis channel send scheduler-failover-impl --as storage --to retry_fix --text-file .../probe-lease-contract.md`; the channel lock is outside the writable sandbox and returned EPERM. This on-disk section is the fallback handoff, not a claim of delivered channel messaging.

New APIs (orchestration exports):

- `try_claim_managed_local_circuit_probe(&AppState, key_id, api_format).await -> Result<LocalProbeLeaseClaim, GatewayError>`
- `try_claim_managed_local_rate_limit_probe(&AppState, key_id, api_format).await -> Result<LocalProbeLeaseClaim, GatewayError>`
- `LocalProbeLeaseClaim::{NotRequired, Acquired(LocalProbeLeaseGuard), Unavailable}`
- Guard: `status() -> ProbeLeaseStatus`, `lost(&mut self).await -> ProbeLeaseLoss`, `report_context() -> Value`, `finish(self).await -> ProbeLeaseStatus`.
- `probe_lease_report_is_current(&StoredProviderCatalogKey, &Value, now_secs) -> bool` for terminal health CAS validation. The report contains owner/generation metadata only, no credential ciphertext.
- `PROBE_LEASES_REPORT_FIELD = "probe_leases"`: attach an array of all acquired guards' report_context values to this field in the per-attempt report. The validator takes one element; validate all elements on every settlement re-read. Missing context on a real probe must not silently bypass ownership checks.

The guard's supervisor is installed before awaiting the claim commit, so cancellation during the claim can also schedule owner-fenced cleanup. It renews every 20 seconds with a 60-second TTL; repository work and release are individually bounded to five seconds. Dropping the guard signals the already-running task to release; it does not start an unbounded new task. Process/runtime death and uncertain commits are recovered by TTL if cleanup cannot complete. Completion awaits release through `finish`; cancellation/drop signals release automatically. `lost()` must participate in the upstream execution select and abort execution on ownership/expiry/storage uncertainty. Lease errors are operational errors, never model failures and never health penalties.

`ProbeLeaseStatus` is Active { until_unix_secs }, Lost(reason), or Released. Loss reasons distinguish Expired, RenewalDeadline (insufficient remaining TTL to safely start a bounded renewal), OwnershipChanged, StorageUnavailable, Contention and WorkerStopped. Renewal refuses to start within six seconds of expiry; a cancelled/expired lease can never be renewed by its old owner. Observe and cancel the actual transport on loss; ignoring the watch future does not make execution exclusive.

Hold HTTP SSE guards through body terminal/cancellation, not merely until an HTTP Response is created. Hold WS guards through the turn terminal. Do not claim in both planner and executor. Existing ownerless try_claim APIs remain compatibility-only; real chat execution must use the managed APIs.

For terminal settlement, attach every guard's report_context to the existing per-attempt report. Main must verify every context against each freshly read key before projecting a complete-success settlement, retaining the exact health/circuit snapshot in the atomic settlement CAS. A CAS conflict requires revalidation of the SAME original owner report. Do not turn loss of ownership into success or retry another model after output delivery. Apply the terminal effect before finishing/dropping the guard; the release recognizes superseding business state and does not clear it. If an effect changes epoch/cooldown while a concurrent renewal checks ownership, terminal outcome handling must retain the already-known model outcome, not replace it with a lease error.

Persisted owner fields are circuit[format].half_open_lease and health[format].rate_limit_probe_lease. Existing half_open_until(_unix_secs) and rate_limit_probe_until_unix_secs deadlines remain authoritative for legacy scheduler filtering. An own-owner release with unchanged generation but changed cooldown removes only its obsolete owner metadata, leaving the new deadline untouched. A different owner, credential or circuit epoch prevents all release writes. Health score, failure counters, probe backoff and Retry-After values are never changed by lease operations.

Credential fencing: the legacy health CAS only has an optional auth_config fence, so it cannot atomically fence a static API Key replacement. Managed lease writes therefore reuse the existing atomic health settlement wrapper with an operation UUID; this preserves exact nullable credential + circuit-epoch checks without new tables/services. These are neutral lease writes, not model attempts or score changes. They create a small receipt per claim/renew/release in the existing 24-hour deduplication table. The same operation identity is retained across its bounded CAS retries.

Scope: circuit/rate-limit probe modules, a new probe_lease module and orchestration exports. Production HTTP/WS/effects integration belongs to main and is not claimed complete by this module. No Cargo command may be run in this turn; main owns gateway compilation and test execution.

## Verification And Remaining Work

Written tests in orchestration/probe_lease.rs:

- virtual_time_renewal_extends_exclusion_beyond_sixty_seconds: renew at 120/140/160/180 from initial time 100, reject another owner at 181, reject renewal at final expiry; both circuit/rate-limit paths preserve score/counters.
- old_owner_cannot_release_new_owner_epoch_credential_or_cooldown: replacement owner, new epoch, new credential, renewed Retry-After, neutral own release, and rejection of renewal after release.
- memory_cas_admits_one_guard_and_drop_releases_only_its_slot: two AppState instances sharing the memory repository race through atomic settlement; one acquires, drop is awaited through the detached worker, a successor acquires, and finishing after a new cooldown does not clear it or report that model outcome as a lease error.

`rustfmt --edition 2021 --config skip_children=true` was run only on probe_lease.rs, circuit_probe.rs, rate_limit_probe.rs and mod.rs. `git diff --check` passes. No Cargo command or test binary was run in this turn, so these are written regressions, not test-pass evidence. Main should run its gateway checks and `cargo test -p aether-gateway --lib orchestration::probe_lease::tests` once the build slot is available.

Production integration remains required: retire duplicate planner reservations, hold the returned guard through SSE body/WS turn lifetime, select ownership loss alongside transport, attach owner report context, and validate it atomically with complete-success settlement. Provider metadata must exist to capture a full credential fence; a missing provider/key fails closed as Unavailable. Existing ownerless reservations cannot be adopted and must expire or be settled before the managed API can acquire.

The channel notification command was attempted, but the sandbox denied the channel lock under `/Users/zhenglizhi/.trellis/channels/` with EPERM. Main can use this on-disk contract directly. No health/effects implementation, executor entry, production migration, paid upstream call or commit was changed/run by this follow-up.

## Duplicate Claim Audit And Integration Points

Read-only audit of the working tree found five legacy call sites. Line numbers reflect this inspection; function names remain the reliable anchors while other owners edit these files.

| Legacy caller | Current locations | Required replacement for managed chat attempts |
| --- | --- | --- |
| planner `circuit_probe_unavailable` | candidate_materialization.rs:301; invoked by Static branch :148 and RequestedModelPage branch :196 | Remove the ownerless write at materialization. Retain normal read-only eligibility filtering; the executor makes the final owned claim. |
| planner `rate_limit_probe_unavailable` | candidate_materialization.rs:339; invoked by Static branch :164 and RequestedModelPage branch :212 | Same change. Do not set request-wide skipped-probe scope merely because this request's own guard holds the slot. |
| dynamic executor `circuit_unavailable_after_request_local_failure` | candidate_loop.rs:952; invoked by run_dynamic_attempt_loop :833 before prepare_attempt :847 | Replace with a read-only recheck or fold it into the final managed execution admission. Never claim here and claim again in prepare/execute. Existing known-unavailable caching must not record this attempt's own lease as a competitor. |
| retry tracker circuit recheck | chat_retry.rs:369 inside ChatRetryTracker::prepare_attempt, attempts > 0 branch | Remove the ownerless claim. Keep attempt/wait-budget calculation and cooldown waiting. Final managed admission must run on both the first attempt and retries, not only attempts > 0. |
| retry tracker rate-limit recheck | chat_retry.rs:380, same branch | Same replacement. Retry waiting completes before acquiring either managed guard. |

The concrete self-rejection chain is planner claim -> dynamic circuit recheck -> prepare_attempt circuit/rate-limit recheck -> a newly added managed claim. All later layers see an active anonymous reservation and reject it. On retries, the dynamic recheck can itself acquire a half-open slot immediately before prepare_attempt acquires it again. It is insufficient to replace only the last call with the managed API.

### One Admission Point Per Real Attempt

1. Planner materializes eligible candidates without occupying a probe. ChatRetryTracker performs bounded waiting/continuation checks without holding a probe or execution permit.
2. At real execution admission, finish local balance/capacity checks and acquire local execution concurrency. Re-read authoritative key state and claim circuit/rate-limit managed leases once, before sending anything upstream. If the second required lease is unavailable/errors, finish/drop the first before proceeding to another candidate. The required-state recheck applies even when an old candidate snapshot did not mark the candidate as probe_required.
3. Capture the attempt credential/epoch context and attach acquired owner reports to the same report_context used by execution, watchdog and terminal effects. Do not increment actual model-attempt counters merely because a planner slot or lease was examined; prepare_attempt currently increments before admission, which the integration owner should account for when distinguishing admitted attempts from skipped slots.
4. Move the guard(s) into the actual execution owner. On loss, cancel that attempt without a model-health penalty. At terminal, perform owner-fenced health settlement and then finish/release guards. A fresh real retry receives a new attempt identity and fresh guards.

HTTP sync integration anchor: SyncAttemptLoopPort::execute_attempt, candidate_loop.rs:292, immediately after acquire_upstream_execution_gate and before capture_attempt_report_context :293 / runtime sync execution :305. Keep the guard until synchronous terminal effects finish.

HTTP SSE integration anchor: execute_stream_candidate_with_watchdog acquires the execution gate at candidate_loop.rs:1502; its execution closure starts after that. StreamAttemptLoopPort::execute_attempt currently captures report context at :1112, BEFORE this gate, and clones it for watchdog use. Main must move/couple capture and owner reports with the final admission point and propagate the same updated context to watchdog/runtime. A local guard in StreamAttemptLoopPort is insufficient because returning Response<Body> ends that scope. Move it into the stream pump/body owner and select lost() through body lifetime. Do not reuse the configurable upstream permit hold policy at :1672: Headers and FirstBody intentionally release early and are invalid probe-lease lifetimes.

WS inspection found no direct legacy try_claim calls in responses/. During this audit the parallel WS owner added responses/probe.rs::ResponsesProbeLeases, and turn.rs now claims it after admission (:384), then captures attempt context (:393), stores it on the turn (:258), exposes lost/run, and finishes it at terminal (:817). These are observed source changes, not runtime verification. The remaining planner ownerless claims still precede this new WS claim and must be removed for managed chat paths. client.rs sends response.create at :747 and :973 (normal/retry paths); both first and retry/reattach paths require the same admission rule, and no prior turn's lease may be attached to a new response.create.

### Ownership Transfer Without Another Claim

LocalProbeLeaseGuard is deliberately non-Clone. Moving it between prepare/admission, execution closure, SSE body task or WS active-turn state transfers ownership while the SAME renewal worker, token and credential fence remain active. No database handoff and no second claim are needed. Use an owned field or `Option<LocalProbeLeaseGuard>::take()` at the transfer boundary. A token/report alone is diagnostic/fencing context, not a live lease owner, and must never be used to reconstruct a guard or adopt an anonymous reservation. Do not add cloneable Arc ownership whose lifetime silently extends past a terminal or cancelled attempt.

There is currently no guard slot in prepare_attempt's Result<bool> API, so storing a guard there without an explicit owned return/transfer would hide its lifetime. Prefer keeping prepare_attempt limited to waiting and acquiring at execute_attempt/turn admission. If main chooses a PreparedAttempt wrapper, it must own the existing guard value and move it exactly once; an independent token-transfer method is unnecessary for this same-process flow.

If persisted ownerless reservations already exist during a local upgrade, let their TTL expire or let the old owning execution settle them. The managed API must not infer ownership from key/format/request ID or forcibly clear that slot. Non-chat legacy callers can retain the old functions, but no managed chat production path may call them before or after managed admission. The existing helper-only tests in circuit_probe.rs and rate_limit_probe.rs are compatibility tests, not production call sites.

Suggested main regression: materialize a due probe through both Static and RequestedModelPage branches, execute first and retry paths, and observe exactly one managed claim/one upstream call. Then hold SSE/WS execution beyond 60 seconds, prove a competing instance remains excluded, cancel the original and observe immediate own-slot release without changing the next cooldown/epoch. This audit did not modify parallel-owned execution files or run Cargo.
