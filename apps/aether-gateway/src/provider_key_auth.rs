use aether_data_contracts::repository::provider_catalog::{
    StoredProviderCatalogEndpoint, StoredProviderCatalogKey,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderKeyCredentialKind {
    RawSecret,
    ServiceAccount,
}

impl ProviderKeyCredentialKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::RawSecret => "raw_secret",
            Self::ServiceAccount => "service_account",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderKeyRuntimeAuthKind {
    ApiKey,
    Bearer,
    ServiceAccount,
    Mixed,
    Unknown,
}

impl ProviderKeyRuntimeAuthKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ApiKey => "api_key",
            Self::Bearer => "bearer",
            Self::ServiceAccount => "service_account",
            Self::Mixed => "mixed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderKeyAuthSemantics {
    credential_kind: ProviderKeyCredentialKind,
    runtime_auth_kind: ProviderKeyRuntimeAuthKind,
}

impl ProviderKeyAuthSemantics {
    pub(crate) const fn credential_kind(self) -> ProviderKeyCredentialKind {
        self.credential_kind
    }

    pub(crate) const fn runtime_auth_kind(self) -> ProviderKeyRuntimeAuthKind {
        self.runtime_auth_kind
    }
}

fn normalized_auth_type(key: &StoredProviderCatalogKey) -> String {
    key.auth_type.trim().to_ascii_lowercase()
}

fn key_has_auth_type_overrides(key: &StoredProviderCatalogKey) -> bool {
    key.auth_type_by_format
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .is_some_and(|items| !items.is_empty())
}

pub(crate) fn provider_key_auth_semantics(
    key: &StoredProviderCatalogKey,
) -> ProviderKeyAuthSemantics {
    let auth_type = normalized_auth_type(key);
    let credential_kind = if matches!(auth_type.as_str(), "service_account" | "vertex_ai") {
        ProviderKeyCredentialKind::ServiceAccount
    } else {
        ProviderKeyCredentialKind::RawSecret
    };

    let runtime_auth_kind = match credential_kind {
        ProviderKeyCredentialKind::ServiceAccount => ProviderKeyRuntimeAuthKind::ServiceAccount,
        ProviderKeyCredentialKind::RawSecret => {
            if key_has_auth_type_overrides(key) {
                ProviderKeyRuntimeAuthKind::Mixed
            } else {
                match auth_type.as_str() {
                    "bearer" => ProviderKeyRuntimeAuthKind::Bearer,
                    "api_key" => ProviderKeyRuntimeAuthKind::ApiKey,
                    _ => ProviderKeyRuntimeAuthKind::Unknown,
                }
            }
        }
    };

    ProviderKeyAuthSemantics {
        credential_kind,
        runtime_auth_kind,
    }
}

pub(crate) fn provider_key_configured_api_formats(key: &StoredProviderCatalogKey) -> Vec<String> {
    let mut seen = BTreeSet::new();
    key.api_formats
        .as_ref()
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(crate::ai_serving::normalize_api_format_alias)
                .filter(|value| seen.insert(value.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn provider_active_api_formats(
    endpoints: &[StoredProviderCatalogEndpoint],
) -> Vec<String> {
    let mut formats = Vec::new();
    let mut seen = BTreeSet::new();
    for endpoint in endpoints.iter().filter(|endpoint| endpoint.is_active) {
        let api_format = crate::ai_serving::normalize_api_format_alias(&endpoint.api_format);
        if api_format.is_empty() || !seen.insert(api_format.clone()) {
            continue;
        }
        formats.push(api_format);
    }
    formats
}

pub(crate) fn provider_key_effective_api_formats(key: &StoredProviderCatalogKey) -> Vec<String> {
    provider_key_configured_api_formats(key)
}
