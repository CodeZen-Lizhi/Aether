use aether_provider_transport::snapshot::{
    GatewayProviderTransportEndpoint, GatewayProviderTransportKey,
    GatewayProviderTransportProvider, GatewayProviderTransportSnapshot,
};
use http::Request;
use serde_json::{json, Value};

use super::{
    build_cross_format_openai_responses_request_body, build_local_openai_chat_request_body,
    build_local_openai_responses_request_body, build_local_openai_responses_upstream_url,
};

fn object_keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .expect("json object")
        .keys()
        .map(String::as_str)
        .collect()
}

fn sample_transport(base_url: &str, api_format: &str) -> GatewayProviderTransportSnapshot {
    GatewayProviderTransportSnapshot {
        provider: GatewayProviderTransportProvider {
            id: "provider-codex".to_string(),
            name: "codex".to_string(),
            provider_type: "codex".to_string(),
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
            id: "endpoint-codex".to_string(),
            provider_id: "provider-codex".to_string(),
            api_format: api_format.to_string(),
            api_family: Some("openai".to_string()),
            endpoint_kind: Some("cli".to_string()),
            is_active: true,
            base_url: base_url.to_string(),
            header_rules: None,
            body_rules: None,
            max_retries: None,
            custom_path: None,
            config: None,
            format_acceptance_config: None,
            proxy: None,
        },
        key: GatewayProviderTransportKey {
            id: "key-codex".to_string(),
            provider_id: "provider-codex".to_string(),
            name: "oauth".to_string(),
            auth_type: "oauth".to_string(),
            is_active: true,
            api_formats: Some(vec![api_format.to_string()]),
            auth_type_by_format: None,
            allow_auth_channel_mismatch_formats: None,
            allowed_models: None,
            capabilities: None,
            rate_multipliers: None,
            expires_at_unix_secs: None,
            proxy: None,
            fingerprint: None,
            upstream_metadata: None,
            decrypted_api_key: "__placeholder__".to_string(),
            decrypted_auth_config: None,
        },
    }
}

#[test]
fn builds_openai_chat_cross_format_request_body_from_openai_responses_source() {
    let body_json = json!({
        "model": "gpt-5",
        "input": "hello",
    });

    let provider_request_body = build_cross_format_openai_responses_request_body(
        &body_json,
        "gpt-5-upstream",
        "openai:responses",
        "openai:chat",
        false,
        false,
        "openai",
        None,
        None,
        &http::HeaderMap::new(),
        false,
    )
    .expect("openai responses to openai chat body should build");

    assert_eq!(provider_request_body["model"], "gpt-5-upstream");
    assert_eq!(provider_request_body["messages"][0]["role"], "user");
    assert_eq!(provider_request_body["messages"][0]["content"], "hello");
}

