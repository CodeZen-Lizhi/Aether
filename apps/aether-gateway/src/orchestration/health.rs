use serde_json::{json, Value};

use super::LocalFailoverClassification;
use crate::handlers::shared::unix_secs_to_rfc3339;

/// P0-2: parse an upstream `Retry-After` header into cooldown seconds.
/// Accepts both the delta-seconds form ("30") and the HTTP-date form
/// ("Wed, 21 Oct 2026 07:28:00 GMT"). Unparseable or absurd values (>10 min)
/// return `None` so the exponential fallback ladder applies instead; the
/// system clock is never trusted for dates in the past.
pub(crate) fn parse_retry_after_secs(value: Option<&str>, now_unix_secs: u64) -> Option<u64> {
    let value = value.map(str::trim).filter(|value| !value.is_empty())?;

    if let Ok(delta_secs) = value.parse::<u64>() {
        return (delta_secs > 0
            && delta_secs <= crate::orchestration::RATE_LIMIT_COOLDOWN_MAX_SECS)
            .then_some(delta_secs);
    }

    let http_date = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    let target_unix_secs = u64::try_from(http_date.timestamp()).ok()?;
    if target_unix_secs <= now_unix_secs {
        // Date already in the past: treat as an immediate retry, which the
        // ladder handles better than a zero-second cooldown.
        return None;
    }
    let delta_secs = target_unix_secs - now_unix_secs;
    (delta_secs <= crate::orchestration::RATE_LIMIT_COOLDOWN_MAX_SECS).then_some(delta_secs)
}

const LOCAL_HEALTH_SCORE_FLOOR: f64 = 0.2;
pub(crate) const LOCAL_KEY_CIRCUIT_FAILURE_THRESHOLD: u64 = 8;
pub(crate) const LOCAL_KEY_CIRCUIT_PROBE_RESERVATION_SECS: u64 = 60;
const LOCAL_KEY_CIRCUIT_MAX_PROBE_INTERVAL_MINUTES: u64 = 32;
// P1-6: rolling success-rate circuit trigger. 5-minute window, minimum sample
// count, and the success-rate floor below which a flapping key trips the
// circuit even without 8 consecutive failures.
pub(crate) const SUCCESS_RATE_WINDOW_SECS: u64 = 300;
pub(crate) const SUCCESS_RATE_WINDOW_MIN_SAMPLES: usize = 10;
pub(crate) const SUCCESS_RATE_WINDOW_THRESHOLD: f64 = 0.2;
// P1-7: recovery ramp. A circuit closes at half health and needs this many
// consecutive successes to return to full 1.0; a failure during the ramp
// re-opens the circuit immediately.
pub(crate) const CIRCUIT_RAMP_INITIAL_HEALTH: f64 = 0.75;
pub(crate) const CIRCUIT_RAMP_REQUIRED_SUCCESSES: u64 = 3;

/// P1-6: evaluate the rolling request-result window (kept on the circuit
/// payload as `request_results_window`, 30s-bucketed) against the success-rate
/// circuit trigger. Returns true when the window has enough samples inside
/// the 5-minute horizon and the success ratio is below the threshold.
pub(crate) fn circuit_success_rate_breached(
    current_circuit_by_format: Option<&Value>,
    api_format: &str,
    now_unix_secs: u64,
) -> bool {
    let Some(window) = current_circuit_by_format
        .and_then(Value::as_object)
        .and_then(|values| values.get(api_format))
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("request_results_window"))
        .and_then(Value::as_array)
    else {
        return false;
    };

    let cutoff = now_unix_secs.saturating_sub(SUCCESS_RATE_WINDOW_SECS);
    let mut samples = 0usize;
    let mut successes = 0usize;
    for entry in window {
        let Some(entry) = entry.as_object() else {
            continue;
        };
        let observed_at = entry.get("ts").and_then(Value::as_u64).unwrap_or(u64::MAX);
        if observed_at < cutoff {
            continue;
        }
        samples += 1;
        if entry.get("ok").and_then(Value::as_bool).unwrap_or(false) {
            successes += 1;
        }
    }

    samples >= SUCCESS_RATE_WINDOW_MIN_SAMPLES
        && (successes as f64 / samples as f64) < SUCCESS_RATE_WINDOW_THRESHOLD
}

