use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use aether_crypto::{
    decrypt_python_fernet_ciphertext, encrypt_python_fernet_plaintext, DEVELOPMENT_ENCRYPTION_KEY,
};
use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
use aether_data::repository::proxy_nodes::InMemoryProxyNodeRepository;
use aether_data_contracts::repository::provider_catalog::{
    ProviderCatalogReadRepository, StoredProviderCatalogKey, StoredProviderCatalogProvider,
};
use axum::body::{to_bytes, Body};
use axum::routing::{any, post};
use axum::{extract::Request, Json, Router};
use http::StatusCode;
use serde_json::json;

use super::super::super::{
    build_router_with_state, build_state_with_execution_runtime_override, sample_endpoint,
    sample_key, sample_proxy_node, start_server,
};
use crate::constants::{
    GATEWAY_HEADER, TRUSTED_ADMIN_SESSION_ID_HEADER, TRUSTED_ADMIN_USER_ID_HEADER,
    TRUSTED_ADMIN_USER_ROLE_HEADER,
};
use crate::data::GatewayDataState;

const PROVIDER_QUOTA_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;

fn run_provider_quota_test<F, Fut>(test_name: &'static str, make_future: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + 'static,
{
    let handle = std::thread::Builder::new()
        .name(test_name.to_string())
        .stack_size(PROVIDER_QUOTA_TEST_STACK_BYTES)
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("test runtime should build");
            runtime.block_on(make_future());
        })
        .expect("provider quota test thread should spawn");

    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

