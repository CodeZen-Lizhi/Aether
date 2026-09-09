use aether_admin::system::AdminSystemConfigProviderKey;
use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey;
use serde_json::{json, Map, Value};

use super::AdminAppState;
use crate::api::ai::admin_endpoint_signature_parts;
use crate::handlers::admin::provider::write::normalize::{
    normalize_allow_auth_channel_mismatch_formats, normalize_auth_type_by_format,
    normalize_default_rate_multiplier, normalize_internal_priority,
    normalize_max_probe_interval_minutes, normalize_rate_multipliers,
};
use crate::handlers::admin::shared::normalize_json_object;
use crate::handlers::shared::normalize_optional_api_key_concurrent_limit;

pub(super) fn matches_imported_key_credentials(
    state: &AdminAppState<'_>,
    imported: &AdminSystemConfigProviderKey,
    existing: &StoredProviderCatalogKey,
) -> bool {
    let auth_type = imported.auth_type.as_deref().unwrap_or_default();
    if auth_type != existing.auth_type {
        return false;
    }
    if let Some(plaintext) = imported.api_key.as_deref().filter(|key| !key.is_empty()) {
        if existing
            .encrypted_api_key
            .as_deref()
            .and_then(|value| state.decrypt_catalog_secret_with_fallbacks(value))
            .as_deref()
            == Some(plaintext)
        {
            return true;
        }
    }
    let Some(config) = imported.auth_config.as_ref() else {
        return false;
    };
    let Some(existing_config) = state.parse_catalog_auth_config_json(existing) else {
        return false;
    };
    match auth_type {
        "service_account" => config
            .get("client_email")
            .and_then(Value::as_str)
            .filter(|email| !email.is_empty())
            .is_some_and(|email| {
                existing_config.get("client_email").and_then(Value::as_str) == Some(email)
            }),
        "oauth" => config
            .as_object()
            .is_some_and(|config| !config.is_empty() && config == &existing_config),
        _ => false,
    }
}

pub(super) fn validate_imported_provider_key(
    key: &AdminSystemConfigProviderKey,
) -> Result<(), String> {
    for (field, value) in [("id", &key.id), ("name", &key.name)] {
        if value.as_deref().is_none_or(|value| value.trim().is_empty()) {
            return Err(format!("渠道 Key {field} 为必填字段"));
        }
    }
    if !matches!(
        key.auth_type.as_deref(),
        Some("api_key" | "bearer" | "service_account" | "oauth")
    ) {
        return Err("渠道 Key auth_type 无效".to_string());
    }
    let formats = key
        .api_formats
        .as_deref()
        .filter(|formats| !formats.is_empty())
        .ok_or("渠道 Key api_formats 不能为空")?;
    for format in formats {
        if admin_endpoint_signature_parts(format)
            .is_none_or(|(canonical, _, _)| canonical != format)
        {
            return Err(format!("渠道 Key api_format 无效: {format}"));
        }
    }
    for (field, value) in [
        ("auth_config", &key.auth_config),
        ("capabilities", &key.capabilities),
        ("fingerprint", &key.fingerprint),
        ("proxy", &key.proxy),
        ("upstream_metadata", &key.upstream_metadata),
        ("status_snapshot", &key.status_snapshot),
        ("health_by_format", &key.health_by_format),
        ("circuit_breaker_by_format", &key.circuit_breaker_by_format),
    ] {
        normalize_json_object(value.clone(), field)?;
    }
    normalize_rate_multipliers(key.rate_multipliers.clone())?;
    for (field, value) in [
        ("adjustment_history", &key.adjustment_history),
        ("utilization_samples", &key.utilization_samples),
    ] {
        if value.as_ref().is_some_and(|value| !value.is_array()) {
            return Err(format!("渠道 Key {field} 必须是数组"));
        }
    }
    if key
        .total_cost_usd
        .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
        || [
            key.last_429_at_unix_secs,
            key.last_probe_increase_at_unix_secs,
            key.total_tokens,
            key.total_response_time_ms,
            key.last_used_at_unix_secs,
            key.last_models_fetch_at_unix_secs,
            key.oauth_invalid_at_unix_secs,
        ]
        .into_iter()
        .flatten()
        .any(|value| value > i64::MAX as u64)
    {
        return Err("渠道 Key 状态或累计用量无效".to_string());
    }
    normalize_internal_priority(key.internal_priority)?;
    normalize_default_rate_multiplier(key.default_rate_multiplier)?;
    normalize_optional_api_key_concurrent_limit(key.concurrent_limit)?;
    normalize_max_probe_interval_minutes(key.max_probe_interval_minutes.unwrap_or(32))?;
    normalize_auth_type_by_format(
        key.auth_type_by_format.clone(),
        "auth_type_by_format",
        formats,
    )?;
    normalize_allow_auth_channel_mismatch_formats(
        key.allow_auth_channel_mismatch_formats.clone(),
        "allow_auth_channel_mismatch_formats",
        formats,
    )?;
    if key.cache_ttl_minutes.is_some_and(|value| value < 0)
        || key
            .expires_at_unix_secs
            .is_some_and(|value| value > i64::MAX as u64)
    {
        return Err("渠道 Key 缓存时间或有效期无效".to_string());
    }
    Ok(())
}

