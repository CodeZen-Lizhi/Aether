use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex as StdMutex, Weak};
use std::time::Duration;

use aether_cache::ExpiringMap;
use aether_contracts::{ExecutionPlan, ExecutionTelemetry};
use aether_data_contracts::repository::provider_catalog::{
    ProviderCatalogKeyAdaptiveState, ProviderCatalogKeyAdaptiveStateUpdate,
    ProviderCatalogKeyHealthStateUpdate,
};
use aether_scheduler_core::{
    build_scheduler_affinity_cache_key_for_api_key_id_with_client_session_and_scope,
    count_recent_rpm_requests_for_provider_key, ClientSessionAffinity, SchedulerAffinityTarget,
};
use aether_usage_runtime::{
    build_stream_terminal_usage_outcome, build_sync_terminal_usage_outcome,
    GatewayStreamReportRequest, GatewaySyncReportRequest, TerminalUsageOutcome,
};
use serde_json::Value;
use tokio::sync::Mutex as TokioMutex;
use tracing::warn;

use super::{
    circuit_ramp_active, circuit_success_rate_breached, classify_failure_disposition,
    local_failover_error_message, project_local_adaptive_rate_limit,
    project_local_adaptive_success, project_local_failure_health,
    project_local_key_circuit_closed_with_ramp,
    project_local_key_circuit_failure_with_success_rate, project_local_key_circuit_open,
    project_local_ramp_success_health, project_local_rate_limit_cooldown,
    project_local_success_health, resolve_local_failover_analysis_for_attempt, FailureScope,
    LocalFailoverAnalysis, LocalFailoverClassification,
};
use crate::client_session_affinity::{
    client_session_affinity_from_report_context_value, CLIENT_SESSION_AFFINITY_REPORT_CONTEXT_FIELD,
};
use crate::clock::current_unix_secs;
use crate::orchestration::local_execution_candidate_metadata_from_report_context;
use crate::scheduler::affinity::{
    scheduler_affinity_policy_context_from_report_context, SCHEDULER_AFFINITY_POLICY_REPORT_FIELD,
    SCHEDULER_AFFINITY_TTL,
};
use crate::scheduler::config::{read_scheduler_ordering_config, SchedulerSchedulingMode};
use crate::AppState;

const HEALTH_SUCCESS_PERSIST_GATE_MAX_ENTRIES: usize = 50_000;
const ADAPTIVE_SUCCESS_PERSIST_GATE_MAX_ENTRIES: usize = 50_000;
const HEALTH_SUCCESS_PERSIST_MIN_INTERVAL_ENV: &str =
    "AETHER_GATEWAY_PROVIDER_KEY_HEALTH_SUCCESS_PERSIST_MIN_INTERVAL_SECS";
const ADAPTIVE_SUCCESS_PERSIST_MIN_INTERVAL_ENV: &str =
    "AETHER_GATEWAY_PROVIDER_KEY_ADAPTIVE_SUCCESS_PERSIST_MIN_INTERVAL_SECS";
const DEFAULT_HEALTH_SUCCESS_PERSIST_MIN_INTERVAL_SECS: u64 = 5;
const DEFAULT_ADAPTIVE_SUCCESS_PERSIST_MIN_INTERVAL_SECS: u64 = 5;
const PROVIDER_KEY_EFFECT_LOCK_PRUNE_THRESHOLD: usize = 8_192;
// Same-process writers are serialized by the per-key lock. Keep remote-writer
// retries bounded so request/report completion cannot accumulate a long DB tail.
const PROVIDER_KEY_STATE_CAS_MAX_ATTEMPTS: usize = 4;

#[derive(Debug)]
struct ProviderKeyEffectLockPoolState {
    entries: HashMap<String, Weak<TokioMutex<()>>>,
    accesses_since_prune: usize,
    next_growth_prune_at: usize,
    #[cfg(test)]
    prune_count: usize,
}

impl ProviderKeyEffectLockPoolState {
    fn new(min_prune_threshold: usize) -> Self {
        Self {
            entries: HashMap::new(),
            accesses_since_prune: 0,
            next_growth_prune_at: min_prune_threshold,
            #[cfg(test)]
            prune_count: 0,
        }
    }
}

#[derive(Debug)]
struct ProviderKeyEffectLockPool {
    state: StdMutex<ProviderKeyEffectLockPoolState>,
    min_prune_threshold: usize,
}

impl Default for ProviderKeyEffectLockPool {
    fn default() -> Self {
        Self::new(PROVIDER_KEY_EFFECT_LOCK_PRUNE_THRESHOLD)
    }
}

impl ProviderKeyEffectLockPool {
    fn new(min_prune_threshold: usize) -> Self {
        let min_prune_threshold = min_prune_threshold.max(1);
        Self {
            state: StdMutex::new(ProviderKeyEffectLockPoolState::new(min_prune_threshold)),
            min_prune_threshold,
        }
    }

    fn lock_for(&self, key_id: &str) -> Arc<TokioMutex<()>> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.accesses_since_prune = state.accesses_since_prune.saturating_add(1);
        let entry_count = state.entries.len();
        let growth_prune_due = entry_count >= state.next_growth_prune_at;
        let maintenance_prune_due = entry_count >= self.min_prune_threshold
            && state.accesses_since_prune >= entry_count.max(self.min_prune_threshold);
        if growth_prune_due || maintenance_prune_due {
            self.prune_inactive_locks(&mut state);
        }

        if let Some(existing) = state.entries.get(key_id).and_then(Weak::upgrade) {
            return existing;
        }
        let lock = Arc::new(TokioMutex::new(()));
        state
            .entries
            .insert(key_id.to_string(), Arc::downgrade(&lock));
        lock
    }

    fn prune_inactive_locks(&self, state: &mut ProviderKeyEffectLockPoolState) {
        state.entries.retain(|_, lock| lock.strong_count() > 0);
        let active_entries = state.entries.len();
        state.next_growth_prune_at = if active_entries < self.min_prune_threshold {
            self.min_prune_threshold
        } else {
            active_entries.saturating_mul(2)
        };
        state.accesses_since_prune = 0;
        #[cfg(test)]
        {
            state.prune_count = state.prune_count.saturating_add(1);
        }
    }
}

static POOL_SCORE_FEEDBACK_GATE: LazyLock<ExpiringMap<String, ()>> =
    LazyLock::new(ExpiringMap::new);
static HEALTH_SUCCESS_PERSIST_GATE: LazyLock<ExpiringMap<String, ()>> =
    LazyLock::new(ExpiringMap::new);
static ADAPTIVE_SUCCESS_PERSIST_GATE: LazyLock<ExpiringMap<String, u64>> =
    LazyLock::new(ExpiringMap::new);
static ADAPTIVE_SUCCESS_PERSIST_GATE_NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);
static PROVIDER_KEY_EFFECT_LOCKS: LazyLock<ProviderKeyEffectLockPool> =
    LazyLock::new(ProviderKeyEffectLockPool::default);
static HEALTH_SUCCESS_PERSIST_MIN_INTERVAL: LazyLock<Duration> = LazyLock::new(|| {
    effect_min_interval_from_env(
        HEALTH_SUCCESS_PERSIST_MIN_INTERVAL_ENV,
        DEFAULT_HEALTH_SUCCESS_PERSIST_MIN_INTERVAL_SECS,
    )
});
static ADAPTIVE_SUCCESS_PERSIST_MIN_INTERVAL: LazyLock<Duration> = LazyLock::new(|| {
    effect_min_interval_from_env(
        ADAPTIVE_SUCCESS_PERSIST_MIN_INTERVAL_ENV,
        DEFAULT_ADAPTIVE_SUCCESS_PERSIST_MIN_INTERVAL_SECS,
    )
});

fn effect_min_interval_from_env(key: &str, default_secs: u64) -> Duration {
    let parsed = std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default_secs);
    Duration::from_secs(parsed.min(300))
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LocalExecutionEffectContext<'a> {
    pub(crate) plan: &'a ExecutionPlan,
    pub(crate) report_context: Option<&'a Value>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LocalAttemptFailureEffect {
    pub(crate) status_code: u16,
    pub(crate) classification: LocalFailoverClassification,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LocalAdaptiveRateLimitEffect<'a> {
    pub(crate) status_code: u16,
    pub(crate) classification: LocalFailoverClassification,
    pub(crate) headers: Option<&'a BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LocalHealthFailureEffect {
    pub(crate) status_code: u16,
    pub(crate) classification: LocalFailoverClassification,
    /// Parsed `Retry-After` seconds when the failure carried one (P0-2).
    /// Only meaningful for 429 failures; `None` elsewhere.
    pub(crate) retry_after_secs: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LocalHealthSuccessEffect;

#[derive(Debug, Clone, Copy)]
pub(crate) struct LocalAdaptiveSuccessEffect;

#[derive(Debug, Clone, Copy)]
pub(crate) enum LocalExecutionEffect<'a> {
    AttemptFailure(LocalAttemptFailureEffect),
    AdaptiveRateLimit(LocalAdaptiveRateLimitEffect<'a>),
    HealthFailure(LocalHealthFailureEffect),
    HealthSuccess(LocalHealthSuccessEffect),
    AdaptiveSuccess(LocalAdaptiveSuccessEffect),
}

/// Inputs for the terminal effects of a failed streaming attempt.
///
/// The status/body are deliberately supplied by the transport-specific caller:
/// a WebSocket terminal event may carry its own status and error body, while a
/// normal stream failure gets them from the HTTP response. Keeping this type at
/// the orchestration boundary prevents each transport from rebuilding the
/// health and adaptive effect sequence independently.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LocalStreamFailureEffect<'a> {
    pub(crate) status_code: u16,
    pub(crate) headers: &'a BTreeMap<String, String>,
    pub(crate) response_text: Option<&'a str>,
    pub(crate) stream_timeout: bool,
}

impl<'a> LocalStreamFailureEffect<'a> {
    pub(crate) const fn new(
        status_code: u16,
        headers: &'a BTreeMap<String, String>,
        response_text: Option<&'a str>,
    ) -> Self {
        Self {
            status_code,
            headers,
            response_text,
            stream_timeout: false,
        }
    }

    pub(crate) const fn with_stream_timeout(mut self) -> Self {
        self.stream_timeout = true;
        self
    }
}

#[derive(Debug, Clone)]
enum LocalExecutionAuthConfigFence {
    Unfenced,
    Fenced(String),
}

impl LocalExecutionAuthConfigFence {
    fn encrypted_auth_config(&self) -> Option<&str> {
        match self {
            Self::Unfenced => None,
            Self::Fenced(ciphertext) => Some(ciphertext),
        }
    }
}

const ADAPTIVE_RPM_RECENT_CANDIDATE_LIMIT: usize = 512;
const LOCAL_EXECUTION_SCHEDULER_AFFINITY_MAX_ENTRIES: usize = 10_000;

pub(crate) async fn apply_local_execution_effect(
    state: &AppState,
    context: LocalExecutionEffectContext<'_>,
    effect: LocalExecutionEffect<'_>,
) {
    match effect {
        LocalExecutionEffect::AttemptFailure(effect) => {
            record_attempt_failure_effect(state, context, effect).await;
        }
        LocalExecutionEffect::AdaptiveRateLimit(effect) => {
            record_adaptive_rate_limit_effect(state, context, effect).await;
        }
        LocalExecutionEffect::HealthFailure(effect) => {
            record_health_failure_effect(state, context, effect).await;
        }
        LocalExecutionEffect::HealthSuccess(effect) => {
            record_health_success_effect(state, context, effect).await;
        }
        LocalExecutionEffect::AdaptiveSuccess(effect) => {
            record_adaptive_success_effect(state, context, effect).await;
        }
    }
}

