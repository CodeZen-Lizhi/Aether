use std::collections::BTreeMap;
use std::sync::Arc;

use aether_contracts::ResolvedTransportProfile;
use serde_json::Value;

use crate::ai_serving::planner::common::{
    enforce_provider_body_stream_policy, request_requires_body_stream_field,
};
use crate::ai_serving::planner::redaction::{
    request_identity_response_encoding_when_redacted, resolve_provider_chat_pii_redaction,
};
use crate::ai_serving::transport::{
    build_same_format_provider_headers, SameFormatProviderCompatibilityEdit,
    SameFormatProviderCompatibilityEditAction, SameFormatProviderHeadersInput,
};
use crate::ai_serving::{CandidateFailureDiagnostic, GatewayProviderTransportSnapshot};
use crate::{AppState, GatewayError};

mod policy;
mod prepare;

use self::prepare::prepare_local_same_format_provider_candidate;
use super::payload::{
    mark_skipped_local_same_format_provider_candidate,
    mark_skipped_local_same_format_provider_candidate_with_extra_data,
    mark_skipped_local_same_format_provider_candidate_with_failure_diagnostic,
};
use super::{
    LocalSameFormatProviderCandidateAttempt, LocalSameFormatProviderDecisionInput,
    LocalSameFormatProviderSpec,
};
use crate::ai_serving::planner::standard::{
    openai_provider_request_contract_failure_extra_data, openai_responses_reasoning_replay_policy,
    same_format_provider_request_body_failure_extra_data,
};

pub(crate) fn resolve_same_format_provider_transport_unsupported_reason_for_trace(
    transport: &GatewayProviderTransportSnapshot,
    provider_api_format: &str,
) -> Option<&'static str> {
    let provider_api_format =
        match crate::ai_serving::normalize_api_format_alias(provider_api_format).as_str() {
            "openai:chat" => "openai:chat",
            "openai:responses" => "openai:responses",
            "openai:responses:compact" => "openai:responses:compact",
            "openai:search" => "openai:search",
            "openai:embedding" => "openai:embedding",
            "openai:rerank" => "openai:rerank",
            "claude:messages" => "claude:messages",
            "gemini:generate_content" => "gemini:generate_content",
            "gemini:embedding" => "gemini:embedding",
            "jina:embedding" => "jina:embedding",
            "jina:rerank" => "jina:rerank",
            "doubao:embedding" => "doubao:embedding",
            "aliyun:multimodal_embedding" => "aliyun:multimodal_embedding",
            _ => return Some("transport_api_format_unsupported"),
        };
    let behavior = policy::classify_same_format_provider_request_behavior(
        transport,
        provider_api_format,
        crate::ai_serving::planner::spec_metadata::LocalExecutionSurfaceSpecMetadata {
            api_format: provider_api_format,
            require_streaming: false,
            requested_model_family: None,
            decision_kind: "trace_candidate_metadata",
            report_kind: Some("trace_candidate_metadata"),
        },
        None,
    );
    let _ = behavior;
    let _ = transport;
    None
}

pub(crate) struct LocalSameFormatProviderCandidatePayloadParts {
    pub(super) transport: Arc<GatewayProviderTransportSnapshot>,
    pub(super) auth_header: Option<String>,
    pub(super) auth_value: Option<String>,
    pub(super) provider_api_format: String,
    pub(super) mapped_model: String,
    pub(super) report_kind: &'static str,
    pub(super) upstream_is_stream: bool,
    pub(super) upstream_url: String,
    pub(super) provider_request_headers: BTreeMap<String, String>,
    pub(super) provider_request_body: Value,
    pub(super) transport_profile: Option<ResolvedTransportProfile>,
    pub(super) compatibility_edits: Vec<SameFormatProviderCompatibilityEdit>,
    pub(super) request_redacted: bool,
}

