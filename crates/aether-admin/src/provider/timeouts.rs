use aether_contracts::{
    chat_retry::STREAM_TOTAL_TIMEOUT_CONFIG_KEY, MAX_EXECUTION_REQUEST_TIMEOUT_MS,
};
use serde_json::{json, Map, Value};

/// Validate direct config writes before applying the API's seconds override.
/// A present null removes the override; an absent field preserves the config value.
pub fn normalize_stream_total_timeout(
    config: &mut Map<String, Value>,
    seconds: Option<Option<f64>>,
) -> Result<(), String> {
    if let Some(value) = config.get(STREAM_TOTAL_TIMEOUT_CONFIG_KEY) {
        if value.is_null() {
            config.remove(STREAM_TOTAL_TIMEOUT_CONFIG_KEY);
        } else if !value
            .as_u64()
            .is_some_and(|value| (1_000..=MAX_EXECUTION_REQUEST_TIMEOUT_MS).contains(&value))
        {
            return Err(
                "config.stream_total_timeout_ms must be an integer from 1000 to 1200000"
                    .to_string(),
            );
        }
    }
    if let Some(seconds) = seconds {
        match seconds {
            Some(seconds) => {
                let milliseconds = (seconds * 1_000.0).round();
                if !(1.0..=MAX_EXECUTION_REQUEST_TIMEOUT_MS as f64 / 1_000.0).contains(&seconds)
                    || milliseconds / 1_000.0 != seconds
                {
                    return Err("stream_total_timeout must be from 1 to 1200 seconds with at most three decimal places".to_string());
                }
                config.insert(
                    STREAM_TOTAL_TIMEOUT_CONFIG_KEY.to_string(),
                    json!(milliseconds as u64),
                );
            }
            None => {
                config.remove(STREAM_TOTAL_TIMEOUT_CONFIG_KEY);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_contracts::chat_retry::resolve_stream_total_timeout_ms;

    #[test]
    fn stream_total_timeout_save_clear_preserves_unrelated_config() {
        let original = json!({"future": {"enabled": true}, "failover_rules": {"stream_failover_budget_ms": 1800}});
        let mut config = original.as_object().unwrap().clone();
        normalize_stream_total_timeout(&mut config, Some(Some(5.001))).unwrap();
        assert_eq!(config[STREAM_TOTAL_TIMEOUT_CONFIG_KEY], 5001);
        normalize_stream_total_timeout(&mut config, None).unwrap();
        assert_eq!(
            resolve_stream_total_timeout_ms(Some(&Value::Object(config.clone()))),
            5001
        );
        normalize_stream_total_timeout(&mut config, Some(None)).unwrap();
        assert_eq!(Value::Object(config.clone()), original);
        assert_eq!(
            resolve_stream_total_timeout_ms(Some(&Value::Object(config))),
            900_000
        );
    }

    #[test]
    fn stream_total_timeout_validates_seconds_and_direct_config_consistently() {
        for value in [0.0, 0.999, 1200.001, 1.0001, f64::NAN, f64::INFINITY] {
            assert!(normalize_stream_total_timeout(&mut Map::new(), Some(Some(value))).is_err());
        }
        for value in [
            json!(999),
            json!(1200001),
            json!(1000.5),
            json!("5000"),
            json!(true),
        ] {
            let mut config = Map::from_iter([(STREAM_TOTAL_TIMEOUT_CONFIG_KEY.to_string(), value)]);
            assert!(normalize_stream_total_timeout(&mut config, None).is_err());
        }
        for milliseconds in [1000, 1001, 900000, 1200000] {
            let mut config = Map::from_iter([(
                STREAM_TOTAL_TIMEOUT_CONFIG_KEY.to_string(),
                json!(milliseconds),
            )]);
            normalize_stream_total_timeout(&mut config, None).unwrap();
            assert_eq!(config[STREAM_TOTAL_TIMEOUT_CONFIG_KEY], milliseconds);
        }
        let mut config =
            Map::from_iter([(STREAM_TOTAL_TIMEOUT_CONFIG_KEY.to_string(), Value::Null)]);
        normalize_stream_total_timeout(&mut config, None).unwrap();
        assert!(config.is_empty());
    }
}