async fn gateway_refreshes_admin_provider_quota_locally_for_antigravity_with_trusted_admin_principal_inner(
) {
    #[derive(Debug, Clone)]
    struct SeenExecutionRuntimeRequest {
        url: String,
        authorization: String,
        provider_api_format: String,
        request_body: Option<serde_json::Value>,
    }

    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/endpoints/providers/provider-antigravity/refresh-quota",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("unexpected upstream hit"))
            }
        }),
    );

    let seen_execution_runtime = Arc::new(Mutex::new(None::<SeenExecutionRuntimeRequest>));
    let seen_execution_runtime_clone = Arc::clone(&seen_execution_runtime);
    let execution_runtime = Router::new().route(
        "/v1/execute/sync",
        any(move |request: Request| {
            let seen_execution_runtime_inner = Arc::clone(&seen_execution_runtime_clone);
            async move {
                let plan: aether_contracts::ExecutionPlan = serde_json::from_slice(
                    &to_bytes(request.into_body(), usize::MAX)
                        .await
                        .expect("body should read"),
                )
                .expect("plan should parse");
                *seen_execution_runtime_inner
                    .lock()
                    .expect("mutex should lock") = Some(SeenExecutionRuntimeRequest {
                    url: plan.url.clone(),
                    authorization: plan
                        .headers
                        .get("authorization")
                        .cloned()
                        .unwrap_or_default(),
                    provider_api_format: plan.provider_api_format.clone(),
                    request_body: plan.body.json_body.clone(),
                });
                let result = aether_contracts::ExecutionResult {
                    request_id: plan.request_id,
                    candidate_id: None,
                    status_code: 200,
                    headers: BTreeMap::new(),
                    response_observation: None,
                    body: Some(aether_contracts::ResponseBody {
                        json_body: Some(json!({
                            "models": {
                                "claude-sonnet-4": {
                                    "displayName": "Claude Sonnet 4",
                                    "quotaInfo": {
                                        "remainingFraction": 0.25,
                                        "resetTime": "2026-03-27T00:00:00Z"
                                    }
                                },
                                "gemini-2.5-pro": {
                                    "displayName": "Gemini 2.5 Pro"
                                }
                            }
                        })),
                        body_bytes_b64: None,
                    }),
                    telemetry: None,
                    error: None,
                };
                (StatusCode::OK, Json(result))
            }
        }),
    );

    let encrypted_auth_config = encrypt_python_fernet_plaintext(
        DEVELOPMENT_ENCRYPTION_KEY,
        r#"{
            "project_id":"project-ant-123",
            "client_version":"1.18.4",
            "session_id":"session-ant-1"
        }"#,
    )
    .expect("auth config ciphertext should build");
    let key = StoredProviderCatalogKey::new(
        "key-antigravity-a".to_string(),
        "provider-antigravity".to_string(),
        "default".to_string(),
        "oauth".to_string(),
        None,
        true,
    )
    .expect("key should build")
    .with_transport_fields(
        Some(json!(["gemini:generate_content"])),
        encrypt_python_fernet_plaintext(DEVELOPMENT_ENCRYPTION_KEY, "ya29.ant-token")
            .expect("api key ciphertext should build"),
        Some(encrypted_auth_config),
        None,
        None,
        None,
        None,
        None,
    )
    .expect("key transport should build");

    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![StoredProviderCatalogProvider::new(
            "provider-antigravity".to_string(),
            "antigravity".to_string(),
            Some("https://example.com".to_string()),
            "antigravity".to_string(),
        )
        .expect("provider should build")],
        vec![sample_endpoint(
            "endpoint-antigravity-chat",
            "provider-antigravity",
            "gemini:generate_content",
            "https://daily-cloudcode-pa.googleapis.com",
        )],
        vec![key],
    ));

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let (execution_runtime_url, execution_runtime_handle) = start_server(execution_runtime).await;
    let gateway = build_router_with_state(
        build_state_with_execution_runtime_override(execution_runtime_url.clone())
            .with_data_state_for_tests(
                GatewayDataState::with_provider_catalog_repository_for_tests(
                    provider_catalog_repository.clone(),
                )
                .with_encryption_key_for_tests(DEVELOPMENT_ENCRYPTION_KEY),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .post(format!(
            "{gateway_url}/api/admin/endpoints/providers/provider-antigravity/refresh-quota"
        ))
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["success"], 1);
    assert_eq!(payload["failed"], 0);
    assert_eq!(payload["total"], 1);
    assert_eq!(payload["results"][0]["status"], "success");
    assert_eq!(
        payload["results"][0]["quota_snapshot"]["provider_type"],
        "antigravity"
    );
    assert_eq!(
        payload["results"][0]["quota_snapshot"]["usage_ratio"],
        json!(0.75)
    );
    assert_eq!(
        payload["results"][0]["quota_snapshot"]["windows"]
            .as_array()
            .map(Vec::len),
        Some(1usize)
    );
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    let seen_execution_runtime_request = seen_execution_runtime
        .lock()
        .expect("mutex should lock")
        .clone()
        .expect("execution runtime request should be captured");
    assert_eq!(
        seen_execution_runtime_request.url,
        "https://daily-cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels"
    );
    assert_eq!(
        seen_execution_runtime_request.authorization,
        "Bearer ya29.ant-token"
    );
    assert_eq!(
        seen_execution_runtime_request.provider_api_format,
        "antigravity:fetch_available_models"
    );
    assert_eq!(
        seen_execution_runtime_request.request_body,
        Some(json!({ "project": "project-ant-123" }))
    );

    let reloaded = provider_catalog_repository
        .list_keys_by_ids(&["key-antigravity-a".to_string()])
        .await
        .expect("keys should read");
    assert_eq!(reloaded.len(), 1);
    assert_eq!(reloaded[0].oauth_invalid_reason, None);
    assert_eq!(
        reloaded[0]
            .upstream_metadata
            .as_ref()
            .and_then(|value| value.get("antigravity"))
            .and_then(|value| value.get("models"))
            .and_then(|value| value.get("claude-sonnet-4"))
            .and_then(|value| value.get("remaining_fraction")),
        Some(&json!(0.25))
    );
    assert_eq!(
        reloaded[0]
            .upstream_metadata
            .as_ref()
            .and_then(|value| value.get("antigravity"))
            .and_then(|value| value.get("models"))
            .and_then(|value| value.get("claude-sonnet-4"))
            .and_then(|value| value.get("used_percent")),
        Some(&json!(75.0))
    );
    assert_eq!(
        reloaded[0]
            .status_snapshot
            .as_ref()
            .and_then(|value| value.get("quota"))
            .and_then(|value| value.get("provider_type")),
        Some(&json!("antigravity"))
    );
    assert_eq!(
        reloaded[0]
            .status_snapshot
            .as_ref()
            .and_then(|value| value.get("quota"))
            .and_then(|value| value.get("usage_ratio")),
        Some(&json!(0.75))
    );
    assert_eq!(
        reloaded[0]
            .status_snapshot
            .as_ref()
            .and_then(|value| value.get("quota"))
            .and_then(|value| value.get("windows"))
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(1usize)
    );

    gateway_handle.abort();
    execution_runtime_handle.abort();
    upstream_handle.abort();
}
