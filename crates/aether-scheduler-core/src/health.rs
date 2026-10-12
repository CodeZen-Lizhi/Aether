use aether_data_contracts::repository::candidates::{
    RequestCandidateStatus, StoredRequestCandidate,
};
use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey;

const ACTIVE_REQUEST_WINDOW_SECS: u64 = 300;
pub const PROVIDER_KEY_RPM_WINDOW_SECS: u64 = 60;
const HEALTH_DEGRADED_THRESHOLD: f64 = 0.8;
const HEALTH_LOW_THRESHOLD: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProviderKeyHealthBucket {
    Low,
    Degraded,
    Healthy,
}

impl ProviderKeyHealthBucket {
    fn from_score(score: f64) -> Self {
        let score = score.clamp(0.0, 1.0);
        if score < HEALTH_LOW_THRESHOLD {
            return Self::Low;
        }
        if score < HEALTH_DEGRADED_THRESHOLD {
            return Self::Degraded;
        }
        Self::Healthy
    }
}

pub fn count_recent_active_requests_for_provider(
    recent_candidates: &[StoredRequestCandidate],
    provider_id: &str,
    now_unix_secs: u64,
) -> usize {
    recent_candidates
        .iter()
        .filter(|candidate| candidate.provider_id.as_deref() == Some(provider_id))
        .filter(|candidate| is_recently_active(candidate, now_unix_secs))
        .count()
}

pub fn count_recent_active_requests_for_provider_key(
    recent_candidates: &[StoredRequestCandidate],
    key_id: &str,
    now_unix_secs: u64,
) -> usize {
    recent_candidates
        .iter()
        .filter(|candidate| candidate.key_id.as_deref() == Some(key_id))
        .filter(|candidate| is_recently_active(candidate, now_unix_secs))
        .count()
}

pub fn count_recent_active_requests_for_api_key(
    recent_candidates: &[StoredRequestCandidate],
    api_key_id: &str,
    now_unix_secs: u64,
) -> usize {
    recent_candidates
        .iter()
        .filter(|candidate| candidate.api_key_id.as_deref() == Some(api_key_id))
        .filter(|candidate| is_recently_active(candidate, now_unix_secs))
        .count()
}

/// 返回手动配置的 RPM 上限；未配置或非正值时返回 `None`。
pub fn effective_provider_key_rpm_limit(
    key: &StoredProviderCatalogKey,
    _now_unix_secs: u64,
) -> Option<usize> {
    key.rpm_limit
        .filter(|limit| *limit > 0)
        .and_then(|limit| usize::try_from(limit).ok())
}

pub fn count_recent_rpm_requests_for_provider_key(
    recent_candidates: &[StoredRequestCandidate],
    key_id: &str,
    now_unix_secs: u64,
) -> usize {
    count_recent_rpm_requests_for_provider_key_since(recent_candidates, key_id, now_unix_secs, None)
}

pub fn count_recent_rpm_requests_for_provider_key_since(
    recent_candidates: &[StoredRequestCandidate],
    key_id: &str,
    now_unix_secs: u64,
    reset_after_unix_secs: Option<u64>,
) -> usize {
    let mut attempted_count = 0usize;
    let mut max_observed = 0usize;

    for candidate in recent_candidates {
        if candidate.key_id.as_deref() != Some(key_id) {
            continue;
        }
        if !is_recent_rpm_observation(candidate, now_unix_secs) {
            continue;
        }
        let observed_at_unix_secs = candidate
            .started_at_unix_ms
            .map(|ms| ms / 1000)
            .unwrap_or(candidate.created_at_unix_ms / 1000);
        if reset_after_unix_secs.is_some_and(|reset_after| observed_at_unix_secs <= reset_after) {
            continue;
        }
        attempted_count += 1;
        max_observed = max_observed.max(candidate.concurrent_requests.unwrap_or_default() as usize);
    }

    max_observed.max(attempted_count)
}

/// 按手动 RPM 上限和最近请求数判断当前 Key 是否仍可接收请求。
pub fn provider_key_rpm_allows_request(
    key: &StoredProviderCatalogKey,
    recent_candidates: &[StoredRequestCandidate],
    now_unix_secs: u64,
) -> bool {
    provider_key_rpm_allows_request_since(key, recent_candidates, now_unix_secs, None)
}

/// 从指定重置时刻之后统计最近 RPM 窗口内的请求，并按手动上限判定准入。
pub fn provider_key_rpm_allows_request_since(
    key: &StoredProviderCatalogKey,
    recent_candidates: &[StoredRequestCandidate],
    now_unix_secs: u64,
    reset_after_unix_secs: Option<u64>,
) -> bool {
    let Some(effective_limit) = effective_provider_key_rpm_limit(key, now_unix_secs) else {
        return true;
    };
    if effective_limit == 0 {
        return false;
    }

    let current_usage = count_recent_rpm_requests_for_provider_key_since(
        recent_candidates,
        key.id.as_str(),
        now_unix_secs,
        reset_after_unix_secs,
    );
    current_usage < effective_limit
}

