use crate::handlers::admin::request::AdminAppState;
use crate::LocalProviderDeleteTaskState;
use serde_json::json;

pub(crate) const ADMIN_PROVIDER_MAPPING_PREVIEW_MAX_KEYS: usize = 200;
pub(crate) const ADMIN_PROVIDER_MAPPING_PREVIEW_MAX_MODELS: usize = 500;
pub(crate) const ADMIN_PROVIDER_MAPPING_PREVIEW_FETCH_LIMIT: usize = 10_000;
pub(crate) const ADMIN_PROVIDER_OAUTH_DATA_UNAVAILABLE_DETAIL: &str =
    "Admin provider OAuth data unavailable";
pub(crate) const PROVIDER_MAX_TRANSFER_COUNT_CONFIG_KEY: &str = "max_transfer_count";
pub(crate) const PROVIDER_MAX_TRANSFER_TIMEOUT_SECONDS_CONFIG_KEY: &str =
    "max_transfer_timeout_seconds";

pub(crate) fn normalize_provider_transfer_limit(
    value: i64,
    field_name: &str,
) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("{field_name} 必须是非负整数"))
}

pub(crate) fn normalize_provider_transfer_limit_json(
    value: &serde_json::Value,
    field_name: &str,
) -> Result<u64, String> {
    value
        .as_u64()
        .ok_or_else(|| format!("{field_name} 必须是非负整数"))
}

pub(crate) fn provider_transfer_limit_from_config(
    config: Option<&serde_json::Map<String, serde_json::Value>>,
    field_name: &str,
) -> u64 {
    config
        .and_then(|config| config.get(field_name))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

pub(crate) fn build_admin_provider_delete_task_payload(
    task: &LocalProviderDeleteTaskState,
) -> serde_json::Value {
    json!({
        "task_id": task.task_id,
        "provider_id": task.provider_id,
        "status": task.status,
        "stage": task.stage,
        "total_keys": task.total_keys,
        "deleted_keys": task.deleted_keys,
        "total_endpoints": task.total_endpoints,
        "deleted_endpoints": task.deleted_endpoints,
        "message": task.message,
    })
}

pub(crate) fn put_admin_provider_delete_task(
    state: &AdminAppState<'_>,
    task: &LocalProviderDeleteTaskState,
) {
    state.as_ref().put_provider_delete_task(task.clone());
}

pub(crate) fn normalize_provider_billing_type(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "monthly_quota" | "pay_as_you_go" | "free_tier" => Ok(normalized),
        _ => Err("billing_type 仅支持 monthly_quota / pay_as_you_go / free_tier".to_string()),
    }
}

pub(crate) fn parse_optional_rfc3339_unix_secs(
    value: &str,
    field_name: &str,
) -> Result<u64, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field_name} 不能为空"));
    }
    let parsed = chrono::DateTime::parse_from_rfc3339(trimmed)
        .map_err(|_| format!("{field_name} 必须是合法的 RFC3339 时间"))?;
    u64::try_from(parsed.timestamp()).map_err(|_| format!("{field_name} 超出有效时间范围"))
}
