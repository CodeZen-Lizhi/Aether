use std::sync::Arc;

use crate::ai_serving::planner::candidate_preparation::resolve_candidate_mapped_model;
use crate::ai_serving::planner::candidate_resolution::EligibleLocalExecutionCandidate;
use crate::ai_serving::planner::spec_metadata::local_same_format_provider_spec_metadata;
use crate::ai_serving::transport::SameFormatProviderRequestBehavior;
use crate::ai_serving::GatewayProviderTransportSnapshot;
use crate::AppState;

use super::super::payload::mark_skipped_local_same_format_provider_candidate;
use super::super::LocalSameFormatProviderDecisionInput;
use super::super::LocalSameFormatProviderSpec;
use super::policy::{
    classify_same_format_provider_request_behavior, resolve_same_format_provider_direct_auth,
    same_format_provider_transport_supported, same_format_provider_transport_unsupported_reason,
};

pub(super) struct PreparedSameFormatProviderCandidate {
    pub(super) transport: Arc<GatewayProviderTransportSnapshot>,
    pub(super) behavior: SameFormatProviderRequestBehavior,
    pub(super) auth_header: Option<String>,
    pub(super) auth_value: Option<String>,
    pub(super) provider_api_format: String,
    pub(super) mapped_model: String,
    pub(super) report_kind: &'static str,
    pub(super) upstream_is_stream: bool,
    pub(super) force_body_stream_field: bool,
}

pub(super) async fn prepare_local_same_format_provider_candidate(
    state: &AppState,
    trace_id: &str,
    input: &LocalSameFormatProviderDecisionInput,
    eligible: &EligibleLocalExecutionCandidate,
    candidate_index: u32,
    candidate_id: &str,
    spec: LocalSameFormatProviderSpec,
) -> Option<PreparedSameFormatProviderCandidate> {
    let spec_metadata = local_same_format_provider_spec_metadata(spec);
    let candidate = &eligible.candidate;
    let transport = Arc::clone(&eligible.transport);
    let provider_api_format = eligible.provider_api_format.as_str();
    let behavior = classify_same_format_provider_request_behavior(
        &transport,
        provider_api_format,
        spec_metadata,
        spec.operation,
    );

    if !same_format_provider_transport_supported(
        &behavior,
        &transport,
        spec.family,
        provider_api_format,
    ) {
        let skip_reason = same_format_provider_transport_unsupported_reason(
            &behavior,
            &transport,
            spec.family,
            provider_api_format,
        )
        .unwrap_or("transport_unsupported");
        mark_skipped_local_same_format_provider_candidate(
            state,
            input,
            trace_id,
            candidate,
            candidate_index,
            candidate_id,
            skip_reason,
        )
        .await;
        return None;
    }

    let auth = resolve_same_format_provider_direct_auth(
        &behavior,
        &transport,
        spec.family,
        provider_api_format,
    );
    let (auth_header, auth_value) = match auth {
        Some((name, value)) => (Some(name), Some(value)),
        None => {
            mark_skipped_local_same_format_provider_candidate(
                state,
                input,
                trace_id,
                candidate,
                candidate_index,
                candidate_id,
                "transport_auth_unavailable",
            )
            .await;
            return None;
        }
    };

    let mapped_model = match resolve_candidate_mapped_model(candidate) {
        Ok(mapped_model) => mapped_model,
        Err(skip_reason) => {
            mark_skipped_local_same_format_provider_candidate(
                state,
                input,
                trace_id,
                candidate,
                candidate_index,
                candidate_id,
                skip_reason,
            )
            .await;
            return None;
        }
    };

    Some(PreparedSameFormatProviderCandidate {
        transport,
        behavior,
        auth_header,
        auth_value,
        provider_api_format: provider_api_format.to_string(),
        mapped_model,
        report_kind: behavior.report_kind,
        upstream_is_stream: behavior.upstream_is_stream,
        force_body_stream_field: behavior.force_body_stream_field,
    })
}