pub(crate) async fn resolve_local_same_format_provider_candidate_payload_parts(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    body_json: &serde_json::Value,
    input: &LocalSameFormatProviderDecisionInput,
    attempt: &LocalSameFormatProviderCandidateAttempt,
    spec: LocalSameFormatProviderSpec,
) -> Result<Option<LocalSameFormatProviderCandidatePayloadParts>, GatewayError> {
    let candidate = &attempt.eligible.candidate;
    if let Some(skip_reason) = same_format_provider_operation_skip_reason(
        &attempt.eligible.transport,
        attempt.eligible.provider_api_format.as_str(),
        spec.operation,
    ) {
        mark_skipped_local_same_format_provider_candidate(
            state,
            input,
            trace_id,
            candidate,
            attempt.candidate_index,
            &attempt.candidate_id,
            skip_reason,
        )
        .await;
        return Ok(None);
    }
    let Some(prepared) = prepare_local_same_format_provider_candidate(
        state,
        trace_id,
        input,
        &attempt.eligible,
        attempt.candidate_index,
        &attempt.candidate_id,
        spec,
    )
    .await
    else {
        return Ok(None);
    };
    let model_directive_resolution = input
        .model_directive_policy
        .resolve_reasoning(spec.api_format, Some(&input.requested_model));
    let model_directive_mapping =
        match model_directive_resolution.mapping_patch_for_mapped_model(&prepared.mapped_model) {
            Ok(mapping) => mapping,
            Err(skip_reason) => {
                mark_skipped_local_same_format_provider_candidate(
                    state,
                    input,
                    trace_id,
                    candidate,
                    attempt.candidate_index,
                    &attempt.candidate_id,
                    skip_reason,
                )
                .await;
                return Ok(None);
            }
        };
    let effective_headers = input.effective_headers(&parts.headers);
    let reasoning_replay_policy = openai_responses_reasoning_replay_policy(
        prepared.transport.provider.provider_type.as_str(),
        prepared.transport.endpoint.base_url.as_str(),
    );
    let redaction = resolve_provider_chat_pii_redaction(
        state,
        parts,
        body_json,
        &input.auth_context,
        spec.api_format,
        reasoning_replay_policy,
        &attempt.candidate_id,
    )
    .await?;
    let body_json = redaction.body_json.as_ref();
    let mut transport = Arc::clone(&prepared.transport);

    let Some(base_provider_request) =
        super::super::request::build_same_format_provider_request_body_with_compatibility_report(
            body_json,
            prepared.provider_api_format.as_str(),
            &prepared.mapped_model,
            spec,
            prepared.transport.endpoint.body_rules.as_ref(),
            Some(effective_headers),
            prepared.upstream_is_stream,
            prepared.force_body_stream_field,
            false,
            reasoning_replay_policy,
        )
    else {
        mark_skipped_local_same_format_provider_candidate_with_extra_data(
            state,
            input,
            trace_id,
            candidate,
            attempt.candidate_index,
            &attempt.candidate_id,
            "provider_request_body_missing",
            same_format_provider_request_body_failure_extra_data(
                body_json,
                attempt.eligible.provider_api_format.as_str(),
                prepared.transport.endpoint.body_rules.as_ref(),
                "same_format",
            ),
        )
        .await;
        return Ok(None);
    };
    let mut provider_request_body = base_provider_request.body;
    let mut compatibility_edits = base_provider_request.compatibility_edits;
    if let Some(mapping) = model_directive_mapping.as_ref() {
        let before_mapping = base_provider_request_body.clone();
        crate::ai_serving::apply_model_directive_mapping_patch(
            &mut provider_request_body,
            mapping,
        );
        if before_mapping != provider_request_body {
            compatibility_edits.push(SameFormatProviderCompatibilityEdit {
                field: "model_directive_mapping".to_string(),
                action: SameFormatProviderCompatibilityEditAction::RuntimeRewrite,
                detail: "applied configured model directive mapping patch".to_string(),
            });
        }
        // Directive mapping is a deep-merge patch and may overwrite/add `stream`;
        // re-enforce stream-field policy afterward.
        enforce_provider_body_stream_policy(
            &mut provider_request_body,
            prepared.provider_api_format.as_str(),
            prepared.upstream_is_stream,
            request_requires_body_stream_field(body_json, prepared.force_body_stream_field),
        );
    }

    let source_model = body_json
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(input.requested_model.as_str());
    if let Err(violation) = crate::ai_serving::finalize_openai_provider_request(
            &mut provider_request_body,
            crate::ai_serving::OpenAiProviderRequestFinalization {
                source_api_format: spec.api_format,
                provider_api_format: prepared.provider_api_format.as_str(),
                provider_type: transport.provider.provider_type.as_str(),
                provider_model: prepared.mapped_model.as_str(),
                source_model,
                body_rules: transport.endpoint.body_rules.as_ref(),
                upstream_is_stream: prepared.upstream_is_stream,
                require_body_stream_field: request_requires_body_stream_field(
                    body_json,
                    prepared.force_body_stream_field,
                ),
            },
            reasoning_replay_policy,
        )
    {
        mark_skipped_local_same_format_provider_candidate_with_extra_data(
            state,
            input,
            trace_id,
            candidate,
            attempt.candidate_index,
            &attempt.candidate_id,
            "provider_request_body_build_failed",
            Some(openai_provider_request_contract_failure_extra_data(
                &violation,
                spec.api_format,
                prepared.provider_api_format.as_str(),
                "same_format_provider_request_finalization",
            )),
        )
        .await;
        return Ok(None);
    }

    if crate::ai_serving::transport::enforce_same_format_provider_api_operation_body_policy(
        &mut provider_request_body,
        spec.operation,
    ) {
        compatibility_edits.push(SameFormatProviderCompatibilityEdit {
            field: "stream".to_string(),
            action: SameFormatProviderCompatibilityEditAction::RuntimeRewrite,
            detail: "removed stream field for non-streaming API operation".to_string(),
        });
    }

    let transport_profile = crate::ai_serving::transport::resolve_transport_profile(&transport);
    let upstream_url = super::super::request::build_same_format_upstream_url(
        parts,
        &transport,
        &prepared.mapped_model,
        prepared.provider_api_format.as_str(),
        spec,
        prepared.upstream_is_stream,
        Some(&provider_request_body),
    );
    let Some(upstream_url) = upstream_url else {
        mark_skipped_local_same_format_provider_candidate_with_failure_diagnostic(
            state,
            input,
            trace_id,
            candidate,
            attempt.candidate_index,
            &attempt.candidate_id,
            "upstream_url_missing",
            CandidateFailureDiagnostic::upstream_url_missing(
                attempt.eligible.provider_api_format.as_str(),
                attempt.eligible.provider_api_format.as_str(),
                "same_format_provider_url",
            ),
        )
        .await;
        return Ok(None);
    };

    let extra_headers = BTreeMap::new();
    let Some(mut provider_request_headers) = build_same_format_provider_headers(
        SameFormatProviderHeadersInput {
            headers: effective_headers,
            provider_request_body: &provider_request_body,
            original_request_body: body_json,
            header_rules: transport.endpoint.header_rules.as_ref(),
            behavior: prepared.behavior,
            api_operation: spec.operation,
            auth_header: prepared.auth_header.as_deref(),
            auth_value: prepared.auth_value.as_deref(),
            extra_headers: &extra_headers,
        },
    ) else {
        mark_skipped_local_same_format_provider_candidate_with_failure_diagnostic(
            state,
            input,
            trace_id,
            candidate,
            attempt.candidate_index,
            &attempt.candidate_id,
            "transport_header_rules_apply_failed",
            CandidateFailureDiagnostic::header_rules_apply_failed(
                attempt.eligible.provider_api_format.as_str(),
                attempt.eligible.provider_api_format.as_str(),
                "same_format_provider_headers",
            ),
        )
        .await;
        return Ok(None);
    };
    request_identity_response_encoding_when_redacted(
        &mut provider_request_headers,
        redaction.redacted,
    );

    Ok(Some(LocalSameFormatProviderCandidatePayloadParts {
        transport,
        auth_header: prepared.auth_header,
        auth_value: prepared.auth_value,
        provider_api_format: prepared.provider_api_format,
        mapped_model: prepared.mapped_model,
        report_kind: prepared.report_kind,
        upstream_is_stream: prepared.upstream_is_stream,
        upstream_url,
        provider_request_headers,
        provider_request_body,
        transport_profile,
        compatibility_edits,
        request_redacted: redaction.redacted,
    }))
}

fn same_format_provider_operation_skip_reason(
    transport: &GatewayProviderTransportSnapshot,
    provider_api_format: &str,
    operation: Option<crate::ai_serving::ApiOperation>,
) -> Option<&'static str> {
    (!crate::ai_serving::transport::transport_supports_api_operation(
        transport,
        provider_api_format,
        operation,
    ))
    .then_some("transport_operation_unsupported")
}
