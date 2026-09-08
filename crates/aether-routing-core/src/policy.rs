use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::actions::{
    RoutingAction, RoutingRulePhase, RoutingSchedulingMode, RoutingSetPriorityMode,
};
use crate::conditions::RoutingConditionContext;
use crate::model::{RoutingGroupConfig, RoutingModelPolicy};
use crate::mutations::{validate_header_patch, validate_json_patch_operations, MutationPlan};
use crate::ranking::RankingOverlay;
use crate::validation::validate_routing_group_config;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RoutingPolicyError {
    #[error("routing group config is invalid: {0}")]
    InvalidConfig(String),
    #[error("model is not allowed by routing group: {0}")]
    ModelNotAllowed(String),
    #[error("mutation action is invalid: {0}")]
    InvalidMutation(String),
}

#[derive(Debug, Clone)]
pub struct RoutingPolicyInput<'a> {
    pub group_id: Option<&'a str>,
    pub group_version: Option<i64>,
    pub selection_source: &'a str,
    pub requested_model: &'a str,
    pub resolved_model: &'a str,
    pub api_format: &'a str,
    pub user_id: Option<&'a str>,
    pub api_key_id: Option<&'a str>,
    pub headers: &'a Value,
    pub body: &'a Value,
    pub phase: RoutingRulePhase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchedRoutingRule {
    pub id: String,
    pub priority: i32,
    pub phase: RoutingRulePhase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRoutingPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_version: Option<i64>,
    pub selection_source: String,
    pub requested_model: String,
    pub resolved_model: String,
    pub priority_mode: RoutingSetPriorityMode,
    pub scheduling_mode: RoutingSchedulingMode,
    pub ranking_overlay: RankingOverlay,
    pub mutation_plan: MutationPlan,
    #[serde(default)]
    pub matched_rules: Vec<MatchedRoutingRule>,
}

#[derive(Clone, Copy)]
enum RoutingPolicyScope {
    Complete,
    Simplified,
}

pub fn resolve_routing_policy(
    config: &RoutingGroupConfig,
    input: RoutingPolicyInput<'_>,
) -> Result<ResolvedRoutingPolicy, RoutingPolicyError> {
    validate_routing_group_config(config)
        .map_err(|error| RoutingPolicyError::InvalidConfig(error.to_string()))?;

    if !model_allowed(&config.allowed_models, input.requested_model)
        && !model_allowed(&config.allowed_models, input.resolved_model)
    {
        return Err(RoutingPolicyError::ModelNotAllowed(
            input.requested_model.to_string(),
        ));
    }

    let mut policy = ResolvedRoutingPolicy {
        group_id: input.group_id.map(str::to_string),
        group_version: input.group_version,
        selection_source: input.selection_source.to_string(),
        requested_model: input.requested_model.to_string(),
        resolved_model: input.resolved_model.to_string(),
        priority_mode: config.default_policy.priority_mode,
        scheduling_mode: config.default_policy.scheduling_mode,
        ranking_overlay: RankingOverlay::default(),
        mutation_plan: MutationPlan::default(),
        matched_rules: Vec::new(),
    };

    for model_policy in matching_model_policies(config, input.requested_model, input.resolved_model)
    {
        apply_model_policy(&mut policy, model_policy);
    }

    resolve_routing_rules_and_actions(&mut policy, config, input, RoutingPolicyScope::Complete)?;
    Ok(policy)
}

