use crate::handlers::admin::request::AdminAppState;
use aether_contracts::ProxySnapshot;
use serde_json::{Map, Value};

pub(in super::super) async fn admin_provider_ops_resolve_proxy_snapshot(
    state: &AdminAppState<'_>,
    connector_config: Option<&Map<String, Value>>,
) -> Option<ProxySnapshot> {
    state
        .resolve_admin_connector_proxy_snapshot(connector_config)
        .await
}
