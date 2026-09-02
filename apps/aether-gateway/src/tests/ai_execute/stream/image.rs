use super::{
    any, build_router_with_state, build_state_with_execution_runtime_override, json, start_server,
    to_bytes, Arc, Body, Json, Mutex, Request, Router, StatusCode, TRACE_ID_HEADER,
};
use aether_crypto::{encrypt_python_fernet_plaintext, DEVELOPMENT_ENCRYPTION_KEY};
use aether_data::repository::auth::{
    InMemoryAuthApiKeySnapshotRepository, StoredAuthApiKeySnapshot,
};
use aether_data::repository::candidate_selection::InMemoryMinimalCandidateSelectionReadRepository;
use aether_data::repository::candidates::InMemoryRequestCandidateRepository;
use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
use aether_data_contracts::repository::candidate_selection::{
    StoredMinimalCandidateSelectionRow, StoredProviderModelMapping,
};
use aether_data_contracts::repository::provider_catalog::{
    StoredProviderCatalogEndpoint, StoredProviderCatalogKey, StoredProviderCatalogProvider,
};
use sha2::{Digest, Sha256};

const STREAM_IMAGE_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;

fn run_stream_image_test<F, Fut>(test_name: &'static str, make_future: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + 'static,
{
    let handle = std::thread::Builder::new()
        .name(test_name.to_string())
        .stack_size(STREAM_IMAGE_TEST_STACK_BYTES)
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("test runtime should build");
            runtime.block_on(make_future());
        })
        .expect("stream image test thread should spawn");

    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[derive(Debug, Clone)]
struct SeenImageBridgeExecutionPlan {
    trace_id: String,
    client_api_format: String,
    provider_api_format: String,
    url: String,
    plan_stream: bool,
    auth_header: String,
    body_json: serde_json::Value,
}