/// R11: simplified policy resolution for the single-strategy configuration
/// surface. Model policies, model restrictions, global-key ordering and key
/// priority overlays are retired here. Provider priority and the key entity's
/// priority remain authoritative; other rules retain their conditions, phase
/// and stop-processing behavior. The complete resolver keeps its legacy API.
pub fn resolve_routing_policy_simplified(
    config: &RoutingGroupConfig,
    input: RoutingPolicyInput<'_>,
) -> Result<ResolvedRoutingPolicy, RoutingPolicyError> {
    validate_routing_group_config(config)
        .map_err(|error| RoutingPolicyError::InvalidConfig(error.to_string()))?;

    let mut policy = ResolvedRoutingPolicy {
        group_id: input.group_id.map(str::to_string),
        group_version: input.group_version,
        selection_source: input.selection_source.to_string(),
        requested_model: input.requested_model.to_string(),
        resolved_model: input.resolved_model.to_string(),
        priority_mode: RoutingSetPriorityMode::Provider,
        scheduling_mode: config
            .default_policy
            .scheduling_mode
            .for_simplified_policy(),
        ranking_overlay: RankingOverlay::default(),
        mutation_plan: MutationPlan::default(),
        matched_rules: Vec::new(),
    };

    resolve_routing_rules_and_actions(&mut policy, config, input, RoutingPolicyScope::Simplified)?;
    Ok(policy)
}

fn resolve_routing_rules_and_actions(
    policy: &mut ResolvedRoutingPolicy,
    config: &RoutingGroupConfig,
    input: RoutingPolicyInput<'_>,
    scope: RoutingPolicyScope,
) -> Result<(), RoutingPolicyError> {
    let condition_context = RoutingConditionContext {
        model: input.requested_model,
        api_format: input.api_format,
        user_id: input.user_id,
        api_key_id: input.api_key_id,
        headers: input.headers,
        body: input.body,
    };

    let mut rules = config
        .rules
        .iter()
        .filter(|rule| rule.enabled && rule.phase == input.phase)
        .collect::<Vec<_>>();
    rules.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then(left.id.cmp(&right.id))
    });
    for rule in rules {
        if !rule.conditions.matches(&condition_context) {
            continue;
        }
        for action in &rule.actions {
            match scope {
                RoutingPolicyScope::Complete => {
                    apply_action(policy, action, input.requested_model, input.resolved_model)?;
                }
                RoutingPolicyScope::Simplified => apply_simplified_action(
                    policy,
                    action,
                    &rule.id,
                    input.requested_model,
                    input.resolved_model,
                )?,
            }
        }
        policy.matched_rules.push(MatchedRoutingRule {
            id: rule.id.clone(),
            priority: rule.priority,
            phase: rule.phase,
        });
        if rule.stop_processing {
            break;
        }
    }

    Ok(())
}

fn apply_model_policy(policy: &mut ResolvedRoutingPolicy, model_policy: &RoutingModelPolicy) {
    if !model_policy.allowed_providers.is_empty() {
        policy.ranking_overlay.allowed_providers = model_policy.allowed_providers.clone();
    }
    if !model_policy.allowed_keys.is_empty() {
        policy.ranking_overlay.allowed_keys = model_policy.allowed_keys.clone();
    }
    policy.ranking_overlay.provider_priority_overrides.extend(
        model_policy
            .provider_priority_overrides
            .iter()
            .map(|(key, value)| (key.clone(), *value)),
    );
    policy.ranking_overlay.key_priority_overrides.extend(
        model_policy
            .key_priority_overrides
            .iter()
            .map(|(key, value)| (key.clone(), *value)),
    );
}

