use aether_contracts::chat_retry::CHAT_POLICY_VERSION;
use serde_json::{json, Map, Value};

pub fn validate_legacy_attempts(value: i32) -> Result<i32, String> {
    if !(0..=99).contains(&value) {
        return Err("max_retries must be an integer from 0 to 99".to_string());
    }
    Ok(value.max(1))
}

pub fn validate_scope_aliases(
    rules: Option<&Value>,
    field: &str,
    legacy: Option<i32>,
) -> Result<(), String> {
    if let Some(canonical) = rules.and_then(|rules| rules.get(field)) {
        let normalized = legacy
            .map(validate_legacy_attempts)
            .transpose()?
            .map(i64::from);
        if canonical.as_i64() != normalized || (canonical.is_null() && legacy.is_some()) {
            return Err(format!("{field} conflicts with max_retries"));
        }
    }
    Ok(())
}

/// Merge only supplied fields so older clients cannot erase new policy settings.
pub fn merge_failover_rules(existing: Option<&Value>, patch: Value) -> Result<Value, String> {
    let patch = patch
        .as_object()
        .ok_or("failover_rules must be an object")?;
    let mut rules = existing
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (key, value) in patch {
        if value.is_null() {
            rules.remove(key);
        } else {
            rules.insert(key.clone(), value.clone());
        }
    }
    for field in ["max_attempts", "provider_max_attempts"] {
        if let Some(value) = rules.get(field) {
            if !value
                .as_u64()
                .is_some_and(|value| (1..=99).contains(&value))
            {
                return Err(format!("{field} must be an integer from 1 to 99"));
            }
        }
    }
    if let Some(value) = patch.get("max_retries").filter(|value| !value.is_null()) {
        let legacy = value
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
            .ok_or("max_retries must be an integer from 0 to 99")?;
        let normalized = validate_legacy_attempts(legacy)?;
        if patch
            .get("max_attempts")
            .filter(|value| !value.is_null())
            .is_some_and(|value| value.as_i64() != Some(i64::from(normalized)))
        {
            return Err("max_attempts conflicts with max_retries".to_string());
        }
        rules.insert("max_attempts".to_string(), json!(normalized));
    }
    if let Some(value) = rules.get("stream_failover_budget_ms") {
        if !value.as_u64().is_some_and(|value| {
            (1..=aether_contracts::MAX_EXECUTION_REQUEST_TIMEOUT_MS).contains(&value)
        }) {
            return Err(
                "stream_failover_budget_ms must be an integer from 1 to 1200000".to_string(),
            );
        }
    }
    if let Some(value) = patch.get("chat_policy_version") {
        if value.as_u64() != Some(u64::from(CHAT_POLICY_VERSION)) {
            return Err("unsupported chat_policy_version".to_string());
        }
    }
    rules.insert(
        "chat_policy_version".to_string(),
        json!(CHAT_POLICY_VERSION),
    );
    Ok(Value::Object(rules))
}

pub fn normalize_config_rules(config: &mut Map<String, Value>) -> Result<(), String> {
    if let Some(rules) = config.get("failover_rules").cloned() {
        config.insert(
            "failover_rules".to_string(),
            merge_failover_rules(None, rules)?,
        );
    }
    Ok(())
}

pub fn set_scope_attempts(
    config: &mut Map<String, Value>,
    field: &str,
    value: Option<i32>,
) -> Result<(), String> {
    let normalized = value.map(validate_legacy_attempts).transpose()?;
    let mut rules = config
        .get("failover_rules")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(value) = normalized {
        rules.insert(field.to_string(), json!(value));
    } else {
        rules.remove(field);
    }
    rules.insert(
        "chat_policy_version".to_string(),
        json!(CHAT_POLICY_VERSION),
    );
    config.insert("failover_rules".to_string(), Value::Object(rules));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_preserves_unknown_rules_and_normalizes_old_writes() {
        let original = json!({"custom": {"future": true}, "stream_failover_budget_ms": 1200, "stop_on_status_codes": [400]});
        let result = merge_failover_rules(Some(&original), json!({"max_retries": 2})).unwrap();
        assert_eq!(result["max_attempts"], 2);
        assert_eq!(result["custom"], original["custom"]);
        assert_eq!(
            result["stop_on_status_codes"],
            original["stop_on_status_codes"]
        );
        assert_eq!(result["stream_failover_budget_ms"], 1200);
        assert_eq!(
            merge_failover_rules(Some(&result), result.clone()).unwrap(),
            result
        );
    }

    #[test]
    fn rejects_conflicting_or_invalid_writes() {
        for patch in [
            json!({"max_attempts": 0}),
            json!({"max_attempts": 100}),
            json!({"max_attempts": 2.5}),
            json!({"stream_failover_budget_ms": 0}),
            json!({"stream_failover_budget_ms": 1200001}),
            json!({"max_attempts": 2, "max_retries": 3}),
        ] {
            assert!(merge_failover_rules(None, patch).is_err());
        }
        assert_eq!(
            merge_failover_rules(None, json!({"max_retries": 0})).unwrap()["max_attempts"],
            1
        );
    }

    #[test]
    fn canonical_chat_edit_preserves_the_legacy_non_chat_value() {
        let existing = json!({"max_retries": 2});
        let result = merge_failover_rules(Some(&existing), json!({"max_attempts": 3})).unwrap();
        assert_eq!(result["max_attempts"], 3);
        assert_eq!(result["max_retries"], 2);
    }
}
