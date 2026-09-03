use std::cmp::Ordering;

use crate::SchedulerPriorityMode;

use super::types::SchedulerRankableCandidate;

pub(super) fn candidate_priority_slot(
    candidate: &SchedulerRankableCandidate,
    priority_mode: SchedulerPriorityMode,
) -> i32 {
    match priority_mode {
        SchedulerPriorityMode::Provider => candidate.provider_priority,
        SchedulerPriorityMode::GlobalKey => candidate
            .key_global_priority_for_format
            .unwrap_or(candidate.key_internal_priority),
    }
}

pub(super) fn compare_candidate_priority_slot(
    left: &SchedulerRankableCandidate,
    right: &SchedulerRankableCandidate,
    priority_mode: SchedulerPriorityMode,
) -> Ordering {
    match priority_mode {
        SchedulerPriorityMode::Provider => left
            .provider_priority
            .cmp(&right.provider_priority)
            .then(left.key_internal_priority.cmp(&right.key_internal_priority)),
        SchedulerPriorityMode::GlobalKey => left
            .key_global_priority_for_format
            .unwrap_or(left.key_internal_priority)
            .cmp(
                &right
                    .key_global_priority_for_format
                    .unwrap_or(right.key_internal_priority),
            )
            .then(left.provider_priority.cmp(&right.provider_priority))
            .then(left.key_internal_priority.cmp(&right.key_internal_priority)),
    }
}
