use std::collections::BTreeMap;
use std::sync::Arc;

use aether_contracts::ResolvedTransportProfile;
use serde_json::{json, Value};

use crate::ai_serving::planner::candidate_preparation::{
    prepare_header_authenticated_candidate, OauthPreparationContext,
};
use crate::ai_serving::planner::candidate_resolution::EligibleLocalExecutionCandidate;
use crate::ai_serving::planner::common::{
    endpoint_config_forces_body_stream_field, enforce_provider_body_stream_policy,
    request_requires_body_stream_field, OPENAI_CHAT_STREAM_PLAN_KIND,
};
use crate::ai_serving::planner::redaction::{
    request_identity_response_encoding_when_redacted, resolve_provider_chat_pii_redaction,
};
use crate::ai_serving::planner::standard::{
    apply_deepseek_tool_call_thinking_compat, build_cross_format_openai_chat_request_body,
    build_cross_format_openai_chat_upstream_url, build_local_openai_chat_request_body,
    build_local_openai_chat_upstream_url, openai_provider_request_contract_failure_extra_data,
    openai_responses_reasoning_replay_policy, request_body_build_failure_extra_data,
    request_conversion_failure_extra_data,
};
use crate::ai_serving::transport::auth::resolve_local_openai_bearer_auth;
use crate::ai_serving::transport::local_openai_chat_transport_unsupported_reason;
use crate::ai_serving::transport::{
    build_openai_image_headers, build_openai_image_upstream_url,
    build_standard_provider_request_headers, openai_image_transport_unsupported_reason,
    resolve_openai_image_auth, ProviderOpenAiImageHeadersInput,
    StandardProviderRequestHeadersInput,
};
use crate::ai_serving::{
    ai_local_execution_contract_for_formats, project_openai_image_api_request_body,
    request_conversion_direct_auth, request_conversion_kind, CandidateFailureDiagnostic,
    GatewayProviderTransportSnapshot, OpenAiImageOperation,
};
use crate::ai_serving::{ConversionMode, ExecutionStrategy};
use crate::stage_metrics::observe_gateway_stage_ms;
use crate::{AppState, GatewayError};

use super::support::{
    mark_skipped_local_openai_chat_candidate,
    mark_skipped_local_openai_chat_candidate_with_extra_data,
    mark_skipped_local_openai_chat_candidate_with_failure_diagnostic, LocalOpenAiChatDecisionInput,
};

pub(crate) struct LocalOpenAiChatCandidatePayloadParts {
    pub(super) client_api_format: String,
    pub(super) auth_header: String,
    pub(super) auth_value: String,
    pub(super) mapped_model: String,
    pub(super) provider_api_format: String,
    pub(super) provider_request_body: Value,
    pub(super) provider_request_headers: BTreeMap<String, String>,
    pub(super) upstream_url: String,
    pub(super) execution_strategy: ExecutionStrategy,
    pub(super) conversion_mode: ConversionMode,
    pub(super) report_kind: String,
    pub(super) envelope_name: Option<&'static str>,
    pub(super) transport: Arc<GatewayProviderTransportSnapshot>,
    pub(super) request_redacted: bool,
    pub(super) transport_profile: Option<ResolvedTransportProfile>,
    pub(super) image_request_summary: Option<Value>,
}

#[derive(Default)]
pub(crate) struct LocalOpenAiChatRequestPreparation;

fn is_grok_text_provider_api_format(provider_api_format: &str) -> bool {
    matches!(
        crate::ai_serving::normalize_api_format_alias(provider_api_format).as_str(),
        "openai:chat" | "openai:responses" | "openai:responses:compact" | "claude:messages"
    )
}

