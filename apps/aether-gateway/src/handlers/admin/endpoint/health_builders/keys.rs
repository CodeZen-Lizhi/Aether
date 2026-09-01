use crate::handlers::admin::request::AdminAppState;
use crate::provider_key_auth::provider_key_effective_api_formats;
use aether_scheduler_core::{
    count_recent_rpm_requests_for_provider_key_since,
    provider_key_circuit_payload_is_active_open_at,
};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) async fn build_admin_key_rpm_payload(
    state: &AdminAppState<'_>,
    key_id: &str,
) -> Option<serde_json::Value> {
    if !state.has_provider_catalog_data_reader() {
        return None;
    }

    let key = state
        .read_provider_catalog_keys_by_ids(&[key_id.to_string()])
        .await
        .ok()
        .and_then(|mut keys| keys.drain(..).next())?;
    let now_unix_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let recent_candidates = state.read_recent_request_candidates(256).await.ok()?;
    let reset_after_unix_secs = state.provider_key_rpm_reset_at(key.id.as_str(), now_unix_secs);
    let current_rpm = count_recent_rpm_requests_for_provider_key_since(
        &recent_candidates,
        key.id.as_str(),
        now_unix_secs,
        reset_after_unix_secs,
    );

    Some(json!({
        "key_id": key.id,
        "current_rpm": current_rpm,
        "rpm_limit": key.rpm_limit,
    }))
}

fn default_key_health_payload() -> serde_json::Value {
    json!({
        "health_score": 1.0,
        "consecutive_failures": 0,
        "last_failure_at": serde_json::Value::Null,
    })
}

fn default_key_circuit_payload() -> serde_json::Value {
    json!({
        "open": false,
        "open_at": serde_json::Value::Null,
        "next_probe_at": serde_json::Value::Null,
        "half_open_until": serde_json::Value::Null,
        "half_open_successes": 0,
        "half_open_failures": 0,
    })
}

pub(crate) async fn recover_admin_key_health(
    state: &AdminAppState<'_>,
    key_id: &str,
    api_format: Option<&str>,
) -> Option<serde_json::Value> {
    let key = state
        .read_provider_catalog_keys_by_ids(&[key_id.to_string()])
        .await
        .ok()
        .and_then(|mut keys| keys.drain(..).next())?;

    let api_format = api_format.map(str::trim).filter(|value| !value.is_empty());
    let (health_by_format, circuit_breaker_by_format, message, details) =
        if let Some(api_format) = api_format {
            let mut health_by_format = key
                .health_by_format
                .as_ref()
                .and_then(serde_json::Value::as_object)
                .cloned()
                .unwrap_or_default();
            let mut circuit_breaker_by_format = key
                .circuit_breaker_by_format
                .as_ref()
                .and_then(serde_json::Value::as_object)
                .cloned()
                .unwrap_or_default();
            health_by_format.insert(api_format.to_string(), default_key_health_payload());
            circuit_breaker_by_format.insert(api_format.to_string(), default_key_circuit_payload());
            (
                serde_json::Value::Object(health_by_format),
                serde_json::Value::Object(circuit_breaker_by_format),
                format!("Key 的 {api_format} 格式已恢复"),
                json!({
                    "api_format": api_format,
                    "health_score": 1.0,
                    "circuit_breaker_open": false,
                    "is_active": true,
                }),
            )
        } else {
            (
                json!({}),
                json!({}),
                "Key 所有格式已恢复".to_string(),
                json!({
                    "health_score": 1.0,
                    "circuit_breaker_open": false,
                    "is_active": true,
                }),
            )
        };

    let updated = state
        .update_provider_catalog_key_health_state(
            key_id,
            true,
            Some(&health_by_format),
            Some(&circuit_breaker_by_format),
        )
        .await
        .ok()?;
    if !updated {
        return None;
    }

    Some(json!({
        "message": message,
        "details": details,
    }))
}

pub(crate) async fn recover_all_admin_key_health(
    state: &AdminAppState<'_>,
) -> Option<serde_json::Value> {
    if !state.has_provider_catalog_data_reader() {
        return None;
    }

    let providers = state
        .list_provider_catalog_providers(false)
        .await
        .ok()
        .unwrap_or_default();
    let provider_ids = providers
        .iter()
        .map(|provider| provider.id.clone())
        .collect::<Vec<_>>();
    let keys = if provider_ids.is_empty() {
        Vec::new()
    } else {
        state
            .list_provider_catalog_key_summaries_by_provider_ids(&provider_ids)
            .await
            .ok()
            .unwrap_or_default()
    };

    let recovered_keys = keys
        .into_iter()
        .filter(|key| {
            key.circuit_breaker_by_format
                .as_ref()
                .and_then(serde_json::Value::as_object)
                .map(|formats| {
                    formats.values().any(|circuit| {
                        circuit
                            .get("open")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    if recovered_keys.is_empty() {
        return Some(json!({
            "message": "没有需要恢复的 Key",
            "recovered_count": 0,
            "recovered_keys": [],
        }));
    }

    let empty_health = json!({});
    let empty_circuit = json!({});
    let mut payload_items = Vec::new();
    for key in recovered_keys {
        let updated = state
            .update_provider_catalog_key_health_state(
                &key.id,
                true,
                Some(&empty_health),
                Some(&empty_circuit),
            )
            .await
            .ok()?;
        if !updated {
            continue;
        }
        let provider = state
            .read_provider_catalog_providers_by_ids(std::slice::from_ref(&key.provider_id))
            .await
            .ok()
            .and_then(|mut providers| providers.drain(..).next());
        let endpoints = state
            .list_provider_catalog_endpoints_by_provider_ids(std::slice::from_ref(&key.provider_id))
            .await
            .ok()
            .unwrap_or_default();
        let api_formats = provider
            .as_ref()
            .map(|provider| provider_key_effective_api_formats(&key))
            .unwrap_or_default();
        payload_items.push(json!({
            "key_id": key.id,
            "key_name": key.name,
            "provider_id": key.provider_id,
            "api_formats": api_formats,
        }));
    }

    Some(json!({
        "message": format!("已恢复 {} 个 Key", payload_items.len()),
        "recovered_count": payload_items.len(),
        "recovered_keys": payload_items,
    }))
}
