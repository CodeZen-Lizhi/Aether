use super::{
    any, build_router_with_state, build_state_with_execution_runtime_override,
    encrypt_python_fernet_plaintext, json, run_stream_provider_gemini_test, start_server,
    strip_sse_keepalive_comments, to_bytes, Arc, Body, Bytes, Digest, HeaderName, HeaderValue,
    InMemoryAuthApiKeySnapshotRepository, InMemoryMinimalCandidateSelectionReadRepository,
    InMemoryProviderCatalogReadRepository, InMemoryRequestCandidateRepository, Json, Mutex,
    Request, RequestCandidateReadRepository, RequestCandidateStatus, Response, Router, Sha256,
    StatusCode, StoredAuthApiKeySnapshot, StoredMinimalCandidateSelectionRow,
    StoredProviderCatalogEndpoint, StoredProviderCatalogKey, StoredProviderCatalogProvider,
    StoredProviderModelMapping, DEVELOPMENT_ENCRYPTION_KEY, TRACE_ID_HEADER,
};

#[test]
fn gateway_executes_gemini_cli_stream_via_local_decision_gate_with_local_stream_decision() {
    run_stream_provider_gemini_test(
        "gateway_executes_gemini_cli_stream_via_local_decision_gate_with_local_stream_decision",
        gateway_executes_gemini_cli_stream_via_local_decision_gate_with_local_stream_decision_impl,
    );
}