fn apply_action(
    policy: &mut ResolvedRoutingPolicy,
    action: &RoutingAction,
    requested_model: &str,
    resolved_model: &str,
) -> Result<(), RoutingPolicyError> {
    match action {
        RoutingAction::RestrictModels { models } => {
            if !model_allowed(models, requested_model) && !model_allowed(models, resolved_model) {
                return Err(RoutingPolicyError::ModelNotAllowed(
                    requested_model.to_string(),
                ));
            }
        }
        RoutingAction::RestrictProviders { provider_ids } => {
            policy.ranking_overlay.allowed_providers = provider_ids.clone();
        }
        RoutingAction::RestrictKeys { key_ids } => {
            policy.ranking_overlay.allowed_keys = key_ids.clone();
        }
        RoutingAction::SetScheduling {
            priority_mode,
            scheduling_mode,
        } => {
            if let Some(priority_mode) = priority_mode {
                policy.priority_mode = *priority_mode;
            }
            if let Some(scheduling_mode) = scheduling_mode {
                policy.scheduling_mode = *scheduling_mode;
            }
        }
        RoutingAction::SetProviderPriority {
            provider_id,
            priority,
        } => {
            policy
                .ranking_overlay
                .provider_priority_overrides
                .insert(provider_id.clone(), *priority);
        }
        RoutingAction::SetKeyPriority { key_id, priority } => {
            policy
                .ranking_overlay
                .key_priority_overrides
                .insert(key_id.clone(), *priority);
        }
        RoutingAction::JsonPatchBody { patch } => {
            validate_json_patch_operations(patch)
                .map_err(|error| RoutingPolicyError::InvalidMutation(error.to_string()))?;
            policy.mutation_plan.body_patch.extend(patch.clone());
        }
        RoutingAction::PatchHeaders { patch } => {
            validate_header_patch(patch)
                .map_err(|error| RoutingPolicyError::InvalidMutation(error.to_string()))?;
            policy.mutation_plan.header_patch.extend(patch.clone());
        }
    }
    Ok(())
}

fn apply_simplified_action(
    policy: &mut ResolvedRoutingPolicy,
    action: &RoutingAction,
    rule_id: &str,
    requested_model: &str,
    resolved_model: &str,
) -> Result<(), RoutingPolicyError> {
    match action {
        RoutingAction::RestrictModels { .. } | RoutingAction::SetKeyPriority { .. } => Ok(()),
        RoutingAction::SetScheduling {
            scheduling_mode, ..
        } => {
            // This exact namespace belongs to the retired per-model UI helper.
            // Mutations and the rule's conditions/stop gate remain independent.
            if rule_id.starts_with("ui_model_scheduling:") {
                return Ok(());
            }
            if let Some(mode) = scheduling_mode {
                policy.scheduling_mode = mode.for_simplified_policy();
            }
            Ok(())
        }
        _ => apply_action(policy, action, requested_model, resolved_model),
    }
}

fn matching_model_policies<'a>(
    config: &'a RoutingGroupConfig,
    requested_model: &str,
    resolved_model: &str,
) -> Vec<&'a RoutingModelPolicy> {
    config
        .model_policies
        .iter()
        .filter(|policy| {
            model_pattern_matches(&policy.model, requested_model)
                || model_pattern_matches(&policy.model, resolved_model)
        })
        .collect()
}

fn model_allowed(patterns: &[String], requested_model: &str) -> bool {
    patterns.is_empty()
        || patterns
            .iter()
            .any(|pattern| model_pattern_matches(pattern, requested_model))
}