/// 用已聚合的窗口使用量按手动 RPM 上限判断当前 Key 是否可接收请求。
pub fn provider_key_rpm_usage_allows_request(
    key: &StoredProviderCatalogKey,
    current_usage: usize,
    now_unix_secs: u64,
) -> bool {
    effective_provider_key_rpm_limit(key, now_unix_secs)
        .is_none_or(|effective_limit| current_usage < effective_limit)
}

pub fn provider_key_health_score(key: &StoredProviderCatalogKey, api_format: &str) -> Option<f64> {
    let score = key
        .health_by_format
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .and_then(|values| values.get(api_format))
        .and_then(serde_json::Value::as_object)
        .and_then(|payload| payload.get("health_score"))
        .and_then(json_value_as_f64)?;
    Some(score.clamp(0.0, 1.0))
}

pub fn aggregate_provider_key_health_score(key: &StoredProviderCatalogKey) -> Option<f64> {
    let health_by_format = key.health_by_format.as_ref()?.as_object()?;
    let mut scores = Vec::new();
    for payload in health_by_format.values() {
        let Some(score) = payload
            .as_object()
            .and_then(|payload| payload.get("health_score"))
            .and_then(json_value_as_f64)
        else {
            continue;
        };
        scores.push(score.clamp(0.0, 1.0));
    }
    scores.into_iter().reduce(f64::min)
}

pub fn effective_provider_key_health_score(
    key: &StoredProviderCatalogKey,
    api_format: &str,
) -> Option<f64> {
    provider_key_health_score(key, api_format).or_else(|| aggregate_provider_key_health_score(key))
}

pub fn provider_key_health_bucket(
    key: &StoredProviderCatalogKey,
    api_format: &str,
) -> Option<ProviderKeyHealthBucket> {
    effective_provider_key_health_score(key, api_format).map(ProviderKeyHealthBucket::from_score)
}

pub fn is_provider_key_circuit_open(key: &StoredProviderCatalogKey, api_format: &str) -> bool {
    key.circuit_breaker_by_format
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .and_then(|values| values.get(api_format))
        .and_then(serde_json::Value::as_object)
        .and_then(|payload| payload.get("open"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

pub fn is_provider_key_circuit_open_at(
    key: &StoredProviderCatalogKey,
    api_format: &str,
    now_unix_secs: u64,
) -> bool {
    let Some(payload) = key
        .circuit_breaker_by_format
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .and_then(|values| values.get(api_format))
    else {
        return false;
    };
    provider_key_circuit_payload_is_active_open_at(payload, now_unix_secs)
}

pub fn any_provider_key_circuit_open_at(
    key: &StoredProviderCatalogKey,
    now_unix_secs: u64,
) -> bool {
    key.circuit_breaker_by_format
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .is_some_and(|values| {
            values.values().any(|payload| {
                provider_key_circuit_payload_is_active_open_at(payload, now_unix_secs)
            })
        })
}

pub fn provider_key_circuit_payload_is_active_open_at(
    payload: &serde_json::Value,
    now_unix_secs: u64,
) -> bool {
    let Some(payload) = payload.as_object() else {
        return false;
    };
    if !payload
        .get("open")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return false;
    }
    if let Some(next_probe_at) = payload
        .get("next_probe_at_unix_secs")
        .and_then(serde_json::Value::as_u64)
    {
        return now_unix_secs < next_probe_at;
    }
    if let Some(next_probe_at) = payload
        .get("next_probe_at")
        .and_then(serde_json::Value::as_str)
        .and_then(rfc3339_to_unix_secs)
    {
        return now_unix_secs < next_probe_at;
    }
    true
}

fn rfc3339_to_unix_secs(value: &str) -> Option<u64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .and_then(|value| u64::try_from(value.timestamp()).ok())
}

fn is_recently_active(candidate: &StoredRequestCandidate, now_unix_secs: u64) -> bool {
    if candidate.finished_at_unix_ms.is_some() {
        return false;
    }

    if !matches!(
        candidate.status,
        RequestCandidateStatus::Pending | RequestCandidateStatus::Streaming
    ) {
        return false;
    }

    let observed_at_unix_secs = candidate
        .started_at_unix_ms
        .map(|ms| ms / 1000)
        .unwrap_or(candidate.created_at_unix_ms / 1000);
    now_unix_secs.saturating_sub(observed_at_unix_secs) <= ACTIVE_REQUEST_WINDOW_SECS
}

fn is_recent_rpm_observation(candidate: &StoredRequestCandidate, now_unix_secs: u64) -> bool {
    if !candidate.status.is_attempted(candidate.started_at_unix_ms) {
        return false;
    }

    let observed_at_unix_secs = candidate
        .started_at_unix_ms
        .map(|ms| ms / 1000)
        .unwrap_or(candidate.created_at_unix_ms / 1000);
    now_unix_secs.saturating_sub(observed_at_unix_secs) <= PROVIDER_KEY_RPM_WINDOW_SECS
}

fn json_value_as_f64(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|raw| raw as f64))
        .or_else(|| value.as_u64().map(|raw| raw as f64))
}