fn image_bridge_hash_api_key(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn image_bridge_auth_snapshot(api_key_id: &str, user_id: &str) -> StoredAuthApiKeySnapshot {
    StoredAuthApiKeySnapshot::new(
        user_id.to_string(),
        "alice".to_string(),
        Some("alice@example.com".to_string()),
        "user".to_string(),
        "local".to_string(),
        true,
        false,
        None,
        Some(serde_json::json!([
            "openai:chat",
            "openai:responses",
            "openai:image"
        ])),
        Some(serde_json::json!(["gpt-image-2"])),
        api_key_id.to_string(),
        Some("default".to_string()),
        true,
        false,
        false,
        Some(60),
        Some(5),
        Some(4_102_444_800_i64),
        None,
        Some(serde_json::json!([
            "openai:chat",
            "openai:responses",
            "openai:image"
        ])),
        Some(serde_json::json!(["gpt-image-2"])),
    )
    .expect("auth snapshot should build")
}

fn image_bridge_candidate_row(
    prefix: &str,
    provider_name: &str,
    provider_type: &str,
) -> StoredMinimalCandidateSelectionRow {
    let key_auth_type = if provider_type == "chatgpt_web" {
        "bearer"
    } else {
        "api_key"
    };
    StoredMinimalCandidateSelectionRow {
        provider_id: format!("provider-{prefix}"),
        provider_name: provider_name.to_string(),
        provider_type: provider_type.to_string(),
        provider_is_active: true,
        endpoint_id: format!("endpoint-{prefix}"),
        endpoint_api_format: "openai:image".to_string(),
        endpoint_api_family: Some("openai".to_string()),
        endpoint_kind: Some("image".to_string()),
        endpoint_is_active: true,
        key_id: format!("key-{prefix}"),
        key_name: "prod".to_string(),
        key_auth_type: key_auth_type.to_string(),
        key_is_active: true,
        key_api_formats: Some(vec!["openai:image".to_string()]),
        key_allowed_models: None,
        key_capabilities: None,
        model_id: format!("model-{prefix}"),
        global_model_id: format!("global-model-{prefix}"),
        global_model_name: "gpt-image-2".to_string(),
        global_model_mappings: None,
        global_model_supports_streaming: Some(false),
        model_provider_model_name: "gpt-image-2".to_string(),
        model_provider_model_mappings: Some(vec![StoredProviderModelMapping {
            name: "gpt-image-2".to_string(),
            priority: 1,
            api_formats: Some(vec!["openai:image".to_string()]),
            endpoint_ids: None,
            operations: None,
        }]),
        model_supports_streaming: Some(false),
        model_is_active: true,
        model_is_available: true,
    }
}

fn image_bridge_provider_catalog_provider(
    prefix: &str,
    provider_name: &str,
    provider_type: &str,
    base_url: &str,
) -> StoredProviderCatalogProvider {
    StoredProviderCatalogProvider::new(
        format!("provider-{prefix}"),
        provider_name.to_string(),
        Some(base_url.to_string()),
        provider_type.to_string(),
    )
    .expect("provider should build")
    .with_transport_fields(
        true,
        false,
        None,
        Some(2),
        None,
        Some(20.0),
        None,
        None,
    )
}

fn image_bridge_provider_catalog_endpoint(
    prefix: &str,
    base_url: &str,
) -> StoredProviderCatalogEndpoint {
    StoredProviderCatalogEndpoint::new(
        format!("endpoint-{prefix}"),
        format!("provider-{prefix}"),
        "openai:image".to_string(),
        Some("openai".to_string()),
        Some("image".to_string()),
        true,
    )
    .expect("endpoint should build")
    .with_transport_fields(
        base_url.to_string(),
        None,
        None,
        Some(2),
        None,
        None,
        None,
        None,
    )
    .expect("endpoint transport should build")
}

fn image_bridge_provider_catalog_key(
    prefix: &str,
    provider_type: &str,
) -> StoredProviderCatalogKey {
    let auth_type = if provider_type == "chatgpt_web" {
        "bearer"
    } else {
        "api_key"
    };
    StoredProviderCatalogKey::new(
        format!("key-{prefix}"),
        format!("provider-{prefix}"),
        "prod".to_string(),
        auth_type.to_string(),
        None,
        true,
    )
    .expect("key should build")
    .with_transport_fields(
        Some(serde_json::json!(["openai:image"])),
        encrypt_python_fernet_plaintext(DEVELOPMENT_ENCRYPTION_KEY, "sk-upstream-image-bridge")
            .expect("api key should encrypt"),
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("key transport should build")
}

async fn start_image_bridge_gateway(
    prefix: &str,
    provider_name: &str,
    provider_type: &str,
    base_url: &str,
    execution_runtime_url: String,
) -> (
    String,
    tokio::task::JoinHandle<()>,
    String,
    Arc<InMemoryRequestCandidateRepository>,
) {
    let client_api_key = format!("sk-client-{prefix}");
    let auth_repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some(image_bridge_hash_api_key(&client_api_key)),
        image_bridge_auth_snapshot(&format!("api-key-{prefix}"), &format!("user-{prefix}")),
    )]));
    let candidate_selection_repository =
        Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
            image_bridge_candidate_row(prefix, provider_name, provider_type),
        ]));
    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![image_bridge_provider_catalog_provider(
            prefix,
            provider_name,
            provider_type,
            base_url,
        )],
        vec![image_bridge_provider_catalog_endpoint(prefix, base_url)],
        vec![image_bridge_provider_catalog_key(prefix, provider_type)],
    ));
    let request_candidate_repository = Arc::new(InMemoryRequestCandidateRepository::default());
    let gateway_state = build_state_with_execution_runtime_override(execution_runtime_url)
        .with_data_state_for_tests(
            crate::data::GatewayDataState::with_auth_candidate_selection_provider_catalog_and_request_candidate_repository_for_tests(
                auth_repository,
                candidate_selection_repository,
                provider_catalog_repository,
                Arc::clone(&request_candidate_repository),
                DEVELOPMENT_ENCRYPTION_KEY,
            ),
        );
    let gateway = build_router_with_state(gateway_state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;
    (
        gateway_url,
        gateway_handle,
        client_api_key,
        request_candidate_repository,
    )
}