/// Old backups omit these fields. Preserve existing target state only for omitted
/// fields; explicit null/zero in a new backup must overwrite the target value.
pub(super) fn apply_imported_provider_key_state(
    record: &mut StoredProviderCatalogKey,
    key: &AdminSystemConfigProviderKey,
    raw: &Map<String, Value>,
) {
    if raw.contains_key("learned_rpm_limit") {
        record.learned_rpm_limit = key.learned_rpm_limit;
    }
    if raw.contains_key("concurrent_429_count") {
        record.concurrent_429_count = key.concurrent_429_count;
    }
    if raw.contains_key("rpm_429_count") {
        record.rpm_429_count = key.rpm_429_count;
    }
    if raw.contains_key("last_429_at_unix_secs") {
        record.last_429_at_unix_secs = key.last_429_at_unix_secs;
    }
    if raw.contains_key("last_429_type") {
        record.last_429_type = key.last_429_type.clone();
    }
    if raw.contains_key("adjustment_history") {
        record.adjustment_history = key.adjustment_history.clone();
    }
    if raw.contains_key("utilization_samples") {
        record.utilization_samples = key.utilization_samples.clone();
    }
    if raw.contains_key("last_probe_increase_at_unix_secs") {
        record.last_probe_increase_at_unix_secs = key.last_probe_increase_at_unix_secs;
    }
    if raw.contains_key("last_rpm_peak") {
        record.last_rpm_peak = key.last_rpm_peak;
    }
    if raw.contains_key("request_count") {
        record.request_count = key.request_count;
    }
    if raw.contains_key("total_tokens") {
        record.total_tokens = key.total_tokens.unwrap_or(0);
    }
    if raw.contains_key("total_cost_usd") {
        record.total_cost_usd = key.total_cost_usd.unwrap_or(0.0);
    }
    if raw.contains_key("success_count") {
        record.success_count = key.success_count;
    }
    if raw.contains_key("error_count") {
        record.error_count = key.error_count;
    }
    if raw.contains_key("total_response_time_ms") {
        record.total_response_time_ms = key.total_response_time_ms;
    }
    if raw.contains_key("last_used_at_unix_secs") {
        record.last_used_at_unix_secs = key.last_used_at_unix_secs;
    }
    if raw.contains_key("last_models_fetch_at_unix_secs") {
        record.last_models_fetch_at_unix_secs = key.last_models_fetch_at_unix_secs;
    }
    if raw.contains_key("last_models_fetch_error") {
        record.last_models_fetch_error = key.last_models_fetch_error.clone();
    }
    if raw.contains_key("upstream_metadata") {
        record.upstream_metadata = key.upstream_metadata.clone();
    }
    if raw.contains_key("oauth_invalid_at_unix_secs") {
        record.oauth_invalid_at_unix_secs = key.oauth_invalid_at_unix_secs;
    }
    if raw.contains_key("oauth_invalid_reason") {
        record.oauth_invalid_reason = key.oauth_invalid_reason.clone();
    }
    if raw.contains_key("status_snapshot") {
        record.status_snapshot = key.status_snapshot.clone();
    }
    if raw.contains_key("health_by_format") {
        record.health_by_format = key.health_by_format.clone();
    }
    if raw.contains_key("circuit_breaker_by_format") {
        record.circuit_breaker_by_format = key.circuit_breaker_by_format.clone();
    }
}

