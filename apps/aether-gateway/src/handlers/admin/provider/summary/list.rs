use crate::handlers::admin::request::AdminAppState;
use crate::handlers::admin::shared::unix_secs_to_rfc3339;
use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogEndpoint;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) async fn build_admin_providers_payload(
    state: &AdminAppState<'_>,
    skip: usize,
    limit: usize,
    is_active: Option<bool>,
) -> Option<serde_json::Value> {
    let state = state.as_ref();
    if !state.has_provider_catalog_data_reader() {
        return None;
    }

    let active_only = is_active.unwrap_or(false);
    let mut providers = state
        .list_provider_catalog_providers(active_only)
        .await
        .ok()
        .unwrap_or_default();
    if matches!(is_active, Some(false)) {
        providers.retain(|provider| !provider.is_active);
    }
    // R11-5: mirror the scheduler's view — the provider list is ordered by
    // the default routing group's provider priority overrides (ascending;
    // unconfigured providers sink to the tail, ties break by name), and the
    // resolved priority ships in the payload so the UI shows the same number
    // the scheduler uses.
    let provider_priorities = read_system_default_provider_priorities(state).await;
    providers.sort_by(|left, right| {
        let left_priority = provider_priorities
            .get(&left.id)
            .copied()
            .unwrap_or(i32::MAX);
        let right_priority = provider_priorities
            .get(&right.id)
            .copied()
            .unwrap_or(i32::MAX);
        left_priority
            .cmp(&right_priority)
            .then_with(|| left.name.cmp(&right.name))
    });

    let providers = providers
        .into_iter()
        .skip(skip)
        .take(limit)
        .collect::<Vec<_>>();
    let provider_ids = providers
        .iter()
        .map(|provider| provider.id.clone())
        .collect::<Vec<_>>();
    let endpoints = if provider_ids.is_empty() {
        Vec::new()
    } else {
        state
            .list_provider_catalog_endpoints_by_provider_ids(&provider_ids)
            .await
            .ok()
            .unwrap_or_default()
    };
    let key_stats = if provider_ids.is_empty() {
        Vec::new()
    } else {
        state
            .list_provider_catalog_key_stats_by_provider_ids(&provider_ids)
            .await
            .ok()
            .unwrap_or_default()
    };
    let first_endpoint_by_provider = endpoints
        .into_iter()
        .filter(|endpoint| endpoint.is_active)
        .fold(
            BTreeMap::<String, StoredProviderCatalogEndpoint>::new(),
            |mut acc, endpoint| {
                acc.entry(endpoint.provider_id.clone()).or_insert(endpoint);
                acc
            },
        );
    let has_any_key_by_provider =
        key_stats
            .into_iter()
            .fold(BTreeSet::<String>::new(), |mut acc, stats| {
                if stats.total_keys > 0 {
                    acc.insert(stats.provider_id);
                }
                acc
            });

    Some(serde_json::Value::Array(
        providers
            .into_iter()
            .map(|provider| {
                let provider_id = provider.id.clone();
                let endpoint = first_endpoint_by_provider.get(&provider_id);
                json!({
                    "id": provider_id.clone(),
                    "name": provider.name,
                    "priority": provider_priorities
                        .get(&provider_id)
                        .copied(),
                    "api_format": endpoint.map(|item| item.api_format.clone()),
                    "base_url": endpoint.map(|item| item.base_url.clone()),
                    "api_key": has_any_key_by_provider.contains(&provider_id).then_some("***"),
                    "is_active": provider.is_active,
                    "created_at": provider.created_at_unix_ms.and_then(unix_secs_to_rfc3339),
                    "updated_at": provider.updated_at_unix_secs.and_then(unix_secs_to_rfc3339),
                })
            })
            .collect(),
    ))
}

async fn read_system_default_provider_priorities(
    state: &crate::AppState,
) -> std::collections::HashMap<String, i32> {
    use aether_data_contracts::repository::routing_profiles::RoutingGroupLookupKey;
    let Ok(Some(group)) = state
        .find_routing_group(RoutingGroupLookupKey::SystemDefault)
        .await
    else {
        return std::collections::HashMap::new();
    };
    let config = match serde_json::from_value::<aether_routing_core::RoutingGroupConfig>(
        group.config_json,
    ) {
        Ok(config) => config,
        Err(_) => return std::collections::HashMap::new(),
    };
    config
        .rules
        .iter()
        .flat_map(|rule| rule.actions.iter())
        .filter_map(|action| match action {
            aether_routing_core::RoutingAction::SetProviderPriority {
                provider_id,
                priority,
            } => Some((provider_id.clone(), *priority)),
            _ => None,
        })
        .collect()
}
