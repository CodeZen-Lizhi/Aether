use aether_dispatch_core::{DispatchCandidateRef, DispatchRankFacts, KeyRef, ProviderEndpointRef};

use crate::ai_serving::EligibleLocalExecutionCandidate;

pub(crate) fn dispatch_ref_for_local_candidate(
    eligible: &EligibleLocalExecutionCandidate,
) -> DispatchCandidateRef {
    let rank = DispatchRankFacts {
        ranking_reason: eligible.ranking.as_ref().and_then(|ranking| {
            ranking
                .promoted_by
                .or(ranking.demoted_by)
                .map(str::to_string)
        }),
    };

    DispatchCandidateRef::SingleKey {
        key: key_ref_for_candidate(eligible),
        rank,
    }
}

pub(crate) fn key_ref_for_candidate(eligible: &EligibleLocalExecutionCandidate) -> KeyRef {
    KeyRef {
        provider_id: eligible.candidate.provider_id.clone(),
        endpoint_id: eligible.candidate.endpoint_id.clone(),
        key_id: eligible.candidate.key_id.clone(),
        model_id: eligible.candidate.model_id.clone(),
        selected_provider_model_name: eligible.candidate.selected_provider_model_name.clone(),
        api_format: eligible.candidate.endpoint_api_format.clone(),
    }
}

pub(crate) fn provider_endpoint_ref_for_candidate(
    eligible: &EligibleLocalExecutionCandidate,
) -> ProviderEndpointRef {
    ProviderEndpointRef {
        provider_id: eligible.candidate.provider_id.clone(),
        endpoint_id: eligible.candidate.endpoint_id.clone(),
        model_id: eligible.candidate.model_id.clone(),
        selected_provider_model_name: eligible.candidate.selected_provider_model_name.clone(),
        api_format: eligible.candidate.endpoint_api_format.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use aether_dispatch_core::DispatchCandidateRef;
    use aether_provider_transport::snapshot::{
        GatewayProviderTransportEndpoint, GatewayProviderTransportKey,
        GatewayProviderTransportProvider,
    };
    use aether_scheduler_core::SchedulerMinimalCandidateSelectionCandidate;

    use super::dispatch_ref_for_local_candidate;
    use crate::ai_serving::EligibleLocalExecutionCandidate;
    use crate::orchestration::LocalExecutionCandidateMetadata;

    #[test]
    fn single_key_maps_to_key_ref() {
        let eligible = sample_eligible();

        let dispatch_ref = dispatch_ref_for_local_candidate(&eligible);

        match dispatch_ref {
            DispatchCandidateRef::SingleKey { key, rank } => {
                assert_eq!(key.key_id, "key-1");
            }
            other => panic!("expected key ref, got {other:?}"),
        }
    }

    fn sample_eligible() -> EligibleLocalExecutionCandidate {
        EligibleLocalExecutionCandidate {
            kind: crate::ai_serving::LocalExecutionCandidateKind::SingleKey,
            candidate: SchedulerMinimalCandidateSelectionCandidate {
                provider_id: "provider-1".to_string(),
                provider_name: "Provider 1".to_string(),
                provider_type: "custom".to_string(),
                provider_priority: 0,
                endpoint_id: "endpoint-1".to_string(),
                endpoint_api_format: "openai:chat".to_string(),
                key_id: "key-1".to_string(),
                key_name: "Key 1".to_string(),
                key_auth_type: "api_key".to_string(),
                key_internal_priority: 0,
                rate_limit_probe_required: false,
                circuit_probe_required: false,
                key_global_priority_for_format: None,
                key_capabilities: None,
                model_id: "model-1".to_string(),
                global_model_id: "global-model-1".to_string(),
                global_model_name: "gpt-5".to_string(),
                selected_provider_model_name: "gpt-5".to_string(),
                supports_streaming: true,
                mapping_matched_model: None,
            },
            transport: Arc::new(crate::ai_serving::GatewayProviderTransportSnapshot {
                provider: GatewayProviderTransportProvider {
                    id: "provider-1".to_string(),
                    name: "Provider 1".to_string(),
                    provider_type: "custom".to_string(),
                    website: None,
                    is_active: true,
                    enable_format_conversion: false,
                    concurrent_limit: None,
                    max_retries: None,
                    proxy: None,
                    request_timeout_secs: None,
                    stream_first_byte_timeout_secs: None,
                    config: None,
                },
                endpoint: GatewayProviderTransportEndpoint {
                    id: "endpoint-1".to_string(),
                    provider_id: "provider-1".to_string(),
                    api_format: "openai:chat".to_string(),
                    api_family: Some("openai".to_string()),
                    endpoint_kind: Some("chat".to_string()),
                    is_active: true,
                    base_url: "https://example.com".to_string(),
                    header_rules: None,
                    body_rules: None,
                    max_retries: None,
                    custom_path: None,
                    config: None,
                    format_acceptance_config: None,
                    proxy: None,
                },
                key: GatewayProviderTransportKey {
                    id: "key-1".to_string(),
                    provider_id: "provider-1".to_string(),
                    name: "Key 1".to_string(),
                    auth_type: "api_key".to_string(),
                    is_active: true,
                    api_formats: Some(vec!["openai:chat".to_string()]),
                    auth_type_by_format: None,
                    allow_auth_channel_mismatch_formats: None,
                    allowed_models: None,
                    capabilities: None,
                    rate_multipliers: None,
                    expires_at_unix_secs: None,
                    proxy: None,
                    fingerprint: None,
                    upstream_metadata: None,
                    decrypted_api_key: "secret".to_string(),
                    decrypted_auth_config: None,
                },
            }),
            provider_api_format: "openai:chat".to_string(),
            orchestration: LocalExecutionCandidateMetadata::default(),
            ranking: None,
        }
    }
}