/// Upstream failure taxonomy for scheduling state (P0-1).
///
/// The class decides which per-key state a failure feeds:
/// - `CredentialDead` — 401/403/quota-exhausted: the credential itself is unusable;
///   one failure trips the long circuit (fast lane) instead of 8 consecutive misses.
/// - `RateLimited` — 429: normal business throttling, not a fault. Feeds only the
///   rate-limit cooldown window (P0-2); never counts
///   toward the failure cooldown or circuit consecutive-failure counters.
/// - `Transient` — 5xx/transport: keeps the legacy 60s/8-failure cooldown and
///   8-consecutive-failure circuit accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamFailureClass {
    CredentialDead,
    RateLimited,
    Transient,
}

impl UpstreamFailureClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CredentialDead => "credential_dead",
            Self::RateLimited => "rate_limited",
            Self::Transient => "transient",
        }
    }

    /// Classify purely from the HTTP status code. Error-body keyword hints are
    /// intentionally NOT consulted here: the classifier layer owns policy text
    /// matching, and this function stays a cheap, total mapping usable from
    /// scheduler-core without the policy engine.
    pub fn from_status_code(status_code: u16) -> Self {
        match status_code {
            401..=403 => Self::CredentialDead,
            429 => Self::RateLimited,
            _ => Self::Transient,
        }
    }
}

/// Per-format rate-limit cooldown state stored inside `health_by_format` (P0-2).
///
/// A 429 sets `rate_limit_cooldown_until_unix_secs`; until then the key is
/// skipped by selection. `consecutive_rate_limits` drives the exponential
/// fallback (30s → 1m → 2m → 4m, capped at 10m) when the upstream omits
/// `Retry-After`. A successful outcome resets the ladder; unrelated failures
/// preserve it so they cannot erase an active backoff window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderKeyRateLimitCooldown {
    pub until_unix_secs: u64,
    pub consecutive_rate_limits: u64,
}

pub const RATE_LIMIT_COOLDOWN_BASE_SECS: u64 = 30;
pub const RATE_LIMIT_COOLDOWN_MAX_SECS: u64 = 600;
/// An expired rate-limit window admits at most one probe during this bounded
/// reservation window across Gateway instances. Terminal 429/success clears
/// the marker; TTL recovery handles cancellation or process loss before that.
pub const RATE_LIMIT_PROBE_RESERVATION_SECS: u64 = 60;

impl ProviderKeyRateLimitCooldown {
    /// Chat policy keeps provider deadlines intact; only the local fallback is capped.
    pub fn project_chat(
        current: Option<Self>,
        observed_at_unix_secs: u64,
        retry_after_secs: Option<u64>,
    ) -> Self {
        let previous = current.unwrap_or(Self {
            until_unix_secs: 0,
            consecutive_rate_limits: 0,
        });
        let consecutive = previous.consecutive_rate_limits.saturating_add(1);
        let shift = consecutive.saturating_sub(1).min(6) as u32;
        let seconds = retry_after_secs.unwrap_or_else(|| (1u64 << shift).min(60));
        Self {
            until_unix_secs: observed_at_unix_secs
                .saturating_add(seconds)
                .max(previous.until_unix_secs),
            consecutive_rate_limits: consecutive,
        }
    }

    /// Project the next cooldown after a 429 observed at `observed_at_unix_secs`.
    /// `retry_after_secs` (from the upstream header) wins when present and sane;
    /// otherwise the exponential ladder advances with the consecutive count.
    pub fn project(
        current: Option<Self>,
        observed_at_unix_secs: u64,
        retry_after_secs: Option<u64>,
    ) -> Self {
        let previous = current.unwrap_or(Self {
            until_unix_secs: 0,
            consecutive_rate_limits: 0,
        });
        let consecutive = previous.consecutive_rate_limits.saturating_add(1);
        let cooldown_secs = match retry_after_secs
            .filter(|value| *value > 0 && *value <= RATE_LIMIT_COOLDOWN_MAX_SECS)
        {
            Some(value) => value,
            None => {
                let shift = u32::try_from(consecutive.saturating_sub(1)).unwrap_or(u32::MAX);
                RATE_LIMIT_COOLDOWN_BASE_SECS
                    .saturating_mul(1u64.checked_shl(shift).unwrap_or(u64::MAX))
                    .min(RATE_LIMIT_COOLDOWN_MAX_SECS)
            }
        };
        Self {
            until_unix_secs: observed_at_unix_secs.saturating_add(cooldown_secs),
            consecutive_rate_limits: consecutive,
        }
    }

    pub fn from_health_payload(payload: &serde_json::Value) -> Option<Self> {
        let payload = payload.as_object()?;
        let until = payload
            .get("rate_limit_cooldown_until_unix_secs")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let consecutive = payload
            .get("consecutive_rate_limits")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if until == 0 {
            return None;
        }
        Some(Self {
            until_unix_secs: until,
            consecutive_rate_limits: consecutive,
        })
    }

