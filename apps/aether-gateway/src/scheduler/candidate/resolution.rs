use std::collections::BTreeSet;

use aether_scheduler_core::candidate_key;

use super::runtime::{current_candidate_runtime_skip_reason, CandidateRuntimeSelectionSnapshot};
use super::{SchedulerMinimalCandidateSelectionCandidate, SchedulerSkippedCandidate};

pub(super) fn resolve_scheduler_candidate_selectability(
    candidates: Vec<SchedulerMinimalCandidateSelectionCandidate>,
    runtime_snapshot: &CandidateRuntimeSelectionSnapshot,
    now_unix_secs: u64,
) -> (
    Vec<SchedulerMinimalCandidateSelectionCandidate>,
    Vec<SchedulerSkippedCandidate>,
) {
    let mut selected = Vec::with_capacity(candidates.len());
    let mut skipped = Vec::new();
    let mut emitted_selected_keys = BTreeSet::new();
    let mut emitted_skipped_keys = BTreeSet::new();

    for candidate in candidates {
        let key = candidate_key(&candidate);
        if let Some(skip_reason) =
            current_candidate_runtime_skip_reason(&candidate, runtime_snapshot, now_unix_secs)
        {
            if emitted_skipped_keys.insert(key) {
                let cooldown = (skip_reason == "key_rate_limit_cooldown")
                    .then(|| {
                        runtime_snapshot
                            .provider_key_rpm_states
                            .get(candidate.key_id.as_str())
                            .and_then(|key| {
                                aether_scheduler_core::provider_key_rate_limit_cooldown(
                                    key,
                                    candidate.endpoint_api_format.as_str(),
                                )
                            })
                    })
                    .flatten();
                skipped.push(SchedulerSkippedCandidate {
                    candidate,
                    skip_reason,
                    rate_limit_cooldown_started_at_unix_secs: cooldown
                        .as_ref()
                        .and_then(|value| value.started_at_unix_secs),
                    rate_limit_cooldown_until_unix_secs: cooldown
                        .as_ref()
                        .map(|value| value.until_unix_secs),
                });
            }
            continue;
        }
        if emitted_selected_keys.insert(key) {
            selected.push(candidate);
        }
    }

    (selected, skipped)
}