#[test]
fn maps_openai_responses_additional_tools_without_message_name() {
    let body_json = json!({
        "model": "gpt-5",
        "input": [
            {
                "type": "additional_tools",
                "role": "developer",
                "tools": [{
                    "type": "function",
                    "name": "get_weather",
                    "description": "Get the weather",
                    "parameters": {
                        "type": "object",
                        "properties": {}
                    }
                }]
            },
            {
                "role": "user",
                "content": "What is the weather?"
            }
        ]
    });

    let provider_request_body = build_cross_format_openai_responses_request_body(
        &body_json,
        "gpt-5-upstream",
        "openai:responses",
        "openai:chat",
        false,
        false,
        "openai",
        None,
        None,
        &http::HeaderMap::new(),
        false,
    )
    .expect("Responses additional tools should map to a Chat request body");

    assert_eq!(
        provider_request_body["messages"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(provider_request_body["messages"][0]["role"], "user");
    assert!(provider_request_body["messages"][0].get("name").is_none());
    assert_eq!(provider_request_body["tools"][0]["type"], "function");
    assert_eq!(
        provider_request_body["tools"][0]["function"]["name"],
        "get_weather"
    );
}

#[test]
fn local_openai_responses_wrapper_preserves_body_order_after_edits() {
    let body_json: Value = serde_json::from_str(
        r#"{
            "text": {"format": {"type": "text"}},
            "input": [],
            "model": "gpt-5.4",
            "store": false,
            "tools": [],
            "stream": true,
            "include": ["reasoning.encrypted_content"],
            "reasoning": {"effort": "high"},
            "tool_choice": "auto"
        }"#,
    )
    .expect("request body should parse");

    let provider_request_body = build_local_openai_responses_request_body(
        &body_json,
        "gpt-5.4",
        true,
        false,
        "custom",
        "openai:responses",
        None,
        None,
        &http::HeaderMap::new(),
        false,
    )
    .expect("local openai responses body should build");

    assert_eq!(
        object_keys(&provider_request_body),
        vec![
            "text",
            "input",
            "model",
            "store",
            "tools",
            "stream",
            "include",
            "reasoning",
            "tool_choice"
        ]
    );
    assert!(provider_request_body.get("parallel_tool_calls").is_none());
    assert!(provider_request_body.get("instructions").is_none());
}

#[test]
fn local_openai_responses_wrapper_defers_reasoning_replay_filtering() {
    let body_json = json!({
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

    let provider_request_body = build_local_openai_responses_request_body(
        &body_json,
        "gpt-5.4",
        false,
        false,
        "codex",
        "openai:responses",
        None,
        None,
        &http::HeaderMap::new(),
        false,
    )
    .expect("local OpenAI Responses body should build");

    let input = provider_request_body["input"]
        .as_array()
        .expect("input array");
    // This provider-agnostic normalization layer cannot decide whether an
    // id-less/foreign reasoning item is opaque state required by DeepSeek.
    // The provider-aware finalization pass applies the strict or DeepSeek
    // replay policy once the selected upstream base URL is known.
    assert_eq!(input.len(), 3);
    assert_eq!(input[0]["id"], "rs_provider_123");
    assert_eq!(input[1]["id"], "item_72d3bd8d367d01977ace23f1");
    assert_eq!(input[2]["type"], "message");
}

#[test]
fn local_openai_responses_compact_wrapper_preserves_body_for_same_format_requests() {
    let body_json = json!({
        "model": "gpt-5.4",
        "input": [],
        "store": true
    });

    let provider_request_body = build_local_openai_responses_request_body(
        &body_json,
        "gpt-5.4",
        false,
        false,
        "openai",
        "openai:responses:compact",
        None,
        None,
        &http::HeaderMap::new(),
        false,
    )
    .expect("local openai compact body should build");

    // codex 客户端仿真移除后，compact 格式对 custom 供应商走通用透传。
    assert_eq!(provider_request_body.get("store"), Some(&json!(true)));
    assert!(provider_request_body.get("stream").is_none());
}

#[test]
fn local_openai_responses_wrapper_applies_model_directive_before_body_rules() {
    let body_json = json!({
        "model": "gpt-5.6-sol-max",
        "input": "hello",
        "reasoning": {"effort": "low", "summary": "auto"}
    });
    let body_rules = json!([
        {"action":"set","path":"metadata.override_seen","value":true}
    ]);

    let provider_request_body = build_local_openai_responses_request_body(
        &body_json,
        "gpt-5.6-sol",
        false,
        false,
        "openai",
        "openai:responses",
        Some(&body_rules),
        None,
        &http::HeaderMap::new(),
        true,
    )
    .expect("local openai responses body should build");

    assert_eq!(provider_request_body["reasoning"]["effort"], "max");
    assert_eq!(provider_request_body["reasoning"]["summary"], "auto");
    assert_eq!(provider_request_body["metadata"]["override_seen"], true);
}

#[test]
fn final_openai_provider_contract_uses_the_mapped_model_for_reasoning() {
    let alias = json!({
        "model": "deployment-alias",
        "input": "hello",
        "reasoning": {"effort": "max"}
    });
    assert!(build_local_openai_responses_request_body(
        &alias,
        "gpt-5.6-sol",
        false,
        false,
        "openai",
        "openai:responses",
        None,
        None,
        &http::HeaderMap::new(),
        false,
    )
    .is_some());
    let remapped = build_local_openai_responses_request_body(
        &alias,
        "gpt-5.4",
        false,
        false,
        "openai",
        "openai:responses",
        None,
        None,
        &http::HeaderMap::new(),
        false,
    )
    .expect("explicit reasoning effort should pass through to the mapped model");
    assert_eq!(remapped["reasoning"]["effort"], "max");

    let minimal = json!({
        "model": "deployment-alias",
        "messages": [{"role": "user", "content": "hello"}],
        "reasoning_effort": "minimal"
    });
    let minimal = build_local_openai_chat_request_body(
        &minimal,
        "gpt-5.6-terra",
        false,
        false,
        None,
        &http::HeaderMap::new(),
        false,
    )
    .expect("explicit chat reasoning effort should be validated by the upstream");
    assert_eq!(minimal["reasoning_effort"], "minimal");

    let opaque_mapping = json!({
        "model": "gpt-5.6-sol-max",
        "input": "hello",
        "reasoning": {"effort": "max", "mode": "pro"},
        "prompt_cache_options": {"mode": "explicit", "ttl": "30m"}
    });
    assert!(build_local_openai_responses_request_body(
        &opaque_mapping,
        "azure-production",
        false,
        false,
        "openai",
        "openai:responses",
        None,
        None,
        &http::HeaderMap::new(),
        false,
    )
    .is_some());
    assert!(build_local_openai_responses_request_body(
        &opaque_mapping,
        "gpt-5.4",
        false,
        false,
        "openai",
        "openai:responses",
        None,
        None,
        &http::HeaderMap::new(),
        false,
    )
    .is_none());
}

#[test]
fn final_openai_provider_contract_validates_body_rule_output() {
    let body = json!({
        "model": "gpt-5.6-sol",
        "input": "hello",
        "reasoning": {"effort": "max"}
    });
    let model_override = json!([
        {"action":"set","path":"model","value":"gpt-5.4"}
    ]);
    let provider_request = build_local_openai_responses_request_body(
        &body,
        "gpt-5.6-sol",
        false,
        false,
        "openai",
        "openai:responses",
        Some(&model_override),
        None,
        &http::HeaderMap::new(),
        false,
    )
    .expect("body rule output should preserve explicit reasoning effort");
    assert_eq!(provider_request["model"], "gpt-5.4");
    assert_eq!(provider_request["reasoning"]["effort"], "max");

    let cache_override = json!([
        {"action":"set","path":"prompt_cache_options.ttl","value":"1h"}
    ]);
    assert!(build_local_openai_responses_request_body(
        &json!({"model":"gpt-5.6-sol","input":"hello"}),
        "gpt-5.6-sol",
        false,
        false,
        "openai",
        "openai:responses",
        Some(&cache_override),
        None,
        &http::HeaderMap::new(),
        false,
    )
    .is_none());
}

#[test]
fn local_openai_responses_upstream_url_preserves_codex_base_path() {
    let request = Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .body(())
        .expect("request should build");
    let (parts, _) = request.into_parts();

    let upstream_url = build_local_openai_responses_upstream_url(
        &parts,
        &sample_transport("https://tiger.bookapi.cc/codex", "openai:responses"),
        false,
    )
    .expect("openai responses upstream url should build");

    assert_eq!(upstream_url, "https://tiger.bookapi.cc/codex/responses");
}

#[test]
fn preserves_metadata_for_openai_responses_cross_format_requests() {
    let body_json = json!({
        "model": "claude-sonnet-4-5",
        "metadata": {"trace_id": "abc"},
        "messages": [{
            "role": "user",
            "content": [{"type": "text", "text": "hello"}]
        }],
    });

    let provider_request_body = build_cross_format_openai_responses_request_body(
        &body_json,
        "gpt-5-upstream",
        "claude:messages",
        "openai:responses",
        true,
        false,
        "custom",
        None,
        None,
        &http::HeaderMap::new(),
        false,
    )
    .expect("claude to openai responses request should build");

    // codex 专属的 metadata 剥离已随仿真删除，通用链路保留元数据。
    assert_eq!(
        provider_request_body.get("metadata"),
        Some(&json!({"trace_id": "abc"}))
    );
}

#[test]
fn openai_chat_to_codex_responses_preserves_json_mode_chat_messages() {
    let body_json = json!({
        "model": "gpt-5.5",
        "messages": [
            {"role": "system", "content": "Return a JSON object."},
            {"role": "user", "content": "Why did this JSON request fail?"}
        ],
        "response_format": {"type": "json_object"}
    });

    let provider_request_body = build_cross_format_openai_responses_request_body(
        &body_json,
        "gpt-5.5-upstream",
        "openai:chat",
        "openai:responses",
        false,
        false,
        "codex",
        None,
        None,
        &http::HeaderMap::new(),
        false,
    )
    .expect("openai chat to codex responses request should build");

    assert_eq!(
        provider_request_body["text"]["format"]["type"],
        "json_object"
    );
    assert_eq!(provider_request_body["input"][0]["role"], "user");
    assert_eq!(
        provider_request_body["input"][0]["content"][0]["text"],
        "Why did this JSON request fail?"
    );
    assert_eq!(
        provider_request_body["instructions"],
        "Return a JSON object."
    );
}

#[test]
fn applies_codex_defaults_unless_body_rules_handle_the_field() {
    let body_json = json!({
        "model": "claude-sonnet-4-5",
        "messages": [{
            "role": "user",
            "content": [{"type": "text", "text": "hello"}]
        }],
        "metadata": {"trace_id": "abc"},
        "store": true
    });
    let body_rules = json!([
        {"action":"set","path":"store","value":true},
        {"action":"set","path":"instructions","value":"Custom instructions"},
        {"action":"set","path":"metadata","value":{"trace_id":"keep-me"}}
    ]);

    let provider_request_body = build_cross_format_openai_responses_request_body(
        &body_json,
        "gpt-5-upstream",
        "claude:messages",
        "openai:responses",
        true,
        false,
        "codex",
        Some(&body_rules),
        None,
        &http::HeaderMap::new(),
        false,
    )
    .expect("claude cli to codex request should build");

    assert_eq!(provider_request_body["store"], true);
    assert_eq!(provider_request_body["instructions"], "Custom instructions");
    assert_eq!(provider_request_body["metadata"]["trace_id"], "keep-me");
}

#[test]
fn omits_codex_prompt_cache_key_for_openai_responses_cross_format_requests() {
    let body_json = json!({
        "model": "claude-sonnet-4-5",
        "messages": [{
            "role": "user",
            "content": [{"type": "text", "text": "hello"}]
        }],
    });

    let provider_request_body = build_cross_format_openai_responses_request_body(
        &body_json,
        "gpt-5-upstream",
        "claude:messages",
        "openai:responses",
        true,
        false,
        "codex",
        None,
        None,
        &http::HeaderMap::new(),
        false,
    )
    .expect("claude cli to codex request should build");

    assert!(provider_request_body.get("prompt_cache_key").is_none());
}

#[test]
fn omits_codex_prompt_cache_key_for_openai_chat_cross_format_requests() {
    let body_json = json!({
        "model": "gpt-5",
        "messages": [{
            "role": "user",
            "content": "hello"
        }],
    });

    let provider_request_body = super::build_cross_format_openai_chat_request_body(
        &body_json,
        "gpt-5-upstream",
        "codex",
        "openai:responses",
        false,
        false,
        None,
        None,
        &http::HeaderMap::new(),
        false,
    )
    .expect("openai chat to codex request should build");

    assert!(provider_request_body.get("prompt_cache_key").is_none());
}
