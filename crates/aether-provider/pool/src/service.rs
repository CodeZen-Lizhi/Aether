use std::collections::BTreeMap;
use std::sync::Arc;

use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogEndpoint;

use crate::capability::ProviderPoolCapability;
use crate::provider::ProviderPoolAdapter;
use crate::providers::DefaultProviderPoolAdapter;

#[derive(Clone)]
pub struct ProviderPoolService {
    adapters: BTreeMap<String, Arc<dyn ProviderPoolAdapter>>,
    default_adapter: Arc<dyn ProviderPoolAdapter>,
}

impl std::fmt::Debug for ProviderPoolService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderPoolService")
            .field("provider_types", &self.adapters.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Default for ProviderPoolService {
    fn default() -> Self {
        Self {
            adapters: BTreeMap::new(),
            default_adapter: Arc::new(DefaultProviderPoolAdapter),
        }
    }
}

impl ProviderPoolService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_adapter(mut self, adapter: Arc<dyn ProviderPoolAdapter>) -> Self {
        self.adapters
            .insert(adapter.provider_type().trim().to_ascii_lowercase(), adapter);
        self
    }

    pub fn adapter(&self, provider_type: &str) -> Arc<dyn ProviderPoolAdapter> {
        self.adapters
            .get(provider_type.trim().to_ascii_lowercase().as_str())
            .cloned()
            .unwrap_or_else(|| self.default_adapter.clone())
    }

    pub fn provider_types(&self) -> impl Iterator<Item = &str> {
        self.adapters.keys().map(String::as_str)
    }

    pub fn provider_types_for_capability(&self, capability: ProviderPoolCapability) -> Vec<String> {
        self.adapters
            .iter()
            .filter(|(_, adapter)| adapter.capabilities().supports(capability))
            .map(|(provider_type, _)| provider_type.clone())
            .collect()
    }

    pub fn supports_quota_refresh(&self, provider_type: &str) -> bool {
        self.adapter(provider_type).supports_quota_refresh()
    }

    pub fn quota_refresh_endpoint_for_provider(
        &self,
        provider_type: &str,
        endpoints: &[StoredProviderCatalogEndpoint],
        include_inactive: bool,
    ) -> Option<StoredProviderCatalogEndpoint> {
        self.adapter(provider_type)
            .quota_refresh_endpoint(endpoints, include_inactive)
    }

    pub fn quota_refresh_unsupported_message(&self, provider_type: &str) -> String {
        self.adapter(provider_type)
            .quota_refresh_unsupported_message()
    }

    pub fn quota_refresh_missing_endpoint_message(&self, provider_type: &str) -> String {
        self.adapter(provider_type)
            .quota_refresh_missing_endpoint_message()
    }
}