/// Apply the provider/key effects shared by every successful streaming
/// transport. Usage persistence and request-candidate terminal status remain
/// owned by the report layer; this helper only projects execution health,
/// adaptive state.
pub(crate) async fn apply_local_stream_success_effects(
    state: &AppState,
    context: LocalExecutionEffectContext<'_>,
    _payload: &GatewayStreamReportRequest,
) {
    apply_local_execution_effect(
        state,
        context,
        LocalExecutionEffect::HealthSuccess(LocalHealthSuccessEffect),
    )
    .await;
    apply_local_execution_effect(
        state,
        context,
        LocalExecutionEffect::AdaptiveSuccess(LocalAdaptiveSuccessEffect),
    )
    .await;
}

/// Apply the provider/key effects shared by every failed streaming attempt.
/// The returned analysis is the same failover classification used by the
/// normal stream runtime, allowing the caller to make a transport-specific
/// retry/close decision without re-running policy evaluation.
pub(crate) async fn apply_local_stream_failure_effects(
    state: &AppState,
    context: LocalExecutionEffectContext<'_>,
    effect: LocalStreamFailureEffect<'_>,
) -> LocalFailoverAnalysis {
    let analysis = resolve_local_failover_analysis_for_attempt(
        state,
        context.plan,
        context.report_context,
        effect.status_code,
        effect.response_text,
    )
    .await;

    apply_local_execution_effect(
        state,
        context,
        LocalExecutionEffect::AttemptFailure(LocalAttemptFailureEffect {
            status_code: effect.status_code,
            classification: analysis.classification,
        }),
    )
    .await;
    apply_local_execution_effect(
        state,
        context,
        LocalExecutionEffect::AdaptiveRateLimit(LocalAdaptiveRateLimitEffect {
            status_code: effect.status_code,
            classification: analysis.classification,
            headers: Some(effect.headers),
        }),
    )
    .await;
    apply_local_execution_effect(
        state,
        context,
        LocalExecutionEffect::HealthFailure(LocalHealthFailureEffect {
            status_code: effect.status_code,
            classification: analysis.classification,
            retry_after_secs: None,
        }),
    )
    .await;

    analysis
}

fn report_context_string_field<'a>(
    report_context: Option<&'a Value>,
    field: &str,
) -> Option<&'a str> {
    report_context
        .and_then(|context| context.get(field))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn report_context_u64_field(report_context: Option<&Value>, field: &str) -> Option<u64> {
    report_context
        .and_then(|context| context.get(field))
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|value| value.parse::<u64>().ok()))
        })
}

fn local_scheduler_affinity_cache_key(report_context: Option<&Value>) -> Option<String> {
    let client_session_affinity = local_client_session_affinity(report_context);
    let policy_context = scheduler_affinity_policy_context_from_report_context(report_context);
    if report_context
        .and_then(|context| context.get(SCHEDULER_AFFINITY_POLICY_REPORT_FIELD))
        .is_some()
        && policy_context.is_none()
    {
        return None;
    }
    build_scheduler_affinity_cache_key_for_api_key_id_with_client_session_and_scope(
        report_context_string_field(report_context, "api_key_id")?,
        report_context_string_field(report_context, "client_api_format")?,
        report_context_string_field(report_context, "model")?,
        client_session_affinity.as_ref(),
        policy_context
            .as_ref()
            .and_then(|context| context.scope.as_ref()),
    )
}

fn local_client_session_affinity(report_context: Option<&Value>) -> Option<ClientSessionAffinity> {
    let report_context = report_context?;
    if let Some(affinity) = client_session_affinity_from_report_context_value(
        report_context.get(CLIENT_SESSION_AFFINITY_REPORT_CONTEXT_FIELD),
    ) {
        return Some(affinity);
    }

    let headers = header_map_from_report_context(report_context.get("original_headers"));
    let body_json = report_context
        .get("original_request_body")
        .filter(|value| !value.is_null());

    crate::client_session_affinity::client_session_affinity_from_api_request(
        report_context_string_field(Some(report_context), "client_api_format").unwrap_or_default(),
        &headers,
        body_json,
    )
}

fn header_map_from_report_context(headers: Option<&Value>) -> http::HeaderMap {
    let mut header_map = http::HeaderMap::new();
    let Some(headers) = headers.and_then(Value::as_object) else {
        return header_map;
    };

    for (name, value) in headers {
        let Some(value) = value.as_str() else {
            continue;
        };
        let Ok(name) = http::header::HeaderName::from_bytes(name.as_bytes()) else {
            continue;
        };
        let Ok(value) = http::HeaderValue::from_str(value) else {
            continue;
        };
        header_map.insert(name, value);
    }

    header_map
}

fn local_scheduler_affinity_target(plan: &ExecutionPlan) -> Option<SchedulerAffinityTarget> {
    let provider_id = plan.provider_id.trim();
    let endpoint_id = plan.endpoint_id.trim();
    let key_id = plan.key_id.trim();
    if provider_id.is_empty() || endpoint_id.is_empty() || key_id.is_empty() {
        return None;
    }

    Some(SchedulerAffinityTarget {
        provider_id: provider_id.to_string(),
        endpoint_id: endpoint_id.to_string(),
        key_id: key_id.to_string(),
    })
}

async fn capture_local_execution_auth_config_fence(
    _state: &AppState,
    _plan: &ExecutionPlan,
) -> Option<LocalExecutionAuthConfigFence> {
    Some(LocalExecutionAuthConfigFence::Unfenced)
}

async fn local_scheduler_affinity_matches_failed_target(
    _state: &AppState,
    _plan: &ExecutionPlan,
    cached_target: &SchedulerAffinityTarget,
    failed_target: &SchedulerAffinityTarget,
) -> bool {
    if cached_target == failed_target {
        return true;
    }
    cached_target.provider_id == failed_target.provider_id
        && cached_target.endpoint_id == failed_target.endpoint_id
}

async fn scheduler_cache_affinity_enabled(
    state: &AppState,
    report_context: Option<&Value>,
) -> bool {
    if report_context
        .and_then(|context| context.get(SCHEDULER_AFFINITY_POLICY_REPORT_FIELD))
        .is_some()
    {
        return scheduler_affinity_policy_context_from_report_context(report_context)
            .is_some_and(|context| context.cache_affinity_enabled());
    }
    match read_scheduler_ordering_config(state).await {
        Ok(config) => config.scheduling_mode == SchedulerSchedulingMode::CacheAffinity,
        Err(error) => {
            warn!(
                event_name = "orchestration_scheduler_affinity_config_load_failed",
                log_type = "event",
                error = ?error,
                "failed to load scheduler config while checking cache affinity mode"
            );
            SchedulerSchedulingMode::default() == SchedulerSchedulingMode::CacheAffinity
        }
    }
}

async fn remember_successful_local_scheduler_affinity(
    state: &AppState,
    context: LocalExecutionEffectContext<'_>,
) {
    if !scheduler_cache_affinity_enabled(state, context.report_context).await {
        return;
    }
    let Some(cache_key) = local_scheduler_affinity_cache_key(context.report_context) else {
        return;
    };
    let Some(target) = local_scheduler_affinity_target(context.plan) else {
        return;
    };
    let expected_epoch =
        local_execution_candidate_metadata_from_report_context(context.report_context)
            .scheduler_affinity_epoch;

    let _ = state.remember_scheduler_affinity_target_for_epoch(
        &cache_key,
        target,
        SCHEDULER_AFFINITY_TTL,
        LOCAL_EXECUTION_SCHEDULER_AFFINITY_MAX_ENTRIES,
        expected_epoch,
    );
}

fn total_tokens_used(outcome: &TerminalUsageOutcome) -> u64 {
    outcome
        .standardized_usage
        .as_ref()
        .map(|usage| {
            usage
                .input_tokens
                .saturating_add(usage.output_tokens)
                .max(0) as u64
        })
        .unwrap_or(0)
}

fn resolve_ttfb_ms(telemetry: Option<&ExecutionTelemetry>) -> Option<u64> {
    telemetry.and_then(|telemetry| telemetry.ttfb_ms.or(telemetry.elapsed_ms))
}

/// P1-5: record one latency observation into the in-memory EWMA tracker.
/// Streaming attempts surface `candidate_ttfb_ms` (what the client feels);
/// sync attempts fall back to `candidate_elapsed_ms`. Absent timing simply
/// skips the observation — the tracker stays a pure signal, never a gate.
fn record_scheduler_latency_observation(context: LocalExecutionEffectContext<'_>) {
    let Some(report_context) = context.report_context else {
        return;
    };
    let latency_ms = report_context
        .get("candidate_ttfb_ms")
        .and_then(Value::as_u64)
        .or_else(|| {
            report_context
                .get("candidate_elapsed_ms")
                .and_then(Value::as_u64)
        });
    let Some(latency_ms) = latency_ms else {
        return;
    };
    let tracker = crate::scheduler::latency_tracker::SchedulerLatencyTracker::shared();
    tracker.record(
        context.plan.provider_id.as_str(),
        context.plan.endpoint_id.as_str(),
        context.plan.key_id.as_str(),
        context.plan.provider_api_format.as_str(),
        latency_ms,
    );
}

async fn record_attempt_failure_effect(
    state: &AppState,
    context: LocalExecutionEffectContext<'_>,
    effect: LocalAttemptFailureEffect,
) {
    if !local_candidate_failure_should_invalidate_affinity_for_provider(
        &context.plan.provider_api_format,
        effect.classification,
        effect.status_code,
    ) {
        return;
    }

    unbind_scheduler_affinity_for_failed_candidate(state, context).await;
}

/// P0-3: remove the session-affinity binding pointing at the failed candidate's
/// key/endpoint. Reused by the 429 cooldown and credential-dead circuit paths,
/// which must not keep steering a session back onto a key that just entered a
/// cooldown window or an open circuit.
async fn unbind_scheduler_affinity_for_failed_candidate(
    state: &AppState,
    context: LocalExecutionEffectContext<'_>,
) {
    if let Some(cache_key) = local_scheduler_affinity_cache_key(context.report_context) {
        let Some(failed_target) = local_scheduler_affinity_target(context.plan) else {
            return;
        };
        let Some(cached_target) =
            state.read_scheduler_affinity_target(&cache_key, SCHEDULER_AFFINITY_TTL)
        else {
            return;
        };
        if local_scheduler_affinity_matches_failed_target(
            state,
            context.plan,
            &cached_target,
            &failed_target,
        )
        .await
        {
            let _ = state.remove_scheduler_affinity_cache_entry(&cache_key);
        }
    }
}

/// P0-2: project a 429 into the per-format rate-limit cooldown on the key row.
async fn record_rate_limit_cooldown_effect(
    state: &AppState,
    context: LocalExecutionEffectContext<'_>,
    effect: LocalHealthFailureEffect,
) {
    let api_format = context.plan.provider_api_format.trim();
    if api_format.is_empty() {
        return;
    }
    let Some(auth_config_fence) =
        capture_local_execution_auth_config_fence(state, context.plan).await
    else {
        return;
    };

    let effect_lock = PROVIDER_KEY_EFFECT_LOCKS.lock_for(&context.plan.key_id);
    let _effect_guard = effect_lock.lock().await;
    let observed_at_unix_secs = current_unix_secs();

    for _ in 0..PROVIDER_KEY_STATE_CAS_MAX_ATTEMPTS {
        let Some(current_key) = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&context.plan.key_id))
            .await
            .ok()
            .and_then(|mut keys| keys.drain(..).next())
        else {
            return;
        };
        if auth_config_fence
            .encrypted_auth_config()
            .is_some_and(|expected| current_key.encrypted_auth_config.as_deref() != Some(expected))
        {
            return;
        }
        let Some(health_by_format) = project_local_rate_limit_cooldown(
            current_key.health_by_format.as_ref(),
            api_format,
            observed_at_unix_secs,
            effect.retry_after_secs,
        ) else {
            // Anti write-amplification gate declined the write: the existing
            // window already covers the new deadline within tolerance.
            return;
        };
        let update = ProviderCatalogKeyHealthStateUpdate {
            key_id: context.plan.key_id.clone(),
            expected_encrypted_auth_config: auth_config_fence
                .encrypted_auth_config()
                .map(ToOwned::to_owned),
            expected_health_by_format: current_key.health_by_format.clone(),
            expected_circuit_breaker_by_format: current_key.circuit_breaker_by_format.clone(),
            health_by_format: Some(health_by_format),
            circuit_breaker_by_format: current_key.circuit_breaker_by_format,
        };
        match state
            .compare_and_update_provider_catalog_key_health_state(&update)
            .await
        {
            Ok(true) => return,
            Ok(false) => tokio::task::yield_now().await,
            Err(err) => {
                warn!(
                    "gateway orchestration effects: failed to persist rate-limit cooldown for provider {} key {}: {:?}",
                    context.plan.provider_id, context.plan.key_id, err
                );
                return;
            }
        }
    }
    warn!(
        "gateway orchestration effects: rate-limit cooldown CAS retries exhausted for provider {} key {}",
        context.plan.provider_id, context.plan.key_id
    );
}

