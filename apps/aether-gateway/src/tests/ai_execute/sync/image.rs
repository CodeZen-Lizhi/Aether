use super::{
    any, build_router_with_state, build_state_with_execution_runtime_override, json, start_server,
    to_bytes, Arc, Json, Mutex, Request, Router, StatusCode, TRACE_ID_HEADER,
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

const IMAGE_SYNC_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;

fn run_image_sync_test<F, Fut>(test_name: &'static str, make_future: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + 'static,
{
    let handle = std::thread::Builder::new()
        .name(test_name.to_string())
        .stack_size(IMAGE_SYNC_TEST_STACK_BYTES)
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("test runtime should build");
            runtime.block_on(make_future());
        })
        .expect("image sync test thread should spawn");

    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn gateway_converts_openai_image_sync_to_gemini_image_provider() {
    run_image_sync_test(
        "gateway_converts_openai_image_sync_to_gemini_image_provider",
        gateway_converts_openai_image_sync_to_gemini_image_provider_impl,
    );
}

async fn gateway_converts_openai_image_sync_to_gemini_image_provider_impl() {
    #[derive(Debug, Clone)]
    struct SeenExecutionRuntimeSyncRequest {
        trace_id: String,
        url: String,
        auth_header_value: String,
        has_model_field: bool,
        prompt: String,
        response_modalities: Vec<String>,
        image_size: String,
    }

    fn hash_api_key(value: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(value.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn sample_auth_snapshot(api_key_id: &str, user_id: &str) -> StoredAuthApiKeySnapshot {
        StoredAuthApiKeySnapshot::new(
            user_id.to_string(),
            "alice".to_string(),
            Some("alice@example.com".to_string()),
            "user".to_string(),
            "local".to_string(),
            true,
            false,
            Some(serde_json::json!(["openai", "google"])),
            Some(serde_json::json!(["openai:image"])),
            Some(serde_json::json!(["gpt-image-2"])),
            api_key_id.to_string(),
            Some("default".to_string()),
            true,
            false,
            false,
            Some(60),
            Some(5),
            Some(4_102_444_800_i64),
            Some(serde_json::json!(["openai", "google"])),
            Some(serde_json::json!(["openai:image"])),
            Some(serde_json::json!(["gpt-image-2"])),
        )
        .expect("auth snapshot should build")
    }

    fn sample_candidate_row() -> StoredMinimalCandidateSelectionRow {
        StoredMinimalCandidateSelectionRow {
            provider_id: "provider-gemini-image-bridge-1".to_string(),
            provider_name: "google".to_string(),
            provider_type: "google".to_string(),
            provider_is_active: true,
            endpoint_id: "endpoint-gemini-image-bridge-1".to_string(),
            endpoint_api_format: "gemini:generate_content".to_string(),
            endpoint_api_family: Some("gemini".to_string()),
            endpoint_kind: Some("chat".to_string()),
            endpoint_is_active: true,
            key_id: "key-gemini-image-bridge-1".to_string(),
            key_name: "prod".to_string(),
            key_auth_type: "api_key".to_string(),
            key_is_active: true,
            key_api_formats: Some(vec!["gemini:generate_content".to_string()]),
            key_allowed_models: None,
            key_capabilities: None,
            model_id: "model-gemini-image-bridge-1".to_string(),
            global_model_id: "global-model-gemini-image-bridge-1".to_string(),
            global_model_name: "gpt-image-2".to_string(),
            global_model_supports_streaming: Some(true),
            model_provider_model_name: "gemini-2.5-flash-image-upstream".to_string(),
            model_provider_model_mappings: Some(vec![StoredProviderModelMapping {
                name: "gemini-2.5-flash-image-upstream".to_string(),
                priority: 1,
                api_formats: Some(vec!["gemini:generate_content".to_string()]),
                endpoint_ids: None,
                operations: None,
            }]),
            model_supports_streaming: Some(true),
            model_is_active: true,
            model_is_available: true,
        }
    }

    fn sample_provider_catalog_provider() -> StoredProviderCatalogProvider {
        StoredProviderCatalogProvider::new(
            "provider-gemini-image-bridge-1".to_string(),
            "google".to_string(),
            Some("https://generativelanguage.googleapis.com".to_string()),
            "google".to_string(),
        )
        .expect("provider should build")
        .with_transport_fields(true, false, None, Some(2), None, Some(20.0), None, None)
    }

    fn sample_provider_catalog_endpoint() -> StoredProviderCatalogEndpoint {
        StoredProviderCatalogEndpoint::new(
            "endpoint-gemini-image-bridge-1".to_string(),
            "provider-gemini-image-bridge-1".to_string(),
            "gemini:generate_content".to_string(),
            Some("gemini".to_string()),
            Some("chat".to_string()),
            true,
        )
        .expect("endpoint should build")
        .with_transport_fields(
            "https://generativelanguage.googleapis.com".to_string(),
            None,
            Some(serde_json::json!([
                {"action":"drop","path":"model"}
            ])),
            Some(2),
            None,
            None,
            None,
            None,
        )
        .expect("endpoint transport should build")
    }

    fn sample_provider_catalog_key() -> StoredProviderCatalogKey {
        StoredProviderCatalogKey::new(
            "key-gemini-image-bridge-1".to_string(),
            "provider-gemini-image-bridge-1".to_string(),
            "prod".to_string(),
            "api_key".to_string(),
            None,
            true,
        )
        .expect("key should build")
        .with_transport_fields(
            Some(serde_json::json!(["gemini:generate_content"])),
            encrypt_python_fernet_plaintext(DEVELOPMENT_ENCRYPTION_KEY, "sk-upstream-gemini-image")
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

    let seen_execution_runtime = Arc::new(Mutex::new(None::<SeenExecutionRuntimeSyncRequest>));
    let seen_execution_runtime_clone = Arc::clone(&seen_execution_runtime);
    let execution_runtime = Router::new().route(
        "/v1/execute/sync",
        any(move |request: Request| {
            let seen_execution_runtime_inner = Arc::clone(&seen_execution_runtime_clone);
            async move {
                let (parts, body) = request.into_parts();
                let raw_body = to_bytes(body, usize::MAX).await.expect("body should read");
                let payload: serde_json::Value = serde_json::from_slice(&raw_body)
                    .expect("execution runtime payload should parse");
                let body_json = payload
                    .get("body")
                    .and_then(|value| value.get("json_body"))
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                *seen_execution_runtime_inner
                    .lock()
                    .expect("mutex should lock") = Some(SeenExecutionRuntimeSyncRequest {
                    trace_id: parts
                        .headers
                        .get(TRACE_ID_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string(),
                    url: payload
                        .get("url")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    auth_header_value: payload
                        .get("headers")
                        .and_then(|value| value.get("x-goog-api-key"))
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    has_model_field: body_json.get("model").is_some(),
                    prompt: body_json
                        .get("contents")
                        .and_then(|value| value.get(0))
                        .and_then(|value| value.get("parts"))
                        .and_then(|value| value.get(0))
                        .and_then(|value| value.get("text"))
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    response_modalities: body_json
                        .get("generationConfig")
                        .and_then(|value| value.get("responseModalities"))
                        .and_then(|value| value.as_array())
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                                .collect()
                        })
                        .unwrap_or_default(),
                    image_size: body_json
                        .get("generationConfig")
                        .and_then(|value| value.get("imageSize"))
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string(),
                });
                Json(json!({
                    "request_id": "trace-openai-image-to-gemini-123",
                    "status_code": 200,
                    "headers": {
                        "content-type": "application/json"
                    },
                    "body": {
                        "json_body": {
                            "modelVersion": "gemini-2.5-flash-image-upstream",
                            "usageMetadata": {
                                "promptTokenCount": 11,
                                "candidatesTokenCount": 22,
                                "totalTokenCount": 33
                            },
                            "candidates": [{
                                "content": {
                                    "role": "model",
                                    "parts": [
                                        {"text": "revised kite prompt"},
                                        {"inlineData": {"mimeType": "image/png", "data": "aGVsbG8="}}
                                    ]
                                },
                                "finishReason": "STOP"
                            }]
                        }
                    },
                    "telemetry": {
                        "elapsed_ms": 37
                    }
                }))
            }
        }),
    );

    let client_api_key = "sk-client-openai-image-to-gemini";
    let auth_repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some(hash_api_key(client_api_key)),
        sample_auth_snapshot(
            "key-openai-image-client-bridge-1",
            "user-openai-image-bridge-1",
        ),
    )]));
    let candidate_selection_repository =
        Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
            sample_candidate_row(),
        ]));
    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider_catalog_provider()],
        vec![sample_provider_catalog_endpoint()],
        vec![sample_provider_catalog_key()],
    ));

    let (execution_runtime_url, execution_runtime_handle) = start_server(execution_runtime).await;
    let gateway_state = build_state_with_execution_runtime_override(execution_runtime_url)
        .with_data_state_for_tests(
            crate::data::GatewayDataState::with_auth_candidate_selection_provider_catalog_and_request_candidate_repository_for_tests(
                auth_repository,
                candidate_selection_repository,
                provider_catalog_repository,
                Arc::new(InMemoryRequestCandidateRepository::default()),
                DEVELOPMENT_ENCRYPTION_KEY,
            ),
        );
    let gateway = build_router_with_state(gateway_state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/images/generations"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .header(
            http::header::AUTHORIZATION,
            format!("Bearer {client_api_key}"),
        )
        .header(TRACE_ID_HEADER, "trace-openai-image-to-gemini-123")
        .body(
            "{\"model\":\"gpt-image-2\",\"prompt\":\"Draw a red kite\",\"size\":\"1024x1024\",\"response_format\":\"b64_json\"}",
        )
        .send()
        .await
        .expect("request should succeed");

    let response_status = response.status();
    let response_body = response.text().await.expect("body should read");
    assert_eq!(response_status, StatusCode::OK, "{response_body}");
    let response_json: serde_json::Value =
        serde_json::from_str(&response_body).expect("body should parse");
    assert_eq!(response_json["data"][0]["b64_json"], "aGVsbG8=");
    assert_eq!(
        response_json["data"][0]["revised_prompt"],
        "revised kite prompt"
    );
    assert_eq!(response_json["model"], "gemini-2.5-flash-image-upstream");
    assert_eq!(response_json["usage"]["input_tokens"], 11);
    assert_eq!(response_json["usage"]["output_tokens"], 22);

    let seen_execution_runtime_request = seen_execution_runtime
        .lock()
        .expect("mutex should lock")
        .clone()
        .expect("execution runtime sync should be captured");
    assert_eq!(
        seen_execution_runtime_request.trace_id,
        "trace-openai-image-to-gemini-123"
    );
    assert_eq!(
        seen_execution_runtime_request.url,
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image-upstream:generateContent"
    );
    assert_eq!(
        seen_execution_runtime_request.auth_header_value,
        "sk-upstream-gemini-image"
    );
    assert!(!seen_execution_runtime_request.has_model_field);
    assert_eq!(seen_execution_runtime_request.prompt, "Draw a red kite");
    assert_eq!(
        seen_execution_runtime_request.response_modalities,
        vec!["TEXT".to_string(), "IMAGE".to_string()]
    );
    assert_eq!(seen_execution_runtime_request.image_size, "1024x1024");

    gateway_handle.abort();
    execution_runtime_handle.abort();
}