    pub fn to_json(self) -> serde_json::Value {
        serde_json::json!({
            "rate_limit_cooldown_until_unix_secs": self.until_unix_secs,
            "consecutive_rate_limits": self.consecutive_rate_limits,
        })
    }
}

/// True while the key's per-format rate-limit cooldown is still active (P0-2).
pub fn provider_key_rate_limit_cooldown_active_at(
    key: &StoredProviderCatalogKey,
    api_format: &str,
    now_unix_secs: u64,
) -> bool {
    key.health_by_format
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .and_then(|values| values.get(api_format))
        .and_then(ProviderKeyRateLimitCooldown::from_health_payload)
        .is_some_and(|cooldown| now_unix_secs < cooldown.until_unix_secs)
}

/// Read the current per-format rate-limit cooldown snapshot (P0-2).
pub fn provider_key_rate_limit_cooldown(
    key: &StoredProviderCatalogKey,
    api_format: &str,
) -> Option<ProviderKeyRateLimitCooldown> {
    key.health_by_format
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .and_then(|values| values.get(api_format))
        .and_then(ProviderKeyRateLimitCooldown::from_health_payload)
}

/// Parse a rate-limit cooldown straight out of a `health_by_format` entry
/// payload (P0-2). Shared with the orchestration projection layer so the
/// read and write sides agree on the field names.
pub fn provider_key_rate_limit_cooldown_payload(
    payload: &serde_json::Value,
) -> Option<ProviderKeyRateLimitCooldown> {
    ProviderKeyRateLimitCooldown::from_health_payload(payload)
}

/// Per-format reservation used to serialize the first request after a
/// rate-limit cooldown expires. This is deliberately independent from health
/// score and the ordinary key circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderKeyRateLimitProbe {
    pub until_unix_secs: u64,
}

impl ProviderKeyRateLimitProbe {
    pub fn from_health_payload(payload: &serde_json::Value) -> Option<Self> {
        let until_unix_secs = payload
            .as_object()?
            .get("rate_limit_probe_until_unix_secs")
            .and_then(serde_json::Value::as_u64)?;
        Some(Self { until_unix_secs })
    }
}

/// True while another Gateway owns the first post-cooldown rate-limit probe.
pub fn provider_key_rate_limit_probe_active_at(
    key: &StoredProviderCatalogKey,
    api_format: &str,
    now_unix_secs: u64,
) -> bool {
    key.health_by_format
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .and_then(|values| values.get(api_format))
        .and_then(ProviderKeyRateLimitProbe::from_health_payload)
        .is_some_and(|probe| now_unix_secs < probe.until_unix_secs)
}

/// Parse a probe reservation directly from a `health_by_format` entry.
pub fn provider_key_rate_limit_probe_payload(
    payload: &serde_json::Value,
) -> Option<ProviderKeyRateLimitProbe> {
    ProviderKeyRateLimitProbe::from_health_payload(payload)
}

#[cfg(test)]
mod tests {
    use aether_data_contracts::repository::candidates::{
        RequestCandidateStatus, StoredRequestCandidate,
    };
    use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey;

    use super::{
        aggregate_provider_key_health_score, count_recent_active_requests_for_api_key,
        count_recent_active_requests_for_provider, count_recent_active_requests_for_provider_key,
        count_recent_rpm_requests_for_provider_key,
        count_recent_rpm_requests_for_provider_key_since, effective_provider_key_health_score,
        effective_provider_key_rpm_limit, is_provider_key_circuit_open,
        is_provider_key_circuit_open_at, provider_key_health_bucket, provider_key_health_score,
        provider_key_rpm_allows_request, provider_key_rpm_allows_request_since,
        ProviderKeyHealthBucket,
    };

    fn stored_candidate(
        id: &str,
        status: RequestCandidateStatus,
        created_at_unix_secs: i64,
    ) -> StoredRequestCandidate {
        let created_at_unix_ms = created_at_unix_secs * 1000;
        StoredRequestCandidate::new(
            id.to_string(),
            format!("req-{id}"),
            None,
            None,
            None,
            None,
            0,
            0,
            Some("provider-a".to_string()),
            Some("endpoint-a".to_string()),
            Some("key-a".to_string()),
            status,
            None,
            false,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            created_at_unix_ms,
            Some(created_at_unix_ms),
            Some(created_at_unix_ms),
        )
        .expect("candidate should build")
    }

    fn provider_catalog_key(id: &str) -> StoredProviderCatalogKey {
        StoredProviderCatalogKey::new(
            id.to_string(),
            "provider-a".to_string(),
            "primary".to_string(),
            "api_key".to_string(),
            None,
            true,
        )
        .expect("provider key should build")
    }