/// P0-1 fast lane: a credential-dead failure (401/403/quota) trips the long
/// circuit immediately instead of waiting for 8 consecutive failures. Reuses
/// the existing hard-block circuit projection so probe/backoff semantics are
/// identical to the legacy path.
async fn record_credential_dead_circuit_effect(
    state: &AppState,
    context: LocalExecutionEffectContext<'_>,
    effect: LocalHealthFailureEffect,
) {
    let api_format = context.plan.provider_api_format.trim();
    if api_format.is_empty() {
        return;
    }
    let Some(auth_config_fence) =
        capture_local_execution_auth_config_fence(state, context.plan).await
    else {
        return;
    };

    let effect_lock = PROVIDER_KEY_EFFECT_LOCKS.lock_for(&context.plan.key_id);
    let _effect_guard = effect_lock.lock().await;
    let observed_at_unix_secs = current_unix_secs();

    for _ in 0..PROVIDER_KEY_STATE_CAS_MAX_ATTEMPTS {
        let Some(current_key) = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&context.plan.key_id))
            .await
            .ok()
            .and_then(|mut keys| keys.drain(..).next())
        else {
            return;
        };
        if auth_config_fence
            .encrypted_auth_config()
            .is_some_and(|expected| current_key.encrypted_auth_config.as_deref() != Some(expected))
        {
            return;
        }
        let circuit_breaker_by_format = project_local_key_circuit_open(
            current_key.circuit_breaker_by_format.as_ref(),
            api_format,
            &format!("credential_dead_{}", effect.status_code),
            observed_at_unix_secs,
            current_key.max_probe_interval_minutes,
        )
        .or_else(|| current_key.circuit_breaker_by_format.clone());
        let update = ProviderCatalogKeyHealthStateUpdate {
            key_id: context.plan.key_id.clone(),
            expected_encrypted_auth_config: auth_config_fence
                .encrypted_auth_config()
                .map(ToOwned::to_owned),
            expected_health_by_format: current_key.health_by_format.clone(),
            expected_circuit_breaker_by_format: current_key.circuit_breaker_by_format,
            health_by_format: current_key.health_by_format,
            circuit_breaker_by_format,
        };
        match state
            .compare_and_update_provider_catalog_key_health_state(&update)
            .await
        {
            Ok(true) => return,
            Ok(false) => tokio::task::yield_now().await,
            Err(err) => {
                warn!(
                    "gateway orchestration effects: failed to persist credential-dead circuit for provider {} key {}: {:?}",
                    context.plan.provider_id, context.plan.key_id, err
                );
                return;
            }
        }
    }
    warn!(
        "gateway orchestration effects: credential-dead circuit CAS retries exhausted for provider {} key {}",
        context.plan.provider_id, context.plan.key_id
    );
}

async fn record_adaptive_rate_limit_effect(
    state: &AppState,
    context: LocalExecutionEffectContext<'_>,
    effect: LocalAdaptiveRateLimitEffect<'_>,
) {
    if !local_candidate_failure_should_apply_key_effects(
        &context.plan.provider_api_format,
        effect.classification,
        effect.status_code,
    ) {
        return;
    }
    let Some(auth_config_fence) =
        capture_local_execution_auth_config_fence(state, context.plan).await
    else {
        return;
    };
    let effect_lock = PROVIDER_KEY_EFFECT_LOCKS.lock_for(&context.plan.key_id);
    let _effect_guard = effect_lock.lock().await;
    let observed_at_unix_secs = current_unix_secs();
    let current_rpm = state
        .read_recent_request_candidates(ADAPTIVE_RPM_RECENT_CANDIDATE_LIMIT)
        .await
        .ok()
        .map(|recent_candidates| {
            count_recent_rpm_requests_for_provider_key(
                &recent_candidates,
                &context.plan.key_id,
                observed_at_unix_secs,
            ) as u32
        });

    for _ in 0..PROVIDER_KEY_STATE_CAS_MAX_ATTEMPTS {
        let Some(current_key) = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&context.plan.key_id))
            .await
            .ok()
            .and_then(|mut keys| keys.drain(..).next())
        else {
            return;
        };
        if auth_config_fence
            .encrypted_auth_config()
            .is_some_and(|expected| current_key.encrypted_auth_config.as_deref() != Some(expected))
        {
            return;
        }
        let Some(projection) = project_local_adaptive_rate_limit(
            &current_key,
            effect.classification,
            effect.status_code,
            current_rpm,
            effect.headers,
            observed_at_unix_secs,
        ) else {
            return;
        };
        let expected = ProviderCatalogKeyAdaptiveState::from(&current_key);
        let mut next = expected.clone();
        next.rpm_429_count = Some(projection.rpm_429_count);
        next.learned_rpm_limit = projection.learned_rpm_limit;
        next.last_429_at_unix_secs = Some(projection.last_429_at_unix_secs);
        next.last_429_type = Some(projection.last_429_type);
        next.adjustment_history = projection.adjustment_history;
        next.utilization_samples = projection.utilization_samples;
        next.last_probe_increase_at_unix_secs = projection.last_probe_increase_at_unix_secs;
        next.last_rpm_peak = projection.last_rpm_peak;
        let update = ProviderCatalogKeyAdaptiveStateUpdate {
            key_id: context.plan.key_id.clone(),
            expected_encrypted_auth_config: auth_config_fence
                .encrypted_auth_config()
                .map(ToOwned::to_owned),
            expected,
            next,
            status_snapshot_patch: adaptive_status_snapshot_patch(&projection.status_snapshot),
            updated_at_unix_secs: Some(observed_at_unix_secs),
        };
        provider_key_adaptive_success_persist_gate_reset(&context.plan.key_id);
        match state
            .compare_and_update_provider_catalog_key_adaptive_state(&update)
            .await
        {
            Ok(true) => return,
            Ok(false) => tokio::task::yield_now().await,
            Err(err) => {
                warn!(
                    "gateway orchestration effects: failed to persist adaptive rate-limit projection for provider {} endpoint {} key {}: {:?}",
                    context.plan.provider_id, context.plan.endpoint_id, context.plan.key_id, err
                );
                return;
            }
        }
    }
    warn!(
        "gateway orchestration effects: adaptive rate-limit CAS retries exhausted for provider {} endpoint {} key {}",
        context.plan.provider_id, context.plan.endpoint_id, context.plan.key_id
    );
}

fn adaptive_status_snapshot_patch(status_snapshot: &Value) -> Value {
    const OWNED_FIELDS: [&str; 6] = [
        "observation_count",
        "header_observation_count",
        "latest_upstream_limit",
        "learning_confidence",
        "enforcement_active",
        "known_boundary",
    ];
    let Some(snapshot) = status_snapshot.as_object() else {
        return serde_json::json!({});
    };
    Value::Object(
        OWNED_FIELDS
            .into_iter()
            .filter_map(|field| {
                snapshot
                    .get(field)
                    .cloned()
                    .map(|value| (field.to_string(), value))
            })
            .collect(),
    )
}

async fn record_adaptive_success_effect(
    state: &AppState,
    context: LocalExecutionEffectContext<'_>,
    _effect: LocalAdaptiveSuccessEffect,
) {
    let Some(auth_config_fence) =
        capture_local_execution_auth_config_fence(state, context.plan).await
    else {
        return;
    };
    let observed_at_unix_secs = current_unix_secs();
    let Some(current_key) = state
        .read_provider_catalog_keys_by_ids(std::slice::from_ref(&context.plan.key_id))
        .await
        .ok()
        .and_then(|mut keys| keys.drain(..).next())
    else {
        return;
    };
    if auth_config_fence
        .encrypted_auth_config()
        .is_some_and(|expected| current_key.encrypted_auth_config.as_deref() != Some(expected))
    {
        return;
    }
    if current_key.rpm_limit.is_some()
        || current_key
            .learned_rpm_limit
            .filter(|value| *value > 0)
            .is_none()
    {
        return;
    }
    let Some(gate_token) = provider_key_adaptive_success_persist_gate_admit(&context.plan.key_id)
    else {
        return;
    };

    let effect_lock = PROVIDER_KEY_EFFECT_LOCKS.lock_for(&context.plan.key_id);
    let _effect_guard = effect_lock.lock().await;
    if !provider_key_adaptive_success_persist_gate_admission_is_current(
        &context.plan.key_id,
        gate_token,
    ) {
        return;
    }
    let Some(recent_candidates) = state
        .read_recent_request_candidates(ADAPTIVE_RPM_RECENT_CANDIDATE_LIMIT)
        .await
        .ok()
    else {
        return;
    };
    let current_rpm = count_recent_rpm_requests_for_provider_key(
        &recent_candidates,
        &context.plan.key_id,
        observed_at_unix_secs,
    ) as u32;

    for _ in 0..PROVIDER_KEY_STATE_CAS_MAX_ATTEMPTS {
        let Some(current_key) = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&context.plan.key_id))
            .await
            .ok()
            .and_then(|mut keys| keys.drain(..).next())
        else {
            return;
        };
        if auth_config_fence
            .encrypted_auth_config()
            .is_some_and(|expected| current_key.encrypted_auth_config.as_deref() != Some(expected))
        {
            return;
        }
        if current_key.rpm_limit.is_some()
            || current_key
                .learned_rpm_limit
                .filter(|value| *value > 0)
                .is_none()
        {
            return;
        }
        let Some(projection) =
            project_local_adaptive_success(&current_key, current_rpm, observed_at_unix_secs)
        else {
            return;
        };
        let expected = ProviderCatalogKeyAdaptiveState::from(&current_key);
        let mut next = expected.clone();
        next.learned_rpm_limit = projection.learned_rpm_limit;
        next.adjustment_history = projection.adjustment_history;
        next.utilization_samples = projection.utilization_samples;
        next.last_probe_increase_at_unix_secs = projection.last_probe_increase_at_unix_secs;
        let update = ProviderCatalogKeyAdaptiveStateUpdate {
            key_id: context.plan.key_id.clone(),
            expected_encrypted_auth_config: auth_config_fence
                .encrypted_auth_config()
                .map(ToOwned::to_owned),
            expected,
            next,
            status_snapshot_patch: adaptive_status_snapshot_patch(&projection.status_snapshot),
            updated_at_unix_secs: Some(observed_at_unix_secs),
        };
        match state
            .compare_and_update_provider_catalog_key_adaptive_state(&update)
            .await
        {
            Ok(true) => return,
            Ok(false) => tokio::task::yield_now().await,
            Err(err) => {
                warn!(
                    "gateway orchestration effects: failed to persist adaptive success projection for provider {} endpoint {} key {}: {:?}",
                    context.plan.provider_id, context.plan.endpoint_id, context.plan.key_id, err
                );
                return;
            }
        }
    }
    warn!(
        "gateway orchestration effects: adaptive success CAS retries exhausted for provider {} endpoint {} key {}",
        context.plan.provider_id, context.plan.endpoint_id, context.plan.key_id
    );
}