pub(crate) fn project_local_failure_health(
    current_health_by_format: Option<&Value>,
    api_format: &str,
    classification: LocalFailoverClassification,
    status_code: u16,
    observed_at_unix_secs: u64,
) -> Option<Value> {
    if !local_candidate_failure_should_project_health(classification, status_code) {
        return None;
    }

    let api_format = api_format.trim();
    if api_format.is_empty() {
        return None;
    }

    // 429 is rate limiting, not a fault: it never counts toward failure health
    // or the circuit's consecutive-failure ladder (P0-1). Its state lives in
    // project_local_rate_limit_cooldown instead.
    if status_code == 429 {
        return None;
    }

    let mut health_by_format = current_health_by_format
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let current = health_by_format
        .get(api_format)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let previous_failures = current
        .get("consecutive_failures")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0) as u64;
    let consecutive_failures = previous_failures.saturating_add(1);

    let mut projected = serde_json::Map::new();
    projected.insert(
        "health_score".to_string(),
        json!(projected_failure_health_score(
            classification,
            status_code,
            consecutive_failures
        )),
    );
    projected.insert(
        "consecutive_failures".to_string(),
        json!(consecutive_failures),
    );
    projected.insert(
        "last_failure_at".to_string(),
        json!(unix_secs_to_rfc3339(observed_at_unix_secs)),
    );
    // Preserve the rate-limit cooldown fields: a 5xx between two 429s must not
    // reset the rate-limit backoff ladder or clear an active cooldown window.
    for field in [
        "rate_limit_cooldown_until_unix_secs",
        "consecutive_rate_limits",
        "rate_limit_probe_until_unix_secs",
    ] {
        if let Some(value) = current.get(field) {
            projected.insert(field.to_string(), value.clone());
        }
    }
    health_by_format.insert(api_format.to_string(), Value::Object(projected));

    Some(Value::Object(health_by_format))
}

pub(crate) fn project_local_success_health(
    current_health_by_format: Option<&Value>,
    api_format: &str,
) -> Option<Value> {
    let api_format = api_format.trim();
    if api_format.is_empty() {
        return None;
    }

    let mut health_by_format = current_health_by_format
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    health_by_format.insert(
        api_format.to_string(),
        json!({
            "health_score": 1.0,
            "consecutive_failures": 0,
            "last_failure_at": Value::Null,
            "rate_limit_cooldown_until_unix_secs": Value::Null,
            "consecutive_rate_limits": 0,
            "rate_limit_probe_until_unix_secs": Value::Null,
        }),
    );
    Some(Value::Object(health_by_format))
}

/// Project a 429 into the per-format rate-limit cooldown (P0-2).
///
/// Writes `rate_limit_cooldown_until_unix_secs` / `consecutive_rate_limits`
/// into the key's `health_by_format` payload. Returns `None` (no write) when
/// the projected cooldown barely moved, so a 429 storm does not turn into a
/// per-request DB write (anti write-amplification, 5s threshold).
pub(crate) fn project_local_rate_limit_cooldown(
    current_health_by_format: Option<&Value>,
    api_format: &str,
    observed_at_unix_secs: u64,
    retry_after_secs: Option<u64>,
) -> Option<Value> {
    let api_format = api_format.trim();
    if api_format.is_empty() {
        return None;
    }

    let mut health_by_format = current_health_by_format
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let current = health_by_format
        .get(api_format)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    // Older projections can retain the 429 ladder count after their deadline
    // was cleared. It is not an active cooldown, but it must still seed the
    // next fallback window instead of resetting back to 30 seconds.
    let previous_cooldown = aether_scheduler_core::provider_key_rate_limit_cooldown_payload(
        &Value::Object(current.clone()),
    )
    .or_else(|| {
        current
            .get("consecutive_rate_limits")
            .and_then(Value::as_u64)
            .map(
                |consecutive_rate_limits| aether_scheduler_core::ProviderKeyRateLimitCooldown {
                    until_unix_secs: 0,
                    consecutive_rate_limits,
                },
            )
    });
    let cooldown = aether_scheduler_core::ProviderKeyRateLimitCooldown::project(
        previous_cooldown,
        observed_at_unix_secs,
        retry_after_secs,
    );

    // Anti write-amplification: skip the CAS write when the active window was
    // already covering the new deadline within the 5-second threshold. A 429
    // must still clear an in-flight probe marker even when its deadline barely
    // moves, otherwise a stale reservation could suppress the next recovery.
    let has_probe_reservation = current
        .get("rate_limit_probe_until_unix_secs")
        .is_some_and(|value| !value.is_null());
    if let Some(existing) = aether_scheduler_core::provider_key_rate_limit_cooldown_payload(
        &Value::Object(current.clone()),
    ) {
        let still_active = existing.until_unix_secs > observed_at_unix_secs;
        let barely_moved = cooldown.until_unix_secs.abs_diff(existing.until_unix_secs) <= 5;
        if still_active && barely_moved && !has_probe_reservation {
            return None;
        }
    }

    let mut projected = current;
    projected.insert(
        "rate_limit_cooldown_until_unix_secs".to_string(),
        json!(cooldown.until_unix_secs),
    );
    projected.insert(
        "consecutive_rate_limits".to_string(),
        json!(cooldown.consecutive_rate_limits),
    );
    projected.insert("rate_limit_probe_until_unix_secs".to_string(), Value::Null);
    health_by_format.insert(api_format.to_string(), Value::Object(projected));
    Some(Value::Object(health_by_format))
}

