use serde::Serialize;
use serde_json::Value;

pub const CHAT_POLICY_VERSION: u32 = 1;
pub const DEFAULT_STREAM_FAILOVER_BUDGET_MS: u64 = 90_000;

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