    #[test]
    fn counts_only_recently_active_provider_requests() {
        let recent_candidates = vec![
            StoredRequestCandidate::new(
                "one".to_string(),
                "req-one".to_string(),
                None,
                Some("api-key-1".to_string()),
                None,
                None,
                0,
                0,
                Some("provider-a".to_string()),
                Some("endpoint-a".to_string()),
                Some("key-a".to_string()),
                RequestCandidateStatus::Pending,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                95,
                Some(95),
                None,
            )
            .expect("candidate should build"),
            StoredRequestCandidate::new(
                "two".to_string(),
                "req-two".to_string(),
                None,
                Some("api-key-1".to_string()),
                None,
                None,
                0,
                0,
                Some("provider-a".to_string()),
                Some("endpoint-a".to_string()),
                Some("key-a".to_string()),
                RequestCandidateStatus::Streaming,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                96,
                Some(96),
                None,
            )
            .expect("candidate should build"),
            StoredRequestCandidate::new(
                "three".to_string(),
                "req-three".to_string(),
                None,
                Some("api-key-1".to_string()),
                None,
                None,
                0,
                0,
                Some("provider-a".to_string()),
                Some("endpoint-a".to_string()),
                Some("key-a".to_string()),
                RequestCandidateStatus::Success,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                97,
                Some(97),
                Some(98),
            )
            .expect("candidate should build"),
        ];

        assert_eq!(
            count_recent_active_requests_for_provider(&recent_candidates, "provider-a", 100),
            2
        );
        assert_eq!(
            count_recent_active_requests_for_api_key(&recent_candidates, "api-key-1", 100),
            2
        );
    }

    #[test]
    fn provider_key_concurrency_counts_only_recent_active_requests() {
        let recent_candidates = vec![
            StoredRequestCandidate::new(
                "pending".to_string(),
                "req-pending".to_string(),
                None,
                Some("api-key-1".to_string()),
                None,
                None,
                0,
                0,
                Some("provider-a".to_string()),
                Some("endpoint-a".to_string()),
                Some("key-a".to_string()),
                RequestCandidateStatus::Pending,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                900_000,
                Some(900_000),
                None,
            )
            .expect("candidate should build"),
            StoredRequestCandidate::new(
                "streaming".to_string(),
                "req-streaming".to_string(),
                None,
                Some("api-key-1".to_string()),
                None,
                None,
                0,
                0,
                Some("provider-a".to_string()),
                Some("endpoint-a".to_string()),
                Some("key-a".to_string()),
                RequestCandidateStatus::Streaming,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                950_000,
                Some(950_000),
                None,
            )
            .expect("candidate should build"),
            StoredRequestCandidate::new(
                "finished".to_string(),
                "req-finished".to_string(),
                None,
                Some("api-key-1".to_string()),
                None,
                None,
                0,
                0,
                Some("provider-a".to_string()),
                Some("endpoint-a".to_string()),
                Some("key-a".to_string()),
                RequestCandidateStatus::Success,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                975_000,
                Some(975_000),
                Some(976_000),
            )
            .expect("candidate should build"),
            StoredRequestCandidate::new(
                "failed".to_string(),
                "req-failed".to_string(),
                None,
                Some("api-key-1".to_string()),
                None,
                None,
                0,
                0,
                Some("provider-a".to_string()),
                Some("endpoint-a".to_string()),
                Some("key-a".to_string()),
                RequestCandidateStatus::Failed,
                None,
                false,
                Some(429),
                None,
                Some("upstream failure".to_string()),
                Some(20),
                None,
                None,
                None,
                980_000,
                Some(980_000),
                Some(981_000),
            )
            .expect("candidate should build"),
            StoredRequestCandidate::new(
                "cancelled".to_string(),
                "req-cancelled".to_string(),
                None,
                Some("api-key-1".to_string()),
                None,
                None,
                0,
                0,
                Some("provider-a".to_string()),
                Some("endpoint-a".to_string()),
                Some("key-a".to_string()),
                RequestCandidateStatus::Cancelled,
                None,
                false,
                Some(499),
                None,
                Some("client cancelled".to_string()),
                Some(10),
                None,
                None,
                None,
                982_000,
                Some(982_000),
                Some(983_000),
            )
            .expect("candidate should build"),
            StoredRequestCandidate::new(
                "stale".to_string(),
                "req-stale".to_string(),
                None,
                Some("api-key-1".to_string()),
                None,
                None,
                0,
                0,
                Some("provider-a".to_string()),
                Some("endpoint-a".to_string()),
                Some("key-a".to_string()),
                RequestCandidateStatus::Pending,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                699_000,
                Some(699_000),
                None,
            )
            .expect("candidate should build"),
            StoredRequestCandidate::new(
                "other-key".to_string(),
                "req-other-key".to_string(),
                None,
                Some("api-key-1".to_string()),
                None,
                None,
                0,
                0,
                Some("provider-a".to_string()),
                Some("endpoint-a".to_string()),
                Some("key-b".to_string()),
                RequestCandidateStatus::Pending,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                990_000,
                Some(990_000),
                None,
            )
            .expect("candidate should build"),
        ];

        assert_eq!(
            count_recent_active_requests_for_provider_key(&recent_candidates, "key-a", 1_000),
            2
        );
    }