/// Atomically reserving the first request after an expired 429 cooldown keeps
/// multiple Gateway instances from probing the same key at once. The caller
/// must still perform the fenced compare-and-set write; this is only the pure
/// JSON projection used by that write.
pub(crate) fn project_local_rate_limit_probe_reservation(
    current_health_by_format: Option<&Value>,
    api_format: &str,
    observed_at_unix_secs: u64,
) -> Option<Value> {
    let api_format = api_format.trim();
    if api_format.is_empty() {
        return None;
    }

    let mut health_by_format = current_health_by_format
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut current = health_by_format
        .get(api_format)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let cooldown = aether_scheduler_core::provider_key_rate_limit_cooldown_payload(
        &Value::Object(current.clone()),
    )?;
    if observed_at_unix_secs < cooldown.until_unix_secs {
        return None;
    }
    if aether_scheduler_core::provider_key_rate_limit_probe_payload(&Value::Object(current.clone()))
        .is_some_and(|probe| observed_at_unix_secs < probe.until_unix_secs)
    {
        return None;
    }

    current.insert(
        "rate_limit_probe_until_unix_secs".to_string(),
        json!(observed_at_unix_secs
            .saturating_add(aether_scheduler_core::RATE_LIMIT_PROBE_RESERVATION_SECS)),
    );
    health_by_format.insert(api_format.to_string(), Value::Object(current));
    Some(Value::Object(health_by_format))
}

pub(crate) fn project_local_key_circuit_open(
    current_circuit_by_format: Option<&Value>,
    api_format: &str,
    reason: &str,
    observed_at_unix_secs: u64,
    max_probe_interval_minutes: i32,
) -> Option<Value> {
    let api_format = api_format.trim();
    let reason = reason.trim();
    if api_format.is_empty() || reason.is_empty() {
        return None;
    }

    let mut circuit_by_format = current_circuit_by_format
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let current = circuit_by_format
        .get(api_format)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let max_probe_interval_minutes =
        normalize_max_probe_interval_minutes(max_probe_interval_minutes);
    let probe_interval_minutes =
        next_circuit_probe_interval_minutes(&current, max_probe_interval_minutes);
    let next_probe_at_unix_secs =
        next_probe_at_unix_secs(observed_at_unix_secs, probe_interval_minutes);
    let open_at = current
        .get("open_at")
        .filter(|_| current_bool(&current, "open"))
        .cloned()
        .unwrap_or_else(|| json!(unix_secs_to_rfc3339(observed_at_unix_secs)));
    let half_open_failures = current
        .get("half_open_failures")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(u64::from(current_bool(&current, "open")));
    let request_results_window =
        append_request_result_window(&current, observed_at_unix_secs, false);
    circuit_by_format.insert(
        api_format.to_string(),
        json!({
            "open": true,
            "open_at": open_at,
            "reason": reason,
            "next_probe_at": unix_secs_to_rfc3339(next_probe_at_unix_secs),
            "next_probe_at_unix_secs": next_probe_at_unix_secs,
            "probe_interval_minutes": probe_interval_minutes,
            "max_probe_interval_minutes": max_probe_interval_minutes,
            "last_failure_at": unix_secs_to_rfc3339(observed_at_unix_secs),
            "last_probe_failure_at": if half_open_failures > 0 {
                json!(unix_secs_to_rfc3339(observed_at_unix_secs))
            } else {
                Value::Null
            },
            "half_open_until": Value::Null,
            "half_open_until_unix_secs": Value::Null,
            "half_open_successes": 0,
            "half_open_failures": half_open_failures,
            "request_results_window": request_results_window,
        }),
    );
    Some(Value::Object(circuit_by_format))
}

/// Reserve the single half-open probe after a circuit's next-probe deadline.
/// The caller persists this projection with a compare-and-set, making the
/// reservation the ownership boundary across Gateway instances.
pub(crate) fn project_local_key_circuit_probe_reservation(
    current_circuit_by_format: Option<&Value>,
    api_format: &str,
    observed_at_unix_secs: u64,
) -> Option<Value> {
    let api_format = api_format.trim();
    if api_format.is_empty() {
        return None;
    }

    let mut circuit_by_format = current_circuit_by_format
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut current = circuit_by_format
        .get(api_format)
        .and_then(Value::as_object)
        .cloned()?;
    if !current_bool(&current, "open") {
        return None;
    }
    if current
        .get("next_probe_at_unix_secs")
        .and_then(Value::as_u64)
        .is_some_and(|until| observed_at_unix_secs < until)
    {
        return None;
    }
    if current
        .get("half_open_until_unix_secs")
        .and_then(Value::as_u64)
        .is_some_and(|until| observed_at_unix_secs < until)
    {
        return None;
    }

    let until_unix_secs =
        observed_at_unix_secs.saturating_add(LOCAL_KEY_CIRCUIT_PROBE_RESERVATION_SECS);
    current.insert(
        "half_open_until".to_string(),
        json!(unix_secs_to_rfc3339(until_unix_secs)),
    );
    current.insert(
        "half_open_until_unix_secs".to_string(),
        json!(until_unix_secs),
    );
    circuit_by_format.insert(api_format.to_string(), Value::Object(current));
    Some(Value::Object(circuit_by_format))
}

