use super::enabled_key_capability_short_names;
use crate::handlers::shared::unix_secs_to_rfc3339;
use crate::provider_key_auth::provider_key_effective_api_formats;
use crate::AppState;
use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey;
use aether_scheduler_core::provider_key_circuit_payload_is_active_open_at;
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};

fn grouped_key_masked_label(
    _state: &AppState,
    key: &StoredProviderCatalogKey,
    _provider_type: &str,
) -> &'static str {
    match key.auth_type.trim() {
        "service_account" | "vertex_ai" => "[Service Account]",
        "oauth" => "[OAuth Token]",
        _ => "[API Key]",
    }
}

pub(crate) async fn build_admin_keys_grouped_by_format_payload(
    state: &AppState,
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
    let provider_metadata_by_id = providers
        .iter()
        .map(|provider| {
            (
                provider.id.clone(),
                (provider.name.clone(), provider.is_active),
            )
        })
        .collect::<HashMap<_, _>>();

    let (endpoints_result, keys_result) = tokio::join!(
        state.list_provider_catalog_endpoints_by_provider_ids(&provider_ids),
        state.list_provider_catalog_keys_by_provider_ids(&provider_ids),
    );

    let active_endpoints = endpoints_result
        .ok()
        .unwrap_or_default()
        .into_iter()
        .filter(|endpoint| endpoint.is_active)
        .collect::<Vec<_>>();
    let endpoint_base_url_by_provider_and_format = active_endpoints
        .iter()
        .map(|endpoint| {
            (
                (endpoint.provider_id.clone(), endpoint.api_format.clone()),
                endpoint.base_url.clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut endpoints_by_provider = HashMap::<String, Vec<_>>::new();
    for endpoint in active_endpoints {
        endpoints_by_provider
            .entry(endpoint.provider_id.clone())
            .or_default()
            .push(endpoint);
    }
    let provider_active_api_formats_by_provider: HashMap<String, Vec<String>> =
        endpoints_by_provider
            .iter()
            .map(|(provider_id, endpoints)| {
                (
                    provider_id.clone(),
                    crate::provider_key_auth::provider_active_api_formats(endpoints),
                )
            })
            .collect();

    let mut keys = keys_result.ok().unwrap_or_default();
    keys.sort_by(|left, right| left.id.cmp(&right.id));

    let now_unix_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    let mut grouped = BTreeMap::<String, Vec<serde_json::Value>>::new();
    for key in keys {
        let Some((provider_name, provider_is_active)) = provider_metadata_by_id.get(&key.provider_id)
        else {
            continue;
        };
        let request_count = u64::from(key.request_count.unwrap_or(0));
        let success_count = u64::from(key.success_count.unwrap_or(0));
        let success_rate = if request_count > 0 {
            Some(success_count as f64 / request_count as f64)
        } else {
            None
        };
        let avg_response_time_ms = if success_count > 0 {
            Some(key.total_response_time_ms.unwrap_or(0) as f64 / success_count as f64)
        } else {
            None
        };
        let health_by_format = key
            .health_by_format
            .as_ref()
            .and_then(serde_json::Value::as_object)
            .cloned()
            .unwrap_or_default();
        let circuit_by_format = key
            .circuit_breaker_by_format
            .as_ref()
            .and_then(serde_json::Value::as_object)
            .cloned()
            .unwrap_or_default();
        let capability_names = enabled_key_capability_short_names(key.capabilities.as_ref());
        // R11-6: keys without a configured `api_formats` list used to vanish
        // from this view entirely. Fall back to the provider's active endpoint
        // formats (the same join the scheduler uses to serve them) so the
        // view shows every key that can actually take traffic. Selection is
        // unaffected either way — this is display parity only.
        let api_formats = match provider_key_effective_api_formats(&key) {
            formats if !formats.is_empty() => formats,
            _ => provider_active_api_formats_by_provider
                .get(&key.provider_id)
                .cloned()
                .unwrap_or_default(),
        };
        if api_formats.is_empty() {
            continue;
        }

        for api_format in &api_formats {
            let format_health = health_by_format
                .get(api_format)
                .cloned()
                .unwrap_or_else(|| json!({}));
            let format_circuit = circuit_by_format
                .get(api_format)
                .cloned()
                .unwrap_or_else(|| json!({}));
            grouped.entry(api_format.clone()).or_default().push(json!({
                "id": key.id,
                "provider_id": key.provider_id,
                "name": key.name,
                "auth_type": key.auth_type,
                "api_key_masked": grouped_key_masked_label(state, &key, provider_name),
                "rate_multipliers": key.rate_multipliers,
                "default_rate_multiplier": key.default_rate_multiplier,
                "is_active": key.is_active,
                "provider_active": provider_is_active,
                "provider_name": provider_name,
                "api_formats": api_formats,
                "capabilities": capability_names,
                "success_rate": success_rate,
                "avg_response_time_ms": avg_response_time_ms,
                "request_count": request_count,
                "api_format": api_format,
                "endpoint_base_url": endpoint_base_url_by_provider_and_format
                    .get(&(key.provider_id.clone(), api_format.clone()))
                    .cloned(),
                "health_score": format_health
                    .get("health_score")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(1.0),
                "circuit_breaker_open": provider_key_circuit_payload_is_active_open_at(
                    &format_circuit,
                    now_unix_secs,
                ),
                "last_used_at": key.last_used_at_unix_secs.and_then(unix_secs_to_rfc3339),
                "created_at": unix_secs_to_rfc3339(key.created_at_unix_ms.unwrap_or(now_unix_secs)),
                "updated_at": unix_secs_to_rfc3339(key.updated_at_unix_secs.unwrap_or(now_unix_secs)),
            }));
        }
    }

    Some(serde_json::Value::Object(
        grouped
            .into_iter()
            .map(|(format, items)| (format, serde_json::Value::Array(items)))
            .collect(),
    ))
}
