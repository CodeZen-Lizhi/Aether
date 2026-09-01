use super::{
    any, build_router_with_state, build_state_with_execution_runtime_override, json,
    next_non_keepalive_chunk, start_server, strip_sse_keepalive_comments, to_bytes, Arc, Body,
    Bytes, HeaderName, HeaderValue, Json, Mutex, Request, Response, Router, StatusCode,
    TRACE_ID_HEADER,
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
use aether_data_contracts::repository::candidates::{
    RequestCandidateReadRepository, RequestCandidateStatus,
};
use aether_data_contracts::repository::provider_catalog::{
    StoredProviderCatalogEndpoint, StoredProviderCatalogKey, StoredProviderCatalogProvider,
};
use sha2::{Digest, Sha256};

const STREAM_PROVIDER_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;

fn run_stream_provider_test<F, Fut>(test_name: &'static str, make_future: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + 'static,
{
    let handle = std::thread::Builder::new()
        .name(test_name.to_string())
        .stack_size(STREAM_PROVIDER_TEST_STACK_BYTES)
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("test runtime should build");
            runtime.block_on(make_future());
        })
        .expect("stream provider test thread should spawn");

    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn gateway_executes_claude_cli_stream_via_local_decision_gate_without_waiting_for_same_format_prefetch(
) {
    run_stream_provider_test(
        "gateway_executes_claude_cli_stream_via_local_decision_gate_without_waiting_for_same_format_prefetch",
        gateway_executes_claude_cli_stream_via_local_decision_gate_without_waiting_for_same_format_prefetch_impl,
    );
}

async fn gateway_executes_claude_cli_stream_via_local_decision_gate_without_waiting_for_same_format_prefetch_impl(
) {
    #[derive(Debug, Clone)]
    struct SeenExecutionRuntimeStreamRequest {
        trace_id: String,
        url: String,
        model: String,
        stream: bool,
        accept: String,
        authorization: String,
        endpoint_tag: String,
        metadata_mode: String,
        metadata_source: String,
        proxy_node_id: String,
        transport_profile_id: String,
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
            Some(serde_json::json!(["claude"])),
            Some(serde_json::json!(["claude:messages"])),
            Some(serde_json::json!(["claude-code"])),
            api_key_id.to_string(),
            Some("default".to_string()),
            true,
            false,
            false,
            Some(60),
            Some(5),
            Some(4_102_444_800),
            Some(serde_json::json!(["claude"])),
            Some(serde_json::json!(["claude:messages"])),
            Some(serde_json::json!(["claude-code"])),
        )
        .expect("auth snapshot should build")
    }

    fn sample_candidate_row() -> StoredMinimalCandidateSelectionRow {
        StoredMinimalCandidateSelectionRow {
            provider_id: "provider-claude-cli-local-1".to_string(),
            provider_name: "claude".to_string(),
            provider_type: "custom".to_string(),
            provider_is_active: true,
            endpoint_id: "endpoint-claude-cli-local-1".to_string(),
            endpoint_api_format: "claude:messages".to_string(),
            endpoint_api_family: Some("claude".to_string()),
            endpoint_kind: Some("cli".to_string()),
            endpoint_is_active: true,
            key_id: "key-claude-cli-local-1".to_string(),
            key_name: "prod".to_string(),
            key_auth_type: "bearer".to_string(),
            key_is_active: true,
            key_api_formats: Some(vec!["claude:messages".to_string()]),
            key_allowed_models: None,
            key_capabilities: None,
            model_id: "model-claude-cli-local-1".to_string(),
            global_model_id: "global-model-claude-cli-local-1".to_string(),
            global_model_name: "claude-code".to_string(),
            global_model_mappings: None,
            global_model_supports_streaming: Some(true),
            model_provider_model_name: "claude-code-upstream".to_string(),
            model_provider_model_mappings: Some(vec![StoredProviderModelMapping {
                name: "claude-code-upstream".to_string(),
                priority: 1,
                api_formats: Some(vec!["claude:messages".to_string()]),
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
            "provider-claude-cli-local-1".to_string(),
            "claude".to_string(),
            Some("https://example.com".to_string()),
            "custom".to_string(),
        )
        .expect("provider should build")
        .with_transport_fields(
            true,
            false,
            None,
            Some(2),
            Some(serde_json::json!({"url":"http://provider-proxy.internal:8080"})),
            Some(20.0),
            None,
            None,
        )
    }

    fn sample_provider_catalog_endpoint() -> StoredProviderCatalogEndpoint {
        StoredProviderCatalogEndpoint::new(
            "endpoint-claude-cli-local-1".to_string(),
            "provider-claude-cli-local-1".to_string(),
            "claude:messages".to_string(),
            Some("claude".to_string()),
            Some("cli".to_string()),
            true,
        )
        .expect("endpoint should build")
        .with_transport_fields(
            "https://api.anthropic.example".to_string(),
            Some(serde_json::json!([
                {"action":"set","key":"x-endpoint-tag","value":"claude-cli-local"}
            ])),
            Some(serde_json::json!([
                {"action":"set","path":"metadata.mode","value":"safe"},
                {"action":"rename","from":"metadata.client","to":"metadata.source"}
            ])),
            Some(2),
            Some("/custom/v1/messages".to_string()),
            None,
            None,
            None,
        )
        .expect("endpoint transport should build")
    }

    fn sample_provider_catalog_key() -> StoredProviderCatalogKey {
        StoredProviderCatalogKey::new(
            "key-claude-cli-local-1".to_string(),
            "provider-claude-cli-local-1".to_string(),
            "prod".to_string(),
            "bearer".to_string(),
            None,
            true,
        )
        .expect("key should build")
        .with_transport_fields(
            Some(serde_json::json!(["claude:messages"])),
            encrypt_python_fernet_plaintext(DEVELOPMENT_ENCRYPTION_KEY, "sk-upstream-claude-cli")
                .expect("api key should encrypt"),
            None,
            None,
            None,
            None,
            Some(serde_json::json!({"enabled": true, "node_id":"proxy-node-claude-cli-local"})),
            Some(serde_json::json!({"transport_profile":"chrome_136"})),
        )
        .expect("key transport should build")
    }

    let seen_execution_runtime = Arc::new(Mutex::new(None::<SeenExecutionRuntimeStreamRequest>));
    let seen_execution_runtime_clone = Arc::clone(&seen_execution_runtime);
    let seen_report = Arc::new(Mutex::new(false));
    let seen_report_clone = Arc::clone(&seen_report);
    let decision_hits = Arc::new(Mutex::new(0usize));
    let decision_hits_clone = Arc::clone(&decision_hits);
    let plan_hits = Arc::new(Mutex::new(0usize));
    let plan_hits_clone = Arc::clone(&plan_hits);
    let public_hits = Arc::new(Mutex::new(0usize));
    let public_hits_clone = Arc::clone(&public_hits);

    let upstream = Router::new()
        .route(
            "/api/internal/gateway/decision-stream",
            any(move |_request: Request| {
                let decision_hits_inner = Arc::clone(&decision_hits_clone);
                async move {
                    *decision_hits_inner.lock().expect("mutex should lock") += 1;
                    Json(json!({"action": "proxy_public"}))
                }
            }),
        )
        .route(
            "/api/internal/gateway/plan-stream",
            any(move |_request: Request| {
                let plan_hits_inner = Arc::clone(&plan_hits_clone);
                async move {
                    *plan_hits_inner.lock().expect("mutex should lock") += 1;
                    Json(json!({"action": "proxy_public"}))
                }
            }),
        )
        .route(
            "/api/internal/gateway/report-stream",
            any(move |request: Request| {
                let seen_report_inner = Arc::clone(&seen_report_clone);
                async move {
                    let (_parts, body) = request.into_parts();
                    let _raw_body = to_bytes(body, usize::MAX).await.expect("body should read");
                    *seen_report_inner.lock().expect("mutex should lock") = true;
                    Json(json!({"ok": true}))
                }
            }),
        )
        .route(
            "/v1/messages",
            any(move |_request: Request| {
                let public_hits_inner = Arc::clone(&public_hits_clone);
                async move {
                    *public_hits_inner.lock().expect("mutex should lock") += 1;
                    (StatusCode::IM_A_TEAPOT, Body::from("public-route-hit"))
                }
            }),
        );

    let execution_runtime = Router::new().route(
        "/v1/execute/stream",
        any(move |request: Request| {
            let seen_execution_runtime_inner = Arc::clone(&seen_execution_runtime_clone);
            async move {
                let (parts, body) = request.into_parts();
                let raw_body = to_bytes(body, usize::MAX).await.expect("body should read");
                let payload: serde_json::Value =
                    serde_json::from_slice(&raw_body).expect("execution runtime payload should parse");
                *seen_execution_runtime_inner.lock().expect("mutex should lock") =
                    Some(SeenExecutionRuntimeStreamRequest {
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
                        model: payload
                            .get("body")
                            .and_then(|value| value.get("json_body"))
                            .and_then(|value| value.get("model"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        stream: payload
                            .get("body")
                            .and_then(|value| value.get("json_body"))
                            .and_then(|value| value.get("stream"))
                            .and_then(|value| value.as_bool())
                            .unwrap_or(false),
                        accept: payload
                            .get("headers")
                            .and_then(|value| value.get("accept"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        authorization: payload
                            .get("headers")
                            .and_then(|value| value.get("authorization"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        endpoint_tag: payload
                            .get("headers")
                            .and_then(|value| value.get("x-endpoint-tag"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        metadata_mode: payload
                            .get("body")
                            .and_then(|value| value.get("json_body"))
                            .and_then(|value| value.get("metadata"))
                            .and_then(|value| value.get("mode"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        metadata_source: payload
                            .get("body")
                            .and_then(|value| value.get("json_body"))
                            .and_then(|value| value.get("metadata"))
                            .and_then(|value| value.get("source"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        proxy_node_id: payload
                            .get("proxy")
                            .and_then(|value| value.get("node_id"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        transport_profile_id: payload
                            .get("transport_profile").and_then(|value| value.get("profile_id"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    });
                let body_stream = async_stream::stream! {
                    yield Ok::<Bytes, std::convert::Infallible>(Bytes::from_static(
                        b"{\"type\":\"headers\",\"payload\":{\"kind\":\"headers\",\"status_code\":200,\"headers\":{\"content-type\":\"text/event-stream\"}}}\n"
                    ));
                    yield Ok::<Bytes, std::convert::Infallible>(Bytes::from_static(
                        b"{\"type\":\"data\",\"payload\":{\"kind\":\"data\",\"text\":\"event: message_start\\ndata: {\\\"type\\\":\\\"message_start\\\"}\\n\\n\"}}\n"
                    ));
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    yield Ok::<Bytes, std::convert::Infallible>(Bytes::from_static(
                        b"{\"type\":\"data\",\"payload\":{\"kind\":\"data\",\"text\":\"event: message_stop\\ndata: {\\\"type\\\":\\\"message_stop\\\"}\\n\\n\"}}\n"
                    ));
                    yield Ok::<Bytes, std::convert::Infallible>(Bytes::from_static(
                        b"{\"type\":\"telemetry\",\"payload\":{\"kind\":\"telemetry\",\"telemetry\":{\"elapsed_ms\":31,\"ttfb_ms\":11,\"upstream_bytes\":37}}}\n"
                    ));
                    yield Ok::<Bytes, std::convert::Infallible>(Bytes::from_static(
                        b"{\"type\":\"eof\",\"payload\":{\"kind\":\"eof\"}}\n"
                    ));
                };
                let mut response = Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::from_stream(body_stream))
                    .expect("response should build");
                response.headers_mut().insert(
                    http::header::CONTENT_TYPE,
                    HeaderValue::from_static("application/x-ndjson"),
                );
                response
            }
        }),
    );

    let auth_repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some(hash_api_key("sk-client-claude-cli-local")),
        sample_auth_snapshot("api-key-claude-cli-local-1", "user-claude-cli-local-1"),
    )]));
    let candidate_selection_repository =
        Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
            sample_candidate_row(),
        ]));
    let request_candidate_repository = Arc::new(InMemoryRequestCandidateRepository::default());
    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider_catalog_provider()],
        vec![sample_provider_catalog_endpoint()],
        vec![sample_provider_catalog_key()],
    ));

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let (execution_runtime_url, execution_runtime_handle) = start_server(execution_runtime).await;
    let gateway_state = build_state_with_execution_runtime_override(execution_runtime_url.clone())
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

    let mut response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/messages"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .header(http::header::USER_AGENT, "Claude-Code/2.1.0")
        .header(
            http::header::AUTHORIZATION,
            "Bearer sk-client-claude-cli-local",
        )
        .header(TRACE_ID_HEADER, "trace-claude-cli-local-stream-123")
        .body(
            "{\"model\":\"claude-code\",\"messages\":[],\"stream\":true,\"metadata\":{\"client\":\"desktop-claude-cli\"}}",
        )
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        tokio::time::timeout(
            std::time::Duration::from_millis(100),
            next_non_keepalive_chunk(&mut response),
        )
        .await
        .expect("same-format passthrough should yield first chunk before eof"),
        Bytes::from_static(b"event: message_start\ndata: {\"type\":\"message_start\"}\n\n")
    );
    assert_eq!(
        response.text().await.expect("remaining body should read"),
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"
    );

    let seen_execution_runtime_request = seen_execution_runtime
        .lock()
        .expect("mutex should lock")
        .clone()
        .expect("execution runtime stream should be captured");
    assert_eq!(
        seen_execution_runtime_request.trace_id,
        "trace-claude-cli-local-stream-123"
    );
    assert_eq!(
        seen_execution_runtime_request.url,
        "https://api.anthropic.example/custom/v1/messages"
    );
    assert_eq!(seen_execution_runtime_request.model, "claude-code-upstream");
    assert!(seen_execution_runtime_request.stream);
    assert_eq!(seen_execution_runtime_request.accept, "text/event-stream");
    assert_eq!(
        seen_execution_runtime_request.authorization,
        "Bearer sk-upstream-claude-cli"
    );
    assert_eq!(
        seen_execution_runtime_request.endpoint_tag,
        "claude-cli-local"
    );
    assert_eq!(seen_execution_runtime_request.metadata_mode, "safe");
    assert_eq!(
        seen_execution_runtime_request.metadata_source,
        "desktop-claude-cli"
    );
    assert_eq!(
        seen_execution_runtime_request.proxy_node_id,
        "proxy-node-claude-cli-local"
    );
    assert_eq!(
        seen_execution_runtime_request.transport_profile_id,
        "chrome_136"
    );

    let stored_candidates = request_candidate_repository
        .list_by_request_id("trace-claude-cli-local-stream-123")
        .await
        .expect("request candidate trace should read");
    assert_eq!(stored_candidates.len(), 1);
    assert_eq!(stored_candidates[0].status, RequestCandidateStatus::Success);

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(
        !*seen_report.lock().expect("mutex should lock"),
        "report-stream should stay local when request candidate persistence is available"
    );

    assert_eq!(*decision_hits.lock().expect("mutex should lock"), 0);
    assert_eq!(*plan_hits.lock().expect("mutex should lock"), 0);
    assert_eq!(*public_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    execution_runtime_handle.abort();
    upstream_handle.abort();
}

#[test]
fn gateway_executes_claude_chat_stream_via_local_decision_gate_with_local_stream_decision() {
    run_stream_provider_test(
        "gateway_executes_claude_chat_stream_via_local_decision_gate_with_local_stream_decision",
        gateway_executes_claude_chat_stream_via_local_decision_gate_with_local_stream_decision_impl,
    );
}

async fn gateway_executes_claude_chat_stream_via_local_decision_gate_with_local_stream_decision_impl(
) {
    #[derive(Debug, Clone)]
    struct SeenExecutionRuntimeStreamRequest {
        trace_id: String,
        url: String,
        model: String,
        stream: bool,
        accept: String,
        auth_header_value: String,
        endpoint_tag: String,
        metadata_mode: String,
        metadata_source: String,
        proxy_node_id: String,
        transport_profile_id: String,
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
            Some(serde_json::json!(["claude"])),
            Some(serde_json::json!(["claude:messages"])),
            Some(serde_json::json!(["claude-sonnet-4-5"])),
            api_key_id.to_string(),
            Some("default".to_string()),
            true,
            false,
            false,
            Some(60),
            Some(5),
            Some(4_102_444_800),
            Some(serde_json::json!(["claude"])),
            Some(serde_json::json!(["claude:messages"])),
            Some(serde_json::json!(["claude-sonnet-4-5"])),
        )
        .expect("auth snapshot should build")
    }

    fn sample_candidate_row() -> StoredMinimalCandidateSelectionRow {
        StoredMinimalCandidateSelectionRow {
            provider_id: "provider-claude-chat-local-stream-1".to_string(),
            provider_name: "claude".to_string(),
            provider_type: "custom".to_string(),
            provider_is_active: true,
            endpoint_id: "endpoint-claude-chat-local-stream-1".to_string(),
            endpoint_api_format: "claude:messages".to_string(),
            endpoint_api_family: Some("claude".to_string()),
            endpoint_kind: Some("chat".to_string()),
            endpoint_is_active: true,
            key_id: "key-claude-chat-local-stream-1".to_string(),
            key_name: "prod".to_string(),
            key_auth_type: "api_key".to_string(),
            key_is_active: true,
            key_api_formats: Some(vec!["claude:messages".to_string()]),
            key_allowed_models: None,
            key_capabilities: None,
            model_id: "model-claude-chat-local-stream-1".to_string(),
            global_model_id: "global-model-claude-chat-local-stream-1".to_string(),
            global_model_name: "claude-sonnet-4-5".to_string(),
            global_model_mappings: None,
            global_model_supports_streaming: Some(true),
            model_provider_model_name: "claude-sonnet-4-5-upstream".to_string(),
            model_provider_model_mappings: Some(vec![StoredProviderModelMapping {
                name: "claude-sonnet-4-5-upstream".to_string(),
                priority: 1,
                api_formats: Some(vec!["claude:messages".to_string()]),
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
            "provider-claude-chat-local-stream-1".to_string(),
            "claude".to_string(),
            Some("https://example.com".to_string()),
            "custom".to_string(),
        )
        .expect("provider should build")
        .with_transport_fields(
            true,
            false,
            None,
            Some(2),
            Some(serde_json::json!({"url":"http://provider-proxy.internal:8080"})),
            Some(20.0),
            None,
            None,
        )
    }

    fn sample_provider_catalog_endpoint() -> StoredProviderCatalogEndpoint {
        StoredProviderCatalogEndpoint::new(
            "endpoint-claude-chat-local-stream-1".to_string(),
            "provider-claude-chat-local-stream-1".to_string(),
            "claude:messages".to_string(),
            Some("claude".to_string()),
            Some("chat".to_string()),
            true,
        )
        .expect("endpoint should build")
        .with_transport_fields(
            "https://api.anthropic.example".to_string(),
            Some(serde_json::json!([
                {"action":"set","key":"x-endpoint-tag","value":"claude-chat-local-stream"}
            ])),
            Some(serde_json::json!([
                {"action":"set","path":"metadata.mode","value":"safe"},
                {"action":"rename","from":"metadata.client","to":"metadata.source"}
            ])),
            Some(2),
            Some("/custom/v1/messages".to_string()),
            None,
            None,
            None,
        )
        .expect("endpoint transport should build")
    }

    fn sample_provider_catalog_key() -> StoredProviderCatalogKey {
        StoredProviderCatalogKey::new(
            "key-claude-chat-local-stream-1".to_string(),
            "provider-claude-chat-local-stream-1".to_string(),
            "prod".to_string(),
            "api_key".to_string(),
            None,
            true,
        )
        .expect("key should build")
        .with_transport_fields(
            Some(serde_json::json!(["claude:messages"])),
            encrypt_python_fernet_plaintext(
                DEVELOPMENT_ENCRYPTION_KEY,
                "sk-upstream-claude-chat-stream",
            )
            .expect("api key should encrypt"),
            None,
            None,
            None,
            None,
            Some(serde_json::json!({"enabled": true, "node_id":"proxy-node-claude-chat-stream"})),
            Some(serde_json::json!({"transport_profile":"chrome_136"})),
        )
        .expect("key transport should build")
    }

    let seen_execution_runtime = Arc::new(Mutex::new(None::<SeenExecutionRuntimeStreamRequest>));
    let seen_execution_runtime_clone = Arc::clone(&seen_execution_runtime);
    let seen_report = Arc::new(Mutex::new(false));
    let seen_report_clone = Arc::clone(&seen_report);
    let decision_hits = Arc::new(Mutex::new(0usize));
    let decision_hits_clone = Arc::clone(&decision_hits);
    let plan_hits = Arc::new(Mutex::new(0usize));
    let plan_hits_clone = Arc::clone(&plan_hits);
    let public_hits = Arc::new(Mutex::new(0usize));
    let public_hits_clone = Arc::clone(&public_hits);

    let upstream = Router::new()
        .route(
            "/api/internal/gateway/decision-stream",
            any(move |_request: Request| {
                let decision_hits_inner = Arc::clone(&decision_hits_clone);
                async move {
                    *decision_hits_inner.lock().expect("mutex should lock") += 1;
                    Json(json!({"action": "proxy_public"}))
                }
            }),
        )
        .route(
            "/api/internal/gateway/plan-stream",
            any(move |_request: Request| {
                let plan_hits_inner = Arc::clone(&plan_hits_clone);
                async move {
                    *plan_hits_inner.lock().expect("mutex should lock") += 1;
                    Json(json!({"action": "proxy_public"}))
                }
            }),
        )
        .route(
            "/api/internal/gateway/report-stream",
            any(move |request: Request| {
                let seen_report_inner = Arc::clone(&seen_report_clone);
                async move {
                    let (_parts, body) = request.into_parts();
                    let _raw_body = to_bytes(body, usize::MAX).await.expect("body should read");
                    *seen_report_inner.lock().expect("mutex should lock") = true;
                    Json(json!({"ok": true}))
                }
            }),
        )
        .route(
            "/v1/messages",
            any(move |_request: Request| {
                let public_hits_inner = Arc::clone(&public_hits_clone);
                async move {
                    *public_hits_inner.lock().expect("mutex should lock") += 1;
                    (StatusCode::IM_A_TEAPOT, Body::from("public-route-hit"))
                }
            }),
        );

    let execution_runtime = Router::new().route(
        "/v1/execute/stream",
        any(move |request: Request| {
            let seen_execution_runtime_inner = Arc::clone(&seen_execution_runtime_clone);
            async move {
                let (parts, body) = request.into_parts();
                let raw_body = to_bytes(body, usize::MAX).await.expect("body should read");
                let payload: serde_json::Value =
                    serde_json::from_slice(&raw_body).expect("execution runtime payload should parse");
                *seen_execution_runtime_inner.lock().expect("mutex should lock") =
                    Some(SeenExecutionRuntimeStreamRequest {
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
                        model: payload
                            .get("body")
                            .and_then(|value| value.get("json_body"))
                            .and_then(|value| value.get("model"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        stream: payload
                            .get("body")
                            .and_then(|value| value.get("json_body"))
                            .and_then(|value| value.get("stream"))
                            .and_then(|value| value.as_bool())
                            .unwrap_or(false),
                        accept: payload
                            .get("headers")
                            .and_then(|value| value.get("accept"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        auth_header_value: payload
                            .get("headers")
                            .and_then(|value| value.get("x-api-key"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        endpoint_tag: payload
                            .get("headers")
                            .and_then(|value| value.get("x-endpoint-tag"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        metadata_mode: payload
                            .get("body")
                            .and_then(|value| value.get("json_body"))
                            .and_then(|value| value.get("metadata"))
                            .and_then(|value| value.get("mode"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        metadata_source: payload
                            .get("body")
                            .and_then(|value| value.get("json_body"))
                            .and_then(|value| value.get("metadata"))
                            .and_then(|value| value.get("source"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        proxy_node_id: payload
                            .get("proxy")
                            .and_then(|value| value.get("node_id"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        transport_profile_id: payload
                            .get("transport_profile").and_then(|value| value.get("profile_id"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    });
                let frames = concat!(
                    "{\"type\":\"headers\",\"payload\":{\"kind\":\"headers\",\"status_code\":200,\"headers\":{\"content-type\":\"text/event-stream\"}}}\n",
                    "{\"type\":\"data\",\"payload\":{\"kind\":\"data\",\"text\":\"event: message_start\\ndata: {\\\"type\\\":\\\"message_start\\\"}\\n\\n\"}}\n",
                    "{\"type\":\"data\",\"payload\":{\"kind\":\"data\",\"text\":\"event: message_stop\\ndata: {\\\"type\\\":\\\"message_stop\\\"}\\n\\n\"}}\n",
                    "{\"type\":\"telemetry\",\"payload\":{\"kind\":\"telemetry\",\"telemetry\":{\"elapsed_ms\":31,\"ttfb_ms\":11,\"upstream_bytes\":37}}}\n",
                    "{\"type\":\"eof\",\"payload\":{\"kind\":\"eof\"}}\n"
                );
                let mut response = Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::from(frames))
                    .expect("response should build");
                response.headers_mut().insert(
                    http::header::CONTENT_TYPE,
                    HeaderValue::from_static("application/x-ndjson"),
                );
                response
            }
        }),
    );

    let auth_repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some(hash_api_key("sk-client-claude-chat-stream-local")),
        sample_auth_snapshot(
            "api-key-claude-chat-local-stream-1",
            "user-claude-chat-local-stream-1",
        ),
    )]));
    let candidate_selection_repository =
        Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
            sample_candidate_row(),
        ]));
    let request_candidate_repository = Arc::new(InMemoryRequestCandidateRepository::default());
    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider_catalog_provider()],
        vec![sample_provider_catalog_endpoint()],
        vec![sample_provider_catalog_key()],
    ));

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let (execution_runtime_url, execution_runtime_handle) = start_server(execution_runtime).await;
    let gateway_state = build_state_with_execution_runtime_override(execution_runtime_url.clone())
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

    let response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/messages"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .header("x-api-key", "sk-client-claude-chat-stream-local")
        .header(TRACE_ID_HEADER, "trace-claude-chat-local-stream-123")
        .body(
            "{\"model\":\"claude-sonnet-4-5\",\"messages\":[],\"stream\":true,\"metadata\":{\"client\":\"desktop-claude-stream\"}}",
        )
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        strip_sse_keepalive_comments(&response.text().await.expect("body should read")),
        concat!(
            "event: message_start\ndata: {\"type\":\"message_start\"}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        )
    );

    let seen_execution_runtime_request = seen_execution_runtime
        .lock()
        .expect("mutex should lock")
        .clone()
        .expect("execution runtime stream should be captured");
    assert_eq!(
        seen_execution_runtime_request.trace_id,
        "trace-claude-chat-local-stream-123"
    );
    assert_eq!(
        seen_execution_runtime_request.url,
        "https://api.anthropic.example/custom/v1/messages"
    );
    assert_eq!(
        seen_execution_runtime_request.model,
        "claude-sonnet-4-5-upstream"
    );
    assert!(seen_execution_runtime_request.stream);
    assert_eq!(seen_execution_runtime_request.accept, "text/event-stream");
    assert_eq!(
        seen_execution_runtime_request.auth_header_value,
        "sk-upstream-claude-chat-stream"
    );
    assert_eq!(
        seen_execution_runtime_request.endpoint_tag,
        "claude-chat-local-stream"
    );
    assert_eq!(seen_execution_runtime_request.metadata_mode, "safe");
    assert_eq!(
        seen_execution_runtime_request.metadata_source,
        "desktop-claude-stream"
    );
    assert_eq!(
        seen_execution_runtime_request.proxy_node_id,
        "proxy-node-claude-chat-stream"
    );
    assert_eq!(
        seen_execution_runtime_request.transport_profile_id,
        "chrome_136"
    );

    let stored_candidates = request_candidate_repository
        .list_by_request_id("trace-claude-chat-local-stream-123")
        .await
        .expect("request candidate trace should read");
    assert_eq!(stored_candidates.len(), 1);
    assert_eq!(stored_candidates[0].status, RequestCandidateStatus::Success);

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(
        !*seen_report.lock().expect("mutex should lock"),
        "report-stream should stay local when request candidate persistence is available"
    );

    assert_eq!(*decision_hits.lock().expect("mutex should lock"), 0);
    assert_eq!(*plan_hits.lock().expect("mutex should lock"), 0);
    assert_eq!(*public_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    execution_runtime_handle.abort();
    upstream_handle.abort();
}
