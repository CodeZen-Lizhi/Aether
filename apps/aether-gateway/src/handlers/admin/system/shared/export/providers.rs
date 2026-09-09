use super::support::{
    collect_admin_system_export_provider_endpoint_formats,
    decrypt_admin_system_export_provider_config, decrypt_admin_system_export_secret,
    read_admin_backup_string_list, resolve_admin_system_export_key_api_formats,
};
use crate::handlers::admin::request::AdminBackupState;
use crate::GatewayError;
use aether_admin::system::{
    AdminSystemConfigEndpoint, AdminSystemConfigProvider, AdminSystemConfigProviderKey,
    AdminSystemConfigProviderModel,
};
use std::collections::BTreeMap;

pub(crate) async fn build_admin_system_export_providers_payload(
    state: &AdminBackupState<'_>,
    global_model_name_by_id: &BTreeMap<String, String>,
) -> Result<Vec<AdminSystemConfigProvider>, GatewayError> {
    let providers = state.list_provider_catalog_providers(false).await?;
    let provider_ids = providers
        .iter()
        .map(|provider| provider.id.clone())
        .collect::<Vec<_>>();
    let endpoints = state
        .list_provider_catalog_endpoints_by_provider_ids(&provider_ids)
        .await?;
    let keys = state
        .list_provider_catalog_keys_by_provider_ids(&provider_ids)
        .await?;

    let mut endpoints_by_provider = BTreeMap::<String, Vec<_>>::new();
    for endpoint in endpoints {
        endpoints_by_provider
            .entry(endpoint.provider_id.clone())
            .or_default()
            .push(endpoint);
    }
    let mut keys_by_provider = BTreeMap::<String, Vec<_>>::new();
    for key in keys {
        keys_by_provider
            .entry(key.provider_id.clone())
            .or_default()
            .push(key);
    }

    let mut provider_models_by_provider = BTreeMap::<String, Vec<_>>::new();
    for provider in &providers {
        let models = state
            .list_admin_system_backup_provider_models(&provider.id)
            .await?;
        provider_models_by_provider.insert(provider.id.clone(), models);
    }

    providers
        .iter()
        .map(|provider| -> Result<_, GatewayError> {
            let endpoints = endpoints_by_provider
                .remove(&provider.id)
                .unwrap_or_default();
            let provider_endpoint_formats =
                collect_admin_system_export_provider_endpoint_formats(&endpoints);
            let endpoints_data = endpoints
                .iter()
                .map(|endpoint| AdminSystemConfigEndpoint {
                    api_format: endpoint.api_format.clone(),
                    base_url: endpoint.base_url.clone(),
                    header_rules: endpoint.header_rules.clone(),
                    body_rules: endpoint.body_rules.clone(),
                    max_retries: endpoint.max_retries,
                    is_active: endpoint.is_active,
                    custom_path: endpoint.custom_path.clone(),
                    config: endpoint.config.clone(),
                    format_acceptance_config: endpoint.format_acceptance_config.clone(),
                    proxy: endpoint.proxy.clone(),
                })
                .collect::<Vec<_>>();

            let mut keys = keys_by_provider.remove(&provider.id).unwrap_or_default();
            keys.sort_by(|left, right| {
                left.created_at_unix_ms
                    .unwrap_or(0)
                    .cmp(&right.created_at_unix_ms.unwrap_or(0))
                    .then(left.id.cmp(&right.id))
            });
            let keys_data = keys
                .iter()
                .map(|key| -> Result<_, GatewayError> {
                    let api_formats = resolve_admin_system_export_key_api_formats(
                        key.api_formats.as_ref(),
                        &provider_endpoint_formats,
                    )?;
                    let auth_config = key
                        .encrypted_auth_config
                        .as_deref()
                        .map(|ciphertext| {
                            decrypt_admin_system_export_secret(state.admin(), ciphertext)
                        })
                        .transpose()?
                        .map(|plaintext| serde_json::from_str::<serde_json::Value>(&plaintext))
                        .transpose()
                        .map_err(|_| {
                            GatewayError::Internal("渠道认证配置不是有效 JSON".to_string())
                        })?;
                    Ok(AdminSystemConfigProviderKey {
                        id: Some(key.id.clone()),
                        api_key: key
                            .encrypted_api_key
                            .as_deref()
                            .map(|ciphertext| {
                                decrypt_admin_system_export_secret(state.admin(), ciphertext)
                            })
                            .transpose()?,
                        auth_type: Some(key.auth_type.clone()),
                        auth_config,
                        name: Some(key.name.clone()),
                        note: key.note.clone(),
                        api_formats: Some(api_formats),
                        rate_multipliers: key.rate_multipliers.clone(),
                        internal_priority: Some(key.internal_priority),
                        default_rate_multiplier: Some(key.default_rate_multiplier),
                        auth_type_by_format: key.auth_type_by_format.clone(),
                        allow_auth_channel_mismatch_formats: read_admin_backup_string_list(
                            key.allow_auth_channel_mismatch_formats.as_ref(),
                            "provider_api_keys.allow_auth_channel_mismatch_formats",
                        )?,
                        rpm_limit: key.rpm_limit,
                        concurrent_limit: key.concurrent_limit,
                        expires_at_unix_secs: key.expires_at_unix_secs,
                        allowed_models: read_admin_backup_string_list(
                            key.allowed_models.as_ref(),
                            "provider_api_keys.allowed_models",
                        )?,
                        capabilities: key.capabilities.clone(),
                        cache_ttl_minutes: Some(key.cache_ttl_minutes),
                        max_probe_interval_minutes: Some(key.max_probe_interval_minutes),
                        auto_fetch_models: Some(key.auto_fetch_models),
                        locked_models: read_admin_backup_string_list(
                            key.locked_models.as_ref(),
                            "provider_api_keys.locked_models",
                        )?,
                        model_include_patterns: read_admin_backup_string_list(
                            key.model_include_patterns.as_ref(),
                            "provider_api_keys.model_include_patterns",
                        )?,
                        model_exclude_patterns: read_admin_backup_string_list(
                            key.model_exclude_patterns.as_ref(),
                            "provider_api_keys.model_exclude_patterns",
                        )?,
                        is_active: key.is_active,
                        proxy: key.proxy.clone(),
                        fingerprint: key.fingerprint.clone(),
                        learned_rpm_limit: key.learned_rpm_limit,
                        concurrent_429_count: key.concurrent_429_count,
                        rpm_429_count: key.rpm_429_count,
                        last_429_at_unix_secs: key.last_429_at_unix_secs,
                        last_429_type: key.last_429_type.clone(),
                        adjustment_history: key.adjustment_history.clone(),
                        utilization_samples: key.utilization_samples.clone(),
                        last_probe_increase_at_unix_secs: key.last_probe_increase_at_unix_secs,
                        last_rpm_peak: key.last_rpm_peak,
                        request_count: key.request_count,
                        total_tokens: Some(key.total_tokens),
                        total_cost_usd: Some(key.total_cost_usd),
                        success_count: key.success_count,
                        error_count: key.error_count,
                        total_response_time_ms: key.total_response_time_ms,
                        last_used_at_unix_secs: key.last_used_at_unix_secs,
                        last_models_fetch_at_unix_secs: key.last_models_fetch_at_unix_secs,
                        last_models_fetch_error: key.last_models_fetch_error.clone(),
                        upstream_metadata: key.upstream_metadata.clone(),
                        oauth_invalid_at_unix_secs: key.oauth_invalid_at_unix_secs,
                        oauth_invalid_reason: key.oauth_invalid_reason.clone(),
                        status_snapshot: key.status_snapshot.clone(),
                        health_by_format: key.health_by_format.clone(),
                        circuit_breaker_by_format: key.circuit_breaker_by_format.clone(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            let models_data = provider_models_by_provider
                .remove(&provider.id)
                .unwrap_or_default()
                .into_iter()
                .map(|model| AdminSystemConfigProviderModel {
                    global_model_name: global_model_name_by_id.get(&model.global_model_id).cloned(),
                    provider_model_name: model.provider_model_name,
                    provider_model_mappings: model.provider_model_mappings,
                    price_per_request: model.price_per_request,
                    tiered_pricing: model.tiered_pricing,
                    supports_vision: model.supports_vision,
                    supports_function_calling: model.supports_function_calling,
                    supports_streaming: model.supports_streaming,
                    supports_extended_thinking: model.supports_extended_thinking,
                    supports_image_generation: model.supports_image_generation,
                    is_active: model.is_active,
                    config: model.config,
                })
                .collect::<Vec<_>>();

            Ok(AdminSystemConfigProvider {
                id: Some(provider.id.clone()),
                name: provider.name.clone(),
                description: provider.description.clone(),
                website: provider.website.clone(),
                provider_type: Some(provider.provider_type.clone()),
                billing_type: provider.billing_type.clone(),
                monthly_quota_usd: provider.monthly_quota_usd,
                monthly_used_usd: provider.monthly_used_usd,
                quota_reset_day: provider.quota_reset_day,
                quota_last_reset_at_unix_secs: provider.quota_last_reset_at_unix_secs,
                quota_expires_at_unix_secs: provider.quota_expires_at_unix_secs,
                enable_format_conversion: Some(provider.enable_format_conversion),
                is_active: provider.is_active,
                concurrent_limit: provider.concurrent_limit,
                max_retries: provider.max_retries,
                stream_first_byte_timeout: provider.stream_first_byte_timeout_secs,
                request_timeout: provider.request_timeout_secs,
                proxy: provider.proxy.clone(),
                config: decrypt_admin_system_export_provider_config(
                    state.admin(),
                    provider.config.as_ref(),
                )?,
                endpoints: endpoints_data,
                api_keys: keys_data,
                models: models_data,
            })
        })
        .collect()
}
