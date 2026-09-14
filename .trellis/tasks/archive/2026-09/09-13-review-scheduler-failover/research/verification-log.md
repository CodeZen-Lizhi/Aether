# Verification Log

Final status: local implementation, focused behavior verification and review are complete. The following are observed results, not production or live Codex acceptance. Earlier failing snapshots are documented in the owner contracts; corrected results below supersede them.

## Passed checks

| Scope | Observed result |
|---|---|
| Gateway actual HTTP/SSE routes | 16 passed, 25.03s: K1/K1/K2, all three modes and subsequent affinity, explicit/legacy two attempts, price precedence, full Retry-After including 503 across requests, shared configured budget, delivered stream disconnection, abnormal JSON 200, typed timeout/connection versus local error, continuation rejection, due probe, owner/cancellation, non-JSON invalid 200, valid forced SSE, target capacity held through terminal/cancel and local-full backup |
| Gateway Responses WebSocket | 183 passed, including an authenticated real route K1/K1/K2 smoke and independent health readback |
| Gateway orchestration | 168 passed, 13.67s: formulas/classification, fences, CAS/duplicates, pending replay, managed probes |
| Gateway candidate loop | 22 passed, 1.46s |
| Authenticated management retry save/GET | 7 passed, 2.26s; endpoint update uses the actual PUT method |
| Gateway scheduler/cache/affinity | 49 / 85 / 89 passed; overlapping filters are not unique test totals |
| Gateway request diagnostics/task runtime | 7 / 8 passed |
| Data memory + SQLite health | 5 + 5 passed; immutable pending, reopen, receipt/CAS, independent connections and transaction-time probe expiry |
| SQLite migrations | 12 passed, 2.28s |
| Portable export/schema | Export inventory passed; schema checker includes both new migrations. Pending work is excluded from portable exports; committed receipts are retained |
| Scheduler core health/ranking | 20 / 29 passed |
| AI serving attempt loop/ranking | 4 / 4 passed |
| Configuration crates | Contracts 3, admin provider 137, failover 3, transport 5 passed (owner evidence) |
| Frontend | 16 targeted component tests, type-check and targeted ESLint passed |

Cargo-produced test executables were sometimes reused under the repository's `RUST_MIN_STACK=33554432` setting. An earlier direct diagnostics invocation omitted that setting and overflowed; it passed with the configured value. No production change was made for that invocation error. Existing `dispatch/refs.rs:71` unreachable-pattern warning is outside this task.

## UI verification

Main used Playwright with the actual Vue provider form. Changed total attempts 2→3 and budget 90000→60000, saved and reloaded, verified effective-source readback, then restored 2/90000. Desktop 1440×1000 and mobile 390×844 screenshots were inspected; no horizontal overflow. Artifacts: `output/playwright/scheduler-failover/provider-{desktop,mobile}.png`.

This fixture intercepts the public PATCH call using localStorage. It verifies the component flow, not backend persistence. The authenticated management tests use actual handlers with isolated memory repositories; SQLite persistence is verified separately.

## Final review closure

- Enqueue returns the first immutable pending fact, and gateway settlement consumes it. Gateway duplicate-delivery regression: 1 passed / 1.19s; updated memory and SQLite pending regressions: 1 + 1 passed. Duplicate observation time/outcome cannot extend Retry-After.
- Non-JSON HTTP 200 validates a terminal product before accepting success. Real HTML/plain/empty failures and valid forced-SSE cases passed in HTTP16. Final review then found a Responses same-family extension compatibility gap; it now reuses the existing authoritative finalizer. The two targeted Responses compatibility tests passed; the same new artifact's nine HTTP failure tests passed in 11.81s.
- Streaming target admission permit remains held through body terminal/cancellation. The real capacity-one regression passed in HTTP16: occupied target is skipped, backup succeeds, no health penalty, release permits reuse. This origin/proxy target gate is distinct from per-K configured concurrency; the fixture does not prove two K on one endpoint have independent capacity.
- Long real HTTP/SSE probe: 1 passed / 83.18s. Waited 62 actual seconds across the initial 60-second lease; same owner renewed, effective output survived the configured 2-second initial budget, replacement generation stopped the old body with an IO error within 30 seconds, no score/replay, replacement owner preserved.
- Scoped clippy first found a never-loop in the changed sync normalizer. Replaced it with a block and cleaned new clone/unwrap/enum warnings. Scoped clippy for gateway/data/sqlite/scheduler-core/ai-serving passed in 1m13s. One new needless Option reborrow warning remained in the stream return branch; it was removed mechanically. Main's final gateway-only clippy passed (exit 0, 18.36s), recorded in output/scheduler-failover-verification/clippy-gateway-final.log. Cross-checking its warning locations with changed lines found no new-line warnings. Existing unrelated warnings remain; this is not a zero-warning claim.

## Practical limits

All provider calls used isolated local scripted upstreams. No paid model, live relay, actual Codex conversation, production database, commit, push or deployment was used. Real >60-second HTTP/SSE renewal and ownership loss during delivered output are covered above. The analogous >60-second WS route is not exercised end-to-end; WS module and normal route evidence remain separate. HTTP `previous_response_id` is explicitly rejected with 409 when the original physical binding cannot be proven; no cross-K history reconstruction is introduced.

## Acceptance mapping

| Acceptance | Evidence and limits |
|---|---|
| A1 modes | Real initial/fallback/next-request mode tests; core/planner ranking and ordered affinity tests. Configured multiplier is not actual bill prediction. |
| A2 attempts | Actual K1/K1/K2 and key health readback for explicit/legacy 2; management override/inheritance/save/GET tests. |
| A3 health | Weighted/mixed/streak/success/neutral module tests and actual route 500/503/timeout/probe outcomes. |
| A4 error provenance | Real invalid JSON/non-JSON 200, timeout, refused connection, local configuration and target-full tests; trusted credential-root classification in isolated tests. Unknown real relay templates are not claimed covered. |
| A5 waiting | Actual short/long Retry-After and subsequent-request cooldown, including 503; date/invalid parsing and wait-budget boundaries in focused tests. |
| A6 circuit/probe | Actual due-probe, cancellation, >60s renewal and generation loss; storage fencing/expiry and queue admission regressions. Long WS renewal is not a real-route result. |
| A7 atomic effects | SQLite/memory receipt/CAS/reopen/expiry, gateway parallel effects and duplicate canonical pending replay. Unwritable storage reports failed retention, never durable success. |
| A8 protocol/replay | Separate real HTTP/SSE/WS entry tests, delivered SSE disconnect, owned WS continuations and HTTP 409 guard; live Codex/relay not used. |
| A9 time budget | Actual configured cross-K budget and 62s output beyond initial 2s budget; default resolver and existing nonstream/compact/WS clocks checked separately. |
| A10 management | Actual admin save/GET, component tests and desktop/mobile save/reload fixture. UI localStorage + memory API + SQLite storage are separate tests, not one DB-backed browser flow. |
| A11 compatibility | Twelve migration tests, legacy health/config normalization, preserved-rule/override tests, non-chat compatibility scope and strict same-family/cross-format terminal tests. |

Final whitespace and task-context manifest validation passed. The test browser and task-owned review/integration workers were closed. No work commit or deployment was created.