pub(crate) fn project_local_key_circuit_failure(
    current_circuit_by_format: Option<&Value>,
    api_format: &str,
    observed_at_unix_secs: u64,
    consecutive_failures: u64,
    max_probe_interval_minutes: i32,
) -> Option<Value> {
    project_local_key_circuit_failure_with_success_rate(
        current_circuit_by_format,
        api_format,
        observed_at_unix_secs,
        consecutive_failures,
        max_probe_interval_minutes,
        None,
    )
}

/// P1-6: circuit failure projection with an optional rolling success-rate
/// verdict. A flapping key (success/failure alternating) never stacks 8
/// consecutive misses, so the consecutive ladder alone misses it; when the
/// 5-minute success-rate window breaches (>=10 samples, <20% success), the
/// circuit opens through this override.
pub(crate) fn project_local_key_circuit_failure_with_success_rate(
    current_circuit_by_format: Option<&Value>,
    api_format: &str,
    observed_at_unix_secs: u64,
    consecutive_failures: u64,
    max_probe_interval_minutes: i32,
    success_rate_breached: Option<bool>,
) -> Option<Value> {
    let api_format = api_format.trim();
    if api_format.is_empty() {
        return None;
    }

    let mut circuit_by_format = current_circuit_by_format
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let current = circuit_by_format
        .get(api_format)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let request_results_window =
        append_request_result_window(&current, observed_at_unix_secs, false);
    let already_open = current_bool(&current, "open");
    let success_rate_open = success_rate_breached.unwrap_or(false);
    if !already_open
        && !success_rate_open
        && consecutive_failures < LOCAL_KEY_CIRCUIT_FAILURE_THRESHOLD
    {
        circuit_by_format.insert(
            api_format.to_string(),
            json!({
                "open": false,
                "open_at": Value::Null,
                "reason": Value::Null,
                "next_probe_at": Value::Null,
                "next_probe_at_unix_secs": Value::Null,
                "probe_interval_minutes": 0,
                "max_probe_interval_minutes": normalize_max_probe_interval_minutes(max_probe_interval_minutes),
                "failure_count": consecutive_failures,
                "last_failure_at": unix_secs_to_rfc3339(observed_at_unix_secs),
                "last_probe_failure_at": Value::Null,
                "half_open_until": Value::Null,
                "half_open_until_unix_secs": Value::Null,
                "half_open_successes": 0,
                "half_open_failures": 0,
                "request_results_window": request_results_window,
            }),
        );
        return Some(Value::Object(circuit_by_format));
    }

    let max_probe_interval_minutes =
        normalize_max_probe_interval_minutes(max_probe_interval_minutes);
    let probe_interval_minutes =
        next_circuit_probe_interval_minutes(&current, max_probe_interval_minutes);
    let next_probe_at_unix_secs =
        next_probe_at_unix_secs(observed_at_unix_secs, probe_interval_minutes);
    let open_at = current
        .get("open_at")
        .filter(|_| already_open)
        .cloned()
        .unwrap_or_else(|| json!(unix_secs_to_rfc3339(observed_at_unix_secs)));
    let half_open_failures = current
        .get("half_open_failures")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(u64::from(already_open));

    circuit_by_format.insert(
        api_format.to_string(),
        json!({
            "open": true,
            "open_at": open_at,
            "reason": if success_rate_open && !already_open {
                format!("success_rate_window_{}pct", (SUCCESS_RATE_WINDOW_THRESHOLD * 100.0) as u64)
            } else {
                format!("consecutive_failures_{LOCAL_KEY_CIRCUIT_FAILURE_THRESHOLD}")
            },
            "next_probe_at": unix_secs_to_rfc3339(next_probe_at_unix_secs),
            "next_probe_at_unix_secs": next_probe_at_unix_secs,
            "probe_interval_minutes": probe_interval_minutes,
            "max_probe_interval_minutes": max_probe_interval_minutes,
            "failure_count": consecutive_failures,
            "last_failure_at": unix_secs_to_rfc3339(observed_at_unix_secs),
            "last_probe_failure_at": if already_open {
                json!(unix_secs_to_rfc3339(observed_at_unix_secs))
            } else {
                Value::Null
            },
            "half_open_until": Value::Null,
            "half_open_until_unix_secs": Value::Null,
            "half_open_successes": 0,
            "half_open_failures": half_open_failures,
            "request_results_window": request_results_window,
        }),
    );
    Some(Value::Object(circuit_by_format))
}