    #[test]
    fn fixed_provider_key_rpm_limit_takes_precedence() {
        let key = provider_catalog_key("key-a").with_rate_limits(Some(120), None);

        assert_eq!(effective_provider_key_rpm_limit(&key, 100), Some(120));
    }

    #[test]
    fn unset_provider_key_rpm_limit_is_unlimited_despite_usage_counts() {
        let key = provider_catalog_key("key-a")
            .with_rate_limits(None, None)
            .with_usage_counts(Some(10_000), Some(9_000));

        assert_eq!(effective_provider_key_rpm_limit(&key, 100), None);
        assert!(provider_key_rpm_allows_request(&key, &[], 100,));
    }

    #[test]
    fn counts_recent_provider_key_rpm_from_snapshot_or_recent_attempts() {
        let recent_candidates = vec![
            StoredRequestCandidate::new(
                "one".to_string(),
                "req-one".to_string(),
                None,
                None,
                None,
                None,
                0,
                0,
                Some("provider-a".to_string()),
                Some("endpoint-a".to_string()),
                Some("key-a".to_string()),
                RequestCandidateStatus::Success,
                None,
                false,
                Some(200),
                None,
                None,
                Some(10),
                Some(7),
                None,
                None,
                95_000,
                Some(95_000),
                Some(96_000),
            )
            .expect("candidate should build"),
            StoredRequestCandidate::new(
                "two".to_string(),
                "req-two".to_string(),
                None,
                None,
                None,
                None,
                0,
                0,
                Some("provider-a".to_string()),
                Some("endpoint-a".to_string()),
                Some("key-a".to_string()),
                RequestCandidateStatus::Failed,
                None,
                false,
                Some(502),
                None,
                None,
                Some(10),
                None,
                None,
                None,
                98_000,
                Some(98_000),
                Some(99_000),
            )
            .expect("candidate should build"),
        ];

        assert_eq!(
            count_recent_rpm_requests_for_provider_key(&recent_candidates, "key-a", 100),
            7
        );
    }

    #[test]
    fn ignores_rpm_observations_before_reset_watermark() {
        let recent_candidates = vec![
            StoredRequestCandidate::new(
                "one".to_string(),
                "req-one".to_string(),
                None,
                None,
                None,
                None,
                0,
                0,
                Some("provider-a".to_string()),
                Some("endpoint-a".to_string()),
                Some("key-a".to_string()),
                RequestCandidateStatus::Success,
                None,
                false,
                Some(200),
                None,
                None,
                Some(10),
                Some(7),
                None,
                None,
                95_000,
                Some(95_000),
                Some(96_000),
            )
            .expect("candidate should build"),
            StoredRequestCandidate::new(
                "two".to_string(),
                "req-two".to_string(),
                None,
                None,
                None,
                None,
                0,
                0,
                Some("provider-a".to_string()),
                Some("endpoint-a".to_string()),
                Some("key-a".to_string()),
                RequestCandidateStatus::Success,
                None,
                false,
                Some(200),
                None,
                None,
                Some(10),
                Some(2),
                None,
                None,
                99_000,
                Some(99_000),
                Some(100_000),
            )
            .expect("candidate should build"),
        ];

        assert_eq!(
            count_recent_rpm_requests_for_provider_key_since(
                &recent_candidates,
                "key-a",
                100,
                Some(98),
            ),
            2
        );
    }

    #[test]
    fn manual_provider_key_rpm_limit_sets_the_exact_admission_capacity() {
        let key = provider_catalog_key("key-a")
            .with_rate_limits(Some(2), None)
            .with_usage_counts(Some(10_000), Some(9_000));
        let recent_candidates = vec![StoredRequestCandidate::new(
            "one".to_string(),
            "req-one".to_string(),
            None,
            None,
            None,
            None,
            0,
            0,
            Some("provider-a".to_string()),
            Some("endpoint-a".to_string()),
            Some("key-a".to_string()),
            RequestCandidateStatus::Success,
            None,
            false,
            Some(200),
            None,
            None,
            Some(10),
            None,
            None,
            None,
            95_000,
            Some(95_000),
            Some(96_000),
        )
        .expect("candidate should build")];

        assert!(provider_key_rpm_allows_request(
            &key,
            &recent_candidates,
            100
        ));
        assert!(!provider_key_rpm_allows_request(
            &key.clone().with_rate_limits(Some(1), None),
            &recent_candidates,
            100,
        ));
        assert!(provider_key_rpm_allows_request_since(
            &key,
            &recent_candidates,
            100,
            Some(97),
        ));
    }