fn model_pattern_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.trim();
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return value.starts_with(prefix);
    }
    pattern == value
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use crate::actions::{
        RoutingJsonPatchOperation, RoutingRulePhase, RoutingSchedulingMode, RoutingSetPriorityMode,
    };
    use crate::conditions::{RoutingCondition, RoutingConditionOp};
    use crate::model::{RoutingDefaultPolicy, RoutingRule};

    use super::*;

    #[test]
    fn resolves_model_policy_and_matching_rule() {
        let config = RoutingGroupConfig {
            allowed_models: vec!["gpt-*".to_string()],
            default_policy: RoutingDefaultPolicy::default(),
            model_policies: vec![RoutingModelPolicy {
                model: "gpt-5".to_string(),
                allowed_providers: vec!["provider-a".to_string()],
                provider_priority_overrides: BTreeMap::from([("provider-a".to_string(), 0)]),
                ..RoutingModelPolicy::default()
            }],
            rules: vec![RoutingRule {
                id: "high".to_string(),
                priority: 10,
                enabled: true,
                phase: RoutingRulePhase::ClientRequest,
                conditions: RoutingCondition::Predicate {
                    field: "body.reasoning_effort".to_string(),
                    op: RoutingConditionOp::Eq,
                    value: Some(json!("high")),
                },
                actions: vec![RoutingAction::JsonPatchBody {
                    patch: vec![RoutingJsonPatchOperation::Add {
                        path: "/metadata/routing".to_string(),
                        value: json!("high"),
                    }],
                }],
                stop_processing: false,
            }],
        };

        let policy = resolve_routing_policy(
            &config,
            RoutingPolicyInput {
                group_id: Some("group-1"),
                group_version: Some(1),
                selection_source: "explicit",
                requested_model: "gpt-5",
                resolved_model: "gpt-5",
                api_format: "openai:chat",
                user_id: Some("user-1"),
                api_key_id: Some("api-key-1"),
                headers: &json!({}),
                body: &json!({"reasoning_effort":"high"}),
                phase: RoutingRulePhase::ClientRequest,
            },
        )
        .expect("policy should resolve");

        assert_eq!(policy.ranking_overlay.allowed_providers, vec!["provider-a"]);
        assert_eq!(
            policy
                .ranking_overlay
                .provider_priority_overrides
                .get("provider-a"),
            Some(&0)
        );
        assert_eq!(policy.matched_rules.len(), 1);
        assert_eq!(policy.mutation_plan.body_patch.len(), 1);
    }

    #[test]
    fn empty_allowlist_keeps_default_policy_for_models_without_an_override() {
        let config = RoutingGroupConfig {
            allowed_models: vec![],
            default_policy: RoutingDefaultPolicy {
                priority_mode: RoutingSetPriorityMode::GlobalKey,
                scheduling_mode: RoutingSchedulingMode::LoadBalance,
            },
            model_policies: vec![RoutingModelPolicy {
                model: "special-model".to_string(),
                allowed_providers: vec!["provider-special".to_string()],
                provider_priority_overrides: BTreeMap::from([("provider-special".to_string(), 0)]),
                ..RoutingModelPolicy::default()
            }],
            rules: vec![],
        };

        let special = resolve_routing_policy(
            &config,
            RoutingPolicyInput {
                group_id: Some("group-1"),
                group_version: Some(1),
                selection_source: "test",
                requested_model: "special-model",
                resolved_model: "special-model",
                api_format: "openai:chat",
                user_id: None,
                api_key_id: None,
                headers: &json!({}),
                body: &json!({}),
                phase: RoutingRulePhase::ClientRequest,
            },
        )
        .expect("the specially configured model should resolve");

        assert_eq!(special.priority_mode, RoutingSetPriorityMode::GlobalKey);
        assert_eq!(special.scheduling_mode, RoutingSchedulingMode::LoadBalance);
        assert_eq!(
            special.ranking_overlay.allowed_providers,
            vec!["provider-special"]
        );
        assert_eq!(
            special
                .ranking_overlay
                .provider_priority_overrides
                .get("provider-special"),
            Some(&0)
        );

        let ordinary = resolve_routing_policy(
            &config,
            RoutingPolicyInput {
                group_id: Some("group-1"),
                group_version: Some(1),
                selection_source: "test",
                requested_model: "ordinary-model",
                resolved_model: "ordinary-model",
                api_format: "openai:chat",
                user_id: None,
                api_key_id: None,
                headers: &json!({}),
                body: &json!({}),
                phase: RoutingRulePhase::ClientRequest,
            },
        )
        .expect("an unconfigured model should keep using the default policy");

        assert_eq!(ordinary.priority_mode, RoutingSetPriorityMode::GlobalKey);
        assert_eq!(ordinary.scheduling_mode, RoutingSchedulingMode::LoadBalance);
        assert!(ordinary.ranking_overlay.allowed_providers.is_empty());
        assert!(ordinary.ranking_overlay.allowed_keys.is_empty());
        assert!(ordinary
            .ranking_overlay
            .provider_priority_overrides
            .is_empty());
    }

    #[test]
    fn rejects_disallowed_model() {
        let config = RoutingGroupConfig {
            allowed_models: vec!["gpt-5".to_string()],
            ..RoutingGroupConfig::default()
        };

        let err = resolve_routing_policy(
            &config,
            RoutingPolicyInput {
                group_id: None,
                group_version: None,
                selection_source: "test",
                requested_model: "claude",
                resolved_model: "claude",
                api_format: "openai:chat",
                user_id: None,
                api_key_id: None,
                headers: &json!({}),
                body: &json!({}),
                phase: RoutingRulePhase::ClientRequest,
            },
        )
        .unwrap_err();

        assert_eq!(
            err,
            RoutingPolicyError::ModelNotAllowed("claude".to_string())
        );
    }

    #[test]
    fn restrict_model_action_rejects_matching_request() {
        let config = RoutingGroupConfig {
            allowed_models: vec!["*".to_string()],
            rules: vec![RoutingRule {
                id: "restrict".to_string(),
                priority: 1,
                enabled: true,
                phase: RoutingRulePhase::ClientRequest,
                conditions: RoutingCondition::default(),
                actions: vec![RoutingAction::RestrictModels {
                    models: vec!["gpt-5".to_string()],
                }],
                stop_processing: false,
            }],
            ..RoutingGroupConfig::default()
        };

        let err = resolve_routing_policy(
            &config,
            RoutingPolicyInput {
                group_id: None,
                group_version: None,
                selection_source: "test",
                requested_model: "claude",
                resolved_model: "claude",
                api_format: "openai:chat",
                user_id: None,
                api_key_id: None,
                headers: &json!({}),
                body: &json!({}),
                phase: RoutingRulePhase::ClientRequest,
            },
        )
        .unwrap_err();

        assert_eq!(
            err,
            RoutingPolicyError::ModelNotAllowed("claude".to_string())
        );
    }
}

