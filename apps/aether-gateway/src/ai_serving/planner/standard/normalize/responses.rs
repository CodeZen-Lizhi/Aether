use serde_json::Value;

use crate::ai_serving::transport::apply_standard_provider_request_body_rules_with_request_headers;
use crate::ai_serving::{
    build_cross_format_openai_responses_request_body_with_model_directives_and_history_scope as surface_build_cross_format_openai_responses_request_body,
    build_local_openai_responses_request_body_with_model_directives as surface_build_local_openai_responses_request_body,
    GatewayProviderTransportSnapshot,
};

use super::{
    enforce_provider_body_stream_policy, request_requires_body_stream_field,
    validate_final_openai_provider_request,
};

pub(crate) fn build_local_openai_responses_request_body(
    body_json: &Value,
    mapped_model: &str,
    upstream_is_stream: bool,
    force_body_stream_field: bool,
    _provider_type: &str,
    provider_api_format: &str,
    body_rules: Option<&Value>,
    _user_api_key_id: Option<&str>,
    request_headers: &http::HeaderMap,
    enable_model_directives: bool,
) -> Option<Value> {
    let provider_request_body = surface_build_local_openai_responses_request_body(
        body_json,
        mapped_model,
        upstream_is_stream,
        enable_model_directives,
    )?;
    let mut provider_request_body =
        apply_standard_provider_request_body_rules_with_request_headers(
            provider_request_body,
            body_rules,
            body_json,
            request_headers,
        )?;
    enforce_provider_body_stream_policy(
        &mut provider_request_body,
        provider_api_format,
        upstream_is_stream,
        request_requires_body_stream_field(body_json, force_body_stream_field),
    );
    validate_final_openai_provider_request(
        provider_api_format,
        mapped_model,
        body_json,
        &provider_request_body,
    )?;
    Some(provider_request_body)
}

/// Builds a Responses body for a pinned WebSocket continuation.
///
/// This is intentionally an additive variant of the ordinary HTTP builder:
/// the WebSocket framing layer, rather than a JSON-body heuristic, decides
/// how `previous_response_id` is treated as transport state.
pub(crate) fn build_local_openai_responses_request_body_for_websocket_continuation(
    body_json: &Value,
    mapped_model: &str,
    upstream_is_stream: bool,
    force_body_stream_field: bool,
    provider_type: &str,
    provider_api_format: &str,
    body_rules: Option<&Value>,
    user_api_key_id: Option<&str>,
    request_headers: &http::HeaderMap,
    enable_model_directives: bool,
) -> Option<Value> {
    build_local_openai_responses_request_body(
        body_json,
        mapped_model,
        upstream_is_stream,
        force_body_stream_field,
        provider_type,
        provider_api_format,
        body_rules,
        user_api_key_id,
        request_headers,
        enable_model_directives,
    )
}

pub(crate) fn build_cross_format_openai_responses_request_body(
    body_json: &Value,
    mapped_model: &str,
    client_api_format: &str,
    provider_api_format: &str,
    upstream_is_stream: bool,
    force_body_stream_field: bool,
    _provider_type: &str,
    body_rules: Option<&Value>,
    user_api_key_id: Option<&str>,
    request_headers: &http::HeaderMap,
    enable_model_directives: bool,
) -> Option<Value> {
    let _ = user_api_key_id;
    let provider_request_body = surface_build_cross_format_openai_responses_request_body(
        body_json,
        mapped_model,
        client_api_format,
        provider_api_format,
        upstream_is_stream,
        enable_model_directives,
        None,
    )?;
    let mut provider_request_body =
        apply_standard_provider_request_body_rules_with_request_headers(
            provider_request_body,
            body_rules,
            body_json,
            request_headers,
        )?;
    enforce_provider_body_stream_policy(
        &mut provider_request_body,
        provider_api_format,
        upstream_is_stream,
        request_requires_body_stream_field(body_json, force_body_stream_field),
    );
    validate_final_openai_provider_request(
        provider_api_format,
        mapped_model,
        body_json,
        &provider_request_body,
    )?;
    Some(provider_request_body)
}

pub(crate) fn build_local_openai_responses_upstream_url(
    parts: &http::request::Parts,
    transport: &GatewayProviderTransportSnapshot,
    compact: bool,
) -> Option<String> {
    crate::ai_serving::transport::build_local_openai_responses_upstream_url(
        transport,
        compact,
        parts.uri.query(),
    )
}

pub(crate) fn build_cross_format_openai_responses_upstream_url(
    parts: &http::request::Parts,
    transport: &GatewayProviderTransportSnapshot,
    mapped_model: &str,
    client_api_format: &str,
    provider_api_format: &str,
    upstream_is_stream: bool,
) -> Option<String> {
    crate::ai_serving::transport::build_cross_format_openai_responses_upstream_url(
        transport,
        mapped_model,
        client_api_format,
        provider_api_format,
        upstream_is_stream,
        parts.uri.query(),
    )
}