    #[test]
    fn reads_provider_key_health_and_circuit_status_for_api_format() {
        let key = provider_catalog_key("key-a").with_health_fields(
            Some(serde_json::json!({
                "openai:chat": {"health_score": 0.25},
                "openai:responses": {"health_score": 0.75}
            })),
            Some(serde_json::json!({
                "openai:chat": {"open": true},
                "openai:responses": {"open": false}
            })),
        );

        assert_eq!(provider_key_health_score(&key, "openai:chat"), Some(0.25));
        assert_eq!(
            provider_key_health_score(&key, "openai:responses"),
            Some(0.75)
        );
        assert!(is_provider_key_circuit_open(&key, "openai:chat"));
        assert!(!is_provider_key_circuit_open(&key, "openai:responses"));
    }

    #[test]
    fn provider_key_circuit_open_at_allows_probe_after_rfc3339_deadline() {
        let key = provider_catalog_key("key-a").with_health_fields(
            None,
            Some(serde_json::json!({
                "openai:chat": {
                    "open": true,
                    "next_probe_at": "2026-05-24T14:45:27Z"
                }
            })),
        );

        assert!(is_provider_key_circuit_open_at(
            &key,
            "openai:chat",
            1_779_633_926
        ));
        assert!(!is_provider_key_circuit_open_at(
            &key,
            "openai:chat",
            1_779_633_927
        ));
    }

    #[test]
    fn aggregates_provider_key_health_score_with_lower_bound_strategy() {
        let key = provider_catalog_key("key-a").with_health_fields(
            Some(serde_json::json!({
                "openai:chat": {"health_score": 0.85},
                "openai:responses": {"health_score": 0.45},
                "claude:messages": {"health_score": 0.70}
            })),
            None,
        );

        assert_eq!(aggregate_provider_key_health_score(&key), Some(0.45));
        assert_eq!(
            effective_provider_key_health_score(&key, "gemini:generate_content"),
            Some(0.45)
        );
    }

    #[test]
    fn classifies_provider_key_health_bucket_from_effective_score() {
        let low = provider_catalog_key("key-low").with_health_fields(
            Some(serde_json::json!({"openai:chat": {"health_score": 0.30}})),
            None,
        );
        let degraded = provider_catalog_key("key-degraded").with_health_fields(
            Some(serde_json::json!({"openai:chat": {"health_score": 0.65}})),
            None,
        );
        let healthy = provider_catalog_key("key-healthy").with_health_fields(
            Some(serde_json::json!({"openai:chat": {"health_score": 0.92}})),
            None,
        );

        assert_eq!(
            provider_key_health_bucket(&low, "openai:chat"),
            Some(ProviderKeyHealthBucket::Low)
        );
        assert_eq!(
            provider_key_health_bucket(&degraded, "openai:chat"),
            Some(ProviderKeyHealthBucket::Degraded)
        );
        assert_eq!(
            provider_key_health_bucket(&healthy, "openai:chat"),
            Some(ProviderKeyHealthBucket::Healthy)
        );
    }
}
#[cfg(test)]
mod rate_limit_cooldown_tests {
    use serde_json::json;

    use super::{
        provider_key_rate_limit_cooldown_active_at, provider_key_rate_limit_cooldown_payload,
        provider_key_rate_limit_probe_active_at, provider_key_rate_limit_probe_payload,
        ProviderKeyRateLimitCooldown, ProviderKeyRateLimitProbe, UpstreamFailureClass,
        RATE_LIMIT_COOLDOWN_MAX_SECS,
    };
    use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey;

    #[test]
    fn chat_cooldown_starts_at_one_second_and_keeps_provider_deadlines() {
        let mut previous = None;
        for expected in [1, 2, 4, 8, 16, 32, 60, 60] {
            let cooldown = ProviderKeyRateLimitCooldown::project_chat(previous, 100, None);
            assert_eq!(cooldown.until_unix_secs, 100 + expected);
            previous = Some(cooldown);
        }
        let long = ProviderKeyRateLimitCooldown::project_chat(None, 100, Some(3600));
        assert_eq!(long.until_unix_secs, 3700);
        assert_eq!(
            ProviderKeyRateLimitCooldown::project_chat(Some(long), 101, None).until_unix_secs,
            3700
        );
        assert_eq!(
            ProviderKeyRateLimitCooldown::project_chat(None, 100, Some(0)).until_unix_secs,
            100
        );
    }

    #[test]
    fn failure_class_maps_status_codes() {
        assert_eq!(
            UpstreamFailureClass::from_status_code(401),
            UpstreamFailureClass::CredentialDead
        );
        assert_eq!(
            UpstreamFailureClass::from_status_code(402),
            UpstreamFailureClass::CredentialDead
        );
        assert_eq!(
            UpstreamFailureClass::from_status_code(403),
            UpstreamFailureClass::CredentialDead
        );
        assert_eq!(
            UpstreamFailureClass::from_status_code(429),
            UpstreamFailureClass::RateLimited
        );
        assert_eq!(
            UpstreamFailureClass::from_status_code(500),
            UpstreamFailureClass::Transient
        );
        assert_eq!(
            UpstreamFailureClass::from_status_code(503),
            UpstreamFailureClass::Transient
        );
        assert_eq!(
            UpstreamFailureClass::from_status_code(400),
            UpstreamFailureClass::Transient
        );
        assert_eq!(
            UpstreamFailureClass::as_str(UpstreamFailureClass::CredentialDead),
            "credential_dead"
        );
        assert_eq!(
            UpstreamFailureClass::as_str(UpstreamFailureClass::RateLimited),
            "rate_limited"
        );
        assert_eq!(
            UpstreamFailureClass::as_str(UpstreamFailureClass::Transient),
            "transient"
        );
    }

