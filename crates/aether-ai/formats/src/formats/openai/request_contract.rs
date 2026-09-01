use serde_json::Value;

use super::prompt_cache::OpenAiPromptCacheContractViolation;
use super::reasoning::OpenAiReasoningContractViolation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenAiProviderRequestContractViolation {
    Responses(super::responses::request::OpenAiResponsesRequestContractViolation),
    PromptCache(OpenAiPromptCacheContractViolation),
    Reasoning(OpenAiReasoningContractViolation),
}

#[derive(Clone, Copy, Debug)]
pub struct OpenAiProviderRequestFinalization<'a> {
    pub source_api_format: &'a str,
    pub provider_api_format: &'a str,
    pub provider_type: &'a str,
    pub provider_model: &'a str,
    pub source_model: &'a str,
    pub body_rules: Option<&'a Value>,
    pub upstream_is_stream: bool,
    pub require_body_stream_field: bool,
}

pub fn finalize_openai_provider_request(
    body: &mut Value,
    finalization: OpenAiProviderRequestFinalization<'_>,
) -> Result<(), OpenAiProviderRequestContractViolation> {
    finalize_openai_provider_request_with_reasoning_replay_policy(
        body,
        finalization,
        super::responses::OpenAiResponsesReasoningReplayPolicy::OpenAiItemIds,
    )
}

pub fn finalize_openai_provider_request_with_reasoning_replay_policy(
    body: &mut Value,
    finalization: OpenAiProviderRequestFinalization<'_>,
    reasoning_replay_policy: super::responses::OpenAiResponsesReasoningReplayPolicy,
) -> Result<(), OpenAiProviderRequestContractViolation> {
    finalize_openai_provider_request_with_reasoning_replay_policy_inner(
        body,
        finalization,
        reasoning_replay_policy,
        false,
    )
}

pub fn finalize_openai_provider_request_with_reasoning_replay_policy_for_websocket_continuation(
    body: &mut Value,
    finalization: OpenAiProviderRequestFinalization<'_>,
    reasoning_replay_policy: super::responses::OpenAiResponsesReasoningReplayPolicy,
) -> Result<(), OpenAiProviderRequestContractViolation> {
    finalize_openai_provider_request_with_reasoning_replay_policy_inner(
        body,
        finalization,
        reasoning_replay_policy,
        true,
    )
}

fn finalize_openai_provider_request_with_reasoning_replay_policy_inner(
    body: &mut Value,
    finalization: OpenAiProviderRequestFinalization<'_>,
    reasoning_replay_policy: super::responses::OpenAiResponsesReasoningReplayPolicy,
    _websocket_continuation: bool,
) -> Result<(), OpenAiProviderRequestContractViolation> {
    if crate::is_openai_responses_compact_format(finalization.provider_api_format) {
        if let Some(body_object) = body.as_object_mut() {
            super::responses::request::apply_compact_request_projection(body_object);
        }
    }
    super::responses::strip_incompatible_openai_responses_reasoning_items_with_policy(
        body,
        finalization.provider_api_format,
        reasoning_replay_policy,
    );
    crate::enforce_request_body_stream_field(
        body,
        finalization.provider_api_format,
        finalization.upstream_is_stream,
        finalization.require_body_stream_field,
    );
    super::search::apply_openai_search_request_projection(body, finalization.provider_api_format);
    let provider_model = body
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(finalization.provider_model);
    validate_final_openai_provider_request_contract(
        finalization.provider_api_format,
        provider_model,
        finalization.source_model,
        body,
    )
}

pub fn validate_openai_provider_request_contract(
    provider_api_format: &str,
    provider_model: &str,
    source_model: &str,
    body: &Value,
) -> Result<(), OpenAiProviderRequestContractViolation> {
    validate_final_openai_provider_request_contract(
        provider_api_format,
        provider_model,
        source_model,
        body,
    )
}

