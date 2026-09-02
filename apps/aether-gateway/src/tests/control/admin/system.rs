use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use aether_crypto::{encrypt_python_fernet_plaintext, DEVELOPMENT_ENCRYPTION_KEY};
use aether_data::repository::auth::{
    InMemoryAuthApiKeySnapshotRepository, StoredAuthApiKeyExportRecord,
};
use aether_data::repository::auth_modules::{
    InMemoryAuthModuleReadRepository, StoredOAuthProviderModuleConfig,
};
use aether_data::repository::candidates::InMemoryRequestCandidateRepository;
use aether_data::repository::global_models::InMemoryGlobalModelReadRepository;
use aether_data::repository::oauth_providers::InMemoryOAuthProviderRepository;
use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
use aether_data::repository::proxy_nodes::InMemoryProxyNodeRepository;
use aether_data::repository::users::{
    InMemoryUserReadRepository, StoredUserAuthRecord, UpsertUserGroupRecord, UserReadRepository,
};
use aether_data::repository::wallet::{InMemoryWalletRepository, StoredWalletSnapshot};
use aether_data_contracts::repository::global_models::StoredPublicGlobalModel;
use axum::body::Body;
use axum::routing::{any, delete, get, post, put};
use axum::{extract::Request, Router};
use http::StatusCode;
use serde_json::json;

use super::super::{
    build_router_with_state, issue_test_admin_access_token, sample_admin_global_model,
    sample_admin_provider_model, sample_endpoint, sample_key, sample_ldap_module_config,
    sample_oauth_provider_config, sample_provider, sample_proxy_node,
    sample_recent_key_rpm_candidate, start_server, AppState,
};
use crate::constants::{
    GATEWAY_HEADER, TRUSTED_ADMIN_SESSION_ID_HEADER, TRUSTED_ADMIN_USER_ID_HEADER,
    TRUSTED_ADMIN_USER_ROLE_HEADER,
};
use crate::data::GatewayDataState;

