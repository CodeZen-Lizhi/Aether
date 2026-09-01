use crate::handlers::admin::request::AdminAppState;
use crate::handlers::admin::shared::build_admin_usage_counter_health_payload;
use crate::GatewayError;
use aether_admin::system::{
build_admin_api_formats_payload as build_admin_api_formats_payload_pure,
build_admin_system_stats_payload as build_admin_system_stats_payload_pure,
};

pub(crate) fn current_aether_version() -> String {
    option_env!("AETHER_BUILD_VERSION")
        .filter(|version| !version.is_empty())
        .unwrap_or(env!("CARGO_PKG_VERSION"))
        .to_string()
}

pub(crate) async fn build_admin_system_stats_payload(
    state: &AdminAppState<'_>,
) -> Result<serde_json::Value, GatewayError> {
    let providers = state
        .list_provider_catalog_providers(false)
        .await
        .unwrap_or_default();
    let total_providers = providers.len() as u64;
    let active_providers = providers
        .iter()
        .filter(|provider| provider.is_active)
        .count() as u64;
    let stats = state.read_admin_system_stats().await?;
    let now_unix_secs = chrono::Utc::now().timestamp().max(0) as u64;
    let usage_counter_snapshot = state
        .as_ref()
        .read_cached_usage_counter_health()
        .await
        .map_err(|err| GatewayError::Internal(err.to_string()))?;
    let usage_counter =
        build_admin_usage_counter_health_payload(&usage_counter_snapshot, now_unix_secs);

    Ok(build_admin_system_stats_payload_pure(
        stats.total_users,
        stats.active_users,
        total_providers,
        active_providers,
        stats.total_api_keys,
        stats.total_requests,
        usage_counter,
    ))
}

pub(crate) fn build_admin_api_formats_payload() -> serde_json::Value {
    build_admin_api_formats_payload_pure()
}