#[test]
fn gateway_converts_gemini_image_sync_to_openai_image_provider() {
    run_image_sync_test(
        "gateway_converts_gemini_image_sync_to_openai_image_provider",
        gateway_converts_gemini_image_sync_to_openai_image_provider_impl,
    );
}

async fn gateway_converts_gemini_image_sync_to_openai_image_provider_impl() {
    #[derive(Debug, Clone)]
    struct SeenExecutionRuntimeSyncRequest {
        trace_id: String,
        url: String,
        authorization: String,
        model: String,
        prompt: String,
        images: serde_json::Value,
        has_legacy_image_field: bool,
        request_stream: bool,
        body_stream: Option<bool>,
    }

    fn hash_api_key(value: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(value.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn sample_auth_snapshot(api_key_id: &str, user_id: &str) -> StoredAuthApiKeySnapshot {
        StoredAuthApiKeySnapshot::new(
            user_id.to_string(),
            "alice".to_string(),
            Some("alice@example.com".to_string()),
            "user".to_string(),
            "local".to_string(),
            true,
            false,
            Some(serde_json::json!(["gemini", "openai"])),
            Some(serde_json::json!(["gemini:generate_content"])),
            Some(serde_json::json!(["gemini-image"])),
            api_key_id.to_string(),
            Some("default".to_string()),
            true,
            false,
            false,
            Some(60),
            Some(5),
            Some(4_102_444_800_i64),
            Some(serde_json::json!(["gemini", "openai"])),
            Some(serde_json::json!(["gemini:generate_content"])),
            Some(serde_json::json!(["gemini-image"])),
        )
        .expect("auth snapshot should build")
    }

    fn sample_candidate_row() -> StoredMinimalCandidateSelectionRow {
        StoredMinimalCandidateSelectionRow {
            provider_id: "provider-openai-image-bridge-1".to_string(),
            provider_name: "openai".to_string(),
            provider_type: "openai".to_string(),
            provider_is_active: true,
            endpoint_id: "endpoint-openai-image-bridge-1".to_string(),
            endpoint_api_format: "openai:image".to_string(),
            endpoint_api_family: Some("openai".to_string()),
            endpoint_kind: Some("image".to_string()),
            endpoint_is_active: true,
            key_id: "key-openai-image-bridge-1".to_string(),
            key_name: "prod".to_string(),
            key_auth_type: "api_key".to_string(),
            key_is_active: true,
            key_api_formats: Some(vec!["openai:image".to_string()]),
            key_allowed_models: None,
            key_capabilities: None,
            model_id: "model-openai-image-bridge-1".to_string(),
            global_model_id: "global-model-openai-image-bridge-1".to_string(),
            global_model_name: "gemini-image".to_string(),
            global_model_supports_streaming: Some(true),
            model_provider_model_name: "gpt-image-2-upstream".to_string(),
            model_provider_model_mappings: Some(vec![StoredProviderModelMapping {
                name: "gpt-image-2-upstream".to_string(),
                priority: 1,
                api_formats: Some(vec!["openai:image".to_string()]),
                endpoint_ids: None,
                operations: None,
            }]),
            model_supports_streaming: Some(true),
            model_is_active: true,
            model_is_available: true,
        }
    }

    fn sample_provider_catalog_provider() -> StoredProviderCatalogProvider {
        StoredProviderCatalogProvider::new(
            "provider-openai-image-bridge-1".to_string(),
            "openai".to_string(),
            Some("https://api.openai.com/v1".to_string()),
            "openai".to_string(),
        )
        .expect("provider should build")
        .with_transport_fields(true, false, None, Some(2), None, Some(20.0), None, None)
    }

    fn sample_provider_catalog_endpoint() -> StoredProviderCatalogEndpoint {
        StoredProviderCatalogEndpoint::new(
            "endpoint-openai-image-bridge-1".to_string(),
            "provider-openai-image-bridge-1".to_string(),
            "openai:image".to_string(),
            Some("openai".to_string()),
            Some("image".to_string()),
            true,
        )
        .expect("endpoint should build")
        .with_transport_fields(
            "https://api.openai.com/v1".to_string(),
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

    fn sample_provider_catalog_key() -> StoredProviderCatalogKey {
        StoredProviderCatalogKey::new(
            "key-openai-image-bridge-1".to_string(),
            "provider-openai-image-bridge-1".to_string(),
            "prod".to_string(),
            "api_key".to_string(),
            None,
            true,
        )
        .expect("key should build")
        .with_transport_fields(
            Some(serde_json::json!(["openai:image"])),
            encrypt_python_fernet_plaintext(DEVELOPMENT_ENCRYPTION_KEY, "sk-upstream-openai-image")
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

    let seen_execution_runtime = Arc::new(Mutex::new(None::<SeenExecutionRuntimeSyncRequest>));
    let seen_execution_runtime_clone = Arc::clone(&seen_execution_runtime);
    let execution_runtime = Router::new().route(
        "/v1/execute/sync",
        any(move |request: Request| {
            let seen_execution_runtime_inner = Arc::clone(&seen_execution_runtime_clone);
            async move {
                let (parts, body) = request.into_parts();
                let raw_body = to_bytes(body, usize::MAX).await.expect("body should read");
                let payload: serde_json::Value = serde_json::from_slice(&raw_body)
                    .expect("execution runtime payload should parse");
                let body_json = payload
                    .get("body")
                    .and_then(|value| value.get("json_body"))
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                *seen_execution_runtime_inner
                    .lock()
                    .expect("mutex should lock") = Some(SeenExecutionRuntimeSyncRequest {
                    trace_id: parts
                        .headers
                        .get(TRACE_ID_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string(),
                    url: payload
                        .get("url")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    authorization: payload
                        .get("headers")
                        .and_then(|value| value.get("authorization"))
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    model: body_json
                        .get("model")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    prompt: body_json
                        .get("prompt")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    images: body_json.get("images").cloned().unwrap_or_default(),
                    has_legacy_image_field: body_json.get("image").is_some(),
                    request_stream: payload
                        .get("stream")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(true),
                    body_stream: body_json.get("stream").and_then(serde_json::Value::as_bool),
                });
                Json(json!({
                    "request_id": "trace-gemini-image-to-openai-123",
                    "status_code": 200,
                    "headers": {
                        "content-type": "application/json"
                    },
                    "body": {
                        "json_body": {
                            "created": 1776839946,
                            "model": "gpt-image-2-upstream",
                            "usage": {
                                "input_tokens": 3,
                                "output_tokens": 4,
                                "total_tokens": 7
                            },
                            "data": [{
                                "revised_prompt": "converted gemini prompt",
                                "b64_json": "aGVsbG8="
                            }]
                        }
                    },
                    "telemetry": {
                        "elapsed_ms": 43
                    }
                }))
            }
        }),
    );

    let client_api_key = "client-gemini-image-to-openai";
    let auth_repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some(hash_api_key(client_api_key)),
        sample_auth_snapshot(
            "key-gemini-image-client-bridge-1",
            "user-gemini-image-bridge-1",
        ),
    )]));
    let candidate_selection_repository =
        Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
            sample_candidate_row(),
        ]));
    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider_catalog_provider()],
        vec![sample_provider_catalog_endpoint()],
        vec![sample_provider_catalog_key()],
    ));

    let (execution_runtime_url, execution_runtime_handle) = start_server(execution_runtime).await;
    let gateway_state = build_state_with_execution_runtime_override(execution_runtime_url)
        .with_data_state_for_tests(
            crate::data::GatewayDataState::with_auth_candidate_selection_provider_catalog_and_request_candidate_repository_for_tests(
                auth_repository,
                candidate_selection_repository,
                provider_catalog_repository,
                Arc::new(InMemoryRequestCandidateRepository::default()),
                DEVELOPMENT_ENCRYPTION_KEY,
            ),
        );
    let gateway = build_router_with_state(gateway_state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .post(format!(
            "{gateway_url}/v1beta/models/gemini-image:generateContent?key={client_api_key}"
        ))
        .header(http::header::CONTENT_TYPE, "application/json")
        .header(TRACE_ID_HEADER, "trace-gemini-image-to-openai-123")
        .body(
            "{\"generationConfig\":{\"responseModalities\":[\"TEXT\",\"IMAGE\"]},\"contents\":[{\"role\":\"user\",\"parts\":[{\"text\":\"Change the background\"},{\"inlineData\":{\"mimeType\":\"image/png\",\"data\":\"aGVsbG8=\"}}]}]}",
        )
        .send()
        .await
        .expect("request should succeed");

    let response_status = response.status();
    let response_body = response.text().await.expect("body should read");
    assert_eq!(response_status, StatusCode::OK, "{response_body}");
    let response_json: serde_json::Value =
        serde_json::from_str(&response_body).expect("body should parse");
    assert_eq!(response_json["modelVersion"], "gpt-image-2-upstream");
    assert_eq!(
        response_json["candidates"][0]["content"]["parts"][0]["text"],
        "converted gemini prompt"
    );
    assert_eq!(
        response_json["candidates"][0]["content"]["parts"][1]["inlineData"]["mimeType"],
        "image/png"
    );
    assert_eq!(
        response_json["candidates"][0]["content"]["parts"][1]["inlineData"]["data"],
        "aGVsbG8="
    );
    assert_eq!(response_json["usageMetadata"]["promptTokenCount"], 3);
    assert_eq!(response_json["usageMetadata"]["candidatesTokenCount"], 4);

    let seen_execution_runtime_request = seen_execution_runtime
        .lock()
        .expect("mutex should lock")
        .clone()
        .expect("execution runtime sync should be captured");
    assert_eq!(
        seen_execution_runtime_request.trace_id,
        "trace-gemini-image-to-openai-123"
    );
    assert_eq!(
        seen_execution_runtime_request.url,
        "https://api.openai.com/v1/images/edits"
    );
    assert_eq!(
        seen_execution_runtime_request.authorization,
        "Bearer sk-upstream-openai-image"
    );
    assert_eq!(seen_execution_runtime_request.model, "gpt-image-2-upstream");
    assert_eq!(
        seen_execution_runtime_request.prompt,
        "Change the background"
    );
    assert_eq!(
        seen_execution_runtime_request.images,
        json!([{"image_url": "data:image/png;base64,aGVsbG8="}])
    );
    assert!(!seen_execution_runtime_request.has_legacy_image_field);
    assert!(!seen_execution_runtime_request.request_stream);
    assert_eq!(seen_execution_runtime_request.body_stream, None);

    gateway_handle.abort();
    execution_runtime_handle.abort();
}