/// P1-7: success projection during the recovery ramp. Health climbs from the
/// ramp's initial 0.75 toward 1.0 (linear per remaining success), and the
/// ramp counter on the circuit payload counts down; the circuit-side
/// decrement happens in project_local_key_circuit_closed_with_ramp when the
/// caller re-projects the closed circuit. Kept side-effect-free: this only
/// writes the health payload.
pub(crate) fn project_local_ramp_success_health(
    current_health_by_format: Option<&Value>,
    current_circuit_by_format: Option<&Value>,
    api_format: &str,
) -> Option<Value> {
    let api_format = api_format.trim();
    if api_format.is_empty() {
        return None;
    }

    let remaining = current_circuit_by_format
        .and_then(Value::as_object)
        .and_then(|values| values.get(api_format))
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("ramp_remaining_successes"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if remaining == 0 {
        // Not actually ramping — fall back to the plain success projection.
        return project_local_success_health(current_health_by_format, api_format);
    }

    let required = CIRCUIT_RAMP_REQUIRED_SUCCESSES.max(1);
    let progressed = required.saturating_sub(remaining.saturating_sub(1));
    // Linear climb 0.75 → 1.0 across the ramp steps.
    let step = (1.0 - CIRCUIT_RAMP_INITIAL_HEALTH) / u64::from(required) as f64;
    let score = CIRCUIT_RAMP_INITIAL_HEALTH + step * u64::from(progressed) as f64;
    let score = (score * 1000.0).round() / 1000.0;

    let mut health_by_format = current_health_by_format
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut payload = health_by_format
        .get(api_format)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    payload.insert("health_score".to_string(), json!(score));
    payload.insert("consecutive_failures".to_string(), json!(0));
    payload.insert("last_failure_at".to_string(), Value::Null);
    // A success during the ramp clears any lingering rate-limit cooldown.
    payload.insert(
        "rate_limit_cooldown_until_unix_secs".to_string(),
        Value::Null,
    );
    payload.insert("consecutive_rate_limits".to_string(), json!(0));
    payload.insert("rate_limit_probe_until_unix_secs".to_string(), Value::Null);
    health_by_format.insert(api_format.to_string(), Value::Object(payload));
    Some(Value::Object(health_by_format))
}

pub(crate) fn project_local_key_circuit_closed(
    current_circuit_by_format: Option<&Value>,
    api_format: &str,
) -> Option<Value> {
    project_local_key_circuit_closed_with_ramp(current_circuit_by_format, api_format, None)
}

/// P1-7: circuit close with a recovery ramp. When the closed circuit was
/// actually open (a probe succeeded), the entry enters ramping state:
/// `ramp_remaining_successes` counts down on each projected success and a
/// failure during the ramp re-opens immediately (handled by the caller
/// checking `circuit_ramp_active`). A no-op close (was never open) keeps the
/// legacy zeroed payload.
pub(crate) fn project_local_key_circuit_closed_with_ramp(
    current_circuit_by_format: Option<&Value>,
    api_format: &str,
    previous_health_by_format: Option<&Value>,
) -> Option<Value> {
    let api_format = api_format.trim();
    if api_format.is_empty() {
        return None;
    }

    let was_open = current_circuit_by_format
        .and_then(Value::as_object)
        .and_then(|values| values.get(api_format))
        .and_then(Value::as_object)
        .is_some_and(|payload| {
            payload
                .get("open")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        });

    let mut circuit_by_format = current_circuit_by_format
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut payload = serde_json::Map::new();
    payload.insert("open".to_string(), json!(false));
    payload.insert("open_at".to_string(), Value::Null);
    payload.insert("reason".to_string(), Value::Null);
    payload.insert("next_probe_at".to_string(), Value::Null);
    payload.insert("next_probe_at_unix_secs".to_string(), Value::Null);
    payload.insert("half_open_until".to_string(), Value::Null);
    payload.insert("half_open_until_unix_secs".to_string(), Value::Null);
    payload.insert("half_open_successes".to_string(), json!(0));
    payload.insert("half_open_failures".to_string(), json!(0));
    if was_open {
        payload.insert(
            "ramp_remaining_successes".to_string(),
            json!(CIRCUIT_RAMP_REQUIRED_SUCCESSES),
        );
        // The ramp's initial health lives on the health payload; record the
        // seed so project_local_success_health can pick it up on the first
        // ramping success.
        if let Some(health) = previous_health_by_format
            .and_then(Value::as_object)
            .and_then(|values| values.get(api_format))
            .and_then(Value::as_object)
        {
            let mut ramp_seed = serde_json::Map::new();
            ramp_seed.insert(
                "ramp_initial_health".to_string(),
                json!(CIRCUIT_RAMP_INITIAL_HEALTH),
            );
            let _ = health; // health fields merged by the caller, not here
            payload.insert("ramp_health_seed".to_string(), Value::Object(ramp_seed));
        } else {
            payload.insert(
                "ramp_health_seed".to_string(),
                json!({"ramp_initial_health": CIRCUIT_RAMP_INITIAL_HEALTH}),
            );
        }
    } else {
        payload.insert("ramp_remaining_successes".to_string(), json!(0));
    }
    circuit_by_format.insert(api_format.to_string(), Value::Object(payload));
    Some(Value::Object(circuit_by_format))
}

/// P1-7: true while the circuit entry is in its post-close recovery ramp.
pub(crate) fn circuit_ramp_active(
    current_circuit_by_format: Option<&Value>,
    api_format: &str,
) -> bool {
    current_circuit_by_format
        .and_then(Value::as_object)
        .and_then(|values| values.get(api_format))
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("ramp_remaining_successes"))
        .and_then(Value::as_u64)
        .is_some_and(|remaining| remaining > 0)
}