#[cfg(test)]
mod simplified_resolution_tests {
    use serde_json::json;

    use super::{resolve_routing_policy, resolve_routing_policy_simplified, RoutingPolicyInput};
    use crate::model::RoutingGroupConfig;
    use crate::{
        rank_vector_for_candidate, CandidateKind, RoutingCandidateFacts, RoutingRulePhase,
        RoutingSchedulingMode, RoutingSetPriorityMode,
    };

    fn input<'a>() -> RoutingPolicyInput<'a> {
        let headers = Box::leak(Box::new(json!({})));
        let body = Box::leak(Box::new(json!({"model": "gpt-5"})));
        RoutingPolicyInput {
            group_id: Some("group-1"),
            group_version: Some(1),
            selection_source: "system_default",
            requested_model: "gpt-5",
            resolved_model: "gpt-5",
            api_format: "openai:chat",
            user_id: Some("user-1"),
            api_key_id: Some("key-1"),
            headers,
            body,
            phase: RoutingRulePhase::ClientRequest,
        }
    }

    #[test]
    fn simplified_entry_ignores_model_allowlist_gate() {
        // Legacy config gating a model the request does not name: the legacy
        // entry errors, the simplified entry resolves with the unified
        // default policy (R11-1/R11-2).
        let mut config: RoutingGroupConfig = serde_json::from_value(json!({
            "default_policy": {
                "priority_mode": "global_key",
                "scheduling_mode": "load_balance"
            },
            "allowed_models": ["claude-*"],
            "model_policies": [],
            "rules": [{
                "id": "legacy-restriction",
                "actions": [{ "type": "restrict_models", "models": ["claude-*"] }]
            }]
        }))
        .expect("config should parse");

        assert!(resolve_routing_policy(&config, input()).is_err());
        let policy = resolve_routing_policy_simplified(&config, input())
            .expect("simplified resolution should succeed");
        assert_eq!(policy.priority_mode, RoutingSetPriorityMode::Provider);
        assert_eq!(policy.scheduling_mode, RoutingSchedulingMode::CacheAffinity);

        // The retired action must also be ignored without a top-level gate.
        config.allowed_models.clear();
        assert!(resolve_routing_policy(&config, input()).is_err());
        assert_eq!(
            resolve_routing_policy_simplified(&config, input())
                .expect("retired restriction action should be ignored"),
            policy
        );
    }