fn provider_key_adaptive_success_persist_gate_admit(key_id: &str) -> Option<u64> {
    if cfg!(test) {
        return Some(0);
    }
    let interval = *ADAPTIVE_SUCCESS_PERSIST_MIN_INTERVAL;
    if interval.is_zero() {
        return Some(0);
    }
    let token = ADAPTIVE_SUCCESS_PERSIST_GATE_NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
    ADAPTIVE_SUCCESS_PERSIST_GATE
        .insert_if_absent_fresh(
            provider_key_adaptive_success_persist_gate_key(key_id),
            token,
            interval,
            ADAPTIVE_SUCCESS_PERSIST_GATE_MAX_ENTRIES,
        )
        .then_some(token)
}

fn provider_key_adaptive_success_persist_gate_admission_is_current(
    key_id: &str,
    token: u64,
) -> bool {
    if cfg!(test) || ADAPTIVE_SUCCESS_PERSIST_MIN_INTERVAL.is_zero() {
        return true;
    }
    ADAPTIVE_SUCCESS_PERSIST_GATE.get_fresh(
        &provider_key_adaptive_success_persist_gate_key(key_id),
        *ADAPTIVE_SUCCESS_PERSIST_MIN_INTERVAL,
    ) == Some(token)
}

fn provider_key_adaptive_success_persist_gate_reset(key_id: &str) {
    ADAPTIVE_SUCCESS_PERSIST_GATE.remove(&provider_key_adaptive_success_persist_gate_key(key_id));
}

fn provider_key_adaptive_success_persist_gate_key(key_id: &str) -> String {
    format!("adaptive-success:{key_id}")
}

async fn record_health_failure_effect(
    state: &AppState,
    context: LocalExecutionEffectContext<'_>,
    effect: LocalHealthFailureEffect,
) {
    let failure_class =
        aether_scheduler_core::UpstreamFailureClass::from_status_code(effect.status_code);

    match failure_class {
        // 429: rate limiting, not a fault. Project the cooldown window (P0-2),
        // unbind the session affinity from this key (P0-3), and skip the
        // failure-health / circuit accounting entirely (P0-1).
        aether_scheduler_core::UpstreamFailureClass::RateLimited => {
            record_rate_limit_cooldown_effect(state, context, effect).await;
            unbind_scheduler_affinity_for_failed_candidate(state, context).await;
        }
        // 401/403/quota: the credential is dead — trip the long circuit on the
        // first failure instead of eight (P0-1 fast lane) and unbind affinity.
        aether_scheduler_core::UpstreamFailureClass::CredentialDead => {
            if !local_candidate_failure_should_apply_key_effects(
                &context.plan.provider_api_format,
                effect.classification,
                effect.status_code,
            ) {
                return;
            }
            record_credential_dead_circuit_effect(state, context, effect).await;
            unbind_scheduler_affinity_for_failed_candidate(state, context).await;
        }
        // 5xx / transport: legacy accounting (60s/8-failure cooldown +
        // 8-consecutive-failure circuit), unchanged.
        aether_scheduler_core::UpstreamFailureClass::Transient => {
            record_transient_failure_effect(state, context, effect).await;
        }
    }
}

async fn record_transient_failure_effect(
    state: &AppState,
    context: LocalExecutionEffectContext<'_>,
    effect: LocalHealthFailureEffect,
) {
    if !local_candidate_failure_should_apply_key_effects(
        &context.plan.provider_api_format,
        effect.classification,
        effect.status_code,
    ) {
        return;
    }
    let api_format = context.plan.provider_api_format.trim();
    if api_format.is_empty() {
        return;
    }
    let Some(auth_config_fence) =
        capture_local_execution_auth_config_fence(state, context.plan).await
    else {
        return;
    };

    let effect_lock = PROVIDER_KEY_EFFECT_LOCKS.lock_for(&context.plan.key_id);
    let _effect_guard = effect_lock.lock().await;
    let observed_at_unix_secs = current_unix_secs();
    provider_key_health_success_persist_gate_reset(&context.plan.key_id, api_format);

    for _ in 0..PROVIDER_KEY_STATE_CAS_MAX_ATTEMPTS {
        let Some(current_key) = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&context.plan.key_id))
            .await
            .ok()
            .and_then(|mut keys| keys.drain(..).next())
        else {
            return;
        };
        if auth_config_fence
            .encrypted_auth_config()
            .is_some_and(|expected| current_key.encrypted_auth_config.as_deref() != Some(expected))
        {
            return;
        }
        let Some(health_by_format) = project_local_failure_health(
            current_key.health_by_format.as_ref(),
            api_format,
            effect.classification,
            effect.status_code,
            observed_at_unix_secs,
        ) else {
            return;
        };
        let consecutive_failures = health_by_format
            .get(api_format)
            .and_then(|value| value.get("consecutive_failures"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        // P1-7: a failure during the post-close recovery ramp re-opens the
        // circuit immediately, continuing the exponential probe ladder.
        // P1-6: the rolling success-rate verdict opens the circuit even when
        // the consecutive ladder is below threshold (flapping keys).
        let ramp_active =
            circuit_ramp_active(current_key.circuit_breaker_by_format.as_ref(), api_format);
        let success_rate_breached = if ramp_active {
            Some(true)
        } else {
            Some(circuit_success_rate_breached(
                current_key.circuit_breaker_by_format.as_ref(),
                api_format,
                observed_at_unix_secs,
            ))
        };
        let circuit_breaker_by_format = project_local_key_circuit_failure_with_success_rate(
            current_key.circuit_breaker_by_format.as_ref(),
            api_format,
            observed_at_unix_secs,
            consecutive_failures,
            current_key.max_probe_interval_minutes,
            success_rate_breached,
        )
        .or_else(|| current_key.circuit_breaker_by_format.clone());
        let update = ProviderCatalogKeyHealthStateUpdate {
            key_id: context.plan.key_id.clone(),
            expected_encrypted_auth_config: auth_config_fence
                .encrypted_auth_config()
                .map(ToOwned::to_owned),
            expected_health_by_format: current_key.health_by_format,
            expected_circuit_breaker_by_format: current_key.circuit_breaker_by_format,
            health_by_format: Some(health_by_format),
            circuit_breaker_by_format,
        };
        match state
            .compare_and_update_provider_catalog_key_health_state(&update)
            .await
        {
            Ok(true) => return,
            Ok(false) => tokio::task::yield_now().await,
            Err(err) => {
                warn!(
                    "gateway orchestration effects: failed to persist health failure projection for provider {} endpoint {} key {}: {:?}",
                    context.plan.provider_id, context.plan.endpoint_id, context.plan.key_id, err
                );
                return;
            }
        }
    }
    warn!(
        "gateway orchestration effects: health failure CAS retries exhausted for provider {} endpoint {} key {}",
        context.plan.provider_id, context.plan.endpoint_id, context.plan.key_id
    );
}

async fn record_health_success_effect(
    state: &AppState,
    context: LocalExecutionEffectContext<'_>,
    _effect: LocalHealthSuccessEffect,
) {
    remember_successful_local_scheduler_affinity(state, context).await;
    record_scheduler_latency_observation(context);

    let api_format = context.plan.provider_api_format.trim();
    if api_format.is_empty() {
        return;
    }
    let Some(auth_config_fence) =
        capture_local_execution_auth_config_fence(state, context.plan).await
    else {
        return;
    };

    // Health updates replace both JSON snapshots in one write. Serialize the success
    // read/project/write with failure and circuit-clear effects for this provider key so a
    // stale success snapshot cannot overwrite a newer failure counter or open circuit.
    let effect_lock = PROVIDER_KEY_EFFECT_LOCKS.lock_for(&context.plan.key_id);
    let _effect_guard = effect_lock.lock().await;

    let mut persist_gate_checked = false;

    for _ in 0..PROVIDER_KEY_STATE_CAS_MAX_ATTEMPTS {
        let Some(current_key) = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&context.plan.key_id))
            .await
            .ok()
            .and_then(|mut keys| keys.drain(..).next())
        else {
            return;
        };
        if auth_config_fence
            .encrypted_auth_config()
            .is_some_and(|expected| current_key.encrypted_auth_config.as_deref() != Some(expected))
        {
            return;
        }
        // P1-7: success during the recovery ramp decrements the counter and
        // raises health partway (health ramp), a final ramp success clears it
        // to full 1.0; a plain success (no ramp) keeps the legacy full reset.
        let ramp_active =
            circuit_ramp_active(current_key.circuit_breaker_by_format.as_ref(), api_format);
        let projected_health = if ramp_active {
            project_local_ramp_success_health(
                current_key.health_by_format.as_ref(),
                current_key.circuit_breaker_by_format.as_ref(),
                api_format,
            )
        } else {
            project_local_success_health(current_key.health_by_format.as_ref(), api_format)
        };
        let Some(health_by_format) = projected_health else {
            return;
        };
        let circuit_breaker_update_owned =
            current_key
                .circuit_breaker_by_format
                .as_ref()
                .and_then(|current| {
                    project_local_key_circuit_closed_with_ramp(
                        Some(current),
                        api_format,
                        current_key.health_by_format.as_ref(),
                    )
                });
        if current_key.health_by_format.as_ref() == Some(&health_by_format)
            && circuit_breaker_update_owned.as_ref()
                == current_key.circuit_breaker_by_format.as_ref()
        {
            return;
        }
        if !persist_gate_checked {
            if !provider_key_health_success_persist_gate_allows(
                &context.plan.key_id,
                api_format,
                circuit_breaker_update_owned.is_some(),
            ) {
                return;
            }
            persist_gate_checked = true;
        }
        let circuit_breaker_by_format =
            circuit_breaker_update_owned.or_else(|| current_key.circuit_breaker_by_format.clone());
        let update = ProviderCatalogKeyHealthStateUpdate {
            key_id: context.plan.key_id.clone(),
            expected_encrypted_auth_config: auth_config_fence
                .encrypted_auth_config()
                .map(ToOwned::to_owned),
            expected_health_by_format: current_key.health_by_format,
            expected_circuit_breaker_by_format: current_key.circuit_breaker_by_format,
            health_by_format: Some(health_by_format),
            circuit_breaker_by_format,
        };
        match state
            .compare_and_update_provider_catalog_key_health_state(&update)
            .await
        {
            Ok(true) => return,
            Ok(false) => tokio::task::yield_now().await,
            Err(err) => {
                warn!(
                    "gateway orchestration effects: failed to persist health success projection for provider {} endpoint {} key {}: {:?}",
                    context.plan.provider_id, context.plan.endpoint_id, context.plan.key_id, err
                );
                return;
            }
        }
    }
    warn!(
        "gateway orchestration effects: health success CAS retries exhausted for provider {} endpoint {} key {}",
        context.plan.provider_id, context.plan.endpoint_id, context.plan.key_id
    );
}

fn provider_key_health_success_persist_gate_allows(
    key_id: &str,
    api_format: &str,
    closes_circuit: bool,
) -> bool {
    if closes_circuit {
        return true;
    }
    let min_interval = *HEALTH_SUCCESS_PERSIST_MIN_INTERVAL;
    if min_interval.is_zero() {
        return true;
    }
    let key = provider_key_health_success_persist_gate_key(key_id, api_format);
    HEALTH_SUCCESS_PERSIST_GATE.insert_if_absent_fresh(
        key,
        (),
        min_interval,
        HEALTH_SUCCESS_PERSIST_GATE_MAX_ENTRIES,
    )
}