fn finalize_openai_chat_provider_request_body(
    provider_request_body: &mut Value,
    custom_directive_mapping: Option<&Value>,
    provider_api_format: &str,
    upstream_is_stream: bool,
    force_body_stream_field: bool,
    original_body: &Value,
    transport: &GatewayProviderTransportSnapshot,
    mapped_model: &str,
) -> Option<Value> {
    if let Some(mapping) = custom_directive_mapping {
        crate::ai_serving::apply_model_directive_mapping_patch(provider_request_body, mapping);
    }

    // Mapping and endpoint body rules can both write `stream`. The resolved transport
    // policy is authoritative and therefore runs after every body mutation.
    enforce_provider_body_stream_policy(
        provider_request_body,
        provider_api_format,
        upstream_is_stream,
        request_requires_body_stream_field(original_body, force_body_stream_field),
    );
    apply_deepseek_tool_call_thinking_compat(
        provider_request_body,
        transport.provider.provider_type.as_str(),
        transport.endpoint.base_url.as_str(),
        provider_api_format,
        Some(original_body),
    );
    let source_model = original_body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(mapped_model);
    crate::ai_serving::finalize_openai_provider_request(
        provider_request_body,
        crate::ai_serving::OpenAiProviderRequestFinalization {
            source_api_format: "openai:chat",
            provider_api_format,
            provider_type: transport.provider.provider_type.as_str(),
            provider_model: mapped_model,
            source_model,
            body_rules: transport.endpoint.body_rules.as_ref(),
            upstream_is_stream,
            require_body_stream_field: request_requires_body_stream_field(
                original_body,
                force_body_stream_field,
            ),
        },
    )
    .err()
    .map(|violation| {
        openai_provider_request_contract_failure_extra_data(
            &violation,
            "openai:chat",
            provider_api_format,
            "openai_chat_request_finalization",
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn resolve_local_openai_chat_candidate_payload_parts(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    body_json: &serde_json::Value,
    input: &LocalOpenAiChatDecisionInput,
    _preparation: Option<&mut LocalOpenAiChatRequestPreparation>,
    eligible: &EligibleLocalExecutionCandidate,
    candidate_index: u32,
    candidate_id: &str,
    decision_kind: &str,
    report_kind: &str,
    upstream_is_stream: bool,
) -> Result<Option<LocalOpenAiChatCandidatePayloadParts>, GatewayError> {
    let prepare_started_at = std::time::Instant::now();
    let planner_state = crate::ai_serving::PlannerAppState::new(state);
    let candidate = &eligible.candidate;
    let provider_api_format = eligible.provider_api_format.as_str();
    let transport = &eligible.transport;
    let transport_profile = crate::ai_serving::transport::resolve_transport_profile(transport);
    let force_body_stream_field =
        endpoint_config_forces_body_stream_field(transport.endpoint.config.as_ref());
    let model_directives_started_at = std::time::Instant::now();
    let model_directive_resolution = input
        .model_directive_policy
        .resolve_reasoning(provider_api_format, Some(&input.requested_model));
    observe_gateway_stage_ms(
        "openai_chat_payload_model_directives",
        model_directives_started_at.elapsed().as_millis() as u64,
    );
    let redaction_started_at = std::time::Instant::now();
    let redaction = resolve_provider_chat_pii_redaction(
        state,
        parts,
        body_json,
        &input.auth_context,
        "openai:chat",
        crate::ai_serving::OpenAiResponsesReasoningReplayPolicy::OpenAiItemIds,
        candidate_id,
    )
    .await?;
    observe_gateway_stage_ms(
        "openai_chat_payload_redaction",
        redaction_started_at.elapsed().as_millis() as u64,
    );
    let body_json = redaction.body_json.as_ref();
    let effective_headers = input.effective_headers(&parts.headers);

    if provider_api_format == "openai:chat" {
        if let Some(skip_reason) = local_openai_chat_transport_unsupported_reason(transport) {
            mark_skipped_local_openai_chat_candidate(
                state,
                input,
                trace_id,
                candidate,
                candidate_index,
                candidate_id,
                skip_reason,
            )
            .await;
            return Ok(None);
        };

        let auth_prepare_started_at = std::time::Instant::now();
        let prepared_candidate = match prepare_header_authenticated_candidate(
            planner_state,
            transport,
            candidate,
            resolve_local_openai_bearer_auth(transport),
            OauthPreparationContext {
                trace_id,
                api_format: "openai:chat",
                operation: "openai_chat_same_format",
            },
        )
        .await
        {
            Ok(prepared) => prepared,
            Err(skip_reason) => {
                mark_skipped_local_openai_chat_candidate(
                    state,
                    input,
                    trace_id,
                    candidate,
                    candidate_index,
                    candidate_id,
                    skip_reason,
                )
                .await;
                return Ok(None);
            }
        };
        let model_directive_mapping = match model_directive_resolution
            .mapping_patch_for_mapped_model(&prepared_candidate.mapped_model)
        {
            Ok(mapping) => mapping,
            Err(skip_reason) => {
                mark_skipped_local_openai_chat_candidate(
                    state,
                    input,
                    trace_id,
                    candidate,
                    candidate_index,
                    candidate_id,
                    skip_reason,
                )
                .await;
                return Ok(None);
            }
        };
        observe_gateway_stage_ms(
            "openai_chat_payload_auth_prepare",
            auth_prepare_started_at.elapsed().as_millis() as u64,
        );

        let body_build_started_at = std::time::Instant::now();
        let Some(mut provider_request_body) = build_local_openai_chat_request_body(
            body_json,
            &prepared_candidate.mapped_model,
            upstream_is_stream,
            force_body_stream_field,
            transport.endpoint.body_rules.as_ref(),
            effective_headers,
            false,
        ) else {
            mark_skipped_local_openai_chat_candidate_with_extra_data(
                state,
                input,
                trace_id,
                candidate,
                candidate_index,
                candidate_id,
                "provider_request_body_build_failed",
                request_body_build_failure_extra_data(
                    body_json,
                    "openai:chat",
                    provider_api_format,
                ),
            )
            .await;
            return Ok(None);
        };
        observe_gateway_stage_ms(
            "openai_chat_payload_body_build",
            body_build_started_at.elapsed().as_millis() as u64,
        );
        if let Some(extra_data) = finalize_openai_chat_provider_request_body(
            &mut provider_request_body,
            model_directive_mapping.as_ref(),
            "openai:chat",
            upstream_is_stream,
            force_body_stream_field,
            body_json,
            transport,
            &prepared_candidate.mapped_model,
        ) {
            mark_skipped_local_openai_chat_candidate_with_extra_data(
                state,
                input,
                trace_id,
                candidate,
                candidate_index,
                candidate_id,
                "provider_request_body_build_failed",
                Some(extra_data),
            )
            .await;
            return Ok(None);
        }

        let Some(upstream_url) = build_local_openai_chat_upstream_url(parts, transport) else {
            mark_skipped_local_openai_chat_candidate_with_failure_diagnostic(
                state,
                input,
                trace_id,
                candidate,
                candidate_index,
                candidate_id,
                "upstream_url_missing",
                CandidateFailureDiagnostic::upstream_url_missing(
                    "openai:chat",
                    provider_api_format,
                    "openai_chat_same_format_url",
                ),
            )
            .await;
            return Ok(None);
        };

        let Some(resolved_headers) =
            build_standard_provider_request_headers(StandardProviderRequestHeadersInput {
                transport,
                provider_api_format,
                same_format: true,
                headers: effective_headers,
                auth_header: &prepared_candidate.auth_header,
                auth_value: &prepared_candidate.auth_value,
                extra_headers: &BTreeMap::new(),
                header_rules: transport.endpoint.header_rules.as_ref(),
                provider_request_body: &provider_request_body,
                original_request_body: body_json,
                upstream_is_stream,
            })
        else {
            mark_skipped_local_openai_chat_candidate_with_failure_diagnostic(
                state,
                input,
                trace_id,
                candidate,
                candidate_index,
                candidate_id,
                "transport_header_rules_apply_failed",
                CandidateFailureDiagnostic::header_rules_apply_failed(
                    "openai:chat",
                    provider_api_format,
                    "openai_chat_same_format_headers",
                ),
            )
            .await;
            return Ok(None);
        };
        let mut provider_request_headers = resolved_headers.headers;
        let (execution_strategy, conversion_mode) =
            ai_local_execution_contract_for_formats("openai:chat", "openai:chat");
        let resolved_report_kind =
            if decision_kind == OPENAI_CHAT_STREAM_PLAN_KIND || !upstream_is_stream {
                report_kind.to_string()
            } else {
                "openai_chat_sync_finalize".to_string()
            };

        request_identity_response_encoding_when_redacted(
            &mut provider_request_headers,
            redaction.redacted,
        );

        return Ok(Some(LocalOpenAiChatCandidatePayloadParts {
            client_api_format: "openai:chat".to_string(),
            auth_header: resolved_headers.auth_header,
            auth_value: resolved_headers.auth_value,
            mapped_model: prepared_candidate.mapped_model,
            provider_api_format: "openai:chat".to_string(),
            provider_request_body,
            provider_request_headers,
            upstream_url,
            execution_strategy,
            conversion_mode,
            report_kind: resolved_report_kind,
            envelope_name: None,
            transport: Arc::clone(transport),
            request_redacted: redaction.redacted,
            transport_profile,
            image_request_summary: None,
        }));
    };

    let provider_api_format = provider_api_format.trim().to_ascii_lowercase();
    let normalized_provider_api_format =
        crate::ai_serving::normalize_api_format_alias(provider_api_format.as_str());
    if provider_api_format == "openai:image" {
        return resolve_openai_chat_to_openai_image_payload_parts(
            state,
            parts,
            trace_id,
            body_json,
            input,
            eligible,
            candidate_index,
            candidate_id,
            upstream_is_stream,
        )
        .await;
    }

    let Some(conversion_kind) =
        request_conversion_kind("openai:chat", provider_api_format.as_str())
    else {
        mark_skipped_local_openai_chat_candidate(
            state,
            input,
            trace_id,
            candidate,
            candidate_index,
            candidate_id,
            "transport_api_format_unsupported",
        )
        .await;
        return Ok(None);
    };
    if let Some(skip_reason) = crate::ai_serving::request_conversion_transport_unsupported_reason(
        transport,
        conversion_kind,
    ) {
        if let Some(skip_reason) = Some(skip_reason) {
            mark_skipped_local_openai_chat_candidate(
                state,
                input,
                trace_id,
                candidate,
                candidate_index,
                candidate_id,
                skip_reason,
            )
            .await;
            return Ok(None);
        }
    }
    let oauth_context = OauthPreparationContext {
        trace_id,
        api_format: provider_api_format.as_str(),
        operation: "openai_chat_cross_format",
    };
    let prepared_candidate = match prepare_header_authenticated_candidate(
        planner_state,
        transport,
        candidate,
        request_conversion_direct_auth(transport, conversion_kind),
        oauth_context,
    )
    .await
    {
        Ok(prepared) => prepared,
        Err(skip_reason) => {
            mark_skipped_local_openai_chat_candidate(
                state,
                input,
                trace_id,
                candidate,
                candidate_index,
                candidate_id,
                skip_reason,
            )
            .await;
            return Ok(None);
        }
    };
    let model_directive_mapping = match model_directive_resolution
        .mapping_patch_for_mapped_model(&prepared_candidate.mapped_model)
    {
        Ok(mapping) => mapping,
        Err(skip_reason) => {
            mark_skipped_local_openai_chat_candidate(
                state,
                input,
                trace_id,
                candidate,
                candidate_index,
                candidate_id,
                skip_reason,
            )
            .await;
            return Ok(None);
        }
    };

    let Some(mut provider_request_body) = build_cross_format_openai_chat_request_body(
        body_json,
        &prepared_candidate.mapped_model,
        transport.provider.provider_type.as_str(),
        provider_api_format.as_str(),
        upstream_is_stream,
        force_body_stream_field,
        transport.endpoint.body_rules.as_ref(),
        Some(input.auth_context.api_key_id.as_str()),
        effective_headers,
        false,
    ) else {
        mark_skipped_local_openai_chat_candidate_with_extra_data(
            state,
            input,
            trace_id,
            candidate,
            candidate_index,
            candidate_id,
            "provider_request_body_build_failed",
            request_conversion_failure_extra_data(
                body_json,
                "openai:chat",
                provider_api_format.as_str(),
                Some(prepared_candidate.mapped_model.as_str()),
                Some(parts.uri.path()),
                upstream_is_stream,
                "openai_chat_request_conversion",
            ),
        )
        .await;
        return Ok(None);
    };
    if let Some(extra_data) = finalize_openai_chat_provider_request_body(
        &mut provider_request_body,
        model_directive_mapping.as_ref(),
        provider_api_format.as_str(),
        upstream_is_stream,
        force_body_stream_field,
        body_json,
        transport,
        &prepared_candidate.mapped_model,
    ) {
        mark_skipped_local_openai_chat_candidate_with_extra_data(
            state,
            input,
            trace_id,
            candidate,
            candidate_index,
            candidate_id,
            "provider_request_body_build_failed",
            Some(extra_data),
        )
        .await;
        return Ok(None);
    }

    let Some(upstream_url) = build_cross_format_openai_chat_upstream_url(
        parts,
        transport,
        &prepared_candidate.mapped_model,
        provider_api_format.as_str(),
        upstream_is_stream,
    ) else {
        mark_skipped_local_openai_chat_candidate_with_failure_diagnostic(
            state,
            input,
            trace_id,
            candidate,
            candidate_index,
            candidate_id,
            "upstream_url_missing",
            CandidateFailureDiagnostic::upstream_url_missing(
                "openai:chat",
                provider_api_format.as_str(),
                "openai_chat_cross_format_url",
            ),
        )
        .await;
        return Ok(None);
    };
    let Some(resolved_headers) =
        build_standard_provider_request_headers(StandardProviderRequestHeadersInput {
            transport,
            provider_api_format: provider_api_format.as_str(),
            same_format: false,
            headers: effective_headers,
            auth_header: &prepared_candidate.auth_header,
            auth_value: &prepared_candidate.auth_value,
            extra_headers: &BTreeMap::new(),
            header_rules: transport.endpoint.header_rules.as_ref(),
            provider_request_body: &provider_request_body,
            original_request_body: body_json,
            upstream_is_stream,
        })
    else {
        mark_skipped_local_openai_chat_candidate_with_failure_diagnostic(
            state,
            input,
            trace_id,
            candidate,
            candidate_index,
            candidate_id,
            "transport_header_rules_apply_failed",
            CandidateFailureDiagnostic::header_rules_apply_failed(
                "openai:chat",
                provider_api_format.as_str(),
                "openai_chat_cross_format_headers",
            ),
        )
        .await;
        return Ok(None);
    };
    let mut provider_request_headers = resolved_headers.headers;
    request_identity_response_encoding_when_redacted(
        &mut provider_request_headers,
        redaction.redacted,
    );

    let resolved_report_kind = if decision_kind == OPENAI_CHAT_STREAM_PLAN_KIND {
        "openai_chat_stream_success".to_string()
    } else {
        "openai_chat_sync_finalize".to_string()
    };
    let (execution_strategy, conversion_mode) =
        ai_local_execution_contract_for_formats("openai:chat", provider_api_format.as_str());

    Ok(Some(LocalOpenAiChatCandidatePayloadParts {
        client_api_format: "openai:chat".to_string(),
        auth_header: resolved_headers.auth_header,
        auth_value: resolved_headers.auth_value,
        mapped_model: prepared_candidate.mapped_model,
        provider_api_format,
        provider_request_body,
        provider_request_headers,
        upstream_url,
        execution_strategy,
        conversion_mode,
        report_kind: resolved_report_kind,
        envelope_name: None,
        transport: Arc::clone(transport),
        request_redacted: redaction.redacted,
        transport_profile: None,
        image_request_summary: None,
    }))
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
async fn resolve_openai_chat_to_openai_image_payload_parts(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    body_json: &serde_json::Value,
    input: &LocalOpenAiChatDecisionInput,
    eligible: &EligibleLocalExecutionCandidate,
    candidate_index: u32,
    candidate_id: &str,
    upstream_is_stream: bool,
) -> Result<Option<LocalOpenAiChatCandidatePayloadParts>, GatewayError> {
    let candidate = &eligible.candidate;
    let transport = &eligible.transport;
    let provider_api_format = "openai:image";
    if let Some(skip_reason) =
        openai_image_transport_unsupported_reason(transport, provider_api_format)
    {
        mark_skipped_local_openai_chat_candidate(
            state,
            input,
            trace_id,
            candidate,
            candidate_index,
            candidate_id,
            skip_reason,
        )
        .await;
        return Ok(None);
    }

    let prepared_candidate = match prepare_header_authenticated_candidate(
        crate::ai_serving::PlannerAppState::new(state),
        transport,
        candidate,
        resolve_openai_image_auth(transport),
        OauthPreparationContext {
            trace_id,
            api_format: provider_api_format,
            operation: "openai_chat_image_bridge",
        },
    )
    .await
    {
        Ok(prepared) => prepared,
        Err(skip_reason) => {
            mark_skipped_local_openai_chat_candidate(
                state,
                input,
                trace_id,
                candidate,
                candidate_index,
                candidate_id,
                skip_reason,
            )
            .await;
            return Ok(None);
        }
    };

    let upstream_is_stream =
        crate::ai_serving::planner::common::resolve_upstream_is_stream_for_provider(
            transport.endpoint.config.as_ref(),
            provider_api_format,
            upstream_is_stream,
            false,
        );
    let Some((mut provider_request_body, image_request_summary)) =
        build_openai_image_provider_body_from_openai_chat_body(
            body_json,
            &prepared_candidate.mapped_model,
            upstream_is_stream,
        )
    else {
        mark_skipped_local_openai_chat_candidate_with_extra_data(
            state,
            input,
            trace_id,
            candidate,
            candidate_index,
            candidate_id,
            "provider_request_body_build_failed",
            request_body_build_failure_extra_data(body_json, "openai:chat", provider_api_format),
        )
        .await;
        return Ok(None);
    };
    let Some(operation) = openai_image_operation_from_summary(&image_request_summary) else {
        return Ok(None);
    };
    {
        let projected = project_openai_image_api_request_body(
            &provider_request_body,
            &prepared_candidate.mapped_model,
            operation,
            crate::image_capabilities::openai_image_provider_max_generation_count_for_model(
                transport.provider.provider_type.as_str(),
                Some(prepared_candidate.mapped_model.as_str()),
            ),
        );
        let Some(projected) = projected else {
            mark_skipped_local_openai_chat_candidate_with_extra_data(
                state,
                input,
                trace_id,
                candidate,
                candidate_index,
                candidate_id,
                "provider_request_body_build_failed",
                request_body_build_failure_extra_data(
                    body_json,
                    "openai:chat",
                    provider_api_format,
                ),
            )
            .await;
            return Ok(None);
        };
        provider_request_body = projected;
    }

    let upstream_url = {
        let request_path = match operation {
            OpenAiImageOperation::Generate => "/v1/images/generations",
            OpenAiImageOperation::Edit => "/v1/images/edits",
        };
        build_openai_image_upstream_url(transport, Some(request_path), parts.uri.query())
    };
    let Some(mut provider_request_headers) =
        build_openai_image_headers(ProviderOpenAiImageHeadersInput {
            transport,
            headers: &parts.headers,
            auth_header: &prepared_candidate.auth_header,
            auth_value: &prepared_candidate.auth_value,
            accept: if upstream_is_stream {
                Some("text/event-stream")
            } else {
                Some("application/json")
            },
            header_rules: transport.endpoint.header_rules.as_ref(),
            provider_request_body: &provider_request_body,
            original_request_body: body_json,
        })
    else {
        mark_skipped_local_openai_chat_candidate_with_failure_diagnostic(
            state,
            input,
            trace_id,
            candidate,
            candidate_index,
            candidate_id,
            "transport_header_rules_apply_failed",
            CandidateFailureDiagnostic::header_rules_apply_failed(
                "openai:chat",
                provider_api_format,
                "openai_chat_image_bridge_headers",
            ),
        )
        .await;
        return Ok(None);
    };

    let (execution_strategy, conversion_mode) =
        ai_local_execution_contract_for_formats("openai:chat", provider_api_format);

    Ok(Some(LocalOpenAiChatCandidatePayloadParts {
        client_api_format: "openai:chat".to_string(),
        auth_header: prepared_candidate.auth_header,
        auth_value: prepared_candidate.auth_value,
        mapped_model: prepared_candidate.mapped_model,
        provider_api_format: provider_api_format.to_string(),
        provider_request_body,
        provider_request_headers,
        upstream_url,
        execution_strategy,
        conversion_mode,
        report_kind: "openai_chat_stream_success".to_string(),
        envelope_name: None,
        transport: Arc::clone(transport),
        request_redacted: false,
        transport_profile: None,
        image_request_summary: Some(image_request_summary),
    }))
}

fn build_openai_image_provider_body_from_openai_chat_body(
    body_json: &Value,
    requested_model: &str,
    upstream_is_stream: bool,
) -> Option<(Value, Value)> {
    let (prompt, images) = collect_openai_chat_image_prompt_and_images(body_json)?;
    let operation = if images.is_empty() {
        "generate"
    } else {
        "edit"
    };
    let mut image_options = serde_json::Map::new();
    copy_openai_chat_image_option(body_json, &mut image_options, "size");
    copy_openai_chat_image_option(body_json, &mut image_options, "quality");
    copy_openai_chat_image_option(body_json, &mut image_options, "background");
    copy_openai_chat_image_option(body_json, &mut image_options, "output_format");
    copy_openai_chat_image_option(body_json, &mut image_options, "output_compression");
    copy_openai_chat_image_option(body_json, &mut image_options, "moderation");
    copy_openai_chat_image_option(body_json, &mut image_options, "input_fidelity");
    copy_openai_chat_image_option(body_json, &mut image_options, "partial_images");

    let mut body = serde_json::Map::new();
    let requested_model = requested_model.trim();
    if requested_model.is_empty() {
        return None;
    }
    body.insert(
        "model".to_string(),
        Value::String(requested_model.to_string()),
    );
    body.insert("prompt".to_string(), Value::String(prompt));
    body.extend(image_options.clone());
    if operation == "edit" {
        let image_urls = openai_image_inputs_as_api_urls(&images);
        if image_urls.len() != images.len() {
            return None;
        }
        body.insert("images".to_string(), Value::Array(image_urls));
    }
    if upstream_is_stream {
        body.insert("stream".to_string(), Value::Bool(true));
    }
    if let Some(user) = body_json
        .get("user")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        body.insert("user".to_string(), Value::String(user.to_string()));
    }

    let mut summary = serde_json::Map::new();
    summary.insert(
        "operation".to_string(),
        Value::String(operation.to_string()),
    );
    for key in ["output_format", "partial_images", "size", "quality"] {
        if let Some(value) = image_options.get(key) {
            summary.insert(key.to_string(), value.clone());
        }
    }
    Some((Value::Object(body), Value::Object(summary)))
}

fn openai_image_operation_from_summary(summary: &Value) -> Option<OpenAiImageOperation> {
    match summary.get("operation")?.as_str()? {
        "generate" => Some(OpenAiImageOperation::Generate),
        "edit" => Some(OpenAiImageOperation::Edit),
        _ => None,
    }
}

fn copy_openai_chat_image_option(
    body_json: &Value,
    image_options: &mut serde_json::Map<String, Value>,
    key: &str,
) {
    if let Some(value) = body_json.get(key) {
        image_options.insert(key.to_string(), value.clone());
    }
}

fn collect_openai_chat_image_prompt_and_images(body_json: &Value) -> Option<(String, Vec<Value>)> {
    let messages = body_json.get("messages").and_then(Value::as_array)?;
    let mut prompt_parts = Vec::new();
    let mut images = Vec::new();
    for message in messages.iter().filter_map(Value::as_object) {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        let content = message.get("content");
        if matches!(role, "system" | "developer" | "user") {
            if let Some(text) = crate::ai_serving::extract_openai_text_content(content)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
            {
                prompt_parts.push(text);
            }
        }
        if role == "user" {
            collect_openai_chat_image_inputs(content, &mut images);
        }
    }
    let prompt = prompt_parts.join("\n").trim().to_string();
    (!prompt.is_empty()).then_some((prompt, images))
}

fn collect_openai_chat_image_inputs(content: Option<&Value>, images: &mut Vec<Value>) {
    let Some(parts) = content.and_then(Value::as_array) else {
        return;
    };
    for part in parts.iter().filter_map(Value::as_object) {
        let part_type = part
            .get("type")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        if matches!(part_type, "image_url" | "input_image") {
            if let Some(url) = part
                .get("image_url")
                .and_then(|value| {
                    value
                        .as_str()
                        .or_else(|| value.get("url").and_then(Value::as_str))
                })
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                images.push(serde_json::json!({
                    "type": "input_image",
                    "image_url": url,
                }));
            } else if let Some(file_id) = part
                .get("file_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                images.push(serde_json::json!({
                    "type": "input_image",
                    "file_id": file_id,
                }));
            }
        }
    }
}