async fn gateway_executes_gemini_cli_stream_via_local_decision_gate_with_local_stream_decision_impl(
) {
    #[derive(Debug, Clone)]
    struct SeenExecutionRuntimeStreamRequest {
        trace_id: String,
        url: String,
        has_model_field: bool,
        accept: String,
        authorization: String,
        exact_temperature: f64,
        endpoint_tag: String,
        metadata_mode: String,
        metadata_source: String,
        tool_config_present: bool,
        proxy_node_id: String,
        transport_profile_id: String,
    }

    fn hash_api_key(value: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(value.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn sample_auth_snapshot(api_key_id: &str, user_id: &str) -> StoredAuthApiKeySnapshot {
        StoredAuthApiKeySnapshot::new(user_id.to_string(),
"alice".to_string(),
Some("alice@example.com".to_string()),
"user".to_string(),
"local".to_string(),
true,
false,
Some(serde_json::json!(["gemini"])),
Some(serde_json::json!(["gemini:generate_content"])),
Some(serde_json::json!(["gemini-cli"])),
api_key_id.to_string(),
Some("default".to_string()),
true,
false,
false,
Some(60),
Some(5),
Some(4_102_444_800),
Some(serde_json::json!(["gemini"])),
Some(serde_json::json!(["gemini:generate_content"])),
Some(serde_json::json!(["gemini-cli"])),
        )
        .expect("auth snapshot should build")
    }

    fn sample_candidate_row() -> StoredMinimalCandidateSelectionRow {
        StoredMinimalCandidateSelectionRow {
            provider_id: "provider-gemini-cli-local-1".to_string(),
            provider_name: "gemini".to_string(),
            provider_type: "custom".to_string(),
            provider_is_active: true,
            endpoint_id: "endpoint-gemini-cli-local-1".to_string(),
            endpoint_api_format: "gemini:generate_content".to_string(),
            endpoint_api_family: Some("gemini".to_string()),
            endpoint_kind: Some("cli".to_string()),
            endpoint_is_active: true,
            key_id: "key-gemini-cli-local-1".to_string(),
            key_name: "prod".to_string(),
            key_auth_type: "bearer".to_string(),
            key_is_active: true,
            key_api_formats: Some(vec!["gemini:generate_content".to_string()]),
            key_allowed_models: None,
            key_capabilities: None,
            model_id: "model-gemini-cli-local-1".to_string(),
            global_model_id: "global-model-gemini-cli-local-1".to_string(),
            global_model_name: "gemini-cli".to_string(),
            global_model_supports_streaming: Some(true),
            model_provider_model_name: "gemini-cli-upstream".to_string(),
            model_provider_model_mappings: Some(vec![StoredProviderModelMapping {
                name: "gemini-cli-upstream".to_string(),
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
            "provider-gemini-cli-local-1".to_string(),
            "gemini".to_string(),
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
            "endpoint-gemini-cli-local-1".to_string(),
            "provider-gemini-cli-local-1".to_string(),
            "gemini:generate_content".to_string(),
            Some("gemini".to_string()),
            Some("cli".to_string()),
            true,
        )
        .expect("endpoint should build")
        .with_transport_fields(
            "https://generativelanguage.googleapis.com".to_string(),
            Some(serde_json::json!([
                {"action":"set","key":"x-endpoint-tag","value":"gemini-cli-local"}
            ])),
            Some(serde_json::json!([
                {"action":"set","path":"metadata.mode","value":"safe"},
                {"action":"rename","from":"metadata.client","to":"metadata.source"},
                {"action":"drop","path":"toolConfig"}
            ])),
            Some(2),
            Some("/custom/v1beta/models/gemini-cli-upstream:streamGenerateContent".to_string()),
            None,
            None,
            None,
        )
        .expect("endpoint transport should build")
    }

    fn sample_provider_catalog_key() -> StoredProviderCatalogKey {
        StoredProviderCatalogKey::new(
            "key-gemini-cli-local-1".to_string(),
            "provider-gemini-cli-local-1".to_string(),
            "prod".to_string(),
            "bearer".to_string(),
            None,
            true,
        )
        .expect("key should build")
        .with_transport_fields(
            Some(serde_json::json!(["gemini:generate_content"])),
            encrypt_python_fernet_plaintext(DEVELOPMENT_ENCRYPTION_KEY, "sk-upstream-gemini-cli")
                .expect("api key should encrypt"),
            None,
            None,
            None,
            None,
            Some(serde_json::json!({"enabled": true, "node_id":"proxy-node-gemini-cli-local"})),
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
            "/v1beta/models/gemini-cli:streamGenerateContent",
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
                        has_model_field: payload
                            .get("body")
                            .and_then(|value| value.get("json_body"))
                            .and_then(|value| value.get("model"))
                            .is_some(),
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
                        exact_temperature: payload
                            .get("body")
                            .and_then(|value| value.get("json_body"))
                            .and_then(|value| value.get("generationConfig"))
                            .and_then(|value| value.get("temperature"))
                            .and_then(|value| value.as_f64())
                            .unwrap_or_default(),
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
                        tool_config_present: payload
                            .get("body")
                            .and_then(|value| value.get("json_body"))
                            .and_then(|value| value.get("toolConfig"))
                            .is_some(),
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
                    "{\"type\":\"data\",\"payload\":{\"kind\":\"data\",\"text\":\"data: {\\\"candidates\\\":[]}\\n\\n\"}}\n",
                    "{\"type\":\"telemetry\",\"payload\":{\"kind\":\"telemetry\",\"telemetry\":{\"elapsed_ms\":34,\"upstream_bytes\":26}}}\n",
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
        Some(hash_api_key("client-gemini-cli-local")),
        sample_auth_snapshot("api-key-gemini-cli-local-1", "user-gemini-cli-local-1"),
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
        .post(format!(
            "{gateway_url}/v1beta/models/gemini-cli:streamGenerateContent"
        ))
        .header(http::header::CONTENT_TYPE, "application/json")
        .header("user-agent", "GeminiCLI/1.0")
        .header("x-goog-api-key", "client-gemini-cli-local")
        .header(TRACE_ID_HEADER, "trace-gemini-cli-local-stream-123")
        .body(
            "{\"contents\":[],\"generationConfig\":{\"temperature\":0.2},\"metadata\":{\"client\":\"desktop-gemini-cli\"},\"toolConfig\":{\"functionCallingConfig\":{\"mode\":\"AUTO\"}}}",
        )
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        strip_sse_keepalive_comments(&response.text().await.expect("body should read")),
        "data: {\"candidates\":[]}\n\n"
    );

    let seen_execution_runtime_request = seen_execution_runtime
        .lock()
        .expect("mutex should lock")
        .clone()
        .expect("execution runtime stream should be captured");
    assert_eq!(
        seen_execution_runtime_request.trace_id,
        "trace-gemini-cli-local-stream-123"
    );
    assert_eq!(
        seen_execution_runtime_request.url,
        "https://generativelanguage.googleapis.com/custom/v1beta/models/gemini-cli-upstream:streamGenerateContent?alt=sse"
    );
    assert!(!seen_execution_runtime_request.has_model_field);
    assert_eq!(seen_execution_runtime_request.accept, "text/event-stream");
    assert_eq!(
        seen_execution_runtime_request.authorization,
        "Bearer sk-upstream-gemini-cli"
    );
    assert!((seen_execution_runtime_request.exact_temperature - 0.2).abs() < f64::EPSILON);
    assert_eq!(
        seen_execution_runtime_request.endpoint_tag,
        "gemini-cli-local"
    );
    assert_eq!(seen_execution_runtime_request.metadata_mode, "safe");
    assert_eq!(
        seen_execution_runtime_request.metadata_source,
        "desktop-gemini-cli"
    );
    assert!(!seen_execution_runtime_request.tool_config_present);
    assert_eq!(
        seen_execution_runtime_request.proxy_node_id,
        "proxy-node-gemini-cli-local"
    );
    assert_eq!(
        seen_execution_runtime_request.transport_profile_id,
        "chrome_136"
    );

    let stored_candidates = request_candidate_repository
        .list_by_request_id("trace-gemini-cli-local-stream-123")
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
async fn gateway_executes_vertex_ai_gemini_cli_stream_via_local_decision_gate_with_local_stream_decision_impl(
) {
    #[derive(Debug, Clone)]
    struct SeenExecutionRuntimeStreamRequest {
        trace_id: String,
        url: String,
        accept: String,
        authorization: String,
        x_goog_api_key: String,
        user_agent: String,
        has_model_field: bool,
        exact_temperature: f64,
        endpoint_tag: String,
        metadata_mode: String,
        metadata_source: String,
        tool_config_present: bool,
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
            Some(serde_json::json!(["gemini", "vertex"])),
            Some(serde_json::json!(["gemini:generate_content"])),
            Some(serde_json::json!(["gemini-cli"])),
            api_key_id.to_string(),
            Some("default".to_string()),
            true,
            false,
            false,
            Some(60),
            Some(5),
            Some(4_102_444_800),
            Some(serde_json::json!(["gemini", "vertex"])),
            Some(serde_json::json!(["gemini:generate_content"])),
            Some(serde_json::json!(["gemini-cli"])),
        )
        .expect("auth snapshot should build")
    }

    fn sample_candidate_row() -> StoredMinimalCandidateSelectionRow {
        StoredMinimalCandidateSelectionRow {
            provider_id: "provider-vertex-cli-stream-local-1".to_string(),
            provider_name: "vertex".to_string(),
            provider_type: "vertex_ai".to_string(),
            provider_is_active: true,
            endpoint_id: "endpoint-vertex-cli-stream-local-1".to_string(),
            endpoint_api_format: "gemini:generate_content".to_string(),
            endpoint_api_family: Some("gemini".to_string()),
            endpoint_kind: Some("cli".to_string()),
            endpoint_is_active: true,
            key_id: "key-vertex-cli-stream-local-1".to_string(),
            key_name: "prod".to_string(),
            key_auth_type: "api_key".to_string(),
            key_is_active: true,
            key_api_formats: Some(vec!["gemini:generate_content".to_string()]),
            key_allowed_models: None,
            key_capabilities: None,
            model_id: "model-vertex-cli-stream-local-1".to_string(),
            global_model_id: "global-model-vertex-cli-stream-local-1".to_string(),
            global_model_name: "gemini-cli".to_string(),
            global_model_supports_streaming: Some(true),
            model_provider_model_name: "gemini-cli-upstream".to_string(),
            model_provider_model_mappings: Some(vec![StoredProviderModelMapping {
                name: "gemini-cli-upstream".to_string(),
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
            "provider-vertex-cli-stream-local-1".to_string(),
            "vertex".to_string(),
            Some("https://example.com".to_string()),
            "vertex_ai".to_string(),
        )
        .expect("provider should build")
        .with_transport_fields(true, false, None, Some(2), None, Some(20.0), None, None)
    }

    fn sample_provider_catalog_endpoint() -> StoredProviderCatalogEndpoint {
        StoredProviderCatalogEndpoint::new(
            "endpoint-vertex-cli-stream-local-1".to_string(),
            "provider-vertex-cli-stream-local-1".to_string(),
            "gemini:generate_content".to_string(),
            Some("gemini".to_string()),
            Some("cli".to_string()),
            true,
        )
        .expect("endpoint should build")
        .with_transport_fields(
            "https://aiplatform.googleapis.com".to_string(),
            Some(serde_json::json!([
                {"action":"set","key":"x-endpoint-tag","value":"vertex-cli-local"}
            ])),
            Some(serde_json::json!([
                {"action":"set","path":"metadata.mode","value":"safe"},
                {"action":"rename","from":"metadata.client","to":"metadata.source"},
                {"action":"drop","path":"toolConfig"}
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
        let mut key = StoredProviderCatalogKey::new(
            "key-vertex-cli-stream-local-1".to_string(),
            "provider-vertex-cli-stream-local-1".to_string(),
            "prod".to_string(),
            "api_key".to_string(),
            None,
            true,
        )
        .expect("key should build")
        .with_transport_fields(
            Some(serde_json::json!(["gemini:generate_content"])),
            encrypt_python_fernet_plaintext(DEVELOPMENT_ENCRYPTION_KEY, "vertex-upstream-secret")
                .expect("api key should encrypt"),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("key transport should build");
        key.allow_auth_channel_mismatch_formats =
            Some(serde_json::json!(["gemini:generate_content"]));
        key
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
            "/v1beta/models/gemini-cli:streamGenerateContent",
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
                        x_goog_api_key: payload
                            .get("headers")
                            .and_then(|value| value.get("x-goog-api-key"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        user_agent: payload
                            .get("headers")
                            .and_then(|value| value.get("user-agent"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        has_model_field: payload
                            .get("body")
                            .and_then(|value| value.get("json_body"))
                            .and_then(|value| value.get("model"))
                            .is_some(),
                        exact_temperature: payload
                            .get("body")
                            .and_then(|value| value.get("json_body"))
                            .and_then(|value| value.get("generationConfig"))
                            .and_then(|value| value.get("temperature"))
                            .and_then(|value| value.as_f64())
                            .unwrap_or_default(),
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
                        tool_config_present: payload
                            .get("body")
                            .and_then(|value| value.get("json_body"))
                            .and_then(|value| value.get("toolConfig"))
                            .is_some(),
                    });
                let frames = concat!(
                    "{\"type\":\"headers\",\"payload\":{\"kind\":\"headers\",\"status_code\":200,\"headers\":{\"content-type\":\"text/event-stream\"}}}\n",
                    "{\"type\":\"data\",\"payload\":{\"kind\":\"data\",\"text\":\"data: {\\\"candidates\\\":[]}\\n\\n\"}}\n",
                    "{\"type\":\"telemetry\",\"payload\":{\"kind\":\"telemetry\",\"telemetry\":{\"elapsed_ms\":34,\"upstream_bytes\":26}}}\n",
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
        Some(hash_api_key("client-vertex-cli-local")),
        sample_auth_snapshot(
            "api-key-vertex-cli-stream-local-1",
            "user-vertex-cli-stream-local-1",
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
        .post(format!(
            "{gateway_url}/v1beta/models/gemini-cli:streamGenerateContent"
        ))
        .header(http::header::CONTENT_TYPE, "application/json")
        .header("user-agent", "GeminiCLI/1.0")
        .header("x-goog-api-key", "client-vertex-cli-local")
        .header(TRACE_ID_HEADER, "trace-vertex-cli-local-stream-123")
        .body(
            "{\"contents\":[],\"generationConfig\":{\"temperature\":0.2},\"metadata\":{\"client\":\"desktop-vertex-cli\"},\"toolConfig\":{\"functionCallingConfig\":{\"mode\":\"AUTO\"}}}",
        )
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        strip_sse_keepalive_comments(&response.text().await.expect("body should read")),
        "data: {\"candidates\":[]}\n\n"
    );

    let seen_execution_runtime_request = seen_execution_runtime
        .lock()
        .expect("mutex should lock")
        .clone()
        .expect("execution runtime stream should be captured");
    assert_eq!(
        seen_execution_runtime_request.trace_id,
        "trace-vertex-cli-local-stream-123"
    );
    assert_eq!(
        seen_execution_runtime_request.url,
        "https://aiplatform.googleapis.com/v1/publishers/google/models/gemini-cli-upstream:streamGenerateContent?alt=sse&key=vertex-upstream-secret"
    );
    assert_eq!(seen_execution_runtime_request.accept, "text/event-stream");
    assert!(seen_execution_runtime_request.authorization.is_empty());
    assert!(seen_execution_runtime_request.x_goog_api_key.is_empty());
    assert_eq!(seen_execution_runtime_request.user_agent, "GeminiCLI/1.0");
    assert!(!seen_execution_runtime_request.has_model_field);
    assert!((seen_execution_runtime_request.exact_temperature - 0.2).abs() < f64::EPSILON);
    assert_eq!(
        seen_execution_runtime_request.endpoint_tag,
        "vertex-cli-local"
    );
    assert_eq!(seen_execution_runtime_request.metadata_mode, "safe");
    assert_eq!(
        seen_execution_runtime_request.metadata_source,
        "desktop-vertex-cli"
    );
    assert!(!seen_execution_runtime_request.tool_config_present);

    let stored_candidates = request_candidate_repository
        .list_by_request_id("trace-vertex-cli-local-stream-123")
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