fn capture_image_bridge_execution_plan(
    parts: http::request::Parts,
    payload: serde_json::Value,
) -> SeenImageBridgeExecutionPlan {
    SeenImageBridgeExecutionPlan {
        trace_id: parts
            .headers
            .get(TRACE_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string(),
        client_api_format: payload
            .get("client_api_format")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        provider_api_format: payload
            .get("provider_api_format")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        url: payload
            .get("url")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        plan_stream: payload
            .get("stream")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        auth_header: payload
            .get("headers")
            .and_then(|value| value.get("authorization"))
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        body_json: payload
            .get("body")
            .and_then(|value| value.get("json_body"))
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    }
}

fn image_bridge_execution_runtime(
    seen_execution_plan: Arc<Mutex<Option<SeenImageBridgeExecutionPlan>>>,
) -> Router {
    Router::new().route(
        "/v1/execute/stream",
        any(move |request: Request| {
            let seen_execution_plan_inner = Arc::clone(&seen_execution_plan);
            async move {
                let (parts, body) = request.into_parts();
                let raw_body = to_bytes(body, usize::MAX).await.expect("body should read");
                let payload: serde_json::Value = serde_json::from_slice(&raw_body)
                    .expect("execution runtime payload should parse");
                *seen_execution_plan_inner.lock().expect("mutex should lock") =
                    Some(capture_image_bridge_execution_plan(parts, payload));
                let frames = concat!(
                    "{\"type\":\"headers\",\"payload\":{\"kind\":\"headers\",\"status_code\":200,\"headers\":{\"content-type\":\"text/event-stream\"}}}\n",
                    "{\"type\":\"data\",\"payload\":{\"kind\":\"data\",\"text\":\"event: response.output_item.done\\ndata: {\\\"type\\\":\\\"response.output_item.done\\\",\\\"output_index\\\":0,\\\"item\\\":{\\\"id\\\":\\\"ig_bridge_123\\\",\\\"type\\\":\\\"image_generation_call\\\",\\\"result\\\":\\\"aGVsbG8=\\\",\\\"output_format\\\":\\\"png\\\"}}\\n\\n\"}}\n",
                    "{\"type\":\"data\",\"payload\":{\"kind\":\"data\",\"text\":\"event: response.completed\\ndata: {\\\"type\\\":\\\"response.completed\\\",\\\"response\\\":{\\\"id\\\":\\\"resp_bridge_123\\\",\\\"object\\\":\\\"response\\\",\\\"model\\\":\\\"gpt-image-2\\\",\\\"status\\\":\\\"completed\\\",\\\"output\\\":[]}}\\n\\n\"}}\n",
                    "{\"type\":\"eof\",\"payload\":{\"kind\":\"eof\"}}\n"
                );
                let mut response = http::Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::from(frames))
                    .expect("response should build");
                response.headers_mut().insert(
                    http::header::CONTENT_TYPE,
                    http::HeaderValue::from_static("application/x-ndjson"),
                );
                response
            }
        }),
    )
}

#[test]
fn gateway_routes_openai_responses_stream_image_intent_to_openai_image_plan_without_streaming_support(
) {
    run_stream_image_test(
        "gateway_routes_openai_responses_stream_image_intent_to_openai_image_plan_without_streaming_support",
        gateway_routes_openai_responses_stream_image_intent_to_openai_image_plan_without_streaming_support_impl,
    );
}

async fn gateway_routes_openai_responses_stream_image_intent_to_openai_image_plan_without_streaming_support_impl(
) {
    let seen_execution_plan = Arc::new(Mutex::new(None::<SeenImageBridgeExecutionPlan>));
    let execution_runtime = image_bridge_execution_runtime(Arc::clone(&seen_execution_plan));
    let (execution_runtime_url, execution_runtime_handle) = start_server(execution_runtime).await;
    let (gateway_url, gateway_handle, client_api_key, _request_candidate_repository) =
        start_image_bridge_gateway(
            "responses-stream-image-bridge",
            "image-provider",
            "custom",
            "https://images.example.com/v1",
            execution_runtime_url,
        )
        .await;

    let response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/responses"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .header(http::header::AUTHORIZATION, format!("Bearer {client_api_key}"))
        .header(TRACE_ID_HEADER, "trace-responses-stream-image-bridge-123")
        .body(
            r#"{"model":"gpt-image-2","input":"Draw a mountain observatory","tools":[{"type":"image_generation","size":"1024x1024"}],"tool_choice":{"type":"image_generation"},"stream":true}"#,
        )
        .send()
        .await
        .expect("request should succeed");

    let status = response.status();
    let response_text = response.text().await.expect("body should read");
    assert_eq!(status, StatusCode::OK, "{response_text}");
    assert!(response_text.contains("response.output_item.done"));
    assert!(response_text.contains("image_generation_call"));

    let seen_plan = seen_execution_plan
        .lock()
        .expect("mutex should lock")
        .clone()
        .expect("execution plan should be captured");
    assert_eq!(
        seen_plan.trace_id,
        "trace-responses-stream-image-bridge-123"
    );
    assert_eq!(seen_plan.client_api_format, "openai:responses");
    assert_eq!(seen_plan.provider_api_format, "openai:image");
    assert_eq!(
        seen_plan.url,
        "https://images.example.com/v1/images/generations"
    );
    assert!(!seen_plan.plan_stream);
    assert_eq!(seen_plan.auth_header, "Bearer sk-upstream-image-bridge");
    assert_eq!(seen_plan.body_json["model"], "gpt-image-2");
    assert_eq!(seen_plan.body_json["prompt"], "Draw a mountain observatory");
    assert_eq!(seen_plan.body_json["size"], "1024x1024");
    assert!(seen_plan.body_json.get("stream").is_none());
    assert!(seen_plan.body_json.get("input").is_none());
    assert!(seen_plan.body_json.get("tools").is_none());

    gateway_handle.abort();
    execution_runtime_handle.abort();
}
