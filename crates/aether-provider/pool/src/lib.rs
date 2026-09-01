mod capability;
mod plan;
mod provider;
mod quota;
mod quota_refresh;
mod service;

pub mod providers;

pub use capability::{ProviderPoolCapabilities, ProviderPoolCapability};
pub use plan::{derive_oauth_plan_type, derive_plan_tier, normalize_provider_plan_tier};
pub use provider::{ProviderPoolAdapter, ProviderPoolMemberInput};
pub use providers::DefaultProviderPoolAdapter;
pub use quota::{
    provider_pool_key_account_quota_exhausted, provider_pool_key_quota_hard_blocked,
    provider_pool_key_scheduling_label, provider_pool_member_quota_snapshot,
    provider_pool_quota_metadata_provider_type, provider_pool_quota_metadata_updated_at,
    provider_pool_quota_snapshot_updated_at,
};
pub use quota_refresh::ProviderPoolQuotaRequestSpec;
pub use service::ProviderPoolService;

#[cfg(test)]
mod tests {
    use super::*;
    use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey;
    use serde_json::{json, Value};

    fn sample_key(upstream_metadata: Option<Value>) -> StoredProviderCatalogKey {
        let mut key = StoredProviderCatalogKey::new(
            "key-1".to_string(),
            "provider-1".to_string(),
            "key-1".to_string(),
            "oauth".to_string(),
            None,
            true,
        )
        .expect("key should build");
        key.upstream_metadata = upstream_metadata;
        key
    }

    #[derive(Debug, Clone, Default)]
    struct TestAdapter;

    impl ProviderPoolAdapter for TestAdapter {
        fn provider_type(&self) -> &'static str {
            "custom"
        }
    }

    #[test]
    fn service_falls_back_to_default_adapter_for_unknown_types() {
        let service = ProviderPoolService::new();

        assert_eq!(
            service.provider_types().collect::<Vec<_>>(),
            Vec::<&str>::new()
        );
        assert_eq!(service.adapter("unknown").provider_type(), "default");
        assert!(!service.supports_quota_refresh("unknown"));

        let service = service.with_adapter(std::sync::Arc::new(TestAdapter));
        assert_eq!(service.provider_types().collect::<Vec<_>>(), ["custom"]);
        assert_eq!(service.adapter("custom").provider_type(), "custom");
        assert_eq!(service.adapter(" CUSTOM ").provider_type(), "custom");
    }

    #[test]
    fn plan_tier_derivation_reads_metadata_bucket() {
        let key = sample_key(Some(json!({
            "custom": {
                "plan_type": "team"
            }
        })));

        assert_eq!(
            derive_oauth_plan_type("custom", &key, None).as_deref(),
            Some("team")
        );
    }

    #[test]
    fn plan_tier_derivation_ignores_api_key_auth() {
        let mut key = sample_key(Some(json!({
            "custom": {
                "plan_type": "team"
            }
        })));
        key.auth_type = "api_key".to_string();

        assert_eq!(derive_oauth_plan_type("custom", &key, None), None);
    }

    #[test]
    fn quota_metadata_provider_type_comes_from_pool_registry() {
        assert_eq!(
            provider_pool_quota_metadata_provider_type(&json!({
                "custom_provider": {
                    "updated_at": 1_700_000_000u64
                }
            }))
            .as_deref(),
            Some("custom_provider")
        );
    }

    #[test]
    fn provider_quota_exhaustion_uses_snapshot_windows() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_secs();

        let mut exhausted = sample_key(None);
        exhausted.status_snapshot = Some(json!({
            "quota": {
                "version": 2,
                "provider_type": "custom",
                "exhausted": true,
                "windows": [{
                    "code": "weekly",
                    "used_ratio": 1.0,
                    "is_exhausted": true
                }]
            }
        }));
        assert!(provider_pool_key_account_quota_exhausted(&exhausted, "custom"));

        let mut expired = sample_key(None);
        expired.status_snapshot = Some(json!({
            "quota": {
                "version": 2,
                "provider_type": "custom",
                "exhausted": true,
                "updated_at": now.saturating_sub(600),
                "windows": [{
                    "code": "5h",
                    "used_ratio": 1.0,
                    "reset_at": now.saturating_sub(60),
                    "is_exhausted": true
                }]
            }
        }));
        assert!(!provider_pool_key_account_quota_exhausted(&expired, "custom"));
    }
}