    #[test]
    fn simplified_entry_ignores_model_policies() {
        // A per-model policy carries provider/key overlays in the legacy
        // entry; the simplified entry must not apply them (R11-1).
        let mut config: RoutingGroupConfig = serde_json::from_value(json!({
            "default_policy": {
                "priority_mode": "provider",
                "scheduling_mode": "fixed_order"
            },
            "allowed_models": [],
            "model_policies": [{
                "model": "gpt-5",
                "allowed_providers": ["provider-special"],
                "provider_priority_overrides": {"provider-special": 1},
                "key_priority_overrides": {"key-1": 1}
            }],
            "rules": [{
                "id": "ui_model_scheduling:gpt-5",
                "priority": 10000,
                "phase": "client_request",
                "conditions": {"field": "model", "op": "eq", "value": "gpt-5"},
                "actions": [{
                    "type": "set_scheduling",
                    "priority_mode": "global_key",
                    "scheduling_mode": "load_balance"
                }, {
                    "type": "patch_headers",
                    "patch": [{"op": "set", "name": "x-model", "value": "retained"}]
                }],
                "stop_processing": true
            }, {
                "id": "later-scheduling",
                "priority": 10001,
                "actions": [{"type": "set_scheduling", "scheduling_mode": "cost_based"}]
            }]
        }))
        .expect("config should parse");

        let legacy =
            resolve_routing_policy(&config, input()).expect("legacy resolution should succeed");
        assert_eq!(
            legacy
                .ranking_overlay
                .provider_priority("provider-special", i32::MAX),
            1
        );

        let simplified = resolve_routing_policy_simplified(&config, input())
            .expect("simplified resolution should succeed");
        assert_eq!(
            simplified
                .ranking_overlay
                .provider_priority("provider-special", i32::MAX),
            i32::MAX
        );
        assert_eq!(
            simplified.scheduling_mode,
            RoutingSchedulingMode::FixedOrder
        );
        let facts = RoutingCandidateFacts {
            candidate_kind: CandidateKind::Provider,
            provider_id: "provider-special".to_string(),
            endpoint_id: "endpoint-1".to_string(),
            model_id: "gpt-5".to_string(),
            key_id: Some("key-1".to_string()),
            provider_priority: 10,
            key_priority: 20,
        };
        assert_eq!(
            rank_vector_for_candidate(&legacy.ranking_overlay, &facts).key_priority_after,
            1
        );
        assert_eq!(
            rank_vector_for_candidate(&simplified.ranking_overlay, &facts).key_priority_after,
            20
        );
        assert_eq!(legacy.scheduling_mode, RoutingSchedulingMode::LoadBalance);
        assert_eq!(legacy.mutation_plan, simplified.mutation_plan);
        assert_eq!(simplified.mutation_plan.header_patch.len(), 1);
        assert_eq!(simplified.matched_rules.len(), 1);
        assert_eq!(simplified.matched_rules[0].id, "ui_model_scheduling:gpt-5");

        // Similar names outside the exact legacy namespace keep their actions.
        config.rules[0].id = "ui_model_scheduling_custom:gpt-5".to_string();
        assert_eq!(
            resolve_routing_policy_simplified(&config, input())
                .expect("external conditional scheduling should remain active")
                .scheduling_mode,
            RoutingSchedulingMode::CacheAffinity
        );
    }

