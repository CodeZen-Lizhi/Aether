use serde::Serialize;
use serde_json::Value;

pub const CHAT_POLICY_VERSION: u32 = 1;
pub const DEFAULT_STREAM_FAILOVER_BUDGET_MS: u64 = 90_000;
pub const DEFAULT_STREAM_TOTAL_TIMEOUT_MS: u64 = 900_000;
pub const STREAM_TOTAL_TIMEOUT_CONFIG_KEY: &str = "stream_total_timeout_ms";

/// The full HTTP chat stream limit is independent of the legacy first-output budget.
pub fn configured_stream_total_timeout_ms(config: Option<&Value>) -> Option<u64> {
    config?
        .get(STREAM_TOTAL_TIMEOUT_CONFIG_KEY)?
        .as_u64()
        .filter(|value| (1_000..=crate::MAX_EXECUTION_REQUEST_TIMEOUT_MS).contains(value))
}

pub fn resolve_stream_total_timeout_ms(config: Option<&Value>) -> u64 {
    configured_stream_total_timeout_ms(config).unwrap_or(DEFAULT_STREAM_TOTAL_TIMEOUT_MS)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EffectiveChatAttempts {
    pub max_attempts: u32,
    pub source: &'static str,
}

fn configured(config: Option<&Value>, field: &str) -> Option<u32> {
    config?
        .get("failover_rules")?
        .get(field)?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
}

pub fn resolve_legacy_max_attempts(
    provider_config: Option<&Value>,
    endpoint_legacy: Option<i32>,
    provider_legacy: Option<i32>,
) -> EffectiveChatAttempts {
    [
        (
            configured(provider_config, "max_retries"),
            "failover_rules.max_retries",
        ),
        (
            endpoint_legacy
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| *value != 2),
            "endpoint.max_retries",
        ),
        (
            provider_legacy
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| *value != 2),
            "provider.max_retries",
        ),
    ]
    .into_iter()
    .find_map(|(value, source)| {
        value.map(|value| EffectiveChatAttempts {
            max_attempts: value.clamp(1, 99),
            source,
        })
    })
    .unwrap_or(EffectiveChatAttempts {
        max_attempts: 1,
        source: "default",
    })
}

pub fn resolve_chat_max_attempts(
    provider_config: Option<&Value>,
    endpoint_config: Option<&Value>,
    endpoint_legacy: Option<i32>,
    provider_legacy: Option<i32>,
) -> EffectiveChatAttempts {
    let endpoint = endpoint_legacy.and_then(|value| u32::try_from(value).ok());
    let provider = provider_legacy.and_then(|value| u32::try_from(value).ok());
    // Old default twos must not hide any previously effective explicit override.
    let choices = [
        (
            configured(provider_config, "max_attempts"),
            "failover_rules.max_attempts",
        ),
        (
            configured(provider_config, "max_retries"),
            "failover_rules.max_retries",
        ),
        (
            configured(endpoint_config, "max_attempts"),
            "endpoint.max_attempts",
        ),
        (endpoint.filter(|value| *value != 2), "endpoint.max_retries"),
        (
            configured(provider_config, "provider_max_attempts"),
            "provider.max_attempts",
        ),
        (provider.filter(|value| *value != 2), "provider.max_retries"),
        (endpoint, "endpoint.max_retries"),
        (provider, "provider.max_retries"),
    ];
    choices
        .into_iter()
        .find_map(|(value, source)| {
            value.map(|value| EffectiveChatAttempts {
                max_attempts: value.clamp(1, 99),
                source,
            })
        })
        .unwrap_or(EffectiveChatAttempts {
            max_attempts: 1,
            source: "default",
        })
}

pub fn resolve_stream_failover_budget_ms(config: Option<&Value>) -> u64 {
    config
        .and_then(|config| config.get("failover_rules"))
        .and_then(|rules| rules.get("stream_failover_budget_ms"))
        .and_then(Value::as_u64)
        .filter(|value| (1..=crate::MAX_EXECUTION_REQUEST_TIMEOUT_MS).contains(value))
        .unwrap_or(DEFAULT_STREAM_FAILOVER_BUDGET_MS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stream_total_timeout_is_independent_of_legacy_limits() {
        for config in [
            json!({}),
            json!({"failover_rules": {"stream_failover_budget_ms": 300000}}),
            json!({"stream_total_timeout_ms": null}),
            json!({"stream_total_timeout_ms": 999}),
            json!({"stream_total_timeout_ms": 1200001}),
            json!({"stream_total_timeout_ms": "5000"}),
        ] {
            assert_eq!(resolve_stream_total_timeout_ms(Some(&config)), 900_000);
            assert_eq!(configured_stream_total_timeout_ms(Some(&config)), None);
        }
        for timeout in [1_000, 1_001, 900_000, 1_200_000] {
            let config = json!({"stream_total_timeout_ms": timeout});
            assert_eq!(resolve_stream_total_timeout_ms(Some(&config)), timeout);
            assert_eq!(
                configured_stream_total_timeout_ms(Some(&config)),
                Some(timeout)
            );
        }
    }

    #[test]
    fn legacy_defaults_do_not_shadow_overrides_or_create_missing_values() {
        assert_eq!(
            resolve_legacy_max_attempts(None, Some(2), Some(2)).max_attempts,
            1
        );
        for (endpoint, provider, expected) in [
            (None, None, 1),
            (Some(2), None, 2),
            (None, Some(2), 2),
            (Some(2), Some(5), 5),
            (Some(2), Some(0), 1),
            (Some(7), Some(2), 7),
            (Some(999), Some(2), 99),
        ] {
            assert_eq!(
                resolve_chat_max_attempts(None, None, endpoint, provider).max_attempts,
                expected
            );
        }
        let rules = json!({"failover_rules": {"max_retries": 2}});
        assert_eq!(
            resolve_chat_max_attempts(Some(&rules), None, Some(7), Some(5)).max_attempts,
            2
        );
    }

    #[test]
    fn explicit_new_endpoint_two_overrides_provider_and_reads_are_idempotent() {
        let endpoint = json!({"failover_rules": {"max_attempts": 2}});
        let before = endpoint.clone();
        for _ in 0..2 {
            let effective = resolve_chat_max_attempts(None, Some(&endpoint), Some(2), Some(5));
            assert_eq!(effective.max_attempts, 2);
            assert_eq!(effective.source, "endpoint.max_attempts");
        }
        assert_eq!(endpoint, before);
    }

    #[test]
    fn budget_has_independent_default_and_override() {
        assert_eq!(resolve_stream_failover_budget_ms(None), 90_000);
        assert_eq!(
            resolve_stream_failover_budget_ms(Some(
                &json!({"failover_rules": {"stream_failover_budget_ms": 1234}})
            )),
            1234
        );
    }
}
