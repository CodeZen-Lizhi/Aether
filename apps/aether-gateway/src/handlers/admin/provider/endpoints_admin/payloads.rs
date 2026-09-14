use crate::handlers::admin::shared::AdminTypedObjectPatch;
use aether_admin::provider::endpoints as admin_provider_endpoints_pure;
use aether_data_contracts::repository::provider_catalog::{
    StoredProviderCatalogEndpoint, StoredProviderCatalogKey, StoredProviderCatalogProvider,
};
use serde::Deserialize;

pub(super) fn key_api_formats_without_entry(
    key: &StoredProviderCatalogKey,
    api_format: &str,
) -> Option<Vec<String>> {
    admin_provider_endpoints_pure::key_api_formats_without_entry(key, api_format)
}

pub(super) fn endpoint_key_counts_by_format(
    provider_type: &str,
    endpoints: &[StoredProviderCatalogEndpoint],
    keys: &[StoredProviderCatalogKey],
) -> (
    std::collections::BTreeMap<String, usize>,
    std::collections::BTreeMap<String, usize>,
) {
    admin_provider_endpoints_pure::endpoint_key_counts_by_format(provider_type, endpoints, keys)
}

pub(super) fn normalize_endpoint_api_format(api_format: &str) -> String {
    admin_provider_endpoints_pure::normalize_endpoint_api_format(api_format)
}

pub(super) fn build_admin_provider_endpoint_response(
    endpoint: &StoredProviderCatalogEndpoint,
    provider: &StoredProviderCatalogProvider,
    total_keys: usize,
    active_keys: usize,
    now_unix_secs: u64,
) -> serde_json::Value {
    let mut response = admin_provider_endpoints_pure::build_admin_provider_endpoint_response(
        endpoint,
        &provider.name,
        total_keys,
        active_keys,
        now_unix_secs,
    );
    let legacy = aether_contracts::chat_retry::resolve_legacy_max_attempts(
        provider.config.as_ref(),
        endpoint.max_retries,
        provider.max_retries,
    );
    let chat = crate::orchestration::chat_health_policy_applies(&endpoint.api_format);
    let attempts = if chat {
        aether_contracts::chat_retry::resolve_chat_max_attempts(
            provider.config.as_ref(),
            endpoint.config.as_ref(),
            endpoint.max_retries,
            provider.max_retries,
        )
    } else {
        legacy
    };
    response["effective_max_attempts"] = serde_json::json!(attempts.max_attempts);
    response["effective_max_attempts_source"] = serde_json::json!(attempts.source);
    if chat {
        response["chat_policy_version"] =
            serde_json::json!(aether_contracts::chat_retry::CHAT_POLICY_VERSION);
        response["legacy_effective_max_attempts"] = serde_json::json!(legacy.max_attempts);
    }
    response
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminProviderEndpointCreateRequest {
    pub(crate) provider_id: String,
    pub(crate) api_format: String,
    pub(crate) base_url: String,
    #[serde(default)]
    pub(crate) custom_path: Option<String>,
    #[serde(default)]
    pub(crate) header_rules: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) body_rules: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) max_retries: Option<i32>,
    #[serde(default)]
    pub(crate) config: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) proxy: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) format_acceptance_config: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminProviderEndpointUpdateRequest {
    #[serde(default)]
    pub(crate) base_url: Option<String>,
    #[serde(default)]
    pub(crate) custom_path: Option<String>,
    #[serde(default)]
    pub(crate) header_rules: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) body_rules: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) max_retries: Option<i32>,
    #[serde(default)]
    pub(crate) is_active: Option<bool>,
    #[serde(default)]
    pub(crate) config: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) proxy: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) format_acceptance_config: Option<serde_json::Value>,
}

pub(crate) type AdminProviderEndpointUpdatePatch =
    AdminTypedObjectPatch<AdminProviderEndpointUpdateRequest>;