    #[test]
    fn simplified_entry_still_applies_rules_and_provider_priority() {
        // Surviving actions keep their conditions, order, phase and stop gate;
        // retired key overlays must not replace the key entity's priority.
        let mut config: RoutingGroupConfig = serde_json::from_value(json!({
            "default_policy": {
                "priority_mode": "provider",
                "scheduling_mode": "cost_based"
            },
            "allowed_models": [],
            "model_policies": [],
            "rules": [{
                "id": "later-rule",
                "priority": 2,
                "actions": [{
                    "type": "set_provider_priority",
                    "provider_id": "provider-a",
                    "priority": 99
                }]
            }, {
                "id": "rule-1",
                "priority": 1,
                "enabled": true,
                "phase": "client_request",
                "conditions": {"field": "model", "op": "eq", "value": "gpt-5"},
                "stop_processing": true,
                "actions": [{
                    "type": "set_provider_priority",
                    "provider_id": "provider-a",
                    "priority": 5
                }, {
                    "type": "set_key_priority",
                    "key_id": "key-1",
                    "priority": 2
                }, {
                    "type": "set_scheduling",
                    "priority_mode": "global_key",
                    "scheduling_mode": "cost_based"
                }, {
                    "type": "json_patch_body",
                    "patch": [{"op": "add", "path": "/metadata", "value": {"routed": true}}]
                }, {
                    "type": "patch_headers",
                    "patch": [{"op": "set", "name": "x-routing", "value": "retained"}]
                }]
            }, {
                "id": "non-matching-rule",
                "priority": 0,
                "conditions": {"field": "model", "op": "eq", "value": "claude"},
                "actions": [{"type": "restrict_providers", "provider_ids": ["provider-b"]}],
                "stop_processing": true
            }, {
                "id": "provider-phase",
                "priority": 0,
                "phase": "provider_request",
                "actions": [{
                    "type": "patch_headers",
                    "patch": [{"op": "set", "name": "x-routing", "value": "provider"}]
                }]
            }]
        }))
        .expect("config should parse");

        let policy = resolve_routing_policy_simplified(&config, input())
            .expect("simplified resolution should succeed");
        assert_eq!(
            policy
                .ranking_overlay
                .provider_priority("provider-a", i32::MAX),
            5
        );
        assert_eq!(policy.priority_mode, RoutingSetPriorityMode::Provider);
        assert_eq!(policy.scheduling_mode, RoutingSchedulingMode::CostBased);
        assert!(policy.ranking_overlay.allowed_providers.is_empty());
        assert!(policy.ranking_overlay.key_priority_overrides.is_empty());
        assert_eq!(policy.ranking_overlay.key_priority("key-1", 20), 20);
        assert_eq!(policy.matched_rules.len(), 1);
        assert_eq!(policy.matched_rules[0].id, "rule-1");
        assert_eq!(policy.mutation_plan.body_patch.len(), 1);
        assert_eq!(policy.mutation_plan.header_patch.len(), 1);

        let legacy = resolve_routing_policy(&config, input())
            .expect("complete public resolver should keep legacy actions");
        assert_eq!(legacy.priority_mode, RoutingSetPriorityMode::GlobalKey);
        assert_eq!(legacy.ranking_overlay.key_priority("key-1", 20), 2);
        assert_eq!(legacy.mutation_plan, policy.mutation_plan);

        config.rules[1].actions[2] = crate::RoutingAction::SetScheduling {
            priority_mode: Some(RoutingSetPriorityMode::GlobalKey),
            scheduling_mode: Some(RoutingSchedulingMode::LoadBalance),
        };
        assert_eq!(
            resolve_routing_policy_simplified(&config, input())
                .expect("legacy scheduling action should normalize")
                .scheduling_mode,
            RoutingSchedulingMode::CacheAffinity
        );
        assert_eq!(
            resolve_routing_policy(&config, input())
                .expect("complete resolver should retain load balance")
                .scheduling_mode,
            RoutingSchedulingMode::LoadBalance
        );
    }
}
