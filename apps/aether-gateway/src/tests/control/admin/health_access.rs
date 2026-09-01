use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use aether_crypto::DEVELOPMENT_ENCRYPTION_KEY;
use aether_data::repository::auth_modules::InMemoryAuthModuleReadRepository;
use aether_data::repository::candidates::InMemoryRequestCandidateRepository;
use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
use aether_data_contracts::repository::candidates::RequestCandidateStatus;
use aether_data_contracts::repository::provider_catalog::ProviderCatalogReadRepository;
use axum::body::Body;
use axum::routing::{any, get, patch, put};
use axum::{extract::Request, Router};
use http::StatusCode;
use serde_json::json;

use super::super::{
    build_router_with_state, issue_test_admin_access_token, sample_endpoint, sample_key,
    sample_ldap_module_config, sample_management_token, sample_oauth_module_provider,
    sample_provider, sample_request_candidate, start_server, AppState,
};
use crate::constants::{
    GATEWAY_HEADER, TRUSTED_ADMIN_SESSION_ID_HEADER, TRUSTED_ADMIN_USER_ID_HEADER,
    TRUSTED_ADMIN_USER_ROLE_HEADER,
};
use crate::data::GatewayDataState;

const ADMIN_ENDPOINT_HEALTH_DATA_UNAVAILABLE_DETAIL: &str =
    "Admin endpoint health data unavailable";

#[tokio::test]
async fn gateway_recovers_admin_key_health_locally_with_trusted_admin_principal() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/endpoints/health/keys/key-openai",
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
                .with_health_fields(
                    Some(json!({"openai:chat": {
                        "health_score": 0.2,
                        "consecutive_failures": 4,
                        "last_failure_at": "2026-03-26T12:00:00+00:00"
                    }})),
                    Some(json!({"openai:chat": {
                        "open": true,
                        "open_at": "2026-03-26T12:01:00+00:00",
                        "next_probe_at": "2099-03-26T12:05:00+00:00",
                        "half_open_until": null,
                        "half_open_successes": 0,
                        "half_open_failures": 1
                    }})),
                ),
        ],
    ));

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                GatewayDataState::with_provider_catalog_repository_for_tests(
                    provider_catalog_repository.clone(),
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .patch(format!(
            "{gateway_url}/api/admin/endpoints/health/keys/key-openai?api_format=openai:chat"
        ))
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("request should succeed");

    let status = response.status();
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(status, StatusCode::OK, "payload={payload}");
    assert_eq!(payload["message"], "Key 的 openai:chat 格式已恢复");
    assert_eq!(payload["details"]["api_format"], "openai:chat");
    assert_eq!(payload["details"]["health_score"], 1.0);
    assert_eq!(payload["details"]["circuit_breaker_open"], false);
    assert_eq!(payload["details"]["is_active"], true);
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    let recovered_key = provider_catalog_repository
        .list_keys_by_ids(&["key-openai".to_string()])
        .await
        .expect("key should read")
        .into_iter()
        .next()
        .expect("key should exist");
    assert_eq!(recovered_key.is_active, true);
    assert_eq!(
        recovered_key.health_by_format,
        Some(json!({"openai:chat": {
            "health_score": 1.0,
            "consecutive_failures": 0,
            "last_failure_at": null
        }}))
    );
    assert_eq!(
        recovered_key.circuit_breaker_by_format,
        Some(json!({"openai:chat": {
            "open": false,
            "open_at": null,
            "next_probe_at": null,
            "half_open_until": null,
            "half_open_successes": 0,
            "half_open_failures": 0
        }}))
    );

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_recovers_all_admin_key_health_locally_with_trusted_admin_principal() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/api/admin/endpoints/health/keys",
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
            sample_key(
                "key-openai-circuit",
                "provider-openai",
                "openai:chat",
                "sk-test",
            )
            .with_health_fields(
                Some(json!({"openai:chat": {"health_score": 0.3}})),
                Some(json!({"openai:chat": {"open": true}})),
            ),
            sample_key(
                "key-openai-healthy",
                "provider-openai",
                "openai:chat",
                "sk-test-2",
            )
            .with_health_fields(
                Some(json!({"openai:chat": {"health_score": 0.9}})),
                Some(json!({"openai:chat": {"open": false}})),
            ),
        ],
    ));

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                GatewayDataState::with_provider_catalog_repository_for_tests(
                    provider_catalog_repository.clone(),
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .patch(format!("{gateway_url}/api/admin/endpoints/health/keys"))
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("request should succeed");

    let status = response.status();
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(status, StatusCode::OK, "payload={payload}");
    assert_eq!(payload["recovered_count"], 1);
    assert_eq!(payload["recovered_keys"][0]["key_id"], "key-openai-circuit");
    assert_eq!(
        payload["recovered_keys"][0]["provider_id"],
        "provider-openai"
    );
    assert_eq!(
        payload["recovered_keys"][0]["api_formats"],
        json!(["openai:chat"])
    );
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    let keys = provider_catalog_repository
        .list_keys_by_ids(&[
            "key-openai-circuit".to_string(),
            "key-openai-healthy".to_string(),
        ])
        .await
        .expect("keys should read");
    let circuit_key = keys
        .iter()
        .find(|key| key.id == "key-openai-circuit")
        .expect("circuit key should exist");
    let healthy_key = keys
        .iter()
        .find(|key| key.id == "key-openai-healthy")
        .expect("healthy key should exist");
    assert_eq!(circuit_key.health_by_format, Some(json!({})));
    assert_eq!(circuit_key.circuit_breaker_by_format, Some(json!({})));
    assert_eq!(
        healthy_key.circuit_breaker_by_format,
        Some(json!({"openai:chat": {"open": false}}))
    );

    gateway_handle.abort();
    upstream_handle.abort();
}