/// Restore the exported configuration directly, including OAuth credentials that
/// the ordinary manual-key creation API intentionally does not accept.
pub(super) fn build_imported_provider_key_record(
    state: &AdminAppState<'_>,
    provider_id: &str,
    key: &AdminSystemConfigProviderKey,
    existing: Option<&StoredProviderCatalogKey>,
    proxy: Option<Value>,
    now: u64,
) -> Result<StoredProviderCatalogKey, String> {
    validate_imported_provider_key(key)?;
    let encrypt = |plaintext: &str| {
        state
            .encrypt_catalog_secret_with_fallbacks(plaintext)
            .ok_or_else(|| "渠道凭证加密失败，请检查目标系统加密配置".to_string())
    };
    let mut record = match existing {
        Some(existing) => existing.clone(),
        None => StoredProviderCatalogKey::new(
            key.id.clone().expect("validated key id"),
            provider_id.to_string(),
            key.name.clone().expect("validated key name"),
            key.auth_type.clone().expect("validated auth type"),
            key.capabilities.clone(),
            key.is_active,
        )
        .map_err(|err| err.to_string())?,
    };
    record.name = key.name.clone().expect("validated key name");
    record.auth_type = key.auth_type.clone().expect("validated auth type");
    record.api_formats = key.api_formats.as_ref().map(|formats| json!(formats));
    record.encrypted_api_key = key.api_key.as_deref().map(encrypt).transpose()?;
    record.encrypted_auth_config = key
        .auth_config
        .as_ref()
        .map(|value| serde_json::to_string(value).map_err(|err| err.to_string()))
        .transpose()?
        .as_deref()
        .map(encrypt)
        .transpose()?;
    record.note = key.note.clone();
    record.capabilities = key.capabilities.clone();
    record.is_active = key.is_active;
    record.rate_multipliers = key.rate_multipliers.clone();
    record.internal_priority = key.internal_priority.unwrap_or(50);
    record.default_rate_multiplier = key.default_rate_multiplier.unwrap_or(1.0);
    record.auth_type_by_format = key.auth_type_by_format.clone();
    record.allow_auth_channel_mismatch_formats = key
        .allow_auth_channel_mismatch_formats
        .as_ref()
        .map(|formats| json!(formats));
    record.rpm_limit = key.rpm_limit;
    record.concurrent_limit = key.concurrent_limit;
    record.expires_at_unix_secs = key.expires_at_unix_secs;
    record.allowed_models = key.allowed_models.as_ref().map(|models| json!(models));
    record.cache_ttl_minutes = key.cache_ttl_minutes.unwrap_or(5);
    record.max_probe_interval_minutes = key.max_probe_interval_minutes.unwrap_or(32);
    record.auto_fetch_models = key.auto_fetch_models.unwrap_or(false);
    record.locked_models = key.locked_models.as_ref().map(|models| json!(models));
    record.model_include_patterns = key
        .model_include_patterns
        .as_ref()
        .map(|patterns| json!(patterns));
    record.model_exclude_patterns = key
        .model_exclude_patterns
        .as_ref()
        .map(|patterns| json!(patterns));
    record.proxy = proxy;
    record.fingerprint = key.fingerprint.clone();
    record.created_at_unix_ms = record.created_at_unix_ms.or(Some(now));
    record.updated_at_unix_secs = Some(now);
    Ok(record)
}