fn provider_key_health_success_persist_gate_reset(key_id: &str, api_format: &str) {
    let key = provider_key_health_success_persist_gate_key(key_id, api_format);
    HEALTH_SUCCESS_PERSIST_GATE.remove(&key);
}

fn provider_key_health_success_persist_gate_key(key_id: &str, api_format: &str) -> String {
    format!("success:{key_id}:{api_format}")
}

fn local_candidate_failure_should_invalidate_affinity(
    classification: LocalFailoverClassification,
    status_code: u16,
) -> bool {
    if status_code < 400 {
        return false;
    }

    match classification {
        LocalFailoverClassification::RetrySuccessPattern
        | LocalFailoverClassification::RetryStatusCode
        | LocalFailoverClassification::RetryUpstreamFailure => true,
        LocalFailoverClassification::UseDefault | LocalFailoverClassification::StopStatusCode => {
            status_code >= 500
        }
        LocalFailoverClassification::StopErrorPattern
        | LocalFailoverClassification::StopExecutionError
        | LocalFailoverClassification::StopCyberPolicy => false,
    }
}

fn local_candidate_failure_should_invalidate_affinity_for_provider(
    provider_api_format: &str,
    classification: LocalFailoverClassification,
    status_code: u16,
) -> bool {
    if !local_candidate_failure_should_invalidate_affinity(classification, status_code) {
        return false;
    }
    if !provider_api_format
        .trim()
        .eq_ignore_ascii_case("claude:messages")
    {
        return true;
    }

    let disposition =
        classify_failure_disposition(provider_api_format, classification, status_code);
    !(disposition.retry_action == crate::orchestration::FailureRetryAction::Stop
        && disposition.failure_scope == FailureScope::None)
}

fn local_candidate_failure_should_apply_key_effects(
    provider_api_format: &str,
    classification: LocalFailoverClassification,
    status_code: u16,
) -> bool {
    if !provider_api_format
        .trim()
        .eq_ignore_ascii_case("claude:messages")
    {
        return true;
    }

    matches!(
        classify_failure_disposition(provider_api_format, classification, status_code)
            .failure_scope,
        FailureScope::Credential
    )
}

