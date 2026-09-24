# Health Implementation Contract

Owner: health_policy. Implementation is in progress; this file records integration requirements.

- `orchestration::classify_chat_failure_for_plan(plan, status_code, response_text, source)` returns `ChatFailureFact`; source is `ChatFailureSource::{UpstreamResponse, UpstreamTimeout, UpstreamTransport, UpstreamProtocol, Neutral}`.
- Feed that fact using `LocalExecutionEffect::ClassifiedHealthFailure(LocalHealthFailureEffect { status_code, classification, retry_after_secs }, fact)`. Legacy `HealthFailure` remains a status-only fallback, without body-level evidence.
- `parse_chat_retry_after_secs` preserves positive long deadlines and zero. The legacy parser remains unchanged for non-chat.
- `FailureRetryAction::SameCredential` is the new retry disposition for scored chat failures. **Integration must map it to `RetryNextCandidate` in recovery.rs**, since candidate materialization already supplies repeated slots for the same K. Candidate retry scope is `Candidate`, not `Credential`.
- `capture_chat_health_attempt(state, plan).await?` returns `Option<Value>`; insert this under `CHAT_HEALTH_ATTEMPT_REPORT_FIELD` (`chat_health_attempt`) in that actual attempt's report context before executing upstream. Preserve it through terminal reporting. Capture once per actual attempt, not once per logical request; do not regenerate on report replay.
- Captured context contains a random attempt identity, timestamp, credential digest and circuit epoch, never raw credentials. Effects consume the new atomic `settle_provider_catalog_key_health_attempt` repository API. Generation mismatches cannot recover a newer circuit or update affinity.
- Without captured context, a nonempty `plan.candidate_id` gives deduplication but cannot prove original credential/epoch; without either identity, the compatibility path still uses CAS only. The real execution entrypoints need to capture context to satisfy A6/A7.
- Long probe lease renewal/cancel release is owned by execution integration, not this health projection module. First probe success projects 0.1, not the old ramp; an already-open failed probe retains zero and doubles cooldown within 1-32 minutes.
- The capture reads current credentials immediately before execution; the caller still needs to ensure the execution plan matches that credential generation if an administrator changed credentials after plan materialization.

Focused tests added under `orchestration::chat_health`, `orchestration::classifier`, `orchestration::effects` and `health::rate_limit_cooldown_tests`. One gateway test command was cancelled on the shared Cargo lock; no runtime pass is claimed yet.