fn validate_final_openai_provider_request_contract(
    provider_api_format: &str,
    provider_model: &str,
    source_model: &str,
    body: &Value,
) -> Result<(), OpenAiProviderRequestContractViolation> {
    super::responses::request::validate_openai_responses_request_contract(
        body,
        provider_api_format,
    )
    .map_err(OpenAiProviderRequestContractViolation::Responses)?;
    super::prompt_cache::validate_openai_prompt_cache_request_with_source_model(
        provider_api_format,
        provider_model,
        source_model,
        body,
    )
    .map_err(OpenAiProviderRequestContractViolation::PromptCache)?;
    super::reasoning::validate_openai_reasoning_request_with_model_profile(
        provider_api_format,
        provider_api_format,
        provider_model,
        source_model,
        body,
        None,
    )
    .map_err(OpenAiProviderRequestContractViolation::Reasoning)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        finalize_openai_provider_request, validate_openai_provider_request_contract,
        OpenAiProviderRequestFinalization,
    };

    fn finalization_for(
        source_api_format: &'static str,
        provider_api_format: &'static str,
    ) -> OpenAiProviderRequestFinalization<'static> {
        OpenAiProviderRequestFinalization {
            source_api_format,
            provider_api_format,
            provider_type: "custom",
            provider_model: "gpt-5.6-sol",
            source_model: "gpt-5.6-sol",
            body_rules: None,
            upstream_is_stream: false,
            require_body_stream_field: true,
        }
    }

    #[test]
    fn validates_reasoning_and_prompt_cache_against_the_final_provider_model() {
        let body = json!({
            "model": "gpt-5.6-sol",
            "input": [],
            "reasoning": {"effort": "max"},
            "prompt_cache_options": {"mode": "explicit", "ttl": "30m"}
        });
        validate_openai_provider_request_contract(
            "openai:responses",
            "gpt-5.6-sol",
            "gpt-5.6-sol",
            &body,
        )
        .expect("GPT-5.6 request should satisfy the final provider contract");

        assert!(validate_openai_provider_request_contract(
            "openai:responses",
            "gpt-5.4",
            "gpt-5.6-sol",
            &body,
        )
        .is_err());
    }

    #[test]
    fn opaque_provider_models_inherit_source_capabilities_but_concrete_models_do_not() {
        let body = json!({
            "model": "azure-production",
            "input": [],
            "reasoning": {"effort": "max", "mode": "pro"},
            "prompt_cache_options": {"mode": "explicit", "ttl": "30m"}
        });
        validate_openai_provider_request_contract(
            "openai:responses",
            "azure-production",
            "gpt-5.6-sol-max",
            &body,
        )
        .expect("opaque deployments should inherit the concrete source model capability");
        assert!(validate_openai_provider_request_contract(
            "openai:responses",
            "gpt-5.4",
            "gpt-5.6-sol",
            &body,
        )
        .is_err());
    }

    #[test]
    fn finalization_reapplies_compact_projection_after_mutations() {
        let mut body = json!({
            "model": "gpt-5.6-sol",
            "input": [],
            "store": true,
            "include": ["reasoning.encrypted_content"],
            "client_metadata": {"source": "mapping"},
            "stream": true,
            "stream_options": {"include_usage": true},
            "tool_choice": "auto",
            "temperature": 0.5,
            "previous_response_id": "resp_123"
        });
        finalize_openai_provider_request(
            &mut body,
            finalization_for("openai:responses", "openai:responses:compact"),
        )
        .expect("final Compact request should satisfy its provider contract");

        for field in [
            "store",
            "include",
            "client_metadata",
            "stream",
            "stream_options",
            "tool_choice",
            "temperature",
            "previous_response_id",
        ] {
            assert!(body.get(field).is_none(), "{field} must not reach Compact");
        }
    }

    #[test]
    fn finalization_strips_non_replayable_responses_reasoning_history() {
        let mut body = json!({
            "model": "gpt-5.4",
            "input": [
                {"type": "reasoning", "id": "rs_provider_123", "summary": []},
                {
                    "type": "reasoning",
                    "id": "item_72d3bd8d367d01977ace23f1",
                    "summary": []
                },
                {"type": "message", "role": "user", "content": "continue"}
            ]
        });

        finalize_openai_provider_request(
            &mut body,
            OpenAiProviderRequestFinalization {
                source_api_format: "openai:responses",
                provider_api_format: "openai:responses",
                provider_type: "custom",
                provider_model: "gpt-5.4",
                source_model: "gpt-5.4",
                body_rules: None,
                upstream_is_stream: false,
                require_body_stream_field: false,
            },
        )
        .expect("foreign reasoning history should be sanitized before validation");

        let input = body["input"].as_array().expect("input array");
        assert_eq!(input.len(), 2);
        assert_eq!(input[0]["id"], "rs_provider_123");
        assert_eq!(input[1]["type"], "message");
    }

    #[test]
    fn cross_format_compact_finalization_removes_post_conversion_fields() {
        for source_api_format in ["claude:messages", "gemini:generate_content"] {
            let mut body = json!({
                "model": "gpt-5.6-sol",
                "input": [],
                "client_metadata": {"source": "mapping"},
                "include": ["reasoning.encrypted_content"],
                "store": true,
                "stream": true,
                "stream_options": {"include_usage": true},
                "tool_choice": "auto",
                "parallel_tool_calls": true,
                "reasoning": {"effort": "max"},
                "text": {"verbosity": "medium"},
                "tools": [{"type": "function", "name": "lookup", "parameters": {}}]
            });
            finalize_openai_provider_request(
                &mut body,
                OpenAiProviderRequestFinalization {
                    source_api_format,
                    provider_api_format: "openai:responses:compact",
                    provider_type: "custom",
                    provider_model: "gpt-5.6-sol",
                    source_model: "gpt-5.6-sol",
                    body_rules: None,
                    upstream_is_stream: false,
                    require_body_stream_field: true,
                },
            )
            .expect("cross-format Compact request should satisfy its final contract");

            for field in [
                "client_metadata",
                "include",
                "store",
                "stream",
                "stream_options",
                "tool_choice",
            ] {
                assert!(body.get(field).is_none(), "{field} must not reach Compact");
            }
        }
    }
}