#[cfg(test)]
mod tests {
    use aether_contracts::{ExecutionPlan, RequestBody};
    use aether_crypto::{encrypt_python_fernet_plaintext, DEVELOPMENT_ENCRYPTION_KEY};
    use aether_data::repository::candidates::InMemoryRequestCandidateRepository;
    use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
    use aether_data_contracts::repository::candidates::{
        RequestCandidateStatus, StoredRequestCandidate,
    };
    use aether_data_contracts::repository::provider_catalog::{
        ProviderCatalogKeyAdaptiveState, StoredProviderCatalogEndpoint, StoredProviderCatalogKey,
        StoredProviderCatalogProvider,
    };
    use aether_test_support::ManagedRedisServer;
    use aether_usage_runtime::GatewaySyncReportRequest;
    use serde_json::{json, Value};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use super::{
        apply_local_execution_effect, apply_local_stream_failure_effects,
        apply_local_stream_success_effects, local_candidate_failure_should_apply_key_effects,
        LocalAdaptiveRateLimitEffect, LocalAdaptiveSuccessEffect, LocalAttemptFailureEffect,
        LocalExecutionEffect, LocalExecutionEffectContext, LocalHealthFailureEffect,
        LocalHealthSuccessEffect, LocalStreamFailureEffect,
    };
    use crate::data::{GatewayDataConfig, GatewayDataState};
    use crate::orchestration::{
        apply_local_report_effect, LocalFailoverClassification, LocalReportEffect,
    };
    use crate::scheduler::affinity::SCHEDULER_AFFINITY_TTL;
    use crate::usage::GatewayStreamReportRequest;
    use crate::AppState;
    use aether_scheduler_core::{
        build_scheduler_affinity_cache_key_for_api_key_id,
        build_scheduler_affinity_cache_key_for_api_key_id_with_client_session,
        build_scheduler_affinity_cache_key_for_api_key_id_with_client_session_and_scope,
        ClientSessionAffinity, SchedulerAffinityScope, SchedulerAffinityTarget,
    };
    async fn start_managed_redis_or_skip() -> Option<ManagedRedisServer> {
        match ManagedRedisServer::start().await {
            Ok(server) => Some(server),
            Err(err) if err.to_string().contains("No such file or directory") => {
                eprintln!("skipping redis-backed orchestration effect test: {err}");
                None
            }
            Err(err) => panic!("redis server should start: {err}"),
        }
    }
    fn sample_plan() -> ExecutionPlan {
        ExecutionPlan {
            request_id: "req-1".to_string(),
            candidate_id: Some("cand-1".to_string()),
            provider_name: Some("openai".to_string()),
            provider_id: "prov-1".to_string(),
            endpoint_id: "ep-1".to_string(),
            key_id: "key-1".to_string(),
            method: "POST".to_string(),
            url: "https://example.com/v1/chat/completions".to_string(),
            headers: BTreeMap::new(),
            content_type: Some("application/json".to_string()),
            content_encoding: None,
            body: RequestBody::from_json(json!({"model":"gpt-5"})),
            stream: false,
            client_api_format: "openai:chat".to_string(),
            provider_api_format: "openai:chat".to_string(),
            model_name: Some("gpt-5".to_string()),
            proxy: None,
            transport_profile: None,
            timeouts: None,
        }
    }
    fn sample_claude_plan() -> ExecutionPlan {
        let mut plan = sample_plan();
        plan.provider_name = Some("anthropic".to_string());
        plan.provider_api_format = "claude:messages".to_string();
        plan
    }
    fn sample_stream_report() -> GatewayStreamReportRequest {
        GatewayStreamReportRequest {
            trace_id: "trace-stream-effects".to_string(),
            report_kind: "openai_chat_stream_success".to_string(),
            report_context: None,
            status_code: 200,
            headers: BTreeMap::new(),
            provider_body_base64: None,
            provider_body_state: None,
            client_body_base64: None,
            client_body_state: None,
            terminal_summary: None,
            telemetry: None,
        }
    }
    fn session_affinity() -> ClientSessionAffinity {
        ClientSessionAffinity::new(
            Some("generic".to_string()),
            Some("session=session-1;agent=coder".to_string()),
        )
    }
    fn session_report_context() -> Value {
        json!({
            "api_key_id": "api-key-1",
            "client_api_format": "openai:chat",
            "model": "gpt-5",
            "client_session_affinity": {
                "client_family": "generic",
                "session_key": "session=session-1;agent=coder"
            },
            "original_headers": {
                "x-aether-session-id": "raw-session",
                "x-aether-agent-id": "raw-agent"
            },
            "original_request_body": {
                "model": "gpt-5"
            }
        })
    }
    fn session_scheduler_affinity_cache_key() -> String {
        build_scheduler_affinity_cache_key_for_api_key_id_with_client_session(
            "api-key-1",
            "openai:chat",
            "gpt-5",
            Some(&session_affinity()),
        )
        .expect("session scheduler affinity cache key should build")
    }
    fn sample_health_provider() -> StoredProviderCatalogProvider {
        StoredProviderCatalogProvider::new(
            "prov-1".to_string(),
            "openai".to_string(),
            Some("https://example.com".to_string()),
            "custom".to_string(),
        )
        .expect("provider should build")
    }
    fn sample_health_endpoint() -> StoredProviderCatalogEndpoint {
        StoredProviderCatalogEndpoint::new(
            "ep-1".to_string(),
            "prov-1".to_string(),
            "openai:chat".to_string(),
            Some("openai".to_string()),
            Some("chat".to_string()),
            true,
        )
        .expect("endpoint should build")
        .with_transport_fields(
            "https://example.com/v1/chat/completions".to_string(),
            None,
            None,
            Some(2),
            None,
            None,
            None,
            None,
        )
        .expect("endpoint transport should build")
    }
    fn sample_health_key() -> StoredProviderCatalogKey {
        StoredProviderCatalogKey::new(
            "key-1".to_string(),
            "prov-1".to_string(),
            "prod".to_string(),
            "api_key".to_string(),
            None,
            true,
        )
        .expect("key should build")
        .with_transport_fields(
            Some(serde_json::json!(["openai:chat"])),
            encrypt_python_fernet_plaintext(DEVELOPMENT_ENCRYPTION_KEY, "sk-test")
                .expect("api key should encrypt"),
            None,
            Some(serde_json::json!({"openai:chat": 1})),
            None,
            None,
            None,
            None,
        )
        .expect("key transport should build")
    }
    fn health_state() -> AppState {
        let repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
            vec![sample_health_provider()],
            vec![sample_health_endpoint()],
            vec![sample_health_key()],
        ));
        AppState::new()
            .expect("gateway state should build")
            .with_data_state_for_tests(
                GatewayDataState::with_provider_catalog_repository_for_tests(repository)
                    .with_encryption_key_for_tests(DEVELOPMENT_ENCRYPTION_KEY),
            )
    }
    fn health_state_with_key(key: StoredProviderCatalogKey) -> AppState {
        let repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
            vec![sample_health_provider()],
            vec![sample_health_endpoint()],
            vec![key],
        ));
        AppState::new()
            .expect("gateway state should build")
            .with_data_state_for_tests(
                GatewayDataState::with_provider_catalog_repository_for_tests(repository)
                    .with_encryption_key_for_tests(DEVELOPMENT_ENCRYPTION_KEY),
            )
    }
    fn sample_adaptive_key() -> StoredProviderCatalogKey {
        let mut key = sample_health_key();
        key.name = "adaptive".to_string();
        key.rpm_limit = None;
        key.learned_rpm_limit = Some(12);
        key.rpm_429_count = Some(1);
        key
    }
    fn adaptive_state() -> AppState {
        let repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
            vec![sample_health_provider()],
            vec![sample_health_endpoint()],
            vec![sample_adaptive_key()],
        ));
        AppState::new()
            .expect("gateway state should build")
            .with_data_state_for_tests(
                GatewayDataState::with_provider_catalog_repository_for_tests(repository)
                    .with_encryption_key_for_tests(DEVELOPMENT_ENCRYPTION_KEY),
            )
    }
    fn adaptive_state_with_request_candidates(
        key: StoredProviderCatalogKey,
        request_candidates: Vec<StoredRequestCandidate>,
    ) -> AppState {
        let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
            vec![sample_health_provider()],
            vec![sample_health_endpoint()],
            vec![key],
        ));
        let request_candidates =
            Arc::new(InMemoryRequestCandidateRepository::seed(request_candidates));
        AppState::new()
            .expect("gateway state should build")
            .with_data_state_for_tests(
                GatewayDataState::with_provider_catalog_repository_for_tests(provider_catalog)
                    .with_request_candidate_reader(request_candidates)
                    .with_encryption_key_for_tests(DEVELOPMENT_ENCRYPTION_KEY),
            )
    }
    fn fixed_limit_state() -> AppState {
        let mut key = sample_health_key();
        key.rpm_limit = Some(24);

        let repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
            vec![sample_health_provider()],
            vec![sample_health_endpoint()],
            vec![key],
        ));
        AppState::new()
            .expect("gateway state should build")
            .with_data_state_for_tests(
                GatewayDataState::with_provider_catalog_repository_for_tests(repository)
                    .with_encryption_key_for_tests(DEVELOPMENT_ENCRYPTION_KEY),
            )
    }
    #[tokio::test]
    async fn attempt_failure_invalidates_scheduler_affinity_cache() {
        let state = AppState::new().expect("gateway state should build");
        let plan = sample_plan();
        let report_context = json!({
            "api_key_id": "api-key-1",
            "client_api_format": "openai:chat",
            "model": "gpt-5",
        });
        let cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id("api-key-1", "openai:chat", "gpt-5")
                .expect("scheduler affinity cache key should build");

        state.remember_scheduler_affinity_target(
            &cache_key,
            SchedulerAffinityTarget {
                provider_id: "prov-1".to_string(),
                endpoint_id: "ep-1".to_string(),
                key_id: "key-1".to_string(),
            },
            SCHEDULER_AFFINITY_TTL,
            16,
        );
        assert!(state
            .read_scheduler_affinity_target(cache_key.as_str(), SCHEDULER_AFFINITY_TTL)
            .is_some());

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: Some(&report_context),
            },
            LocalExecutionEffect::AttemptFailure(LocalAttemptFailureEffect {
                status_code: 429,
                classification: LocalFailoverClassification::RetryUpstreamFailure,
            }),
        )
        .await;

        assert!(state
            .read_scheduler_affinity_target(cache_key.as_str(), SCHEDULER_AFFINITY_TTL)
            .is_none());
    }
    #[tokio::test]
    async fn attempt_failure_invalidates_session_scoped_scheduler_affinity_cache() {
        let state = AppState::new().expect("gateway state should build");
        let plan = sample_plan();
        let report_context = session_report_context();
        let session_cache_key = session_scheduler_affinity_cache_key();
        let legacy_cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id("api-key-1", "openai:chat", "gpt-5")
                .expect("legacy scheduler affinity cache key should build");

        for cache_key in [&session_cache_key, &legacy_cache_key] {
            state.remember_scheduler_affinity_target(
                cache_key.as_str(),
                SchedulerAffinityTarget {
                    provider_id: "prov-1".to_string(),
                    endpoint_id: "ep-1".to_string(),
                    key_id: "key-1".to_string(),
                },
                SCHEDULER_AFFINITY_TTL,
                16,
            );
        }

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: Some(&report_context),
            },
            LocalExecutionEffect::AttemptFailure(LocalAttemptFailureEffect {
                status_code: 429,
                classification: LocalFailoverClassification::RetryUpstreamFailure,
            }),
        )
        .await;

        assert!(state
            .read_scheduler_affinity_target(session_cache_key.as_str(), SCHEDULER_AFFINITY_TTL)
            .is_none());
        assert!(state
            .read_scheduler_affinity_target(legacy_cache_key.as_str(), SCHEDULER_AFFINITY_TTL)
            .is_some());
    }
    #[tokio::test]
    async fn attempt_failure_keeps_scheduler_affinity_for_non_affinity_candidate() {
        let state = AppState::new().expect("gateway state should build");
        let plan = sample_plan();
        let report_context = json!({
            "api_key_id": "api-key-1",
            "client_api_format": "openai:chat",
            "model": "gpt-5",
        });
        let cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id("api-key-1", "openai:chat", "gpt-5")
                .expect("scheduler affinity cache key should build");
        let affinity_target = SchedulerAffinityTarget {
            provider_id: "prov-2".to_string(),
            endpoint_id: "ep-2".to_string(),
            key_id: "key-2".to_string(),
        };

        state.remember_scheduler_affinity_target(
            &cache_key,
            affinity_target.clone(),
            SCHEDULER_AFFINITY_TTL,
            16,
        );

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: Some(&report_context),
            },
            LocalExecutionEffect::AttemptFailure(LocalAttemptFailureEffect {
                status_code: 524,
                classification: LocalFailoverClassification::RetryUpstreamFailure,
            }),
        )
        .await;

        assert_eq!(
            state.read_scheduler_affinity_target(cache_key.as_str(), SCHEDULER_AFFINITY_TTL),
            Some(affinity_target)
        );
    }
    #[tokio::test]
    async fn attempt_failure_keeps_scheduler_affinity_for_non_failure_status() {
        let state = AppState::new().expect("gateway state should build");
        let plan = sample_plan();
        let report_context = json!({
            "api_key_id": "api-key-1",
            "client_api_format": "openai:chat",
            "model": "gpt-5",
        });
        let cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id("api-key-1", "openai:chat", "gpt-5")
                .expect("scheduler affinity cache key should build");

        state.remember_scheduler_affinity_target(
            &cache_key,
            SchedulerAffinityTarget {
                provider_id: "prov-1".to_string(),
                endpoint_id: "ep-1".to_string(),
                key_id: "key-1".to_string(),
            },
            SCHEDULER_AFFINITY_TTL,
            16,
        );

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: Some(&report_context),
            },
            LocalExecutionEffect::AttemptFailure(LocalAttemptFailureEffect {
                status_code: 200,
                classification: LocalFailoverClassification::UseDefault,
            }),
        )
        .await;

        assert!(state
            .read_scheduler_affinity_target(cache_key.as_str(), SCHEDULER_AFFINITY_TTL)
            .is_some());
    }
    #[tokio::test]
    async fn stream_success_effect_helper_projects_health_and_scheduler_affinity() {
        let state = AppState::new().expect("gateway state should build");
        let plan = sample_plan();
        let report_context = json!({
            "api_key_id": "api-key-1",
            "client_api_format": "openai:chat",
            "model": "gpt-5",
        });
        let cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id("api-key-1", "openai:chat", "gpt-5")
                .expect("scheduler affinity cache key should build");
        let payload = sample_stream_report();

        apply_local_stream_success_effects(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: Some(&report_context),
            },
            &payload,
        )
        .await;

        assert_eq!(
            state.read_scheduler_affinity_target(cache_key.as_str(), SCHEDULER_AFFINITY_TTL),
            Some(SchedulerAffinityTarget {
                provider_id: "prov-1".to_string(),
                endpoint_id: "ep-1".to_string(),
                key_id: "key-1".to_string(),
            })
        );
    }
    #[tokio::test]
    async fn stream_failure_effect_helper_returns_analysis_and_projects_health() {
        let state = health_state();
        let plan = sample_plan();
        let headers = BTreeMap::new();
        let analysis = apply_local_stream_failure_effects(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: None,
            },
            LocalStreamFailureEffect::new(503, &headers, Some("upstream unavailable"))
                .with_stream_timeout(),
        )
        .await;

        assert_eq!(
            analysis.classification,
            LocalFailoverClassification::UseDefault
        );
        assert_eq!(analysis.decision.as_str(), "use_default");
        let stored_key = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&plan.key_id))
            .await
            .expect("provider catalog keys should load")
            .into_iter()
            .next()
            .expect("stored key should exist");
        assert_eq!(
            stored_key
                .health_by_format
                .as_ref()
                .and_then(|value| value.get("openai:chat"))
                .and_then(|value| value.get("consecutive_failures"))
                .and_then(Value::as_u64),
            Some(1)
        );
    }
    #[tokio::test]
    async fn configured_stop_pattern_keeps_scheduler_affinity_cache() {
        let state = AppState::new().expect("gateway state should build");
        let plan = sample_plan();
        let report_context = json!({
            "api_key_id": "api-key-1",
            "client_api_format": "openai:chat",
            "model": "gpt-5",
        });
        let cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id("api-key-1", "openai:chat", "gpt-5")
                .expect("scheduler affinity cache key should build");

        state.remember_scheduler_affinity_target(
            &cache_key,
            SchedulerAffinityTarget {
                provider_id: "prov-1".to_string(),
                endpoint_id: "ep-1".to_string(),
                key_id: "key-1".to_string(),
            },
            SCHEDULER_AFFINITY_TTL,
            16,
        );

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: Some(&report_context),
            },
            LocalExecutionEffect::AttemptFailure(LocalAttemptFailureEffect {
                status_code: 400,
                classification: LocalFailoverClassification::StopErrorPattern,
            }),
        )
        .await;

        assert!(state
            .read_scheduler_affinity_target(cache_key.as_str(), SCHEDULER_AFFINITY_TTL)
            .is_some());
    }
    #[tokio::test]
    async fn success_remembers_scheduler_affinity_cache_for_final_candidate() {
        let state = AppState::new().expect("gateway state should build");
        let plan = sample_plan();
        let report_context = json!({
            "api_key_id": "api-key-1",
            "client_api_format": "openai:chat",
            "model": "gpt-5",
        });
        let cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id("api-key-1", "openai:chat", "gpt-5")
                .expect("scheduler affinity cache key should build");

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: Some(&report_context),
            },
            LocalExecutionEffect::HealthSuccess(LocalHealthSuccessEffect),
        )
        .await;

        assert_eq!(
            state.read_scheduler_affinity_target(cache_key.as_str(), SCHEDULER_AFFINITY_TTL),
            Some(SchedulerAffinityTarget {
                provider_id: "prov-1".to_string(),
                endpoint_id: "ep-1".to_string(),
                key_id: "key-1".to_string(),
            })
        );
    }
    #[tokio::test]
    async fn routing_profile_cache_affinity_overrides_legacy_fixed_mode_on_success() {
        let state = AppState::new()
            .expect("gateway state should build")
            .with_data_state_for_tests(
                GatewayDataState::disabled().with_system_config_values_for_tests(vec![(
                    "scheduling_mode".to_string(),
                    json!("fixed_order"),
                )]),
            );
        let plan = sample_plan();
        let affinity = session_affinity();
        let scope = SchedulerAffinityScope::new("routing-group-1", Some(7));
        let report_context = json!({
            "api_key_id": "api-key-1",
            "client_api_format": "openai:chat",
            "model": "gpt-5",
            "client_session_affinity": {
                "client_family": "generic",
                "session_key": "session=session-1;agent=coder"
            },
            "scheduler_affinity_policy": {
                "scheduling_mode": "cache_affinity",
                "scope": {
                    "routing_group_id": "routing-group-1",
                    "routing_group_version": 7
                }
            }
        });
        let scoped_cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id_with_client_session_and_scope(
                "api-key-1",
                "openai:chat",
                "gpt-5",
                Some(&affinity),
                Some(&scope),
            )
            .expect("scoped scheduler affinity cache key should build");

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: Some(&report_context),
            },
            LocalExecutionEffect::HealthSuccess(LocalHealthSuccessEffect),
        )
        .await;

        assert_eq!(
            state.read_scheduler_affinity_target(scoped_cache_key.as_str(), SCHEDULER_AFFINITY_TTL),
            Some(SchedulerAffinityTarget {
                provider_id: "prov-1".to_string(),
                endpoint_id: "ep-1".to_string(),
                key_id: "key-1".to_string(),
            })
        );
        assert!(state
            .read_scheduler_affinity_target(
                session_scheduler_affinity_cache_key().as_str(),
                SCHEDULER_AFFINITY_TTL
            )
            .is_none());
    }
    #[tokio::test]
    async fn routing_profile_fixed_mode_overrides_legacy_cache_affinity_on_success() {
        let state = AppState::new().expect("gateway state should build");
        let plan = sample_plan();
        let report_context = json!({
            "api_key_id": "api-key-1",
            "client_api_format": "openai:chat",
            "model": "gpt-5",
            "scheduler_affinity_policy": {
                "scheduling_mode": "fixed_order",
                "scope": {
                    "routing_group_id": "routing-group-1",
                    "routing_group_version": 7
                }
            }
        });
        let scope = SchedulerAffinityScope::new("routing-group-1", Some(7));
        let scoped_cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id_with_client_session_and_scope(
                "api-key-1",
                "openai:chat",
                "gpt-5",
                None,
                Some(&scope),
            )
            .expect("scoped scheduler affinity cache key should build");

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: Some(&report_context),
            },
            LocalExecutionEffect::HealthSuccess(LocalHealthSuccessEffect),
        )
        .await;

        assert!(state
            .read_scheduler_affinity_target(scoped_cache_key.as_str(), SCHEDULER_AFFINITY_TTL)
            .is_none());
    }
    #[tokio::test]
    async fn malformed_routing_affinity_context_does_not_fall_back_to_legacy_mode() {
        let state = AppState::new().expect("gateway state should build");
        let plan = sample_plan();
        let report_context = json!({
            "api_key_id": "api-key-1",
            "client_api_format": "openai:chat",
            "model": "gpt-5",
            "scheduler_affinity_policy": {
                "scheduling_mode": "unknown"
            }
        });
        let legacy_cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id("api-key-1", "openai:chat", "gpt-5")
                .expect("legacy scheduler affinity cache key should build");

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: Some(&report_context),
            },
            LocalExecutionEffect::HealthSuccess(LocalHealthSuccessEffect),
        )
        .await;

        assert!(state
            .read_scheduler_affinity_target(legacy_cache_key.as_str(), SCHEDULER_AFFINITY_TTL)
            .is_none());
    }
    #[tokio::test]
    async fn health_success_keeps_scheduler_affinity_after_health_state_update() {
        let state = health_state();
        let plan = sample_plan();
        let report_context = json!({
            "api_key_id": "api-key-1",
            "client_api_format": "openai:chat",
            "model": "gpt-5",
        });
        let cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id("api-key-1", "openai:chat", "gpt-5")
                .expect("scheduler affinity cache key should build");

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: Some(&report_context),
            },
            LocalExecutionEffect::HealthSuccess(LocalHealthSuccessEffect),
        )
        .await;

        assert_eq!(
            state.read_scheduler_affinity_target(cache_key.as_str(), SCHEDULER_AFFINITY_TTL),
            Some(SchedulerAffinityTarget {
                provider_id: "prov-1".to_string(),
                endpoint_id: "ep-1".to_string(),
                key_id: "key-1".to_string(),
            })
        );
    }
    #[tokio::test]
    async fn load_balance_success_does_not_remember_scheduler_affinity_cache() {
        let state = AppState::new()
            .expect("gateway state should build")
            .with_data_state_for_tests(
                GatewayDataState::disabled().with_system_config_values_for_tests(vec![(
                    "scheduling_mode".to_string(),
                    json!("load_balance"),
                )]),
            );
        let plan = sample_plan();
        let report_context = json!({
            "api_key_id": "api-key-1",
            "client_api_format": "openai:chat",
            "model": "gpt-5",
        });
        let cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id("api-key-1", "openai:chat", "gpt-5")
                .expect("scheduler affinity cache key should build");

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: Some(&report_context),
            },
            LocalExecutionEffect::HealthSuccess(LocalHealthSuccessEffect),
        )
        .await;

        assert!(state
            .read_scheduler_affinity_target(cache_key.as_str(), SCHEDULER_AFFINITY_TTL)
            .is_none());
    }
    #[tokio::test]
    async fn success_remembers_session_scoped_scheduler_affinity_cache() {
        let state = AppState::new().expect("gateway state should build");
        let plan = sample_plan();
        let report_context = session_report_context();
        let session_cache_key = session_scheduler_affinity_cache_key();
        let legacy_cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id("api-key-1", "openai:chat", "gpt-5")
                .expect("legacy scheduler affinity cache key should build");

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: Some(&report_context),
            },
            LocalExecutionEffect::HealthSuccess(LocalHealthSuccessEffect),
        )
        .await;

        assert_eq!(
            state
                .read_scheduler_affinity_target(session_cache_key.as_str(), SCHEDULER_AFFINITY_TTL),
            Some(SchedulerAffinityTarget {
                provider_id: "prov-1".to_string(),
                endpoint_id: "ep-1".to_string(),
                key_id: "key-1".to_string(),
            })
        );
        assert!(state
            .read_scheduler_affinity_target(legacy_cache_key.as_str(), SCHEDULER_AFFINITY_TTL)
            .is_none());
    }
    #[tokio::test]
    async fn fallback_success_rewarms_scheduler_affinity_after_failed_candidate_invalidates() {
        let state = AppState::new().expect("gateway state should build");
        let failed_plan = sample_plan();
        let mut success_plan = sample_plan();
        success_plan.provider_id = "prov-2".to_string();
        success_plan.endpoint_id = "ep-2".to_string();
        success_plan.key_id = "key-2".to_string();
        let report_context = json!({
            "api_key_id": "api-key-1",
            "client_api_format": "openai:chat",
            "model": "gpt-5",
        });
        let cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id("api-key-1", "openai:chat", "gpt-5")
                .expect("scheduler affinity cache key should build");

        state.remember_scheduler_affinity_target(
            &cache_key,
            SchedulerAffinityTarget {
                provider_id: "prov-1".to_string(),
                endpoint_id: "ep-1".to_string(),
                key_id: "key-1".to_string(),
            },
            SCHEDULER_AFFINITY_TTL,
            16,
        );
        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &failed_plan,
                report_context: Some(&report_context),
            },
            LocalExecutionEffect::AttemptFailure(LocalAttemptFailureEffect {
                status_code: 429,
                classification: LocalFailoverClassification::RetryUpstreamFailure,
            }),
        )
        .await;
        assert!(state
            .read_scheduler_affinity_target(cache_key.as_str(), SCHEDULER_AFFINITY_TTL)
            .is_none());

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &success_plan,
                report_context: Some(&report_context),
            },
            LocalExecutionEffect::HealthSuccess(LocalHealthSuccessEffect),
        )
        .await;

        assert_eq!(
            state.read_scheduler_affinity_target(cache_key.as_str(), SCHEDULER_AFFINITY_TTL),
            Some(SchedulerAffinityTarget {
                provider_id: "prov-2".to_string(),
                endpoint_id: "ep-2".to_string(),
                key_id: "key-2".to_string(),
            })
        );
    }
    #[test]
    fn anthropic_non_credential_failures_do_not_apply_key_wide_effects() {
        assert!(!local_candidate_failure_should_apply_key_effects(
            "claude:messages",
            LocalFailoverClassification::RetryUpstreamFailure,
            529,
        ));
        assert!(!local_candidate_failure_should_apply_key_effects(
            "claude:messages",
            LocalFailoverClassification::RetryUpstreamFailure,
            429,
        ));
        assert!(!local_candidate_failure_should_apply_key_effects(
            "claude:messages",
            LocalFailoverClassification::RetryUpstreamFailure,
            503,
        ));
        assert!(local_candidate_failure_should_apply_key_effects(
            "claude:messages",
            LocalFailoverClassification::RetryUpstreamFailure,
            401,
        ));
        assert!(local_candidate_failure_should_apply_key_effects(
            "claude:messages",
            LocalFailoverClassification::RetryUpstreamFailure,
            403,
        ));
        assert!(!local_candidate_failure_should_apply_key_effects(
            "claude:messages",
            LocalFailoverClassification::RetryUpstreamFailure,
            400,
        ));
        assert!(local_candidate_failure_should_apply_key_effects(
            "openai:chat",
            LocalFailoverClassification::RetryUpstreamFailure,
            529,
        ));
        assert!(local_candidate_failure_should_apply_key_effects(
            "openai:chat",
            LocalFailoverClassification::RetryUpstreamFailure,
            429,
        ));
        assert!(local_candidate_failure_should_apply_key_effects(
            "openai:chat",
            LocalFailoverClassification::RetryUpstreamFailure,
            503,
        ));
    }
    #[tokio::test]
    async fn health_failure_projection_updates_key_health_for_format() {
        let state = health_state();
        let plan = sample_plan();

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: None,
            },
            LocalExecutionEffect::HealthFailure(LocalHealthFailureEffect {
                status_code: 503,
                classification: LocalFailoverClassification::RetryUpstreamFailure,
                retry_after_secs: None,
            }),
        )
        .await;

        let stored_key = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&plan.key_id))
            .await
            .expect("provider catalog keys should load")
            .into_iter()
            .next()
            .expect("stored key should exist");
        assert_eq!(
            stored_key.health_by_format,
            Some(json!({
                "openai:chat": {
                    "health_score": 0.6,
                    "consecutive_failures": 1,
                    "last_failure_at": stored_key
                        .health_by_format
                        .as_ref()
                        .and_then(|value| value.get("openai:chat"))
                        .and_then(|value| value.get("last_failure_at"))
                        .cloned()
                        .unwrap_or(Value::Null)
                }
            }))
        );
    }
    #[tokio::test]
    async fn runtime_health_failure_does_not_reactivate_admin_disabled_key() {
        let mut key = sample_health_key();
        key.is_active = false;
        let state = health_state_with_key(key);
        let plan = sample_plan();

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: None,
            },
            LocalExecutionEffect::HealthFailure(LocalHealthFailureEffect {
                status_code: 503,
                classification: LocalFailoverClassification::RetryUpstreamFailure,
                retry_after_secs: None,
            }),
        )
        .await;

        let stored_key = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&plan.key_id))
            .await
            .expect("provider catalog keys should load")
            .into_iter()
            .next()
            .expect("stored key should exist");
        assert!(!stored_key.is_active);
        assert_eq!(
            stored_key
                .health_by_format
                .as_ref()
                .and_then(|value| value.get("openai:chat"))
                .and_then(|value| value.get("consecutive_failures"))
                .and_then(Value::as_u64),
            Some(1)
        );
    }
    #[tokio::test]
    async fn health_failure_opens_circuit_after_eight_consecutive_failures() {
        let state = health_state();
        let plan = sample_plan();

        for _ in 0..8 {
            apply_local_execution_effect(
                &state,
                LocalExecutionEffectContext {
                    plan: &plan,
                    report_context: None,
                },
                LocalExecutionEffect::HealthFailure(LocalHealthFailureEffect {
                    status_code: 503,
                    classification: LocalFailoverClassification::RetryUpstreamFailure,
                    retry_after_secs: None,
                }),
            )
            .await;
        }

        let stored_key = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&plan.key_id))
            .await
            .expect("provider catalog keys should load")
            .into_iter()
            .next()
            .expect("stored key should exist");
        let circuit = stored_key
            .circuit_breaker_by_format
            .as_ref()
            .and_then(|value| value.get("openai:chat"))
            .expect("format circuit should be stored");
        assert_eq!(circuit["open"], json!(true));
        assert_eq!(circuit["reason"], json!("consecutive_failures_8"));
        assert_eq!(circuit["probe_interval_minutes"], json!(1));
        assert!(circuit["next_probe_at_unix_secs"].as_u64().is_some());
        assert_eq!(
            circuit["request_results_window"]
                .as_array()
                .map(Vec::len)
                .unwrap_or_default(),
            8
        );
    }
    #[tokio::test]
    async fn concurrent_health_failures_for_one_key_do_not_lose_updates() {
        let state = health_state();
        let plan = sample_plan();
        let mut tasks = tokio::task::JoinSet::new();

        for _ in 0..8 {
            let state = state.clone();
            let plan = plan.clone();
            tasks.spawn(async move {
                apply_local_execution_effect(
                    &state,
                    LocalExecutionEffectContext {
                        plan: &plan,
                        report_context: None,
                    },
                    LocalExecutionEffect::HealthFailure(LocalHealthFailureEffect {
                        status_code: 503,
                        classification: LocalFailoverClassification::RetryUpstreamFailure,
                        retry_after_secs: None,
                    }),
                )
                .await;
            });
        }
        while let Some(result) = tasks.join_next().await {
            result.expect("health failure task should complete");
        }

        let stored_key = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&plan.key_id))
            .await
            .expect("provider catalog keys should load")
            .into_iter()
            .next()
            .expect("stored key should exist");
        let circuit = stored_key
            .circuit_breaker_by_format
            .as_ref()
            .and_then(|value| value.get("openai:chat"))
            .expect("format circuit should be stored");
        assert_eq!(
            stored_key
                .health_by_format
                .as_ref()
                .and_then(|value| value.get("openai:chat"))
                .and_then(|value| value.get("consecutive_failures"))
                .and_then(Value::as_u64),
            Some(8)
        );
        assert_eq!(circuit["open"], json!(true));
        assert_eq!(circuit["reason"], json!("consecutive_failures_8"));
    }
    #[tokio::test]
    async fn health_success_projection_resets_key_health_for_format() {
        let state = health_state();
        let plan = sample_plan();

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: None,
            },
            LocalExecutionEffect::HealthFailure(LocalHealthFailureEffect {
                status_code: 503,
                classification: LocalFailoverClassification::RetryUpstreamFailure,
                retry_after_secs: None,
            }),
        )
        .await;
        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: None,
            },
            LocalExecutionEffect::HealthSuccess(LocalHealthSuccessEffect),
        )
        .await;

        let stored_key = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&plan.key_id))
            .await
            .expect("provider catalog keys should load")
            .into_iter()
            .next()
            .expect("stored key should exist");
        assert_eq!(
            stored_key.health_by_format,
            Some(json!({
                "openai:chat": {
                    "health_score": 1.0,
                    "consecutive_failures": 0,
                    "last_failure_at": Value::Null
                }
            }))
        );
    }
    #[tokio::test]
    async fn health_success_projection_is_rate_limited_until_failure_resets_gate() {
        let state = health_state();
        let plan = sample_plan();

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: None,
            },
            LocalExecutionEffect::HealthSuccess(LocalHealthSuccessEffect),
        )
        .await;
        let first_updated_at = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&plan.key_id))
            .await
            .expect("provider catalog keys should load")
            .into_iter()
            .next()
            .expect("stored key should exist")
            .updated_at_unix_secs;

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: None,
            },
            LocalExecutionEffect::HealthSuccess(LocalHealthSuccessEffect),
        )
        .await;
        let second_updated_at = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&plan.key_id))
            .await
            .expect("provider catalog keys should load")
            .into_iter()
            .next()
            .expect("stored key should exist")
            .updated_at_unix_secs;
        assert_eq!(second_updated_at, first_updated_at);

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: None,
            },
            LocalExecutionEffect::HealthFailure(LocalHealthFailureEffect {
                status_code: 503,
                classification: LocalFailoverClassification::RetryUpstreamFailure,
                retry_after_secs: None,
            }),
        )
        .await;
        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: None,
            },
            LocalExecutionEffect::HealthSuccess(LocalHealthSuccessEffect),
        )
        .await;

        let stored_key = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&plan.key_id))
            .await
            .expect("provider catalog keys should load")
            .into_iter()
            .next()
            .expect("stored key should exist");
        assert_eq!(
            stored_key
                .health_by_format
                .as_ref()
                .and_then(|value| value.get("openai:chat"))
                .and_then(|value| value.get("consecutive_failures"))
                .and_then(Value::as_u64),
            Some(0)
        );
    }
    #[tokio::test]
    async fn health_success_projection_closes_key_circuit_for_format() {
        let mut key = sample_health_key();
        key.circuit_breaker_by_format = Some(json!({
            "openai:chat": {
                "open": true,
                "reason": "account_deactivated_401",
                "next_probe_at_unix_secs": 1_760_001_920u64
            }
        }));
        let state = health_state_with_key(key);
        let plan = sample_plan();

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: None,
            },
            LocalExecutionEffect::HealthSuccess(LocalHealthSuccessEffect),
        )
        .await;

        let stored_key = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&plan.key_id))
            .await
            .expect("provider catalog keys should load")
            .into_iter()
            .next()
            .expect("stored key should exist");
        let circuit = stored_key
            .circuit_breaker_by_format
            .as_ref()
            .and_then(|value| value.get("openai:chat"))
            .expect("format circuit should be stored");
        assert_eq!(circuit["open"], json!(false));
        assert_eq!(circuit["reason"], Value::Null);
        assert_eq!(circuit["next_probe_at_unix_secs"], Value::Null);
    }
    #[tokio::test]
    async fn adaptive_rate_limit_effect_updates_adaptive_key_observation() {
        let state = adaptive_state();
        let plan = sample_plan();
        let cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id("api-key-1", "openai:chat", "gpt-5")
                .expect("scheduler affinity cache key should build");
        let target = SchedulerAffinityTarget {
            provider_id: plan.provider_id.clone(),
            endpoint_id: plan.endpoint_id.clone(),
            key_id: plan.key_id.clone(),
        };
        state.remember_scheduler_affinity_target(
            &cache_key,
            target.clone(),
            SCHEDULER_AFFINITY_TTL,
            16,
        );
        let initial_epoch = state.scheduler_affinity_epoch();

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: None,
            },
            LocalExecutionEffect::AdaptiveRateLimit(LocalAdaptiveRateLimitEffect {
                status_code: 429,
                classification: LocalFailoverClassification::RetryUpstreamFailure,
                headers: Some(&BTreeMap::from([(
                    "x-ratelimit-limit-requests".to_string(),
                    "42".to_string(),
                )])),
            }),
        )
        .await;

        let stored_key = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&plan.key_id))
            .await
            .expect("provider catalog keys should load")
            .into_iter()
            .next()
            .expect("stored key should exist");
        assert_eq!(stored_key.rpm_429_count, Some(2));
        assert_eq!(stored_key.last_429_type.as_deref(), Some("rpm"));
        assert!(stored_key.last_429_at_unix_secs.is_some());
        assert_eq!(
            stored_key
                .status_snapshot
                .as_ref()
                .and_then(|value| value.get("observation_count")),
            Some(&json!(1))
        );
        assert_eq!(
            stored_key
                .status_snapshot
                .as_ref()
                .and_then(|value| value.get("header_observation_count")),
            Some(&json!(1))
        );
        assert_eq!(
            stored_key
                .status_snapshot
                .as_ref()
                .and_then(|value| value.get("latest_upstream_limit")),
            Some(&json!(42))
        );
        assert_eq!(
            stored_key
                .status_snapshot
                .as_ref()
                .and_then(|value| value.get("learning_confidence")),
            Some(&json!(0.3))
        );
        assert_eq!(
            stored_key
                .status_snapshot
                .as_ref()
                .and_then(|value| value.get("enforcement_active")),
            Some(&json!(false))
        );
        assert_eq!(state.scheduler_affinity_epoch(), initial_epoch);
        assert_eq!(
            state.read_scheduler_affinity_target(cache_key.as_str(), SCHEDULER_AFFINITY_TTL),
            Some(target)
        );
    }
    #[tokio::test]
    async fn adaptive_rate_limit_effect_ignores_fixed_limit_key() {
        let state = fixed_limit_state();
        let plan = sample_plan();

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: None,
            },
            LocalExecutionEffect::AdaptiveRateLimit(LocalAdaptiveRateLimitEffect {
                status_code: 429,
                classification: LocalFailoverClassification::RetryUpstreamFailure,
                headers: Some(&BTreeMap::from([(
                    "x-ratelimit-limit-requests".to_string(),
                    "42".to_string(),
                )])),
            }),
        )
        .await;

        let stored_key = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&plan.key_id))
            .await
            .expect("provider catalog keys should load")
            .into_iter()
            .next()
            .expect("stored key should exist");
        assert_eq!(stored_key.rpm_429_count, None);
        assert_eq!(stored_key.last_429_at_unix_secs, None);
        assert_eq!(stored_key.last_429_type, None);
    }
    #[tokio::test]
    async fn adaptive_rate_limit_effect_records_429_as_rpm_observation() {
        let mut key = sample_health_key();
        key.rpm_limit = None;
        key.learned_rpm_limit = Some(20);
        let state = adaptive_state_with_request_candidates(key, Vec::new());
        let plan = sample_plan();

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: None,
            },
            LocalExecutionEffect::AdaptiveRateLimit(LocalAdaptiveRateLimitEffect {
                status_code: 429,
                classification: LocalFailoverClassification::RetryStatusCode,
                headers: None,
            }),
        )
        .await;

        let stored_key = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&plan.key_id))
            .await
            .expect("provider catalog keys should load")
            .into_iter()
            .next()
            .expect("stored key should exist");
        assert_eq!(stored_key.rpm_429_count, Some(1));
        assert_eq!(stored_key.learned_rpm_limit, Some(20));
        assert_eq!(stored_key.last_429_type.as_deref(), Some("rpm"));
    }
    #[tokio::test]
    async fn adaptive_success_effect_expands_limit_from_recent_rpm_usage() {
        let now_unix_secs = chrono::Utc::now().timestamp().max(0) as u64;
        let mut key = sample_adaptive_key();
        key.learned_rpm_limit = Some(20);
        key.last_rpm_peak = Some(25);
        key.last_429_at_unix_secs = Some(now_unix_secs.saturating_sub(600));
        key.adjustment_history = Some(json!([
            {
                "timestamp": "2026-04-19T00:00:00Z",
                "old_limit": 0,
                "new_limit": 20,
                "reason": "rpm_429",
                "confidence": 0.8
            }
        ]));
        key.utilization_samples = Some(json!([
            {"ts": now_unix_secs.saturating_sub(40), "util": 0.90},
            {"ts": now_unix_secs.saturating_sub(30), "util": 0.95},
            {"ts": now_unix_secs.saturating_sub(20), "util": 0.85},
            {"ts": now_unix_secs.saturating_sub(10), "util": 0.80}
        ]));
        let state = adaptive_state_with_request_candidates(
            key,
            vec![StoredRequestCandidate::new(
                "candidate-1".to_string(),
                "req-1".to_string(),
                None,
                None,
                None,
                None,
                0,
                0,
                Some("prov-1".to_string()),
                Some("ep-1".to_string()),
                Some("key-1".to_string()),
                RequestCandidateStatus::Success,
                None,
                false,
                Some(200),
                None,
                None,
                Some(10),
                Some(19),
                None,
                None,
                i64::try_from(now_unix_secs.saturating_sub(30) * 1000)
                    .expect("candidate created_at should fit i64"),
                Some(
                    i64::try_from(now_unix_secs.saturating_sub(30) * 1000)
                        .expect("candidate started_at should fit i64"),
                ),
                Some(
                    i64::try_from(now_unix_secs.saturating_sub(29) * 1000)
                        .expect("candidate finished_at should fit i64"),
                ),
            )
            .expect("request candidate should build")],
        );
        let plan = sample_plan();
        let cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id("api-key-1", "openai:chat", "gpt-5")
                .expect("scheduler affinity cache key should build");
        let target = SchedulerAffinityTarget {
            provider_id: plan.provider_id.clone(),
            endpoint_id: plan.endpoint_id.clone(),
            key_id: plan.key_id.clone(),
        };
        state.remember_scheduler_affinity_target(
            &cache_key,
            target.clone(),
            SCHEDULER_AFFINITY_TTL,
            16,
        );
        let initial_epoch = state.scheduler_affinity_epoch();

        apply_local_execution_effect(
            &state,
            LocalExecutionEffectContext {
                plan: &plan,
                report_context: None,
            },
            LocalExecutionEffect::AdaptiveSuccess(LocalAdaptiveSuccessEffect),
        )
        .await;

        let stored_key = state
            .read_provider_catalog_keys_by_ids(std::slice::from_ref(&plan.key_id))
            .await
            .expect("provider catalog keys should load")
            .into_iter()
            .next()
            .expect("stored key should exist");
        assert_eq!(stored_key.learned_rpm_limit, Some(25));
        assert_eq!(stored_key.utilization_samples, Some(json!([])));
        assert_eq!(
            stored_key
                .adjustment_history
                .as_ref()
                .and_then(Value::as_array)
                .and_then(|items| items.last())
                .and_then(Value::as_object)
                .and_then(|record| record.get("reason"))
                .and_then(Value::as_str),
            Some("high_utilization")
        );
        assert_eq!(state.scheduler_affinity_epoch(), initial_epoch);
        assert_eq!(
            state.read_scheduler_affinity_target(cache_key.as_str(), SCHEDULER_AFFINITY_TTL),
            Some(target)
        );
    }
}