static SYSTEM_UPDATE_TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn gateway_handles_admin_system_version_locally_with_trusted_admin_principal() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/system/version",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("unexpected upstream hit"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(AppState::new().expect("gateway should build"));
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/admin/system/version"))
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert!(payload["version"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_admin_system_aws_regions_locally_with_trusted_admin_principal() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/system/aws-regions",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("unexpected upstream hit"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(AppState::new().expect("gateway should build"));
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/admin/system/aws-regions"))
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let regions = payload["regions"]
        .as_array()
        .expect("regions should be array");
    assert!(regions.len() > 10);
    assert!(regions.iter().any(|value| value == "us-east-1"));
    assert!(regions.iter().any(|value| value == "eu-west-1"));
    assert!(regions.iter().any(|value| value == "ap-southeast-1"));
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_admin_system_aws_regions_locally_with_bearer_admin_session() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/system/aws-regions",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("unexpected upstream hit"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let state = AppState::new().expect("gateway should build");
    let access_token = issue_test_admin_access_token(&state, "device-admin-aws-regions").await;
    let gateway = build_router_with_state(state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/admin/system/aws-regions"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-admin-aws-regions")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let regions = payload["regions"]
        .as_array()
        .expect("regions should be array");
    assert!(regions.iter().any(|value| value == "us-east-1"));
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_admin_system_stats_locally_with_trusted_admin_principal() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/system/stats",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("unexpected upstream hit"))
            }
        }),
    );

    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-openai", "openai", 10),
            sample_provider("provider-anthropic", "anthropic", 20)
                .with_transport_fields(false, false, None, None, None, None, None, None),
        ],
        vec![],
        vec![],
    ));
    let data_state =
        GatewayDataState::with_provider_catalog_reader_for_tests(provider_catalog_repository);

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(data_state),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/admin/system/stats"))
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["users"]["total"], json!(0));
    assert_eq!(payload["users"]["active"], json!(0));
    assert_eq!(payload["providers"]["total"], json!(2));
    assert_eq!(payload["providers"]["active"], json!(1));
    assert_eq!(payload["api_keys"], json!(0));
    assert_eq!(payload["requests"], json!(0));
    assert_eq!(payload["usage_counter"]["status"], json!("idle"));
    assert_eq!(payload["usage_counter"]["outbox_pending_rows"], json!(0));
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_admin_system_config_export_locally_with_trusted_admin_principal() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/system/config/export",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("unexpected upstream hit"))
            }
        }),
    );

    let provider_id = "provider-openai".to_string();
    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider(&provider_id, "openai", 10).with_transport_fields(
                true,
                true,
                Some(8),
                Some(3),
                Some(json!({"node_id": "node-1"})),
                Some(30.0),
                Some(12.5),
                Some(json!({
                    "provider_ops": {
                        "connector": {
                            "credentials": {
                                "refresh_token": encrypt_python_fernet_plaintext(
                                    DEVELOPMENT_ENCRYPTION_KEY,
                                    "provider-refresh-token",
                                )
                                .expect("provider credential should encrypt")
                            }
                        }
                    }
                })),
            ),
        ],
        vec![
            sample_endpoint(
                "endpoint-chat",
                &provider_id,
                "openai",
                "https://api.openai.example",
            ),
            sample_endpoint(
                "endpoint-cli",
                &provider_id,
                "openai:responses",
                "https://api.openai.example",
            ),
        ],
        vec![{
            let mut key = sample_key("key-openai", &provider_id, "openai:chat", "live-api-key");
            key.name = "primary".to_string();
            key.allowed_models = Some(json!(["gpt-5"]));
            key.encrypted_auth_config = Some(
                encrypt_python_fernet_plaintext(
                    DEVELOPMENT_ENCRYPTION_KEY,
                    r#"{"refresh_token":"oauth-refresh"}"#,
                )
                .expect("auth config should encrypt"),
            );
            key
        }],
    ));
    let global_model_repository = Arc::new(
        InMemoryGlobalModelReadRepository::seed(Vec::<StoredPublicGlobalModel>::new())
            .with_admin_global_models(vec![{
                let mut model = sample_admin_global_model("global-gpt-5", "gpt-5", "GPT 5");
                model.usage_count = 7;
                model
            }])
            .with_admin_provider_models(vec![sample_admin_provider_model(
                "model-gpt-5",
                &provider_id,
                "global-gpt-5",
                "gpt-5",
            )]),
    );
    let auth_module_repository = Arc::new(InMemoryAuthModuleReadRepository::seed(
        Vec::<StoredOAuthProviderModuleConfig>::new(),
        Some(sample_ldap_module_config()),
    ));
    let oauth_provider_repository = Arc::new(InMemoryOAuthProviderRepository::seed(vec![
        sample_oauth_provider_config("linuxdo"),
    ]));
    let proxy_node_repository =
        Arc::new(InMemoryProxyNodeRepository::seed(vec![sample_proxy_node(
            "node-1",
        )
        .with_manual_proxy_fields(
            Some("http://proxy.local:8080".to_string()),
            Some("proxy-user".to_string()),
            Some("proxy-pass".to_string()),
        )]));

    let data_state = GatewayDataState::disabled()
        .attach_provider_catalog_repository_for_tests(provider_catalog_repository)
        .with_global_model_repository_for_tests(global_model_repository)
        .attach_auth_module_reader_for_tests(auth_module_repository)
        .attach_oauth_provider_repository_for_tests(oauth_provider_repository)
        .attach_proxy_node_repository_for_tests(proxy_node_repository)
        .with_encryption_key_for_tests(DEVELOPMENT_ENCRYPTION_KEY)
        .with_system_config_values_for_tests(vec![
            (
                "smtp_password".to_string(),
                json!(
                    encrypt_python_fernet_plaintext(DEVELOPMENT_ENCRYPTION_KEY, "smtp-secret",)
                        .expect("smtp secret should encrypt")
                ),
            ),
            ("site_name".to_string(), json!("Aether Test")),
        ]);

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(data_state),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/admin/system/config/export"))
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["version"], "2.3");
    assert!(payload["exported_at"].as_str().is_some());
    assert_eq!(payload["global_models"][0]["name"], "gpt-5");
    assert_eq!(payload["global_models"][0]["usage_count"], json!(7));
    assert_eq!(payload["providers"][0]["name"], "openai");
    assert_eq!(
        payload["providers"][0]["config"]["provider_ops"]["connector"]["credentials"]
            ["refresh_token"],
        "provider-refresh-token"
    );
    assert_eq!(
        payload["providers"][0]["api_keys"][0]["api_key"],
        "live-api-key"
    );
    assert_eq!(
        payload["providers"][0]["api_keys"][0]["auth_config"],
        r#"{"refresh_token":"oauth-refresh"}"#
    );
    assert_eq!(
        payload["providers"][0]["api_keys"][0]["supported_endpoints"],
        json!(["openai:chat"])
    );
    assert_eq!(
        payload["providers"][0]["models"][0]["global_model_name"],
        "gpt-5"
    );
    assert_eq!(payload["ldap_config"]["bind_password"], "");
    assert_eq!(
        payload["oauth_providers"][0]["client_secret"],
        "secret-value"
    );
    assert_eq!(
        payload["proxy_nodes"][0]["proxy_url"],
        "http://proxy.local:8080"
    );
    let smtp_password = payload["system_configs"]
        .as_array()
        .expect("system configs should be array")
        .iter()
        .find(|entry| entry["key"] == "smtp_password")
        .cloned()
        .expect("smtp_password should exist");
    assert_eq!(smtp_password["value"], "smtp-secret");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_admin_system_unavailable_write_routes_locally_with_trusted_admin_principal(
) {
    const DETAIL: &str = "Admin system data unavailable";

    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().fallback(any(move |_request: Request| {
        let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
        async move {
            *upstream_hits_inner.lock().expect("mutex should lock") += 1;
            (StatusCode::OK, Body::from("unexpected upstream hit"))
        }
    }));

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(AppState::new().expect("gateway should build"));
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let client = reqwest::Client::new();
    let unavailable_paths = [
        "/api/admin/system/config/import",
        "/api/admin/system/data/import",
    ];
    let local_paths = [
        "/api/admin/system/cleanup",
        "/api/admin/system/purge/config",
        "/api/admin/system/purge/usage",
        "/api/admin/system/purge/audit-logs",
        "/api/admin/system/purge/request-bodies",
        "/api/admin/system/purge/stats",
    ];

    for path in unavailable_paths {
        let response = client
            .post(format!("{gateway_url}{path}"))
            .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
            .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
            .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
            .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
            .json(&json!({}))
            .send()
            .await
            .expect("request should succeed");

        assert_eq!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "path={path}"
        );
        let payload: serde_json::Value = response.json().await.expect("json body should parse");
        assert_eq!(payload["detail"], DETAIL, "path={path}");
    }

    for path in local_paths {
        let response = client
            .post(format!("{gateway_url}{path}"))
            .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
            .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
            .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
            .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
            .json(&json!({}))
            .send()
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::OK, "path={path}");
        let payload: serde_json::Value = response.json().await.expect("json body should parse");
        assert!(
            payload["message"]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "path={path}"
        );
    }

    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_admin_system_api_formats_locally_with_trusted_admin_principal() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/system/api-formats",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("unexpected upstream hit"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(AppState::new().expect("gateway should build"));
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/admin/system/api-formats"))
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let formats = payload["formats"]
        .as_array()
        .expect("formats should be an array");
    assert_eq!(formats[0]["value"], "openai:chat");
    assert_eq!(formats[0]["default_path"], "/v1/chat/completions");
    let gemini_embedding = formats
        .iter()
        .find(|item| item["value"] == "gemini:embedding")
        .expect("gemini embedding format should exist");
    assert_eq!(
        gemini_embedding["default_path"],
        "/v1beta/models/{model}:{action}"
    );
    assert!(formats
        .iter()
        .any(|item| item["value"] == "openai:embedding"));
    assert!(formats.iter().any(|item| item["value"] == "openai:rerank"));
    assert!(formats.iter().any(|item| item["value"] == "jina:embedding"));
    assert!(formats.iter().any(|item| item["value"] == "jina:rerank"));
    assert!(formats.iter().any(|item| item["value"] == "gemini:video"));
    let gemini_interactions = formats
        .iter()
        .find(|item| item["value"] == "gemini:interactions")
        .expect("gemini interactions format should exist");
    assert_eq!(gemini_interactions["default_path"], "/v1/interactions");
    let aliyun_embedding = formats
        .iter()
        .find(|item| item["value"] == "aliyun:multimodal_embedding")
        .expect("aliyun multimodal embedding format should exist");
    assert_eq!(
        aliyun_embedding["default_path"],
        "/api/v1/services/embeddings/multimodal-embedding/multimodal-embedding"
    );
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_admin_system_configs_locally_with_trusted_admin_principal() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/system/configs",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("unexpected upstream hit"))
            }
        }),
    );

    let data_state = GatewayDataState::disabled().with_system_config_values_for_tests(vec![
        ("request_log_level".to_string(), json!("headers")),
        ("smtp_password".to_string(), json!("encrypted-secret")),
        ("site_name".to_string(), json!("Aether Test")),
        (
            "external_models_proxy_node_id".to_string(),
            json!("proxy-node-hidden"),
        ),
    ]);
    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(data_state),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/admin/system/configs"))
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let items = payload.as_array().expect("payload should be an array");
    assert!(items
        .iter()
        .any(|item| item["key"] == "request_record_level"));
    assert!(!items.iter().any(|item| item["key"] == "request_log_level"));
    assert!(!items
        .iter()
        .any(|item| item["key"] == "external_models_proxy_node_id"));
    let smtp_password = items
        .iter()
        .find(|item| item["key"] == "smtp_password")
        .expect("smtp_password should exist");
    assert_eq!(smtp_password["value"], serde_json::Value::Null);
    assert_eq!(smtp_password["is_set"], json!(true));
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_rejects_external_models_proxy_through_generic_system_config_routes() {
    let data_state = GatewayDataState::disabled().with_system_config_values_for_tests(vec![(
        "external_models_proxy_node_id".to_string(),
        json!("proxy-node-owned-by-models"),
    )]);
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(data_state.clone()),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;
    let client = reqwest::Client::new();
    let config_url =
        format!("{gateway_url}/api/admin/system/configs/external_models_proxy_node_id");

    let get_response = client
        .get(&config_url)
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("generic config get should complete");
    assert_eq!(get_response.status(), StatusCode::BAD_REQUEST);
    let get_payload: serde_json::Value = get_response.json().await.expect("json body should parse");
    assert!(get_payload["detail"]
        .as_str()
        .is_some_and(|detail| detail.contains("/api/admin/models/external/config")));

    let put_response = client
        .put(&config_url)
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .json(&json!({ "value": null }))
        .send()
        .await
        .expect("generic config put should complete");
    assert_eq!(put_response.status(), StatusCode::BAD_REQUEST);
    let put_payload: serde_json::Value = put_response.json().await.expect("json body should parse");
    assert!(put_payload["detail"]
        .as_str()
        .is_some_and(|detail| detail.contains("/api/admin/models/external/config")));

    let delete_response = client
        .delete(&config_url)
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("generic config delete should complete");
    assert_eq!(delete_response.status(), StatusCode::BAD_REQUEST);
    let delete_payload: serde_json::Value = delete_response
        .json()
        .await
        .expect("json body should parse");
    assert!(delete_payload["detail"]
        .as_str()
        .is_some_and(|detail| detail.contains("/api/admin/models/external/config")));

    assert_eq!(
        data_state
            .find_system_config_value_strong("external_models_proxy_node_id")
            .await
            .expect("external models config should remain readable"),
        Some(json!("proxy-node-owned-by-models"))
    );

    gateway_handle.abort();
}