fn current_bool(current: &serde_json::Map<String, Value>, field: &str) -> bool {
    current.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn normalize_max_probe_interval_minutes(value: i32) -> u64 {
    value.clamp(0, LOCAL_KEY_CIRCUIT_MAX_PROBE_INTERVAL_MINUTES as i32) as u64
}

fn next_circuit_probe_interval_minutes(
    current: &serde_json::Map<String, Value>,
    max_probe_interval_minutes: u64,
) -> u64 {
    if max_probe_interval_minutes == 0 {
        return 0;
    }
    if !current_bool(current, "open") {
        return 1.min(max_probe_interval_minutes);
    }
    current
        .get("probe_interval_minutes")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1)
        .saturating_mul(2)
        .min(max_probe_interval_minutes)
}

fn next_probe_at_unix_secs(observed_at_unix_secs: u64, interval_minutes: u64) -> u64 {
    observed_at_unix_secs.saturating_add(interval_minutes.saturating_mul(60))
}

fn append_request_result_window(
    current: &serde_json::Map<String, Value>,
    observed_at_unix_secs: u64,
    ok: bool,
) -> Value {
    let mut window = current
        .get("request_results_window")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // P1-6: the window must hold enough samples for the 5-minute success-rate
    // verdict (>= SUCCESS_RATE_WINDOW_MIN_SAMPLES), so the old 8-entry cap
    // grows to 64. Old payloads under-fill silently — the verdict just needs
    // more traffic before it can fire, which fails safe.
    window.push(json!({
        "ts": observed_at_unix_secs,
        "ok": ok,
    }));
    const WINDOW_KEEP: usize = 64;
    if window.len() > WINDOW_KEEP {
        window = window.split_off(window.len() - WINDOW_KEEP);
    }
    Value::Array(window)
}

