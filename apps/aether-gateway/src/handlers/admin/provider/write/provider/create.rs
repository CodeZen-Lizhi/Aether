use crate::handlers::admin::provider::shared::payloads::AdminProviderCreateRequest;
use crate::handlers::admin::provider::shared::support::{
    normalize_provider_billing_type, normalize_provider_transfer_limit,
    normalize_provider_transfer_limit_json, parse_optional_rfc3339_unix_secs,
    PROVIDER_MAX_TRANSFER_COUNT_CONFIG_KEY, PROVIDER_MAX_TRANSFER_TIMEOUT_SECONDS_CONFIG_KEY,
};
use crate::handlers::admin::provider::write::normalize::normalize_provider_type_input;
use crate::handlers::admin::provider::write::normalize::remove_retired_provider_config;
use crate::handlers::admin::provider::write::normalize::set_responses_websocket_enabled;
use crate::handlers::admin::provider::write::normalize::validate_responses_websocket_config;
use crate::handlers::admin::request::AdminAppState;
use crate::handlers::admin::shared::normalize_json_object;
use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogProvider;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// 构造管理 API 写入记录，并移除已退役的聊天脱敏配置。
pub(crate) async fn build_admin_create_provider_record(
    state: &AdminAppState<'_>,
    payload: AdminProviderCreateRequest,
) -> Result<(StoredProviderCatalogProvider, Option<i32>), String> {
    if payload.name.trim().is_empty() {
        return Err("name 为必填字段".to_string());
    }
    let existing_providers = state
        .list_provider_catalog_providers(false)
        .await
        .map_err(|err| format!("{err:?}"))?;
    let (mut record, priority) =
        build_admin_create_provider_record_from_existing(&existing_providers, payload)?;
    remove_retired_provider_config(&mut record.config);
    Ok((record, priority))
}

/// 根据已有记录构造供应商配置；共享备份入口保留历史扩展字段。
pub(crate) fn build_admin_create_provider_record_from_existing(
    existing_providers: &[StoredProviderCatalogProvider],
    payload: AdminProviderCreateRequest,
) -> Result<(StoredProviderCatalogProvider, Option<i32>), String> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err("name 为必填字段".to_string());
    }

    if existing_providers
        .iter()
        .any(|provider| provider.name == name)
    {
        return Err(format!("提供商名称 '{name}' 已存在"));
    }

    let provider_type =
        normalize_provider_type_input(payload.provider_type.as_deref().unwrap_or("custom"))?;
    let billing_type = normalize_provider_billing_type(
        payload.billing_type.as_deref().unwrap_or("pay_as_you_go"),
    )?;

    let website = payload.website.and_then(|value| {
        let trimmed = value.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    });
    let website = website.map(|value| {
        if value.starts_with("http://") || value.starts_with("https://") {
            value
        } else {
            format!("https://{value}")
        }
    });

    let monthly_quota_usd = match payload.monthly_quota_usd {
        Some(value) if value.is_finite() && value >= 0.0 => Some(value),
        Some(_) => return Err("monthly_quota_usd 必须是非负数".to_string()),
        None => None,
    };
    let quota_reset_day = match payload.quota_reset_day {
        Some(value) if (1..=365).contains(&value) => Some(value),
        Some(_) => return Err("quota_reset_day 必须是 1 到 365 之间的整数".to_string()),
        None => Some(30),
    };
    let quota_last_reset_at_unix_secs = payload
        .quota_last_reset_at
        .as_deref()
        .map(|value| parse_optional_rfc3339_unix_secs(value, "quota_last_reset_at"))
        .transpose()?;
    let quota_expires_at_unix_secs = payload
        .quota_expires_at
        .as_deref()
        .map(|value| parse_optional_rfc3339_unix_secs(value, "quota_expires_at"))
        .transpose()?;
    let is_active = payload.is_active.unwrap_or(true);
    let concurrent_limit = match payload.concurrent_limit {
        Some(value) if value >= 0 => Some(value),
        Some(_) => return Err("concurrent_limit 必须是非负整数".to_string()),
        None => None,
    };
    let max_retries = match payload.max_retries {
        Some(value) => {
            aether_admin::provider::failover::validate_legacy_attempts(value)?;
            Some(value)
        }
        None => None,
    };
    if max_retries.is_some() {
        for rules in [
            payload.failover_rules.as_ref(),
            payload
                .config
                .as_ref()
                .and_then(|config| config.get("failover_rules")),
        ] {
            aether_admin::provider::failover::validate_scope_aliases(
                rules,
                "provider_max_attempts",
                max_retries,
            )?;
        }
    }
    let proxy = normalize_json_object(payload.proxy, "proxy")?;
    let stream_first_byte_timeout_secs =
        super::normalize_provider_stream_first_byte_timeout(payload.stream_first_byte_timeout)?;
    let request_timeout_secs = super::normalize_provider_request_timeout(payload.request_timeout)?;

    let mut config_map = normalize_json_object(payload.config, "config")?
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    aether_admin::provider::timeouts::normalize_stream_total_timeout(
        &mut config_map,
        payload.stream_total_timeout,
    )?;
    for (field_name, payload_value) in [
        (
            PROVIDER_MAX_TRANSFER_COUNT_CONFIG_KEY,
            payload.max_transfer_count,
        ),
        (
            PROVIDER_MAX_TRANSFER_TIMEOUT_SECONDS_CONFIG_KEY,
            payload.max_transfer_timeout_seconds,
        ),
    ] {
        let value = match payload_value {
            Some(value) => Some(normalize_provider_transfer_limit(value, field_name)?),
            None => config_map
                .get(field_name)
                .map(|value| normalize_provider_transfer_limit_json(value, field_name))
                .transpose()?,
        };
        if let Some(value) = value {
            config_map.insert(field_name.to_string(), json!(value));
        }
    }
    if let Some(value) = normalize_json_object(payload.failover_rules, "failover_rules")? {
        let rules = aether_admin::provider::failover::merge_failover_rules(
            config_map.get("failover_rules"),
            value,
        )?;
        config_map.insert("failover_rules".to_string(), rules);
    }
    aether_admin::provider::failover::normalize_config_rules(&mut config_map)?;
    if max_retries.is_some() {
        aether_admin::provider::failover::set_scope_attempts(
            &mut config_map,
            "provider_max_attempts",
            max_retries,
        )?;
    }
    if let Some(enabled) = payload.responses_websocket_enabled {
        set_responses_websocket_enabled(&mut config_map, enabled)?;
    }
    validate_responses_websocket_config(&config_map)?;
    let config = (!config_map.is_empty()).then_some(serde_json::Value::Object(config_map));
    crate::provider_transport::validate_anthropic_compatibility_profile_config(config.as_ref())
        .map_err(|_| "无效的 Anthropic compatibility profile".to_string())?;

    let now_unix_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    let record = StoredProviderCatalogProvider::new(
        Uuid::new_v4().to_string(),
        name.to_string(),
        website,
        provider_type.clone(),
    )
    .map_err(|err| err.to_string())?
    .with_description(
        payload
            .description
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    )
    .with_billing_fields(
        Some(billing_type),
        monthly_quota_usd,
        None,
        quota_reset_day,
        quota_last_reset_at_unix_secs,
        quota_expires_at_unix_secs,
    )
    .with_transport_fields(
        is_active,
        false,
        concurrent_limit,
        max_retries,
        proxy,
        request_timeout_secs,
        stream_first_byte_timeout_secs,
        config,
    )
    .with_timestamps(Some(now_unix_secs), Some(now_unix_secs));

    Ok((record, None))
}