#[tokio::test]
async fn gateway_handles_admin_system_config_detail_locally_with_trusted_admin_principal() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/system/configs/request_record_level",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("unexpected upstream hit"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(AppState::new().expect("gateway should build"));
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/api/admin/system/configs/request_record_level"
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
    assert_eq!(payload["key"], "request_record_level");
    assert_eq!(payload["value"], "full");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_admin_system_format_conversion_default_as_disabled() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/system/configs/enable_format_conversion",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("unexpected upstream hit"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(AppState::new().expect("gateway should build"));
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/api/admin/system/configs/enable_format_conversion"
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
    assert_eq!(payload["key"], "enable_format_conversion");
    assert_eq!(payload["value"], json!(false));
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_admin_system_model_directives_default_as_disabled() {
    let gateway = build_router_with_state(AppState::new().expect("gateway should build"));
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/api/admin/system/configs/enable_model_directives"
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
    assert_eq!(payload["key"], "enable_model_directives");
    assert_eq!(payload["value"], json!(false));

    gateway_handle.abort();
}

#[tokio::test]
async fn gateway_validates_chat_pii_redaction_system_config_locally_with_trusted_admin_principal() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/system/configs/module.chat_pii_redaction.cache_ttl_seconds",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("unexpected upstream hit"))
            }
        }),
    );

    let data_state =
        GatewayDataState::disabled()
            .with_system_config_values_for_tests(Vec::<(String, serde_json::Value)>::new());
    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(data_state),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;
    let client = reqwest::Client::new();

    let get_config = |key: &'static str| {
        let client = client.clone();
        let gateway_url = gateway_url.clone();
        async move {
            let response = client
                .get(format!("{gateway_url}/api/admin/system/configs/{key}"))
                .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
                .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
                .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
                .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
                .send()
                .await
                .expect("request should succeed");
            assert_eq!(response.status(), StatusCode::OK, "key={key}");
            response
                .json::<serde_json::Value>()
                .await
                .expect("json body should parse")
        }
    };
    let put_config = |key: &'static str, value: serde_json::Value| {
        let client = client.clone();
        let gateway_url = gateway_url.clone();
        async move {
            client
                .put(format!("{gateway_url}/api/admin/system/configs/{key}"))
                .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
                .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
                .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
                .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
                .json(&json!({ "value": value }))
                .send()
                .await
                .expect("request should succeed")
        }
    };

    assert_eq!(
        get_config("module.chat_pii_redaction.enabled").await["value"],
        json!(false)
    );
    assert_eq!(
        get_config("module.chat_pii_redaction.cache_ttl_seconds").await["value"],
        json!(300)
    );
    assert_eq!(
        get_config("module.chat_pii_redaction.placeholder_prefix").await["value"],
        json!("AETHER")
    );
    let default_rules_payload = get_config("module.chat_pii_redaction.rules").await;
    let default_rules = default_rules_payload["value"]
        .as_array()
        .expect("default rules should be an array");
    assert!(
        default_rules.iter().any(|rule| {
            rule["name"] == json!("手机号") && rule["features"]["validator"] == json!("cn_phone")
        }),
        "default rules should include 手机号"
    );

    let enabled_response = put_config("module.chat_pii_redaction.enabled", json!(true)).await;
    assert_eq!(enabled_response.status(), StatusCode::OK);
    let enabled_payload: serde_json::Value = enabled_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(enabled_payload["value"], json!(true));

    let rules_response = put_config(
        "module.chat_pii_redaction.rules",
        json!([
            {
                "id": "email",
                "name": "邮箱",
                "pattern": r"(?i)[A-Z0-9._%+-]{1,64}@[A-Z0-9.-]{1,253}\.[A-Z]{2,63}",
                "enabled": true,
                "features": {"validator": "email"},
                "system": true
            },
            {
                "id": "custom_code",
                "name": "自定义规则",
                "pattern": r"CODE-\d{6}",
                "enabled": false,
                "features": {"validator": "custom_code", "experimental": true},
                "system": false
            }
        ]),
    )
    .await;
    assert_eq!(rules_response.status(), StatusCode::OK);
    let rules_payload: serde_json::Value =
        rules_response.json().await.expect("json body should parse");
    assert_eq!(
        rules_payload["value"][1]["features"]["experimental"],
        json!(true)
    );

    let ttl_response = put_config("module.chat_pii_redaction.cache_ttl_seconds", json!(3600)).await;
    assert_eq!(ttl_response.status(), StatusCode::OK);
    let ttl_payload: serde_json::Value = ttl_response.json().await.expect("json body should parse");
    assert_eq!(ttl_payload["value"], json!(3600));

    let prefix_response = put_config(
        "module.chat_pii_redaction.placeholder_prefix",
        json!("vendor_safe"),
    )
    .await;
    assert_eq!(prefix_response.status(), StatusCode::OK);
    let prefix_payload: serde_json::Value = prefix_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(prefix_payload["value"], json!("VENDOR_SAFE"));

    let invalid_prefix_response = put_config(
        "module.chat_pii_redaction.placeholder_prefix",
        json!("bad-prefix"),
    )
    .await;
    assert_eq!(invalid_prefix_response.status(), StatusCode::BAD_REQUEST);

    let invalid_rules_response = put_config(
        "module.chat_pii_redaction.rules",
        json!([
            {
                "id": "broken",
                "name": "坏规则",
                "pattern": "[",
                "enabled": true,
                "features": {"validator": "broken"},
                "system": false
            }
        ]),
    )
    .await;
    assert_eq!(invalid_rules_response.status(), StatusCode::BAD_REQUEST);

    let enabled_default_response =
        put_config("module.chat_pii_redaction.enabled", serde_json::Value::Null).await;
    assert_eq!(enabled_default_response.status(), StatusCode::OK);
    let enabled_default_payload: serde_json::Value = enabled_default_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(enabled_default_payload["value"], json!(false));

    let rules_default_response =
        put_config("module.chat_pii_redaction.rules", serde_json::Value::Null).await;
    assert_eq!(rules_default_response.status(), StatusCode::OK);
    let rules_default_payload: serde_json::Value = rules_default_response
        .json()
        .await
        .expect("json body should parse");
    assert!(!rules_default_payload["value"]
        .as_array()
        .expect("default rules should be an array")
        .is_empty());

    let invalid_ttl_response =
        put_config("module.chat_pii_redaction.cache_ttl_seconds", json!(600)).await;
    assert_eq!(invalid_ttl_response.status(), StatusCode::BAD_REQUEST);

    let ttl_default_response = put_config(
        "module.chat_pii_redaction.cache_ttl_seconds",
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(ttl_default_response.status(), StatusCode::OK);
    let ttl_default_payload: serde_json::Value = ttl_default_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(ttl_default_payload["value"], json!(300));

    let prefix_default_response = put_config(
        "module.chat_pii_redaction.placeholder_prefix",
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(prefix_default_response.status(), StatusCode::OK);
    let prefix_default_payload: serde_json::Value = prefix_default_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(prefix_default_payload["value"], json!("AETHER"));
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}


#[tokio::test]
async fn gateway_sets_admin_system_config_locally_with_trusted_admin_principal() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/system/configs/smtp_password",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("unexpected upstream hit"))
            }
        }),
    );

    let data_state =
        GatewayDataState::disabled().with_encryption_key_for_tests(DEVELOPMENT_ENCRYPTION_KEY);
    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(data_state),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let client = reqwest::Client::new();
    let put_response = client
        .put(format!(
            "{gateway_url}/api/admin/system/configs/smtp_password"
        ))
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .json(&json!({
            "value": "smtp-secret-123",
            "description": "SMTP password",
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(put_response.status(), StatusCode::OK);
    let put_payload: serde_json::Value = put_response.json().await.expect("json body should parse");
    assert_eq!(put_payload["key"], "smtp_password");
    assert_eq!(put_payload["value"], "********");
    assert!(put_payload["updated_at"].as_str().is_some());

    let get_response = client
        .get(format!(
            "{gateway_url}/api/admin/system/configs/smtp_password"
        ))
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(get_response.status(), StatusCode::OK);
    let get_payload: serde_json::Value = get_response.json().await.expect("json body should parse");
    assert_eq!(get_payload["value"], serde_json::Value::Null);
    assert_eq!(get_payload["is_set"], json!(true));
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_deletes_admin_system_config_locally_with_trusted_admin_principal() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/system/configs/custom_flag",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("unexpected upstream hit"))
            }
        }),
    );

    let data_state = GatewayDataState::disabled()
        .with_system_config_values_for_tests(vec![("custom_flag".to_string(), json!(true))]);
    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(data_state),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let client = reqwest::Client::new();
    let delete_response = client
        .delete(format!(
            "{gateway_url}/api/admin/system/configs/custom_flag"
        ))
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(delete_response.status(), StatusCode::OK);
    let delete_payload: serde_json::Value = delete_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(delete_payload["message"], "配置项 'custom_flag' 已删除");

    let get_response = client
        .get(format!(
            "{gateway_url}/api/admin/system/configs/custom_flag"
        ))
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_admin_key_rpm_locally_with_trusted_admin_principal() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/endpoints/rpm/key/key-openai",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("unexpected upstream hit"))
            }
        }),
    );

    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider("provider-openai", "openai", 10)],
        vec![sample_endpoint(
            "endpoint-openai",
            "provider-openai",
            "openai:chat",
            "https://api.openai.example",
        )],
        vec![
            sample_key("key-openai", "provider-openai", "openai:chat", "sk-test")
                .with_rate_limit_fields(Some(60), None, None, None, None, None, None, None, None),
        ],
    ));
    let now_unix_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after epoch")
        .as_secs() as i64;
    let request_candidate_repository = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        sample_recent_key_rpm_candidate(
            "cand-openai-rpm-1",
            "req-openai-rpm-1",
            "endpoint-openai",
            "key-openai",
            now_unix_secs,
            2,
        ),
        sample_recent_key_rpm_candidate(
            "cand-openai-rpm-2",
            "req-openai-rpm-2",
            "endpoint-openai",
            "key-openai",
            now_unix_secs,
            1,
        ),
    ]));

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                GatewayDataState::with_provider_catalog_and_request_candidate_reader_for_tests(
                    provider_catalog_repository,
                    request_candidate_repository,
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/api/admin/endpoints/rpm/key/key-openai"
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
    assert_eq!(payload["key_id"], "key-openai");
    assert_eq!(payload["current_rpm"], 2);
    assert_eq!(payload["rpm_limit"], 60);
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_resets_admin_key_rpm_locally_with_trusted_admin_principal() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/endpoints/rpm/key/key-openai",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("unexpected upstream hit"))
            }
        }),
    );

    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider("provider-openai", "openai", 10)],
        vec![sample_endpoint(
            "endpoint-openai",
            "provider-openai",
            "openai:chat",
            "https://api.openai.example",
        )],
        vec![
            sample_key("key-openai", "provider-openai", "openai:chat", "sk-test")
                .with_rate_limit_fields(Some(60), None, None, None, None, None, None, None, None),
        ],
    ));
    let now_unix_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after epoch")
        .as_secs() as i64;
    let request_candidate_repository = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        sample_recent_key_rpm_candidate(
            "cand-openai-rpm-1",
            "req-openai-rpm-1",
            "endpoint-openai",
            "key-openai",
            now_unix_secs,
            2,
        ),
        sample_recent_key_rpm_candidate(
            "cand-openai-rpm-2",
            "req-openai-rpm-2",
            "endpoint-openai",
            "key-openai",
            now_unix_secs,
            1,
        ),
    ]));

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                GatewayDataState::with_provider_catalog_and_request_candidate_reader_for_tests(
                    provider_catalog_repository,
                    request_candidate_repository,
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let client = reqwest::Client::new();
    let reset_response = client
        .delete(format!(
            "{gateway_url}/api/admin/endpoints/rpm/key/key-openai"
        ))
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(reset_response.status(), StatusCode::OK);
    let reset_payload: serde_json::Value =
        reset_response.json().await.expect("json body should parse");
    assert_eq!(reset_payload["message"], "RPM 计数已重置");

    let rpm_response = client
        .get(format!(
            "{gateway_url}/api/admin/endpoints/rpm/key/key-openai"
        ))
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(rpm_response.status(), StatusCode::OK);
    let rpm_payload: serde_json::Value = rpm_response.json().await.expect("json body should parse");
    assert_eq!(rpm_payload["key_id"], "key-openai");
    assert_eq!(rpm_payload["current_rpm"], 0);
    assert_eq!(rpm_payload["rpm_limit"], 60);
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}