    #[test]
    fn cooldown_projection_uses_retry_after_when_present() {
        let projected = ProviderKeyRateLimitCooldown::project(None, 1_000, Some(30));
        assert_eq!(projected.until_unix_secs, 1_030);
        assert_eq!(projected.consecutive_rate_limits, 1);
    }

    #[test]
    fn cooldown_projection_rejects_absurd_retry_after() {
        // Over-cap and zero Retry-After fall back to the exponential ladder.
        assert_eq!(
            ProviderKeyRateLimitCooldown::project(None, 1_000, Some(0)).until_unix_secs,
            1_030
        );
        assert_eq!(
            ProviderKeyRateLimitCooldown::project(None, 1_000, Some(9_999)).until_unix_secs,
            1_030
        );
    }

    #[test]
    fn cooldown_projection_ladder_doubles_and_caps() {
        let now = 1_000u64;
        let first = ProviderKeyRateLimitCooldown::project(None, now, None);
        assert_eq!(first.until_unix_secs, now + 30);
        assert_eq!(first.consecutive_rate_limits, 1);

        let second = ProviderKeyRateLimitCooldown::project(Some(first), now, None);
        assert_eq!(second.until_unix_secs, now + 60);
        assert_eq!(second.consecutive_rate_limits, 2);

        let third = ProviderKeyRateLimitCooldown::project(Some(second), now, None);
        assert_eq!(third.until_unix_secs, now + 120);

        // Ladder saturates at the cap regardless of consecutive count.
        let many = ProviderKeyRateLimitCooldown {
            until_unix_secs: now,
            consecutive_rate_limits: 20,
        };
        let capped = ProviderKeyRateLimitCooldown::project(Some(many), now, None);
        assert_eq!(capped.until_unix_secs, now + RATE_LIMIT_COOLDOWN_MAX_SECS);
    }

    fn key_with_cooldown(format: &str, until: u64) -> StoredProviderCatalogKey {
        let mut key = StoredProviderCatalogKey::new(
            "key-a".to_string(),
            "provider-a".to_string(),
            "primary".to_string(),
            "api_key".to_string(),
            None,
            true,
        )
        .expect("provider key should build");
        key.health_by_format = Some(json!({
            format: {
                "rate_limit_cooldown_until_unix_secs": until,
                "consecutive_rate_limits": 2,
            }
        }));
        key
    }

    #[test]
    fn cooldown_active_before_deadline_and_inactive_after() {
        let key = key_with_cooldown("openai:chat", 2_000);
        assert!(provider_key_rate_limit_cooldown_active_at(
            &key,
            "openai:chat",
            1_999
        ));
        assert!(!provider_key_rate_limit_cooldown_active_at(
            &key,
            "openai:chat",
            2_000
        ));
        // Different format is untouched.
        assert!(!provider_key_rate_limit_cooldown_active_at(
            &key,
            "claude:messages",
            1_999
        ));
    }

    #[test]
    fn cooldown_payload_roundtrip() {
        let cooldown = ProviderKeyRateLimitCooldown {
            until_unix_secs: 1_234,
            consecutive_rate_limits: 3,
        };
        let payload = cooldown.to_json();
        let parsed = provider_key_rate_limit_cooldown_payload(&payload);
        assert_eq!(parsed, Some(cooldown));
        // Missing fields parse to None.
        assert_eq!(
            provider_key_rate_limit_cooldown_payload(&json!({"consecutive_rate_limits": 3})),
            None
        );
    }

    #[test]
    fn rate_limit_probe_is_format_scoped_and_expires() {
        let mut key = key_with_cooldown("openai:chat", 1_000);
        key.health_by_format
            .as_mut()
            .and_then(serde_json::Value::as_object_mut)
            .and_then(|formats| formats.get_mut("openai:chat"))
            .and_then(serde_json::Value::as_object_mut)
            .expect("test format payload should be present")
            .insert("rate_limit_probe_until_unix_secs".to_string(), json!(1_060));

        assert!(provider_key_rate_limit_probe_active_at(
            &key,
            "openai:chat",
            1_059
        ));
        assert!(!provider_key_rate_limit_probe_active_at(
            &key,
            "openai:chat",
            1_060
        ));
        assert!(!provider_key_rate_limit_probe_active_at(
            &key,
            "claude:messages",
            1_059
        ));

        let probe = ProviderKeyRateLimitProbe {
            until_unix_secs: 1_060,
        };
        assert_eq!(
            provider_key_rate_limit_probe_payload(&json!({
                "rate_limit_probe_until_unix_secs": 1_060
            })),
            Some(probe)
        );
    }
}
