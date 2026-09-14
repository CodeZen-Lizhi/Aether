use serde_json::{json, Map, Value};

use super::classifier::{ChatFailureFact, ChatHealthPenalty};
use super::health::{append_request_result_window, project_local_key_circuit_open};
use crate::handlers::shared::unix_secs_to_rfc3339;

pub(crate) const CHAT_HEALTH_POLICY_VERSION: u64 = 2;
const SCORE_UNITS: u64 = 1_000;
const POINT_UNITS: u64 = 100;

pub(crate) struct ChatHealthProjection {
    pub(crate) health_by_format: Value,
    pub(crate) circuit_breaker_by_format: Value,
}

fn payload(values: Option<&Value>, api_format: &str) -> Map<String, Value> {
    values
        .and_then(|values| values.get(api_format))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn replace_payload(values: Option<&Value>, api_format: &str, payload: Map<String, Value>) -> Value {
    let mut values = values
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    values.insert(api_format.to_owned(), Value::Object(payload));
    Value::Object(values)
}

fn score_units(payload: &Map<String, Value>) -> u64 {
    (payload
        .get("health_score")
        .and_then(Value::as_f64)
        .filter(|score| score.is_finite())
        .unwrap_or(1.0)
        .clamp(0.0, 1.0)
        * SCORE_UNITS as f64)
        .round() as u64
}

fn current_streak(payload: &Map<String, Value>) -> u64 {
    if payload.get("health_policy_version").and_then(Value::as_u64)
        == Some(CHAT_HEALTH_POLICY_VERSION)
    {
        payload
            .get("consecutive_failures")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    } else {
        // The legacy counter was a budget, not a strict sequence of failures.
        0
    }
}

pub(crate) fn project_chat_failure(
    health: Option<&Value>,
    circuit: Option<&Value>,
    api_format: &str,
    fact: ChatFailureFact,
    observed_at: u64,
    retry_after_secs: Option<u64>,
    max_probe_interval_minutes: i32,
) -> Option<ChatHealthProjection> {
    if api_format.trim().is_empty() || fact.penalty == ChatHealthPenalty::Neutral {
        return None;
    }
    let mut next_health = payload(health, api_format);
    let already_open = payload(circuit, api_format)
        .get("open")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let previous_score = if already_open {
        0
    } else {
        score_units(&next_health)
    };
    let streak = current_streak(&next_health).saturating_add(1);
    let extra = match streak {
        1..=2 => 0,
        3..=4 => 1,
        _ => 2,
    };
    let base = match fact.penalty {
        ChatHealthPenalty::Points(base) => u64::from(base),
        ChatHealthPenalty::Unavailable => 10,
        ChatHealthPenalty::Neutral => return None,
    };
    let score = match fact.penalty {
        ChatHealthPenalty::Unavailable => 0,
        _ => previous_score.saturating_sub((base + extra) * POINT_UNITS),
    };
    next_health.insert(
        "health_policy_version".into(),
        json!(CHAT_HEALTH_POLICY_VERSION),
    );
    next_health.insert(
        "health_score".into(),
        json!(score as f64 / SCORE_UNITS as f64),
    );
    next_health.insert("consecutive_failures".into(), json!(streak));
    next_health.insert(
        "last_failure_at".into(),
        json!(unix_secs_to_rfc3339(observed_at)),
    );
    next_health.insert("last_failure_reason".into(), json!(fact.reason));
    next_health.insert("last_failure_base_points".into(), json!(base));
    next_health.insert("last_failure_extra_points".into(), json!(extra));
    next_health.insert(
        "last_health_score_before".into(),
        json!(previous_score as f64 / SCORE_UNITS as f64),
    );
    // A completed probe releases its reservation even when it failed for another reason.
    next_health.insert("rate_limit_probe_until_unix_secs".into(), Value::Null);
    next_health.remove("rate_limit_probe_lease");
    if fact.rate_limited || retry_after_secs.is_some() {
        let previous = Some(aether_scheduler_core::ProviderKeyRateLimitCooldown {
            until_unix_secs: next_health
                .get("rate_limit_cooldown_until_unix_secs")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            consecutive_rate_limits: next_health
                .get("consecutive_rate_limits")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        });
        let cooldown = aether_scheduler_core::ProviderKeyRateLimitCooldown::project_chat(
            previous,
            observed_at,
            retry_after_secs,
        );
        next_health.insert(
            "rate_limit_cooldown_until_unix_secs".into(),
            json!(cooldown.until_unix_secs),
        );
        next_health.insert(
            "consecutive_rate_limits".into(),
            json!(cooldown.consecutive_rate_limits),
        );
    }

    let previous_circuit = payload(circuit, api_format);
    let next_circuit = if score == 0 {
        let opened = project_local_key_circuit_open(
            circuit,
            api_format,
            fact.reason,
            observed_at,
            max_probe_interval_minutes.clamp(1, 32),
        )?;
        let mut opened = payload(Some(&opened), api_format);
        opened.insert(
            "circuit_epoch".into(),
            json!(previous_circuit
                .get("circuit_epoch")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .saturating_add(1)),
        );
        opened
    } else {
        let mut next = previous_circuit.clone();
        next.insert(
            "request_results_window".into(),
            append_request_result_window(&previous_circuit, observed_at, false),
        );
        next
    };
    let mut next_circuit = next_circuit;
    next_circuit.remove("half_open_lease");
    next_circuit.insert(
        "health_policy_version".into(),
        json!(CHAT_HEALTH_POLICY_VERSION),
    );
    next_circuit.insert("failure_count".into(), json!(streak));
    next_circuit.remove("failure_threshold");
    next_circuit.remove("ramp_remaining_successes");
    next_circuit.remove("ramp_health_seed");
    Some(ChatHealthProjection {
        health_by_format: replace_payload(health, api_format, next_health),
        circuit_breaker_by_format: replace_payload(circuit, api_format, next_circuit),
    })
}

pub(crate) fn project_chat_success(
    health: Option<&Value>,
    circuit: Option<&Value>,
    api_format: &str,
    observed_at: u64,
) -> Option<ChatHealthProjection> {
    if api_format.trim().is_empty() {
        return None;
    }
    let mut next_health = payload(health, api_format);
    let mut next_circuit = payload(circuit, api_format);
    let was_open = next_circuit
        .get("open")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let score = if was_open {
        POINT_UNITS
    } else {
        (score_units(&next_health) + POINT_UNITS).min(SCORE_UNITS)
    };
    next_health.insert(
        "health_policy_version".into(),
        json!(CHAT_HEALTH_POLICY_VERSION),
    );
    next_health.insert(
        "health_score".into(),
        json!(score as f64 / SCORE_UNITS as f64),
    );
    next_health.insert("consecutive_failures".into(), json!(0));
    next_health.insert("last_failure_at".into(), Value::Null);
    next_health.insert("rate_limit_cooldown_until_unix_secs".into(), Value::Null);
    next_health.insert("rate_limit_probe_until_unix_secs".into(), Value::Null);
    next_health.remove("rate_limit_probe_lease");
    next_health.insert("consecutive_rate_limits".into(), json!(0));

    let window = append_request_result_window(&next_circuit, observed_at, true);
    next_circuit.insert("request_results_window".into(), window);
    next_circuit.insert(
        "health_policy_version".into(),
        json!(CHAT_HEALTH_POLICY_VERSION),
    );
    next_circuit.insert("open".into(), json!(false));
    next_circuit.remove("half_open_lease");
    for field in [
        "open_at",
        "reason",
        "next_probe_at",
        "next_probe_at_unix_secs",
        "half_open_until",
        "half_open_until_unix_secs",
        "last_failure_at",
        "last_probe_failure_at",
    ] {
        next_circuit.insert(field.into(), Value::Null);
    }
    for field in [
        "half_open_successes",
        "half_open_failures",
        "failure_count",
        "probe_interval_minutes",
    ] {
        next_circuit.insert(field.into(), json!(0));
    }
    for field in [
        "failure_threshold",
        "ramp_remaining_successes",
        "ramp_health_seed",
    ] {
        next_circuit.remove(field);
    }
    Some(ChatHealthProjection {
        health_by_format: replace_payload(health, api_format, next_health),
        circuit_breaker_by_format: replace_payload(circuit, api_format, next_circuit),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::{classify_chat_failure, ChatFailureSource};

    #[test]
    fn weighted_failure_sequences_and_zero_only_circuit() {
        for (status, expected) in [
            (503, vec![0.9, 0.8, 0.6, 0.4, 0.1, 0.0]),
            (500, vec![0.8, 0.6, 0.3, 0.0]),
        ] {
            let mut health = None;
            let mut circuit = None;
            for score in expected {
                let next = project_chat_failure(
                    health.as_ref(),
                    circuit.as_ref(),
                    "openai:chat",
                    classify_chat_failure(status, None, ChatFailureSource::UpstreamResponse, false),
                    100,
                    None,
                    32,
                )
                .unwrap();
                assert_eq!(
                    next.health_by_format["openai:chat"]["health_score"],
                    json!(score)
                );
                assert_eq!(
                    next.circuit_breaker_by_format["openai:chat"]["open"]
                        .as_bool()
                        .unwrap_or(false),
                    score == 0.0
                );
                health = Some(next.health_by_format);
                circuit = Some(next.circuit_breaker_by_format);
            }
        }
    }

    #[test]
    fn migration_preserves_health_resets_legacy_counter_and_success_resets_streak() {
        let health = json!({"openai:chat": {"health_score": 0.625, "consecutive_failures": 7}});
        let failed = project_chat_failure(
            Some(&health),
            None,
            "openai:chat",
            classify_chat_failure(503, None, ChatFailureSource::UpstreamResponse, false),
            100,
            None,
            32,
        )
        .unwrap();
        assert_eq!(
            failed.health_by_format["openai:chat"]["health_score"],
            json!(0.525)
        );
        assert_eq!(
            failed.health_by_format["openai:chat"]["consecutive_failures"],
            json!(1)
        );
        let success = project_chat_success(
            Some(&failed.health_by_format),
            Some(&failed.circuit_breaker_by_format),
            "openai:chat",
            101,
        )
        .unwrap();
        assert_eq!(
            success.health_by_format["openai:chat"]["health_score"],
            json!(0.625)
        );
        assert_eq!(
            success.health_by_format["openai:chat"]["consecutive_failures"],
            json!(0)
        );
    }

    #[test]
    fn rate_limit_counts_and_preserves_long_provider_deadline() {
        let fact = classify_chat_failure(429, None, ChatFailureSource::UpstreamResponse, false);
        let first =
            project_chat_failure(None, None, "openai:chat", fact, 100, Some(3600), 32).unwrap();
        let next = project_chat_failure(
            Some(&first.health_by_format),
            Some(&first.circuit_breaker_by_format),
            "openai:chat",
            fact,
            101,
            None,
            32,
        )
        .unwrap();
        assert_eq!(
            next.health_by_format["openai:chat"]["health_score"],
            json!(0.8)
        );
        assert_eq!(
            next.health_by_format["openai:chat"]["rate_limit_cooldown_until_unix_secs"],
            json!(3700)
        );
    }

    #[test]
    fn explicit_retry_after_is_shared_for_non_rate_limit_failures() {
        for (status, score) in [(500, 0.8), (503, 0.9), (504, 0.9)] {
            let fact =
                classify_chat_failure(status, None, ChatFailureSource::UpstreamResponse, false);
            let cooldown =
                project_chat_failure(None, None, "openai:chat", fact, 100, Some(30), 32).unwrap();
            assert_eq!(
                cooldown.health_by_format["openai:chat"]["health_score"],
                json!(score)
            );
            assert_eq!(
                cooldown.health_by_format["openai:chat"]["rate_limit_cooldown_until_unix_secs"],
                json!(130)
            );
            let ordinary =
                project_chat_failure(None, None, "openai:chat", fact, 100, None, 32).unwrap();
            assert!(ordinary.health_by_format["openai:chat"]
                .get("rate_limit_cooldown_until_unix_secs")
                .is_none());
        }
    }

    #[test]
    fn probe_failure_doubles_delay_and_success_recovers_one_point() {
        let dead = ChatFailureFact {
            penalty: ChatHealthPenalty::Unavailable,
            rate_limited: false,
            reason: "credential_unavailable",
        };
        let first = project_chat_failure(None, None, "openai:chat", dead, 100, None, 0).unwrap();
        assert_eq!(
            first.circuit_breaker_by_format["openai:chat"]["next_probe_at_unix_secs"],
            json!(160)
        );
        let second = project_chat_failure(
            Some(&first.health_by_format),
            Some(&first.circuit_breaker_by_format),
            "openai:chat",
            dead,
            160,
            None,
            32,
        )
        .unwrap();
        assert_eq!(
            second.circuit_breaker_by_format["openai:chat"]["next_probe_at_unix_secs"],
            json!(280)
        );
        let recovered = project_chat_success(
            Some(&second.health_by_format),
            Some(&second.circuit_breaker_by_format),
            "openai:chat",
            280,
        )
        .unwrap();
        assert_eq!(
            recovered.health_by_format["openai:chat"]["health_score"],
            json!(0.1)
        );
        assert_eq!(
            recovered.circuit_breaker_by_format["openai:chat"]["open"],
            json!(false)
        );
        assert!(recovered.circuit_breaker_by_format["openai:chat"]
            .get("ramp_remaining_successes")
            .is_none());
    }
}