fn openai_image_inputs_as_urls(images: &[Value]) -> Vec<Value> {
    images
        .iter()
        .filter_map(|image| {
            image
                .get("image_url")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| Value::String(value.to_string()))
        })
        .collect()
}

fn openai_image_inputs_as_api_urls(images: &[Value]) -> Vec<Value> {
    images
        .iter()
        .filter_map(|image| {
            image
                .get("image_url")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| json!({ "image_url": value }))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
mod tests {

    fn sample_custom_deepseek_responses_transport() -> GatewayProviderTransportSnapshot {
        GatewayProviderTransportSnapshot {
            provider: GatewayProviderTransportProvider {
                id: "provider-1".to_string(),
                name: "deepseek".to_string(),
                provider_type: "custom".to_string(),
                website: None,
                is_active: true,
                enable_format_conversion: true,
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
                api_format: "openai:responses".to_string(),
                api_family: Some("openai".to_string()),
                endpoint_kind: Some("responses".to_string()),
                is_active: true,
                base_url: "https://api.deepseek.com/v1".to_string(),
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
                name: "key".to_string(),
                auth_type: "bearer".to_string(),
                is_active: true,
                api_formats: Some(vec!["openai:responses".to_string()]),
                auth_type_by_format: None,
                allow_auth_channel_mismatch_formats: None,
                allowed_models: None,
                capabilities: None,
                rate_multipliers: None,
                expires_at_unix_secs: None,
                proxy: None,
                fingerprint: None,
                upstream_metadata: None,
                decrypted_api_key: "test-api-key".to_string(),
                decrypted_auth_config: None,
            },
        }
    }
    use super::*;
    use aether_provider_transport::snapshot::{
        GatewayProviderTransportEndpoint, GatewayProviderTransportKey,
        GatewayProviderTransportProvider, GatewayProviderTransportSnapshot,
    };
    use aether_scheduler_core::SchedulerMinimalCandidateSelectionCandidate;

    fn sample_auth_snapshot() -> crate::ai_serving::GatewayAuthApiKeySnapshot {
        crate::ai_serving::GatewayAuthApiKeySnapshot {
            user_id: "user-1".to_string(),
            username: "alice".to_string(),
            email: None,
            user_role: "user".to_string(),
            user_auth_source: "local".to_string(),
            user_is_active: true,
            user_is_deleted: false,
            user_rate_limit: None,
            user_allowed_providers: None,
            user_allowed_api_formats: None,
            user_allowed_models: None,
            api_key_id: "api-key-1".to_string(),
            api_key_name: Some("default".to_string()),
            api_key_is_active: true,
            api_key_is_locked: false,
            api_key_is_standalone: false,
            api_key_rate_limit: None,
            api_key_concurrent_limit: None,
            api_key_expires_at_unix_secs: None,
            api_key_allowed_providers: None,
            api_key_allowed_api_formats: None,
            api_key_allowed_models: None,
            api_key_ip_rules: None,
            currently_usable: true,
        }
    }

    fn sample_input() -> LocalOpenAiChatDecisionInput {
        LocalOpenAiChatDecisionInput {
            auth_context: crate::ai_serving::ExecutionRuntimeAuthContext {
                user_id: "user-1".to_string(),
                api_key_id: "api-key-1".to_string(),
                username: Some("alice".to_string()),
                api_key_name: Some("default".to_string()),
                balance_remaining: Some(10.0),
                access_allowed: true,
                api_key_is_standalone: false,
            },
            requested_model: "gemini-2.5-pro".to_string(),
            auth_snapshot: sample_auth_snapshot(),
            required_capabilities: None,
            request_auth_channel: None,
            client_surface: None,
            gateway_credential_carrier: None,
            client_session_affinity: None,
            original_client_session_id: None,
            routing_policy: None,
            routing_trace_seed: None,
            routing_context: None,
            model_directive_policy: Default::default(),
        }
    }

    fn sample_custom_directive_input() -> LocalOpenAiChatDecisionInput {
        let mut input = sample_input();
        input.requested_model = "gpt-5.6-sol-high".to_string();
        input.model_directive_policy =
            crate::system_features::ModelDirectivePolicySnapshot::from_config_values(
                Some(&json!(true)),
                Some(&json!({
                    "reasoning_effort": {
                        "api_formats": {
                            "openai:chat": {
                                "suffixes": ["high"],
                                "mappings": {
                                    "high": {
                                        "reasoning_effort": "low",
                                        "stream": false
                                    }
                                }
                            }
                        }
                    }
                })),
            );
        input
    }

    #[test]
    fn responses_shaped_chat_request_preserves_deepseek_opaque_reasoning_at_finalization() {
        let reasoning_items = (0..66)
            .map(|index| {
                json!({
                    "type": "reasoning",
                    "encrypted_content": format!("opaque-deepseek-state-{index}"),
                    "content": [{
                        "type": "reasoning_text",
                        "text": format!("provider thinking state {index}")
                    }],
                    "future_capability": {"preserve": true}
                })
            })
            .collect::<Vec<_>>();
        let original_body = json!({
            "model": "deepseek-v4-flash",
            "input": reasoning_items
        });
        let transport = sample_custom_deepseek_responses_transport();
        let mut provider_body = build_cross_format_openai_chat_request_body(
            &original_body,
            "deepseek-v4-flash",
            "custom",
            "openai:responses",
            false,
            false,
            None,
            None,
            &http::HeaderMap::new(),
            false,
        )
        .expect("Responses-shaped chat request should build");

        assert!(finalize_openai_chat_provider_request_body(
            &mut provider_body,
            None,
            "openai:responses",
            false,
            false,
            &original_body,
            &transport,
            "deepseek-v4-flash",
        )
        .is_none());
        let input = provider_body["input"].as_array().expect("provider input");
        assert_eq!(input.len(), 66);
        assert_eq!(input[0]["type"], "reasoning");
        assert_eq!(input[0]["content"][0]["type"], "reasoning_text");
        assert_eq!(input[0]["future_capability"]["preserve"], true);
    }

    fn sample_alias_max_directive_input() -> LocalOpenAiChatDecisionInput {
        let mut input = sample_input();
        input.requested_model = "deployment-alias-max".to_string();
        input.model_directive_policy =
            crate::system_features::ModelDirectivePolicySnapshot::from_config_values(
                Some(&json!(true)),
                None,
            );
        input
    }

    #[test]
    fn openai_chat_image_bridge_builds_images_api_body() {
        let body_json = json!({
            "model": "gpt-image-2",
            "messages": [
                {"role": "user", "content": "Draw a glass city"}
            ],
            "size": "1024x1024",
            "output_format": "png"
        });

        let (provider_body, summary) =
            build_openai_image_provider_body_from_openai_chat_body(&body_json, "gpt-image-2", true)
                .expect("chat image body should convert");

        assert_eq!(provider_body["model"], "gpt-image-2");
        assert_eq!(provider_body["prompt"], "Draw a glass city");
        assert_eq!(provider_body["size"], "1024x1024");
        assert_eq!(provider_body["output_format"], "png");
        assert_eq!(provider_body["stream"], true);
        assert!(provider_body.get("tools").is_none());
        assert!(provider_body.get("input").is_none());
        assert_eq!(summary["operation"], "generate");
        assert_eq!(summary["output_format"], "png");

        let (sync_provider_body, _) = build_openai_image_provider_body_from_openai_chat_body(
            &body_json,
            "gpt-image-2",
            false,
        )
        .expect("chat image body should convert for a sync upstream");
        assert!(sync_provider_body.get("stream").is_none());
    }
}
