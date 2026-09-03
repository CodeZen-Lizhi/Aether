mod aggregates;
mod health;
mod list;
mod value;

pub(super) async fn read_system_default_provider_priorities(
    state: &crate::AppState,
) -> std::collections::HashMap<String, i32> {
    use aether_data_contracts::repository::routing_profiles::RoutingGroupLookupKey;

    let Ok(Some(group)) = state
        .find_routing_group(RoutingGroupLookupKey::SystemDefault)
        .await
    else {
        return std::collections::HashMap::new();
    };
    let config = match serde_json::from_value::<aether_routing_core::RoutingGroupConfig>(
        group.config_json,
    ) {
        Ok(config) => config,
        Err(_) => return std::collections::HashMap::new(),
    };
    config
        .rules
        .iter()
        .filter(|rule| rule.enabled)
        .flat_map(|rule| rule.actions.iter())
        .filter_map(|action| match action {
            aether_routing_core::RoutingAction::SetProviderPriority {
                provider_id,
                priority,
            } => Some((provider_id.clone(), *priority)),
            _ => None,
        })
        .collect()
}

pub(crate) use self::aggregates::{
    build_admin_provider_summary_payload, build_admin_providers_summary_payload,
};
pub(crate) use self::health::build_admin_provider_health_monitor_payload;
pub(crate) use self::list::build_admin_providers_payload;
pub(crate) use self::value::build_admin_provider_summary_value;
