use aether_scheduler_core::SchedulerPriorityMode;

use crate::{AppState, GatewayError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SchedulerSchedulingMode {
    FixedOrder,
    #[default]
    CacheAffinity,
    /// R10: kept for deserialization compatibility only — legacy
    /// `load_balance` values map to CacheAffinity (closest behavior for a
    /// session-shaped single-user workload); the mode itself no longer
    /// rounds-trips back out.
    #[deprecated(note = "soft-deleted: maps to CacheAffinity on read")]
    LoadBalance,
    Economy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SchedulerOrderingConfig {
    pub(crate) priority_mode: SchedulerPriorityMode,
    pub(crate) scheduling_mode: SchedulerSchedulingMode,
    pub(crate) keep_priority_on_conversion: bool,
    /// P1-4: in-flight count participates in ranking (system_config
    /// `ranking_inflight_signal`, default true — read-only signal, no state).
    pub(crate) include_inflight: bool,
    /// P1-5: latency EWMA participates in ranking (system_config
    /// `ranking_latency_signal`, default false — observe-first rollout).
    pub(crate) include_latency: bool,
}

impl Default for SchedulerOrderingConfig {
    fn default() -> Self {
        Self {
            priority_mode: SchedulerPriorityMode::Provider,
            scheduling_mode: SchedulerSchedulingMode::CacheAffinity,
            keep_priority_on_conversion: false,
            include_inflight: false,
            include_latency: false,
        }
    }
}

pub(crate) fn parse_scheduler_priority_mode(
    value: Option<&serde_json::Value>,
) -> SchedulerPriorityMode {
    match value
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("global_key") => SchedulerPriorityMode::GlobalKey,
        _ => SchedulerPriorityMode::Provider,
    }
}

pub(crate) fn parse_keep_priority_on_conversion(value: Option<&serde_json::Value>) -> bool {
    value.and_then(serde_json::Value::as_bool).unwrap_or(false)
}

pub(crate) fn parse_scheduler_scheduling_mode(
    value: Option<&serde_json::Value>,
) -> SchedulerSchedulingMode {
    match value
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("fixed_order") => SchedulerSchedulingMode::FixedOrder,
        Some("economy") => SchedulerSchedulingMode::Economy,
        // R10 soft delete: legacy load_balance configs map to CacheAffinity
        // with a log trail instead of erroring — no data migration required.
        Some("load_balance") => {
            tracing::warn!(
                event_name = "scheduler_load_balance_soft_deleted",
                log_type = "event",
                "legacy scheduling_mode=load_balance mapped to cache_affinity (LoadBalance soft-deleted)"
            );
            SchedulerSchedulingMode::CacheAffinity
        }
        _ => SchedulerSchedulingMode::CacheAffinity,
    }
}

pub(crate) async fn read_scheduler_ordering_config(
    state: &AppState,
) -> Result<SchedulerOrderingConfig, GatewayError> {
    let priority_mode = parse_scheduler_priority_mode(
        state
            .read_system_config_json_value("provider_priority_mode")
            .await?
            .as_ref(),
    );
    let scheduling_mode = parse_scheduler_scheduling_mode(
        state
            .read_system_config_json_value("scheduling_mode")
            .await?
            .as_ref(),
    );
    let keep_priority_on_conversion = parse_keep_priority_on_conversion(
        state
            .read_system_config_json_value("keep_priority_on_conversion")
            .await?
            .as_ref(),
    );
    // Dynamic ranking signals (P1-4/P1-5). In-flight defaults on (read-only,
    // zero state); latency defaults off until its collector has been observed
    // producing sane data.
    let include_inflight = state
        .read_system_config_json_value("ranking_inflight_signal")
        .await?
        .as_ref()
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let include_latency = state
        .read_system_config_json_value("ranking_latency_signal")
        .await?
        .as_ref()
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    Ok(SchedulerOrderingConfig {
        priority_mode,
        scheduling_mode,
        keep_priority_on_conversion,
        include_inflight,
        include_latency,
    })
}