fn local_candidate_failure_should_project_health(
    classification: LocalFailoverClassification,
    status_code: u16,
) -> bool {
    if status_code < 400 {
        return false;
    }
    if status_code == 400 {
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

fn projected_failure_health_score(
    classification: LocalFailoverClassification,
    status_code: u16,
    consecutive_failures: u64,
) -> f64 {
    let base_score = match classification {
        LocalFailoverClassification::RetrySuccessPattern => 0.75,
        _ if status_code >= 500 => 0.6,
        _ => 0.7,
    };

    let penalty = consecutive_failures.saturating_sub(1) as f64 * 0.15;
    let normalized = (base_score - penalty).max(LOCAL_HEALTH_SCORE_FLOOR);
    (normalized * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        project_local_failure_health, project_local_key_circuit_closed,
        project_local_key_circuit_failure, project_local_key_circuit_open,
        project_local_success_health,
    };
    use crate::orchestration::LocalFailoverClassification;

    #[test]
    fn failure_projection_tracks_consecutive_failures_and_degrades_score() {
        let projected = project_local_failure_health(
            Some(&json!({
                "openai:chat": {
                    "health_score": 0.7,
                    "consecutive_failures": 1,
                    "last_failure_at": "2026-01-01T00:00:00+00:00"
                }
            })),
            "openai:chat",
            LocalFailoverClassification::RetryUpstreamFailure,
            503,
            1_760_000_000,
        )
        .expect("projection should exist");

        assert_eq!(projected["openai:chat"]["consecutive_failures"], json!(2));
        assert_eq!(projected["openai:chat"]["health_score"], json!(0.45));
        assert!(projected["openai:chat"]["last_failure_at"].is_string());
    }

    #[test]
    fn failure_projection_ignores_configured_stop_pattern() {
        assert!(project_local_failure_health(
            None,
            "openai:chat",
            LocalFailoverClassification::StopErrorPattern,
            400,
            1_760_000_000,
        )
        .is_none());
    }

    #[test]
    fn failure_projection_ignores_client_bad_request() {
        assert!(project_local_failure_health(
            None,
            "openai:chat",
            LocalFailoverClassification::RetryUpstreamFailure,
            400,
            1_760_000_000,
        )
        .is_none());
    }

    #[test]
    fn success_projection_resets_only_target_format() {
        let projected = project_local_success_health(
            Some(&json!({
                "openai:chat": {
                    "health_score": 0.4,
                    "consecutive_failures": 3,
                    "last_failure_at": "2026-01-01T00:00:00+00:00"
                },
                "openai:responses": {
                    "health_score": 0.8,
                    "consecutive_failures": 1,
                    "last_failure_at": "2026-01-02T00:00:00+00:00"
                }
            })),
            "openai:chat",
        )
        .expect("projection should exist");

        assert_eq!(
            projected["openai:chat"],
            json!({
                "health_score": 1.0,
                "consecutive_failures": 0,
                "last_failure_at": Value::Null,
            })
        );
        assert_eq!(projected["openai:responses"]["health_score"], json!(0.8));
    }

    #[test]
    fn circuit_open_projection_sets_probe_deadline() {
        let projected = project_local_key_circuit_open(
            None,
            "openai:chat",
            "account_deactivated_401",
            1_760_000_000,
            32,
        )
        .expect("projection should exist");

        assert_eq!(projected["openai:chat"]["open"], json!(true));
        assert_eq!(
            projected["openai:chat"]["reason"],
            json!("account_deactivated_401")
        );
        assert_eq!(
            projected["openai:chat"]["next_probe_at_unix_secs"],
            json!(1_760_000_060u64)
        );
        assert_eq!(projected["openai:chat"]["probe_interval_minutes"], json!(1));
    }

    #[test]
    fn consecutive_failure_circuit_opens_after_threshold_and_backs_off() {
        let before_threshold =
            project_local_key_circuit_failure(None, "openai:chat", 1_760_000_000, 7, 32)
                .expect("projection should exist");
        assert_eq!(before_threshold["openai:chat"]["open"], json!(false));

        let opened = project_local_key_circuit_failure(
            Some(&before_threshold),
            "openai:chat",
            1_760_000_060,
            8,
            32,
        )
        .expect("projection should exist");
        assert_eq!(opened["openai:chat"]["open"], json!(true));
        assert_eq!(
            opened["openai:chat"]["reason"],
            json!("consecutive_failures_8")
        );
        assert_eq!(opened["openai:chat"]["probe_interval_minutes"], json!(1));
        assert_eq!(
            opened["openai:chat"]["next_probe_at_unix_secs"],
            json!(1_760_000_120u64)
        );

        let backed_off =
            project_local_key_circuit_failure(Some(&opened), "openai:chat", 1_760_000_120, 9, 32)
                .expect("projection should exist");
        assert_eq!(
            backed_off["openai:chat"]["probe_interval_minutes"],
            json!(2)
        );
        assert_eq!(
            backed_off["openai:chat"]["next_probe_at_unix_secs"],
            json!(1_760_000_240u64)
        );
    }

    #[test]
    fn circuit_closed_projection_resets_format_circuit() {
        let projected = project_local_key_circuit_closed(
            Some(&json!({
                "openai:chat": {
                    "open": true,
                    "reason": "account_deactivated_401",
                    "next_probe_at_unix_secs": 1_760_001_920u64
                }
            })),
            "openai:chat",
        )
        .expect("projection should exist");

        assert_eq!(projected["openai:chat"]["open"], json!(false));
        assert_eq!(projected["openai:chat"]["reason"], Value::Null);
        assert_eq!(
            projected["openai:chat"]["next_probe_at_unix_secs"],
            Value::Null
        );
    }
}

#[cfg(test)]
mod p0_failure_class_tests {
    use serde_json::{json, Value};

    use super::{
        parse_retry_after_secs, project_local_failure_health, project_local_key_circuit_failure,
        project_local_rate_limit_cooldown, project_local_rate_limit_probe_reservation,
        project_local_success_health,
    };
    use crate::orchestration::LocalFailoverClassification;

    const NOW: u64 = 100_000;

    #[test]
    fn rate_limit_failure_is_not_projected_into_failure_health() {
        // 429 must never enter failure health / consecutive-failure accounting.
        let projected = project_local_failure_health(
            Some(&json!({"openai:chat": {"health_score": 0.9, "consecutive_failures": 2}})),
            "openai:chat",
            LocalFailoverClassification::RetryStatusCode,
            429,
            NOW,
        );
        assert!(projected.is_none());
    }

    #[test]
    fn transient_failure_projection_preserves_rate_limit_cooldown_fields() {
        // A 5xx between two 429s must not clear the cooldown ladder.
        let projected = project_local_failure_health(
            Some(&json!({
                "openai:chat": {
                    "health_score": 0.9,
                    "consecutive_failures": 2,
                    "rate_limit_cooldown_until_unix_secs": NOW + 30,
                    "consecutive_rate_limits": 1,
                    "rate_limit_probe_until_unix_secs": NOW + 60
                }
            })),
            "openai:chat",
            LocalFailoverClassification::RetryUpstreamFailure,
            500,
            NOW,
        )
        .expect("500 failure should project");
        let entry = &projected["openai:chat"];
        assert_eq!(entry["consecutive_failures"], json!(3));
        assert_eq!(
            entry["rate_limit_cooldown_until_unix_secs"],
            json!(NOW + 30)
        );
        assert_eq!(entry["consecutive_rate_limits"], json!(1));
        assert_eq!(entry["rate_limit_probe_until_unix_secs"], json!(NOW + 60));
    }

    #[test]
    fn success_projection_clears_rate_limit_cooldown() {
        let projected = project_local_success_health(
            Some(&json!({
                "openai:chat": {
                    "health_score": 0.5,
                    "consecutive_failures": 5,
                    "rate_limit_cooldown_until_unix_secs": NOW + 60,
                    "consecutive_rate_limits": 2,
                    "rate_limit_probe_until_unix_secs": NOW + 60
                }
            })),
            "openai:chat",
        )
        .expect("success should project");
        let entry = &projected["openai:chat"];
        assert_eq!(entry["health_score"], json!(1.0));
        assert_eq!(entry["consecutive_failures"], json!(0));
        assert_eq!(entry["rate_limit_cooldown_until_unix_secs"], Value::Null);
        assert_eq!(entry["consecutive_rate_limits"], json!(0));
        assert_eq!(entry["rate_limit_probe_until_unix_secs"], Value::Null);
    }

    #[test]
    fn rate_limit_cooldown_projection_writes_window() {
        let projected = project_local_rate_limit_cooldown(
            Some(&json!({"openai:chat": {"health_score": 0.9, "consecutive_rate_limits": 1}})),
            "openai:chat",
            NOW,
            Some(30),
        )
        .expect("429 should project cooldown");
        let entry = &projected["openai:chat"];
        assert_eq!(
            entry["rate_limit_cooldown_until_unix_secs"],
            json!(NOW + 30)
        );
        assert_eq!(entry["consecutive_rate_limits"], json!(2));
        // Pre-existing health fields are preserved.
        assert_eq!(entry["health_score"], json!(0.9));
    }

    #[test]
    fn expired_cooldown_accepts_one_short_probe_then_429_replaces_it() {
        let cooldown_expired = json!({
            "openai:chat": {
                "rate_limit_cooldown_until_unix_secs": NOW - 1,
                "consecutive_rate_limits": 2
            }
        });
        let claimed =
            project_local_rate_limit_probe_reservation(Some(&cooldown_expired), "openai:chat", NOW)
                .expect("expired cooldown should admit one probe");
        assert_eq!(
            claimed["openai:chat"]["rate_limit_probe_until_unix_secs"],
            json!(NOW + aether_scheduler_core::RATE_LIMIT_PROBE_RESERVATION_SECS)
        );
        assert!(
            project_local_rate_limit_probe_reservation(Some(&claimed), "openai:chat", NOW,)
                .is_none()
        );

        let re_limited =
            project_local_rate_limit_cooldown(Some(&claimed), "openai:chat", NOW, Some(30))
                .expect("a second 429 should replace the probe with a cooldown");
        assert_eq!(
            re_limited["openai:chat"]["rate_limit_probe_until_unix_secs"],
            Value::Null
        );
        assert_eq!(
            re_limited["openai:chat"]["rate_limit_cooldown_until_unix_secs"],
            json!(NOW + 30)
        );
    }

    #[test]
    fn rate_limit_cooldown_projection_skips_negligible_moves() {
        // Anti write-amplification: still-active window that barely moved → no write.
        let active = json!({
            "openai:chat": {
                "rate_limit_cooldown_until_unix_secs": NOW + 60,
                "consecutive_rate_limits": 1
            }
        });
        let projected =
            project_local_rate_limit_cooldown(Some(&active), "openai:chat", NOW, Some(63));
        assert!(projected.is_none());
        // A meaningfully later deadline still writes.
        let projected =
            project_local_rate_limit_cooldown(Some(&active), "openai:chat", NOW, Some(300));
        assert!(projected.is_some());
    }

    #[test]
    fn parse_retry_after_accepts_delta_and_http_date() {
        assert_eq!(parse_retry_after_secs(Some("30"), NOW), Some(30));
        assert_eq!(parse_retry_after_secs(Some(" 30 "), NOW), Some(30));
        // HTTP-date two minutes past NOW.
        let date = chrono::DateTime::from_timestamp((NOW + 120) as i64, 0)
            .expect("timestamp should build")
            .format("%a, %d %b %Y %H:%M:%S GMT")
            .to_string();
        assert_eq!(parse_retry_after_secs(Some(&date), NOW), Some(120));
    }

    #[test]
    fn parse_retry_after_rejects_garbage_and_past_dates() {
        assert_eq!(parse_retry_after_secs(None, NOW), None);
        assert_eq!(parse_retry_after_secs(Some(""), NOW), None);
        assert_eq!(parse_retry_after_secs(Some("soon"), NOW), None);
        assert_eq!(parse_retry_after_secs(Some("0"), NOW), None);
        assert_eq!(parse_retry_after_secs(Some("99999"), NOW), None);
        let past = chrono::DateTime::from_timestamp((NOW - 60) as i64, 0)
            .expect("timestamp should build")
            .format("%a, %d %b %Y %H:%M:%S GMT")
            .to_string();
        assert_eq!(parse_retry_after_secs(Some(&past), NOW), None);
    }

    #[test]
    fn credential_dead_circuit_ladder_unchanged_for_transient() {
        // Transient path: circuit opens only at 8 consecutive failures (regression guard).
        let below = project_local_key_circuit_failure(None, "openai:chat", NOW, 7, 32)
            .expect("projection should build");
        assert_eq!(below["openai:chat"]["open"], json!(false));

        let at_threshold = project_local_key_circuit_failure(None, "openai:chat", NOW, 8, 32)
            .expect("projection should build");
        assert_eq!(at_threshold["openai:chat"]["open"], json!(true));
        assert_eq!(
            at_threshold["openai:chat"]["next_probe_at_unix_secs"],
            json!(NOW + 60)
        );
    }
}
