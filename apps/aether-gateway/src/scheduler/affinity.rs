use std::time::Duration;

use aether_routing_core::{ResolvedRoutingPolicy, RoutingSchedulingMode};
use aether_scheduler_core::{
    build_scheduler_affinity_cache_key_for_api_key_id_with_client_session,
    build_scheduler_affinity_cache_key_for_api_key_id_with_client_session_and_scope,
    ClientSessionAffinity, SchedulerAffinityScope, SchedulerAffinityTarget,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::state::SchedulerRuntimeState;

pub(crate) const SCHEDULER_AFFINITY_TTL: Duration = Duration::from_secs(300);
pub(crate) const SCHEDULER_AFFINITY_POLICY_REPORT_FIELD: &str = "scheduler_affinity_policy";
pub(crate) const SCHEDULER_AFFINITY_REQUEST_ORDER_REPORT_FIELD: &str =
    "scheduler_affinity_request_order";

pub(crate) fn scheduler_affinity_request_order_from_report_context(
    report_context: Option<&Value>,
) -> Option<uuid::Uuid> {
    let context = report_context?;
    // A WS template may carry an earlier HTTP/turn order. The current logical
    // turn ID is authoritative and is reused across all of that turn's attempts.
    let value = context
        .get("websocket_logical_turn_id")
        .or_else(|| context.get(SCHEDULER_AFFINITY_REQUEST_ORDER_REPORT_FIELD))?;
    let order = uuid::Uuid::parse_str(value.as_str()?).ok()?;
    (order.get_version_num() == 7).then_some(order)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SchedulerAffinityPolicyContext {
    pub(crate) scheduling_mode: RoutingSchedulingMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) scope: Option<SchedulerAffinityScope>,
}

impl SchedulerAffinityPolicyContext {
    pub(crate) fn from_routing_policy(policy: &ResolvedRoutingPolicy) -> Self {
        let scope = policy
            .group_id
            .as_deref()
            .map(str::trim)
            .filter(|group_id| !group_id.is_empty())
            .map(|group_id| SchedulerAffinityScope::new(group_id, policy.group_version));
        Self {
            scheduling_mode: policy.scheduling_mode,
            scope,
        }
    }

    pub(crate) fn cache_affinity_enabled(&self) -> bool {
        matches!(
            self.scheduling_mode,
            RoutingSchedulingMode::CacheAffinity | RoutingSchedulingMode::CostBased
        )
    }
}

pub(crate) fn scheduler_affinity_policy_context_from_report_context(
    report_context: Option<&Value>,
) -> Option<SchedulerAffinityPolicyContext> {
    report_context
        .and_then(|context| context.get(SCHEDULER_AFFINITY_POLICY_REPORT_FIELD))
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

pub(crate) fn insert_scheduler_affinity_policy_report_context_field(
    extra_fields: &mut serde_json::Map<String, Value>,
    routing_policy: Option<&ResolvedRoutingPolicy>,
) {
    let Some(routing_policy) = routing_policy else {
        return;
    };
    let context = SchedulerAffinityPolicyContext::from_routing_policy(routing_policy);
    if let Ok(value) = serde_json::to_value(context) {
        extra_fields.insert(SCHEDULER_AFFINITY_POLICY_REPORT_FIELD.to_string(), value);
    }
}

pub(crate) fn read_cached_scheduler_affinity_target(
    state: &(impl SchedulerRuntimeState + ?Sized),
    api_key_id: &str,
    client_session_affinity: Option<&ClientSessionAffinity>,
    api_format: &str,
    global_model_name: &str,
) -> Option<SchedulerAffinityTarget> {
    let cache_key = build_scheduler_affinity_cache_key_for_api_key_id_with_client_session(
        api_key_id,
        api_format,
        global_model_name,
        client_session_affinity,
    )?;
    state.read_cached_scheduler_affinity_target(&cache_key, SCHEDULER_AFFINITY_TTL)
}

pub(crate) fn read_cached_scheduler_affinity_target_with_policy_context(
    state: &(impl SchedulerRuntimeState + ?Sized),
    api_key_id: &str,
    client_session_affinity: Option<&ClientSessionAffinity>,
    api_format: &str,
    global_model_name: &str,
    policy_context: &SchedulerAffinityPolicyContext,
) -> Option<SchedulerAffinityTarget> {
    if !policy_context.cache_affinity_enabled() {
        return None;
    }
    let cache_key =
        build_scheduler_affinity_cache_key_for_api_key_id_with_client_session_and_scope(
            api_key_id,
            api_format,
            global_model_name,
            client_session_affinity,
            policy_context.scope.as_ref(),
        )?;
    state.read_cached_scheduler_affinity_target(&cache_key, SCHEDULER_AFFINITY_TTL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_ws_order_overrides_template_and_survives_changed_attempt_ids() {
        let template_order = uuid::Uuid::now_v7();
        let logical_order = uuid::Uuid::now_v7();
        for attempt in 1..=3 {
            let context = serde_json::json!({
                "scheduler_affinity_request_order": template_order.to_string(),
                "websocket_logical_turn_id": logical_order.to_string(),
                "request_id": uuid::Uuid::new_v4().to_string(),
                "websocket_turn_attempt": attempt,
            });
            assert_eq!(
                scheduler_affinity_request_order_from_report_context(Some(&context)),
                Some(logical_order)
            );
        }
        let invalid_ws = serde_json::json!({
            "scheduler_affinity_request_order": template_order.to_string(),
            "websocket_logical_turn_id": uuid::Uuid::new_v4().to_string(),
        });
        assert_eq!(
            scheduler_affinity_request_order_from_report_context(Some(&invalid_ws)),
            None
        );
        assert_eq!(
            scheduler_affinity_request_order_from_report_context(None),
            None
        );
    }

    #[test]
    fn policy_scoped_cost_mode_reads_affinity_but_fixed_order_does_not() {
        let state = crate::AppState::new().expect("state should build");
        let session = ClientSessionAffinity::from_session_key("session-1");
        let scope = SchedulerAffinityScope::new("group-1", Some(7));
        let cache_key =
            build_scheduler_affinity_cache_key_for_api_key_id_with_client_session_and_scope(
                "api-key-1",
                "openai:chat",
                "gpt-5",
                Some(&session),
                Some(&scope),
            )
            .expect("cache key should build");
        let target = SchedulerAffinityTarget {
            provider_id: "provider-1".to_string(),
            endpoint_id: "endpoint-1".to_string(),
            key_id: "key-1".to_string(),
        };
        state.remember_scheduler_affinity_target(
            &cache_key,
            target.clone(),
            SCHEDULER_AFFINITY_TTL,
            10,
        );
        for mode in [
            RoutingSchedulingMode::CacheAffinity,
            RoutingSchedulingMode::CostBased,
            RoutingSchedulingMode::FixedOrder,
        ] {
            let context = SchedulerAffinityPolicyContext {
                scheduling_mode: mode,
                scope: Some(scope.clone()),
            };
            let cached = read_cached_scheduler_affinity_target_with_policy_context(
                &state,
                "api-key-1",
                Some(&session),
                "openai:chat",
                "gpt-5",
                &context,
            );
            if mode == RoutingSchedulingMode::FixedOrder {
                assert_eq!(cached, None);
            } else {
                assert_eq!(cached, Some(target.clone()));
            }
            assert_eq!(
                read_cached_scheduler_affinity_target_with_policy_context(
                    &state,
                    "other-api-key",
                    Some(&session),
                    "openai:chat",
                    "gpt-5",
                    &context,
                ),
                None
            );
        }
    }
}
