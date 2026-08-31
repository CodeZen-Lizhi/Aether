use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{
    hash_api_key, sample_endpoint, sample_key, sample_models_candidate_row, sample_provider,
    sample_public_catalog_model, sample_public_global_model,
    sample_public_global_model_with_capabilities, sample_request_candidate,
    InMemoryAnnouncementReadRepository, InMemoryGlobalModelReadRepository,
    InMemoryMinimalCandidateSelectionReadRepository, InMemoryProviderCatalogReadRepository,
    InMemoryRequestCandidateRepository, RequestCandidateStatus, StoredAnnouncement,
    StoredPublicGlobalModel,
};
use crate::data::GatewayDataState;
use crate::tests::{
    any, build_router, build_router_with_state, json, start_server, to_bytes, AppState, Arc, Body,
    Json, Mutex, Request, Router, StatusCode, CONTROL_ROUTE_FAMILY_HEADER,
    CONTROL_ROUTE_KIND_HEADER, TRUSTED_ADMIN_SESSION_ID_HEADER, TRUSTED_ADMIN_USER_ID_HEADER,
    TRUSTED_ADMIN_USER_ROLE_HEADER,
};
use aether_crypto::{encrypt_python_fernet_plaintext, DEVELOPMENT_ENCRYPTION_KEY};
use aether_data::repository::announcements::{AnnouncementListQuery, AnnouncementReadRepository};
use aether_data::repository::auth::{
    InMemoryAuthApiKeySnapshotRepository, StoredAuthApiKeyExportRecord, StoredAuthApiKeySnapshot,
};
use aether_data::repository::auth_modules::{
    InMemoryAuthModuleReadRepository, StoredLdapModuleConfig, StoredOAuthProviderModuleConfig,
};
use aether_data::repository::billing::InMemoryBillingReadRepository;
use aether_data::repository::management_tokens::{
    InMemoryManagementTokenRepository, StoredManagementToken, StoredManagementTokenUserSummary,
    StoredManagementTokenWithUser,
};
use aether_data::repository::usage::InMemoryUsageReadRepository;
use aether_data::repository::users::{
    InMemoryUserReadRepository, StoredUserAuthRecord, StoredUserExportRow, UpsertUserGroupRecord,
    UserReadRepository,
};
use aether_data::repository::wallet::{
    InMemoryWalletRepository, StoredWalletSnapshot, WalletWriteRepository,
};
use aether_data_contracts::repository::billing::{
    AdminBillingMutationOutcome, BillingPlanWriteInput, BillingReadRepository,
    PaymentGatewayConfigWriteInput,
};
use aether_data_contracts::repository::global_models::StoredProviderActiveGlobalModel;
use aether_data_contracts::repository::provider_catalog::ProviderCatalogReadRepository;
use aether_data_contracts::repository::usage::{StoredRequestUsageAudit, UsageRepository};
use axum::response::IntoResponse;
use chrono::{TimeZone, Utc};

#[path = "public_support/dashboard.rs"]
mod dashboard;

#[tokio::test]
async fn gateway_handles_public_catalog_site_info_without_proxying_upstream() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                crate::data::GatewayDataState::disabled().with_system_config_values_for_tests(
                    vec![
                        ("site_name".to_string(), json!("Aether Local")),
                        ("site_subtitle".to_string(), json!("Rust Only")),
                    ],
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/public/site-info"))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["site_name"], "Aether Local");
    assert_eq!(payload["site_subtitle"], "Rust Only");
    assert_eq!(payload.as_object().map(|object| object.len()), Some(2));
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_public_catalog_providers_without_proxying_upstream() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-openai", "openai", 10),
            sample_provider("provider-claude", "claude", 20),
        ],
        vec![
            sample_endpoint(
                "endpoint-openai",
                "provider-openai",
                "openai:chat",
                "https://api.openai.example",
            ),
            sample_endpoint(
                "endpoint-claude",
                "provider-claude",
                "claude:messages",
                "https://api.anthropic.example",
            ),
            sample_endpoint(
                "endpoint-claude-cli",
                "provider-claude",
                "claude:messages",
                "https://api.anthropic.example",
            ),
        ],
        vec![],
    ));

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                crate::data::GatewayDataState::with_provider_catalog_reader_for_tests(
                    provider_catalog_repository,
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/public/providers?limit=10"))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let providers = payload.as_array().expect("providers should be an array");
    assert_eq!(providers.len(), 2);
    assert_eq!(providers[0]["id"], "provider-openai");
    assert!(providers[0].get("name").is_none());
    assert!(providers[0].get("description").is_none());
    assert!(providers[0].get("website").is_none());
    assert_eq!(providers[0]["provider_priority"], 10);
    assert_eq!(providers[0]["endpoints_count"], 1);
    assert_eq!(providers[0]["active_endpoints_count"], 1);
    assert_eq!(providers[0]["models_count"], 0);
    assert_eq!(providers[0]["active_models_count"], 0);
    assert_eq!(providers[1]["id"], "provider-claude");
    assert_eq!(providers[1]["endpoints_count"], 2);
    assert_eq!(providers[1]["active_endpoints_count"], 2);
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_public_catalog_models_without_proxying_upstream() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let global_model_repository = Arc::new(
        InMemoryGlobalModelReadRepository::seed(Vec::<StoredPublicGlobalModel>::new())
            .with_public_catalog_models(vec![
                sample_public_catalog_model(
                    "model-openai-gpt5",
                    "provider-openai",
                    "openai",
                    "gpt-5-preview",
                    "gpt-5",
                    "GPT 5",
                ),
                sample_public_catalog_model(
                    "model-claude-sonnet",
                    "provider-claude",
                    "claude",
                    "claude-sonnet-4-5-20251001",
                    "claude-sonnet-4-5",
                    "Claude Sonnet 4.5",
                ),
            ]),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                crate::data::GatewayDataState::with_global_model_reader_for_tests(
                    global_model_repository,
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/api/public/models?provider_id=provider-openai&limit=10"
        ))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let models = payload.as_array().expect("models should be an array");
    assert_eq!(models.len(), 1);
    assert_eq!(models[0]["id"], "model-openai-gpt5");
    assert!(models[0].get("provider_id").is_none());
    assert!(models[0].get("provider_name").is_none());
    assert_eq!(models[0]["name"], "gpt-5");
    assert_eq!(models[0]["display_name"], "GPT 5");
    assert_eq!(models[0]["tags"], serde_json::Value::Null);
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn public_catalog_excludes_unsupported_embedding_provider() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let mut active_embedding = sample_public_catalog_model(
        "model-openai-embedding-small",
        "provider-openai",
        "openai",
        "text-embedding-3-small",
        "text-embedding-3-small",
        "Text Embedding 3 Small",
    );
    active_embedding.supports_embedding = Some(true);
    active_embedding.supports_streaming = Some(false);
    let mut inactive_embedding = sample_public_catalog_model(
        "model-openai-embedding-inactive",
        "provider-openai",
        "openai",
        "text-embedding-3-large",
        "text-embedding-3-large",
        "Text Embedding 3 Large",
    );
    inactive_embedding.supports_embedding = Some(true);
    inactive_embedding.is_active = false;
    let mut unsupported_embedding = sample_public_catalog_model(
        "model-unsupported-embedding",
        "provider-openai",
        "openai",
        "legacy-embedding",
        "legacy-embedding",
        "Legacy Embedding",
    );
    unsupported_embedding.supports_embedding = Some(false);
    unsupported_embedding.is_active = false;

    let global_model_repository = Arc::new(
        InMemoryGlobalModelReadRepository::seed(Vec::<StoredPublicGlobalModel>::new())
            .with_public_catalog_models(vec![
                active_embedding,
                inactive_embedding,
                unsupported_embedding,
            ]),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                crate::data::GatewayDataState::with_global_model_reader_for_tests(
                    global_model_repository,
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/api/public/models?provider_id=provider-openai&limit=10"
        ))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let models = payload.as_array().expect("models should be an array");
    assert_eq!(models.len(), 1);
    assert_eq!(models[0]["id"], "model-openai-embedding-small");
    assert_eq!(models[0]["supports_embedding"], true);
    assert_eq!(models[0]["supports_streaming"], false);
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_public_catalog_search_models_without_proxying_upstream() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let global_model_repository = Arc::new(
        InMemoryGlobalModelReadRepository::seed(Vec::<StoredPublicGlobalModel>::new())
            .with_public_catalog_models(vec![
                sample_public_catalog_model(
                    "model-openai-gpt5",
                    "provider-openai",
                    "openai",
                    "gpt-5-preview",
                    "gpt-5",
                    "GPT 5",
                ),
                sample_public_catalog_model(
                    "model-claude-sonnet",
                    "provider-claude",
                    "claude",
                    "claude-sonnet-4-5-20251001",
                    "claude-sonnet-4-5",
                    "Claude Sonnet 4.5",
                ),
            ]),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                crate::data::GatewayDataState::with_global_model_reader_for_tests(
                    global_model_repository,
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/api/public/search/models?q=sonnet&limit=20"
        ))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let models = payload.as_array().expect("models should be an array");
    assert_eq!(models.len(), 1);
    assert!(models[0].get("provider_id").is_none());
    assert!(models[0].get("provider_name").is_none());
    assert_eq!(models[0]["name"], "claude-sonnet-4-5");
    assert_eq!(models[0]["display_name"], "Claude Sonnet 4.5");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_public_catalog_stats_without_proxying_upstream() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-openai", "openai", 10),
            sample_provider("provider-claude", "claude", 20),
        ],
        vec![
            sample_endpoint(
                "endpoint-openai",
                "provider-openai",
                "openai:chat",
                "https://api.openai.example",
            ),
            sample_endpoint(
                "endpoint-claude",
                "provider-claude",
                "claude:messages",
                "https://api.anthropic.example",
            ),
        ],
        vec![],
    ));
    let candidate_repository =
        Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
            sample_models_candidate_row("provider-openai", "openai", "openai:chat", "gpt-5", 10),
            sample_models_candidate_row("provider-openai", "openai", "openai:chat", "gpt-4.1", 10),
            sample_models_candidate_row(
                "provider-claude",
                "claude",
                "claude:messages",
                "claude-3-7-sonnet",
                20,
            ),
        ]));

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                crate::data::GatewayDataState::with_provider_catalog_and_minimal_candidate_selection_for_tests(
                    provider_catalog_repository,
                    candidate_repository,
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/public/stats"))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["total_providers"], 2);
    assert_eq!(payload["active_providers"], 2);
    assert_eq!(payload["total_models"], 3);
    assert_eq!(payload["active_models"], 3);
    assert_eq!(
        payload["supported_formats"],
        json!(["claude:messages", "openai:chat"])
    );
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_public_global_models_without_proxying_upstream() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let mut gpt_model = sample_public_global_model("gm-3", "gpt-5", "GPT 5", true);
    gpt_model.config = Some(json!({
        "description": "Public description",
        "model_mappings": ["gpt-5-upstream"],
        "provider_model_mappings": [{"name": "provider-gpt-5"}],
    }));
    let global_model_repository = Arc::new(InMemoryGlobalModelReadRepository::seed(vec![
        sample_public_global_model("gm-1", "claude-sonnet-4-5", "Claude Sonnet 4.5", true),
        sample_public_global_model("gm-2", "disabled-model", "Disabled Model", false),
        gpt_model,
    ]));

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                crate::data::GatewayDataState::with_global_model_reader_for_tests(
                    global_model_repository,
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/public/global-models?search=gpt"))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["total"], 1);
    assert_eq!(payload["models"][0]["name"], "gpt-5");
    assert_eq!(payload["models"][0]["display_name"], "GPT 5");
    assert_eq!(payload["models"][0]["usage_count"], 0);
    assert_eq!(
        payload["models"][0]["config"]["description"],
        "Public description"
    );
    assert!(payload["models"][0]["config"]
        .get("model_mappings")
        .is_none());
    assert!(payload["models"][0]["config"]
        .get("provider_model_mappings")
        .is_none());
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_public_health_api_formats_without_proxying_upstream() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-openai", "openai", 10),
            sample_provider("provider-claude", "claude", 20),
        ],
        vec![
            sample_endpoint(
                "endpoint-openai",
                "provider-openai",
                "openai:chat",
                "https://api.openai.example",
            ),
            sample_endpoint(
                "endpoint-claude",
                "provider-claude",
                "claude:messages",
                "https://api.anthropic.example",
            ),
        ],
        vec![],
    ));
    let now_unix_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after epoch")
        .as_secs() as i64;
    let request_candidate_repository = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        sample_request_candidate(
            "cand-openai-success",
            "req-openai-success",
            "endpoint-openai",
            RequestCandidateStatus::Success,
            now_unix_secs - 3_000,
            Some(now_unix_secs - 2_980),
        ),
        sample_request_candidate(
            "cand-openai-failed",
            "req-openai-failed",
            "endpoint-openai",
            RequestCandidateStatus::Failed,
            now_unix_secs - 2_000,
            Some(now_unix_secs - 1_980),
        ),
        sample_request_candidate(
            "cand-openai-skipped",
            "req-openai-skipped",
            "endpoint-openai",
            RequestCandidateStatus::Skipped,
            now_unix_secs - 1_000,
            Some(now_unix_secs - 980),
        ),
        sample_request_candidate(
            "cand-openai-pending",
            "req-openai-pending",
            "endpoint-openai",
            RequestCandidateStatus::Pending,
            now_unix_secs - 500,
            None,
        ),
        sample_request_candidate(
            "cand-claude-skipped",
            "req-claude-skipped",
            "endpoint-claude",
            RequestCandidateStatus::Skipped,
            now_unix_secs - 1_500,
            Some(now_unix_secs - 1_480),
        ),
    ]));

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                crate::data::GatewayDataState::with_provider_catalog_and_request_candidate_reader_for_tests(
                    provider_catalog_repository,
                    request_candidate_repository,
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/api/public/health/api-formats?lookback_hours=6&per_format_limit=10"
        ))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let formats = payload["formats"]
        .as_array()
        .expect("formats should be an array");
    assert_eq!(formats.len(), 2);
    assert_eq!(formats[0]["api_format"], "claude:messages");
    assert_eq!(formats[0]["api_path"], "/v1/messages");
    assert_eq!(formats[0]["total_attempts"], 1);
    assert_eq!(formats[0]["success_rate"], 1.0);
    assert_eq!(formats[0]["events"].as_array().map(Vec::len), Some(1));
    assert_eq!(formats[0]["timeline"].as_array().map(Vec::len), Some(60));
    assert_eq!(formats[1]["api_format"], "openai:chat");
    assert_eq!(formats[1]["api_path"], "/v1/chat/completions");
    assert_eq!(formats[1]["total_attempts"], 3);
    assert_eq!(formats[1]["success_count"], 1);
    assert_eq!(formats[1]["failed_count"], 1);
    assert_eq!(formats[1]["skipped_count"], 1);
    assert_eq!(formats[1]["success_rate"], 0.5);
    assert_eq!(formats[1]["events"].as_array().map(Vec::len), Some(3));
    assert_eq!(formats[1]["timeline"].as_array().map(Vec::len), Some(60));
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_public_capabilities_without_proxying_upstream() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router().expect("gateway should build");
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/capabilities"))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["capabilities"][0]["name"], "cache_1h");
    assert_eq!(payload["capabilities"][1]["name"], "context_1m");
    assert_eq!(payload["capabilities"][2]["name"], "gemini_files");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_public_user_configurable_capabilities_without_proxying_upstream() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router().expect("gateway should build");
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/capabilities/user-configurable"))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload, json!({ "capabilities": [] }));
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_public_model_capabilities_without_proxying_upstream() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let global_model_repository = Arc::new(InMemoryGlobalModelReadRepository::seed(vec![
        sample_public_global_model_with_capabilities(
            "gm-1",
            "gpt-5",
            "GPT 5",
            json!(["cache_1h", "missing_cap", "gemini_files"]),
        ),
    ]));

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                crate::data::GatewayDataState::with_global_model_reader_for_tests(
                    global_model_repository,
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/capabilities/model/gpt-5"))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["model"], "gpt-5");
    assert_eq!(payload["global_model_name"], "gpt-5");
    assert_eq!(
        payload["supported_capabilities"],
        json!(["cache_1h", "missing_cap", "gemini_files"])
    );
    assert_eq!(
        payload["capability_details"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(payload["capability_details"][0]["name"], "cache_1h");
    assert_eq!(payload["capability_details"][1]["name"], "gemini_files");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_missing_public_model_capabilities_without_proxying_upstream() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router().expect("gateway should build");
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/api/capabilities/model/missing-model"
        ))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["model"], "missing-model");
    assert_eq!(payload["supported_capabilities"], json!([]));
    assert_eq!(payload["capability_details"], json!([]));
    assert_eq!(payload["error"], "模型不存在");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_public_providers_without_proxying_upstream() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-2", "anthropic", 20),
            sample_provider("provider-1", "openai", 10),
        ],
        vec![],
        vec![],
    ));
    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                crate::data::GatewayDataState::with_provider_catalog_reader_for_tests(
                    provider_catalog_repository,
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/v1/providers"))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert!(payload["providers"][0].get("name").is_none());
    assert_eq!(payload["providers"][0]["provider_priority"], 10);
    assert!(payload["providers"][1].get("name").is_none());
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_public_provider_detail_without_proxying_upstream() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider("provider-1", "openai", 10)],
        vec![],
        vec![],
    ));
    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                crate::data::GatewayDataState::with_provider_catalog_reader_for_tests(
                    provider_catalog_repository,
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/v1/providers/provider-1"))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["id"], "provider-1");
    assert!(payload.get("name").is_none());
    assert_eq!(payload["provider_priority"], 10);

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/v1/providers/openai"))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["detail"], "Provider not found");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_public_providers_with_endpoints_without_proxying_upstream() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider("provider-1", "openai", 10)],
        vec![sample_endpoint(
            "endpoint-1",
            "provider-1",
            "openai:chat",
            "https://api.openai.example",
        )],
        vec![],
    ));
    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                crate::data::GatewayDataState::with_provider_catalog_reader_for_tests(
                    provider_catalog_repository,
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/v1/providers?include_endpoints=true"))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["providers"][0]["endpoints"][0]["id"], "endpoint-1");
    assert!(payload["providers"][0]["endpoints"][0]
        .get("base_url")
        .is_none());
    assert_eq!(
        payload["providers"][0]["endpoints"][0]["api_format"],
        "openai:chat"
    );
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_test_connection_alias_without_proxying_upstream() {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router().expect("gateway should build");
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/test-connection"))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::GONE);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(
        payload["detail"],
        "Deprecated endpoint. Please use /v1/test-connection."
    );
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_public_test_connection_without_hitting_fallback_probe() {
    let fallback_probe_hits = Arc::new(Mutex::new(0usize));
    let fallback_probe_hits_clone = Arc::clone(&fallback_probe_hits);
    let fallback_probe = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let fallback_probe_hits_inner = Arc::clone(&fallback_probe_hits_clone);
            async move {
                *fallback_probe_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let provider_hits = Arc::new(Mutex::new(0usize));
    let provider_hits_clone = Arc::clone(&provider_hits);
    let provider = Router::new().route(
        "/chat/completions",
        any(move |request: Request| {
            let provider_hits_inner = Arc::clone(&provider_hits_clone);
            async move {
                *provider_hits_inner.lock().expect("mutex should lock") += 1;
                let headers = request.headers().clone();
                let body = to_bytes(request.into_body(), usize::MAX)
                    .await
                    .expect("body should read");
                let body_json: serde_json::Value =
                    serde_json::from_slice(&body).expect("json body should parse");
                assert_eq!(
                    headers
                        .get("authorization")
                        .and_then(|value| value.to_str().ok()),
                    Some("Bearer sk-test-openai"),
                );
                assert_eq!(body_json["model"], "gpt-5");
                assert_eq!(body_json["messages"][0]["content"], "Health check");
                assert!(
                    body_json.get("max_tokens").is_none(),
                    "public OpenAI-compatible test connection must not force a tiny max_tokens value"
                );
                Json(json!({"id": "resp_local_test"})).into_response()
            }
        }),
    );

    let (provider_url, provider_handle) = start_server(provider).await;
    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider("provider-1", "openai", 10)],
        vec![sample_endpoint(
            "endpoint-1",
            "provider-1",
            "openai:chat",
            &provider_url,
        )],
        vec![sample_key(
            "key-1",
            "provider-1",
            "openai:chat",
            "sk-test-openai",
        )],
    ));

    let (_unused_fallback_probe_url, fallback_probe_handle) = start_server(fallback_probe).await;
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                crate::data::GatewayDataState::with_provider_transport_reader_for_tests(
                    provider_catalog_repository,
                    DEVELOPMENT_ENCRYPTION_KEY,
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/v1/test-connection?provider=provider-1&model=gpt-5&api_format=openai:chat"
        ))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["status"], "success");
    assert!(payload.get("provider").is_none());
    assert_eq!(payload["provider_id"], "provider-1");
    assert_eq!(payload["api_format"], "openai:chat");
    assert_eq!(payload["response_id"], "resp_local_test");
    assert_eq!(*provider_hits.lock().expect("mutex should lock"), 1);
    assert_eq!(*fallback_probe_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    fallback_probe_handle.abort();
    provider_handle.abort();
}

#[tokio::test]
async fn gateway_gemini_test_connection_does_not_force_low_max_output_tokens() {
    let provider_hits = Arc::new(Mutex::new(0usize));
    let provider_hits_clone = Arc::clone(&provider_hits);
    let provider = Router::new().route(
        "/{*path}",
        any(move |request: Request| {
            let provider_hits_inner = Arc::clone(&provider_hits_clone);
            async move {
                *provider_hits_inner.lock().expect("mutex should lock") += 1;
                let body = to_bytes(request.into_body(), usize::MAX)
                    .await
                    .expect("body should read");
                let body_json: serde_json::Value =
                    serde_json::from_slice(&body).expect("json body should parse");
                assert_eq!(body_json["contents"][0]["parts"][0]["text"], "Health check");
                assert!(
                    body_json
                        .get("generationConfig")
                        .and_then(|config| config.get("maxOutputTokens"))
                        .is_none(),
                    "Gemini test connection must not force a tiny maxOutputTokens value"
                );
                Json(json!({
                    "candidates": [{
                        "content": {
                            "role": "model",
                            "parts": [{"text": "ok"}]
                        },
                        "finishReason": "STOP"
                    }],
                    "responseId": "gemini_test_connection_ok"
                }))
                .into_response()
            }
        }),
    );

    let (provider_url, provider_handle) = start_server(provider).await;
    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider("provider-gemini", "google", 10)],
        vec![sample_endpoint(
            "endpoint-gemini",
            "provider-gemini",
            "gemini:generate_content",
            &provider_url,
        )],
        vec![sample_key(
            "key-gemini",
            "provider-gemini",
            "gemini:generate_content",
            "google-api-key",
        )],
    ));

    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(GatewayDataState::with_provider_transport_reader_for_tests(
                provider_catalog_repository,
                DEVELOPMENT_ENCRYPTION_KEY,
            )),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/v1/test-connection?provider=provider-gemini&model=gemini-3-flash-preview&api_format=gemini:generate_content"
        ))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["status"], "success");
    assert_eq!(payload["provider_id"], "provider-gemini");
    assert_eq!(payload["endpoint_id"], "endpoint-gemini");
    assert_eq!(payload["api_format"], "gemini:generate_content");
    assert_eq!(*provider_hits.lock().expect("mutex should lock"), 1);

    gateway_handle.abort();
    provider_handle.abort();
}

async fn assert_public_support_route_returns_local_503(
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
    expected_detail: &str,
) {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router().expect("gateway should build");
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let client = reqwest::Client::new();
    let url = format!("{gateway_url}{path}");
    let request = client.request(method, url);
    let request = if let Some(body) = body {
        request.json(&body)
    } else {
        request
    };
    let response = request.send().await.expect("request should succeed");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["detail"], expected_detail);
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

async fn assert_public_support_route_returns_local_503_with_auth(
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
    expected_detail: &str,
) {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            ("session_id".to_string(), json!("session-maintenance-1")),
        ]),
        chrono::Utc::now() + chrono::Duration::hours(1),
    );
    let session = sample_auth_session(
        "user-auth-1",
        "session-maintenance-1",
        "device-maintenance-1",
        "refresh-maintenance-1",
        now,
    );
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(user, sample_auth_wallet("user-auth-1", now), [session])
            .await;

    let client = reqwest::Client::new();
    let url = format!("{gateway_url}{path}");
    let request = client
        .request(method, url)
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-maintenance-1")
        .header("user-agent", "AetherTest/1.0");
    let request = if let Some(body) = body {
        request.json(&body)
    } else {
        request
    };
    let response = request.send().await.expect("request should succeed");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["detail"], expected_detail);
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

async fn assert_local_route_not_found_response(response: reqwest::Response) {
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["error"]["type"], "http_error");
    assert_eq!(payload["error"]["message"], "Route not found");
}

fn test_auth_secret() -> String {
    std::env::var("JWT_SECRET_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "aether-rust-dev-jwt-secret".to_string())
}

fn test_base64url_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;

    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn build_test_auth_token(
    token_type: &str,
    mut payload: serde_json::Map<String, serde_json::Value>,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> String {
    use hmac::Mac;

    let header = json!({ "alg": "HS256", "typ": "JWT" });
    payload.insert("exp".to_string(), json!(expires_at.timestamp()));
    payload.insert("type".to_string(), json!(token_type));
    let header_segment = test_base64url_encode(
        serde_json::to_vec(&header)
            .expect("jwt header should serialize")
            .as_slice(),
    );
    let payload_segment = test_base64url_encode(
        serde_json::to_vec(&payload)
            .expect("jwt payload should serialize")
            .as_slice(),
    );
    let signing_input = format!("{header_segment}.{payload_segment}");
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(test_auth_secret().as_bytes())
        .expect("jwt secret should build");
    mac.update(signing_input.as_bytes());
    let signature = mac.finalize().into_bytes();
    format!(
        "{header_segment}.{payload_segment}.{}",
        test_base64url_encode(signature.as_slice())
    )
}

struct TestEnvVarGuard {
    key: &'static str,
    previous: Option<String>,
}

impl Drop for TestEnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_deref() {
            std::env::set_var(self.key, previous);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

fn set_test_env_var(key: &'static str, value: &str) -> TestEnvVarGuard {
    let previous = std::env::var(key).ok();
    std::env::set_var(key, value);
    TestEnvVarGuard { key, previous }
}

fn canonicalize_test_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries = map.iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            let mut object = serde_json::Map::new();
            for (key, value) in entries {
                object.insert(key.clone(), canonicalize_test_json(value));
            }
            serde_json::Value::Object(object)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(canonicalize_test_json).collect())
        }
        _ => value.clone(),
    }
}

fn build_test_payment_callback_signature(payload: &serde_json::Value, secret: &str) -> String {
    use hmac::Mac;

    let canonical = serde_json::to_string(&canonicalize_test_json(payload))
        .expect("payment callback payload should serialize");
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes())
        .expect("payment callback secret should build");
    mac.update(canonical.as_bytes());
    let signature = mac.finalize().into_bytes();
    signature.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sample_auth_user(now: chrono::DateTime<chrono::Utc>) -> StoredUserAuthRecord {
    StoredUserAuthRecord::new(
        "user-auth-1".to_string(),
        Some("alice@example.com".to_string()),
        true,
        "alice".to_string(),
        Some("$2y$10$.OBQfixAECpsb8V/VS3csOMf00x2E/jD/gnud20t6RG0yiQosyOZ2".to_string()),
        "user".to_string(),
        "local".to_string(),
        Some(json!(["openai"])),
        Some(json!(["openai:chat"])),
        Some(json!(["gpt-5"])),
        true,
        false,
        Some(now),
        Some(now),
    )
    .expect("auth user should build")
}

fn sample_users_me_management_token(
    token_id: &str,
    user_id: &str,
    username: &str,
    is_active: bool,
) -> StoredManagementTokenWithUser {
    let token = StoredManagementToken::new(
        token_id.to_string(),
        user_id.to_string(),
        format!("{username}-token"),
    )
    .expect("management token should build")
    .with_display_fields(
        Some(format!("{username} token")),
        Some("ae_test".to_string()),
        Some(json!(["127.0.0.1"])),
    )
    .with_runtime_fields(
        Some(4_102_444_800),
        Some(1_711_000_000),
        Some("127.0.0.1".to_string()),
        7,
        is_active,
    )
    .with_timestamps(Some(1_710_000_000), Some(1_711_000_100));
    let user = StoredManagementTokenUserSummary::new(
        user_id.to_string(),
        Some(format!("{username}@example.com")),
        username.to_string(),
        "user".to_string(),
    )
    .expect("management token user should build");
    StoredManagementTokenWithUser::new(token, user)
}

fn sample_auth_wallet(user_id: &str, now: chrono::DateTime<chrono::Utc>) -> StoredWalletSnapshot {
    StoredWalletSnapshot::new(
        "wallet-auth-1".to_string(),
        Some(user_id.to_string()),
        None,
        12.5,
        3.0,
        "finite".to_string(),
        "USD".to_string(),
        "active".to_string(),
        20.0,
        4.5,
        0.0,
        0.0,
        now.timestamp(),
    )
    .expect("wallet should build")
}

fn sample_standalone_auth_wallet(
    api_key_id: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> StoredWalletSnapshot {
    StoredWalletSnapshot::new(
        "wallet-standalone-1".to_string(),
        None,
        Some(api_key_id.to_string()),
        2.0,
        0.5,
        "finite".to_string(),
        "USD".to_string(),
        "active".to_string(),
        5.0,
        2.5,
        0.0,
        0.0,
        now.timestamp(),
    )
    .expect("standalone wallet should build")
}

fn wallet_today_usage_test_time() -> chrono::DateTime<chrono::Utc> {
    let offset =
        chrono::FixedOffset::east_opt(8 * 3600).expect("Asia/Shanghai test offset should be valid");
    let local_today = Utc::now().with_timezone(&offset).date_naive();
    let local_noon = local_today
        .and_hms_opt(12, 0, 0)
        .expect("wallet today test noon should be valid");
    offset
        .from_local_datetime(&local_noon)
        .single()
        .expect("fixed offset local noon should be unambiguous")
        .with_timezone(&Utc)
}

fn sample_auth_session(
    user_id: &str,
    session_id: &str,
    client_device_id: &str,
    refresh_token: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> crate::data::state::StoredUserSessionRecord {
    crate::data::state::StoredUserSessionRecord::new(
        session_id.to_string(),
        user_id.to_string(),
        client_device_id.to_string(),
        None,
        crate::data::state::StoredUserSessionRecord::hash_refresh_token(refresh_token),
        None,
        None,
        Some(now),
        Some(now + chrono::Duration::days(7)),
        None,
        None,
        Some("127.0.0.1".to_string()),
        Some("AetherTest/1.0".to_string()),
        Some(now),
        Some(now),
    )
    .expect("auth session should build")
}

fn sample_usage_auth_snapshot(
    api_key_id: &str,
    user_id: &str,
    api_key_name: &str,
) -> StoredAuthApiKeySnapshot {
    StoredAuthApiKeySnapshot::new(
        user_id.to_string(),
        "alice".to_string(),
        Some("alice@example.com".to_string()),
        "user".to_string(),
        "local".to_string(),
        true,
        false,
        Some(json!(["openai"])),
        Some(json!(["openai:chat"])),
        Some(json!(["gpt-5"])),
        api_key_id.to_string(),
        Some(api_key_name.to_string()),
        true,
        false,
        false,
        Some(60),
        Some(5),
        None,
        Some(json!(["openai"])),
        Some(json!(["openai:chat"])),
        Some(json!(["gpt-5"])),
    )
    .expect("auth api key snapshot should build")
}

async fn start_auth_gateway_with_state(
    user: StoredUserAuthRecord,
    wallet: StoredWalletSnapshot,
    sessions: impl IntoIterator<Item = crate::data::state::StoredUserSessionRecord>,
) -> (
    String,
    Arc<Mutex<usize>>,
    tokio::task::JoinHandle<()>,
    tokio::task::JoinHandle<()>,
) {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));
    let wallet_repository = Arc::new(InMemoryWalletRepository::seed(vec![wallet]));
    let state = AppState::new()
        .expect("gateway should build")
        .with_data_state_for_tests(
            crate::data::GatewayDataState::with_user_and_wallet_for_tests(
                user_repository,
                wallet_repository,
            ),
        )
        .with_auth_sessions_for_tests(sessions);
    let gateway = build_router_with_state(state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;
    (gateway_url, upstream_hits, gateway_handle, upstream_handle)
}

async fn start_auth_gateway_with_usage_state<T>(
    user: StoredUserAuthRecord,
    wallet: StoredWalletSnapshot,
    sessions: impl IntoIterator<Item = crate::data::state::StoredUserSessionRecord>,
    usage_repository: Arc<T>,
) -> (
    String,
    Arc<Mutex<usize>>,
    tokio::task::JoinHandle<()>,
    tokio::task::JoinHandle<()>,
)
where
    T: UsageRepository + 'static,
{
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));
    let wallet_repository = Arc::new(InMemoryWalletRepository::seed(vec![wallet]));
    let state = AppState::new()
        .expect("gateway should build")
        .with_data_state_for_tests(
            crate::data::GatewayDataState::with_user_wallet_and_usage_for_tests(
                user_repository,
                wallet_repository,
                usage_repository,
            ),
        )
        .with_auth_sessions_for_tests(sessions);
    let gateway = build_router_with_state(state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;
    (gateway_url, upstream_hits, gateway_handle, upstream_handle)
}

async fn start_auth_dashboard_gateway_with_state<TUsage, TCatalog>(
    user: StoredUserAuthRecord,
    wallet: StoredWalletSnapshot,
    sessions: impl IntoIterator<Item = crate::data::state::StoredUserSessionRecord>,
    usage_repository: Arc<TUsage>,
    provider_catalog_repository: Arc<TCatalog>,
) -> (
    String,
    Arc<Mutex<usize>>,
    tokio::task::JoinHandle<()>,
    tokio::task::JoinHandle<()>,
)
where
    TUsage: UsageRepository + 'static,
    TCatalog: ProviderCatalogReadRepository + 'static,
{
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));
    let wallet_repository = Arc::new(InMemoryWalletRepository::seed(vec![wallet]));
    let data_state = crate::data::GatewayDataState::with_user_wallet_and_usage_for_tests(
        user_repository,
        wallet_repository,
        usage_repository,
    )
    .with_provider_catalog_reader(provider_catalog_repository);
    let state = AppState::new()
        .expect("gateway should build")
        .with_data_state_for_tests(data_state)
        .with_auth_sessions_for_tests(sessions);
    let gateway = build_router_with_state(state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;
    (gateway_url, upstream_hits, gateway_handle, upstream_handle)
}

async fn start_auth_gateway_with_preferences_state(
    user: StoredUserAuthRecord,
    wallet: StoredWalletSnapshot,
    sessions: impl IntoIterator<Item = crate::data::state::StoredUserSessionRecord>,
    preferences: impl IntoIterator<Item = crate::data::state::StoredUserPreferenceRecord>,
) -> (
    String,
    Arc<Mutex<usize>>,
    tokio::task::JoinHandle<()>,
    tokio::task::JoinHandle<()>,
) {
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));
    let wallet_repository = Arc::new(InMemoryWalletRepository::seed(vec![wallet]));
    let state = AppState::new()
        .expect("gateway should build")
        .with_data_state_for_tests(
            crate::data::GatewayDataState::with_user_and_wallet_for_tests(
                user_repository,
                wallet_repository,
            )
            .with_user_preferences_for_tests(preferences),
        )
        .with_auth_sessions_for_tests(sessions);
    let gateway = build_router_with_state(state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;
    (gateway_url, upstream_hits, gateway_handle, upstream_handle)
}

fn sample_user_usage_audit(
    id: &str,
    request_id: &str,
    user_id: &str,
    model: &str,
    provider_name: &str,
    status: &str,
    created_at: chrono::DateTime<chrono::Utc>,
) -> StoredRequestUsageAudit {
    let mut usage = StoredRequestUsageAudit::new(
        id.to_string(),
        request_id.to_string(),
        Some(user_id.to_string()),
        Some("api-key-user-1".to_string()),
        Some("alice".to_string()),
        Some("default".to_string()),
        provider_name.to_string(),
        model.to_string(),
        Some(format!("{model}-target")),
        Some("provider-1".to_string()),
        Some("endpoint-1".to_string()),
        Some("provider-key-1".to_string()),
        Some("chat".to_string()),
        Some("openai:chat".to_string()),
        Some("openai".to_string()),
        Some("chat".to_string()),
        Some("openai:chat".to_string()),
        Some("openai".to_string()),
        Some("chat".to_string()),
        false,
        status == "streaming",
        120,
        30,
        150,
        1.25,
        1.25,
        Some(if status == "failed" { 500 } else { 200 }),
        if status == "failed" {
            Some("upstream failed".to_string())
        } else {
            None
        },
        None,
        Some(420),
        Some(120),
        status.to_string(),
        if matches!(status, "completed" | "failed" | "cancelled") {
            "settled".to_string()
        } else {
            "pending".to_string()
        },
        created_at.timestamp(),
        created_at.timestamp() + 1,
        Some(created_at.timestamp() + 2),
    )
    .expect("usage should build");
    usage.cache_creation_input_tokens = 10;
    usage.cache_creation_ephemeral_5m_input_tokens = 4;
    usage.cache_creation_ephemeral_1h_input_tokens = 6;
    usage.cache_read_input_tokens = 15;
    usage.cache_creation_cost_usd = 0.05;
    usage.cache_read_cost_usd = 0.02;
    usage
}

async fn start_auth_announcement_gateway_with_state<T>(
    user: StoredUserAuthRecord,
    wallet: StoredWalletSnapshot,
    sessions: impl IntoIterator<Item = crate::data::state::StoredUserSessionRecord>,
    announcement_repository: Arc<T>,
) -> (
    String,
    Arc<Mutex<usize>>,
    tokio::task::JoinHandle<()>,
    tokio::task::JoinHandle<()>,
)
where
    T: AnnouncementReadRepository
        + aether_data::repository::announcements::AnnouncementWriteRepository
        + 'static,
{
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));
    let wallet_repository = Arc::new(InMemoryWalletRepository::seed(vec![wallet]));
    let state = AppState::new()
        .expect("gateway should build")
        .with_data_state_for_tests(
            crate::data::GatewayDataState::with_announcement_user_and_wallet_for_tests(
                announcement_repository,
                user_repository,
                wallet_repository,
            ),
        )
        .with_auth_sessions_for_tests(sessions);
    let gateway = build_router_with_state(state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;
    (gateway_url, upstream_hits, gateway_handle, upstream_handle)
}

async fn start_auth_gateway_with_builder<F>(
    build_state: F,
) -> (
    String,
    Arc<Mutex<usize>>,
    tokio::task::JoinHandle<()>,
    tokio::task::JoinHandle<()>,
)
where
    F: FnOnce() -> AppState,
{
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );
    let (_unused_fallback_probe_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(build_state());
    let (gateway_url, gateway_handle) = start_server(gateway).await;
    (gateway_url, upstream_hits, gateway_handle, upstream_handle)
}

#[tokio::test]
async fn gateway_handles_user_monitoring_rate_limit_status_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-monitoring-rate-limit-status"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );

    let auth_repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some("hash-monitoring-key-1".to_string()),
        StoredAuthApiKeySnapshot::new(
            "user-auth-1".to_string(),
            "alice".to_string(),
            Some("alice@example.com".to_string()),
            "user".to_string(),
            "local".to_string(),
            true,
            false,
            Some(json!(["openai"])),
            Some(json!(["openai:chat"])),
            Some(json!(["gpt-5"])),
            "api-key-monitoring-1".to_string(),
            Some("monitoring-key".to_string()),
            true,
            false,
            false,
            Some(30),
            Some(5),
            None,
            None,
            None,
            None,
        )
        .expect("auth api key snapshot should build")
        .with_user_rate_limit(Some(80)),
    )]));
    let user_repository: Arc<dyn UserReadRepository> =
        Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![
            sample_auth_user(now),
        ]));
    let group = user_repository
        .create_user_group(UpsertUserGroupRecord {
            name: "Monitoring Limits".to_string(),
            description: None,
            priority: 0,
            allowed_providers: None,
            allowed_providers_mode: "unrestricted".to_string(),
            allowed_api_formats: None,
            allowed_api_formats_mode: "unrestricted".to_string(),
            allowed_models: None,
            allowed_models_mode: "unrestricted".to_string(),
            rate_limit: Some(80),
            rate_limit_mode: "custom".to_string(),
        })
        .await
        .expect("group should create")
        .expect("group should exist");
    user_repository
        .add_user_to_group(&group.id, "user-auth-1")
        .await
        .expect("group membership should create");

    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state = crate::data::GatewayDataState::with_auth_api_key_repository_for_tests(
                auth_repository,
            )
            .with_user_reader(Arc::clone(&user_repository));
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-monitoring-rate-limit-status",
                    "device-monitoring-rate-limit-status",
                    "refresh-monitoring-rate-limit-status",
                    now,
                )])
        })
        .await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/monitoring/rate-limit-status"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-monitoring-rate-limit-status")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["user_id"], "user-auth-1");
    let api_keys = payload["api_keys"]
        .as_array()
        .expect("api_keys should be array");
    assert_eq!(api_keys.len(), 1);
    assert_eq!(api_keys[0]["api_key_name"], "monitoring-key");
    assert_eq!(api_keys[0]["limit"], 30);
    assert_eq!(api_keys[0]["remaining"], 30);
    assert_eq!(api_keys[0]["scope"], "key");
    assert_eq!(api_keys[0]["user_limit"], 80);
    assert_eq!(api_keys[0]["user_remaining"], 80);
    assert_eq!(api_keys[0]["key_limit"], 30);
    assert_eq!(api_keys[0]["key_remaining"], 30);
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_rejects_invalid_nested_announcement_paths_as_local_not_found_without_hitting_upstream(
) {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-announcement-user-invalid-nested"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let announcement_repository = Arc::new(InMemoryAnnouncementReadRepository::seed(vec![
        StoredAnnouncement::new(
            "announcement-nested".to_string(),
            "嵌套路由公告".to_string(),
            "用于校验无效 announcement 子路径".to_string(),
            "info".to_string(),
            10,
            true,
            false,
            false,
            Some("admin-1".to_string()),
            Some("admin".to_string()),
            None,
            None,
            now.timestamp(),
            now.timestamp(),
        )
        .expect("announcement should build"),
    ]));
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_announcement_gateway_with_state(
            user,
            sample_auth_wallet("user-auth-1", now),
            [sample_auth_session(
                "user-auth-1",
                "session-announcement-user-invalid-nested",
                "device-announcement-user-invalid-nested",
                "refresh-token-placeholder",
                now,
            )],
            announcement_repository,
        )
        .await;
    let client = reqwest::Client::new();

    let detail_response = client
        .get(format!(
            "{gateway_url}/api/announcements/announcement-nested/history"
        ))
        .send()
        .await
        .expect("request should succeed");

    assert_local_route_not_found_response(detail_response).await;

    let read_status_response = client
        .patch(format!(
            "{gateway_url}/api/announcements/announcement-nested/history/read-status"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header(
            "x-client-device-id",
            "device-announcement-user-invalid-nested",
        )
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({ "is_read": true }))
        .send()
        .await
        .expect("request should succeed");

    assert_local_route_not_found_response(read_status_response).await;

    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);
    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_rejects_invalid_wallet_nested_detail_paths_as_local_not_found_without_hitting_upstream(
) {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-wallet-invalid-detail-1"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(
            user,
            sample_auth_wallet("user-auth-1", now),
            [sample_auth_session(
                "user-auth-1",
                "session-wallet-invalid-detail-1",
                "device-wallet-invalid-detail-1",
                "refresh-token-placeholder",
                now,
            )],
        )
        .await;

    let client = reqwest::Client::new();

    let recharge_response = client
        .get(format!("{gateway_url}/api/wallet/recharge/order-1/confirm"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-wallet-invalid-detail-1")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");
    assert_local_route_not_found_response(recharge_response).await;

    let refund_response = client
        .get(format!("{gateway_url}/api/wallet/refunds/refund-1/status"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-wallet-invalid-detail-1")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");
    assert_local_route_not_found_response(refund_response).await;

    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);
    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_updates_users_me_detail_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-update-detail"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(
            user,
            sample_auth_wallet("user-auth-1", now),
            [sample_auth_session(
                "user-auth-1",
                "session-users-me-update-detail",
                "device-users-me-update-detail",
                "refresh-token-placeholder",
                now,
            )],
        )
        .await;
    let client = reqwest::Client::new();

    let update_response = client
        .put(format!("{gateway_url}/api/users/me"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-update-detail")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({
            "email": "alice+updated@example.com",
            "username": "alice-updated",
            "feature_settings": {
                "chat_pii_redaction": {
                    "enabled": true
                }
            }
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(update_response.status(), StatusCode::OK);
    let update_payload: serde_json::Value = update_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(update_payload["message"], "个人信息更新成功");

    let get_response = client
        .get(format!("{gateway_url}/api/users/me"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-update-detail")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(get_response.status(), StatusCode::OK);
    let get_payload: serde_json::Value = get_response.json().await.expect("json body should parse");
    assert_eq!(get_payload["email"], "alice+updated@example.com");
    assert_eq!(get_payload["username"], "alice-updated");
    assert_eq!(get_payload["auth_source"], "local");
    assert_eq!(get_payload["has_password"], true);
    assert_eq!(
        get_payload["feature_settings"]["chat_pii_redaction"]["enabled"],
        true
    );
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_returns_service_unavailable_for_users_me_detail_update_without_profile_storage() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-update-unavailable"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let user_repository =
        Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]).read_only());
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state =
                crate::data::GatewayDataState::with_user_reader_for_tests(user_repository);
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .without_auth_user_store_for_tests()
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-update-unavailable",
                    "device-users-me-update-unavailable",
                    "refresh-token-placeholder",
                    now,
                )])
        })
        .await;

    let response = reqwest::Client::new()
        .put(format!("{gateway_url}/api/users/me"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-update-unavailable")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({
            "email": "alice+updated@example.com",
            "username": "alice-updated",
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["detail"], "用户资料存储暂不可用");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_changes_users_me_password_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-password-current"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let current = sample_auth_session(
        "user-auth-1",
        "session-users-me-password-current",
        "device-users-me-password-current",
        "refresh-token-current",
        now,
    );
    let other = sample_auth_session(
        "user-auth-1",
        "session-users-me-password-other",
        "device-users-me-password-other",
        "refresh-token-other",
        now - chrono::Duration::hours(6),
    );
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(
            user,
            sample_auth_wallet("user-auth-1", now),
            [current, other],
        )
        .await;
    let client = reqwest::Client::new();

    let update_response = client
        .patch(format!("{gateway_url}/api/users/me/password"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-password-current")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({
            "current_password": "secret123",
            "new_password": "Secret456!",
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(update_response.status(), StatusCode::OK);
    let update_payload: serde_json::Value = update_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(update_payload["message"], "密码修改成功");

    let sessions_response = client
        .get(format!("{gateway_url}/api/users/me/sessions"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-password-current")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(sessions_response.status(), StatusCode::OK);
    let sessions_payload: serde_json::Value = sessions_response
        .json()
        .await
        .expect("json body should parse");
    let sessions = sessions_payload
        .as_array()
        .expect("sessions should be array");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], "session-users-me-password-current");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_returns_service_unavailable_for_users_me_password_change_without_credential_storage(
) {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-password-unavailable"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let user_repository =
        Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]).read_only());
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state =
                crate::data::GatewayDataState::with_user_reader_for_tests(user_repository);
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .without_auth_user_store_for_tests()
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-password-unavailable",
                    "device-users-me-password-unavailable",
                    "refresh-token-placeholder",
                    now,
                )])
        })
        .await;

    let response = reqwest::Client::new()
        .patch(format!("{gateway_url}/api/users/me/password"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-password-unavailable")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({
            "current_password": "secret123",
            "new_password": "Secret456!",
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["detail"], "用户凭证存储暂不可用");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_users_me_endpoint_status_locally_without_proxying_upstream() {
    let now = Utc::now();
    let now_unix_secs = now.timestamp();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-user-endpoint-status-1"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-openai", "openai", 10),
            sample_provider("provider-claude", "claude", 20),
        ],
        vec![
            sample_endpoint(
                "endpoint-openai",
                "provider-openai",
                "openai:chat",
                "https://api.openai.example",
            ),
            sample_endpoint(
                "endpoint-claude",
                "provider-claude",
                "claude:messages",
                "https://api.anthropic.example",
            ),
        ],
        vec![
            sample_key(
                "key-openai",
                "provider-openai",
                "openai:chat",
                "sk-openai-endpoint-status",
            ),
            sample_key(
                "key-claude",
                "provider-claude",
                "claude:messages",
                "sk-claude-endpoint-status",
            ),
        ],
    ));
    let request_candidate_repository = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        sample_request_candidate(
            "cand-openai-success",
            "req-openai-success",
            "endpoint-openai",
            RequestCandidateStatus::Success,
            now_unix_secs - 3_000,
            Some(now_unix_secs - 2_980),
        ),
        sample_request_candidate(
            "cand-openai-failed",
            "req-openai-failed",
            "endpoint-openai",
            RequestCandidateStatus::Failed,
            now_unix_secs - 2_000,
            Some(now_unix_secs - 1_980),
        ),
        sample_request_candidate(
            "cand-claude-skipped",
            "req-claude-skipped",
            "endpoint-claude",
            RequestCandidateStatus::Skipped,
            now_unix_secs - 1_500,
            Some(now_unix_secs - 1_480),
        ),
    ]));
    let upstream_hits = Arc::new(Mutex::new(0usize));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits_inner = Arc::clone(&upstream_hits_clone);
            async move {
                *upstream_hits_inner.lock().expect("mutex should lock") += 1;
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));
    let wallet_repository = Arc::new(InMemoryWalletRepository::seed(vec![sample_auth_wallet(
        "user-auth-1",
        now,
    )]));
    let gateway = build_router_with_state(
        AppState::new()
            .expect("gateway should build")
            .with_data_state_for_tests(
                crate::data::GatewayDataState::with_user_and_wallet_for_tests(
                    user_repository,
                    wallet_repository,
                )
                .with_provider_catalog_reader(provider_catalog_repository)
                .with_request_candidate_reader(request_candidate_repository),
            )
            .with_auth_sessions_for_tests([sample_auth_session(
                "user-auth-1",
                "session-user-endpoint-status-1",
                "device-user-endpoint-status-1",
                "refresh-token-placeholder",
                now,
            )]),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/users/me/endpoint-status"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-user-endpoint-status-1")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let items = payload.as_array().expect("payload should be an array");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["api_format"], "claude:messages");
    assert_eq!(items[0]["display_name"], "Claude Messages");
    assert_eq!(items[0]["health_score"], 1.0);
    assert_eq!(items[0]["timeline"].as_array().map(Vec::len), Some(60));
    assert!(items[0].get("total_endpoints").is_none());
    assert_eq!(items[1]["api_format"], "openai:chat");
    assert_eq!(items[1]["display_name"], "OpenAI Chat");
    assert_eq!(items[1]["health_score"], 0.5);
    assert_eq!(items[1]["timeline"].as_array().map(Vec::len), Some(60));
    assert!(items[1].get("total_keys").is_none());
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_returns_service_unavailable_for_users_me_endpoint_status_without_health_data() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-user-endpoint-status-unavailable"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state =
                crate::data::GatewayDataState::with_user_reader_for_tests(user_repository);
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-user-endpoint-status-unavailable",
                    "device-user-endpoint-status-unavailable",
                    "refresh-token-placeholder",
                    now,
                )])
        })
        .await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/users/me/endpoint-status"))
        .header("authorization", format!("Bearer {access_token}"))
        .header(
            "x-client-device-id",
            "device-user-endpoint-status-unavailable",
        )
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["detail"], "用户端点健康数据暂不可用");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_users_me_preferences_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            ("session_id".to_string(), json!("session-user-pref-1")),
        ]),
        now + chrono::Duration::hours(1),
    );
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_preferences_state(
            user,
            sample_auth_wallet("user-auth-1", now),
            [sample_auth_session(
                "user-auth-1",
                "session-user-pref-1",
                "device-user-pref-1",
                "refresh-token-placeholder",
                now,
            )],
            {
                let mut preference =
                    crate::data::state::StoredUserPreferenceRecord::default_for_user("user-auth-1");
                preference.default_provider_id = Some("provider-openai".to_string());
                preference.default_provider_name = Some("openai".to_string());
                vec![preference]
            },
        )
        .await;

    let client = reqwest::Client::new();
    let get_response = client
        .get(format!("{gateway_url}/api/users/me/preferences"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-user-pref-1")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(get_response.status(), StatusCode::OK);
    let get_payload: serde_json::Value = get_response.json().await.expect("json should parse");
    assert_eq!(get_payload["theme"], "light");
    assert_eq!(get_payload["language"], "zh-CN");
    assert_eq!(get_payload["timezone"], "Asia/Shanghai");
    assert_eq!(get_payload["notifications"]["email"], true);
    assert_eq!(get_payload["default_provider_id"], "provider-openai");
    assert!(get_payload.get("default_provider").is_none());

    let put_response = client
        .put(format!("{gateway_url}/api/users/me/preferences"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-user-pref-1")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({
            "theme": "dark",
            "language": "en-US",
            "timezone": "UTC",
            "bio": "hello",
            "email_notifications": false,
            "usage_alerts": false,
            "announcement_notifications": true,
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(put_response.status(), StatusCode::OK);
    let put_payload: serde_json::Value = put_response.json().await.expect("json should parse");
    assert_eq!(put_payload["message"], "偏好设置更新成功");

    let verify_response = client
        .get(format!("{gateway_url}/api/users/me/preferences"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-user-pref-1")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(verify_response.status(), StatusCode::OK);
    let verify_payload: serde_json::Value =
        verify_response.json().await.expect("json should parse");
    assert_eq!(verify_payload["theme"], "dark");
    assert_eq!(verify_payload["language"], "en-US");
    assert_eq!(verify_payload["timezone"], "UTC");
    assert_eq!(verify_payload["bio"], "hello");
    assert_eq!(verify_payload["notifications"]["email"], false);
    assert_eq!(verify_payload["notifications"]["usage_alerts"], false);
    assert_eq!(verify_payload["notifications"]["announcements"], true);
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_returns_service_unavailable_for_users_me_preferences_update_without_storage() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-user-pref-unavailable"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let user_repository =
        Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]).read_only());
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state =
                crate::data::GatewayDataState::with_user_reader_for_tests(user_repository);
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-user-pref-unavailable",
                    "device-user-pref-unavailable",
                    "refresh-token-placeholder",
                    now,
                )])
        })
        .await;

    let response = reqwest::Client::new()
        .put(format!("{gateway_url}/api/users/me/preferences"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-user-pref-unavailable")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({
            "theme": "dark",
            "language": "en-US",
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let payload: serde_json::Value = response.json().await.expect("json should parse");
    assert_eq!(payload["detail"], "用户偏好设置存储暂不可用");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_users_me_usage_without_legacy_api_key_name_fallback_when_auth_reader_exists(
) {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-usage-no-legacy-fallback"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let usage_repository = Arc::new(InMemoryUsageReadRepository::seed(vec![
        sample_user_usage_audit(
            "usage-users-me-completed-no-legacy-fallback",
            "req-users-me-completed-no-legacy-fallback",
            "user-auth-1",
            "gpt-4.1",
            "OpenAI",
            "completed",
            now - chrono::Duration::minutes(20),
        ),
    ]));
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state = crate::data::GatewayDataState::with_user_wallet_and_usage_for_tests(
                Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![
                    user.clone()
                ])),
                Arc::new(InMemoryWalletRepository::seed(vec![sample_auth_wallet(
                    "user-auth-1",
                    now,
                )])),
                Arc::clone(&usage_repository),
            )
            .with_auth_api_key_reader(Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![])));
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-usage-no-legacy-fallback",
                    "device-users-me-usage-no-legacy-fallback",
                    "refresh-token-users-me-usage-no-legacy-fallback",
                    now,
                )])
        })
        .await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/api/users/me/usage?limit=10&offset=0&search=default"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header(
            "x-client-device-id",
            "device-users-me-usage-no-legacy-fallback",
        )
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["pagination"]["total"], 0);
    assert_eq!(
        payload["records"].as_array().expect("records array").len(),
        0
    );
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_users_me_usage_search_with_model_and_api_key_keywords() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-usage-multi-keyword-search"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let usage_repository = Arc::new(InMemoryUsageReadRepository::seed(vec![
        sample_user_usage_audit(
            "usage-users-me-completed-multi-keyword-search",
            "req-users-me-completed-multi-keyword-search",
            "user-auth-1",
            "gpt-4.1",
            "OpenAI",
            "completed",
            now - chrono::Duration::minutes(20),
        ),
        sample_user_usage_audit(
            "usage-users-me-failed-multi-keyword-search",
            "req-users-me-failed-multi-keyword-search",
            "user-auth-1",
            "claude-3.5-sonnet",
            "Anthropic",
            "failed",
            now - chrono::Duration::minutes(10),
        ),
        sample_user_usage_audit(
            "usage-users-me-streaming-multi-keyword-search",
            "req-users-me-streaming-multi-keyword-search",
            "user-auth-1",
            "gpt-4.1-mini",
            "OpenAI",
            "streaming",
            now - chrono::Duration::minutes(5),
        ),
    ]));
    let auth_repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some("hash-api-key-user-1".to_string()),
        sample_usage_auth_snapshot("api-key-user-1", "user-auth-1", "renamed-key"),
    )]));
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state = crate::data::GatewayDataState::with_user_wallet_and_usage_for_tests(
                Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![
                    user.clone()
                ])),
                Arc::new(InMemoryWalletRepository::seed(vec![sample_auth_wallet(
                    "user-auth-1",
                    now,
                )])),
                Arc::clone(&usage_repository),
            )
            .with_auth_api_key_reader(auth_repository);
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-usage-multi-keyword-search",
                    "device-users-me-usage-multi-keyword-search",
                    "refresh-token-users-me-usage-multi-keyword-search",
                    now,
                )])
        })
        .await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/api/users/me/usage?limit=10&offset=0&search=gpt%20renamed-key"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header(
            "x-client-device-id",
            "device-users-me-usage-multi-keyword-search",
        )
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["pagination"]["total"], 2);
    assert_eq!(
        payload["records"].as_array().expect("records array").len(),
        2
    );
    assert_eq!(payload["records"][0]["model"], "gpt-4.1-mini");
    assert_eq!(payload["records"][1]["model"], "gpt-4.1");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_users_me_usage_active_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            ("session_id".to_string(), json!("session-users-me-active-1")),
        ]),
        now + chrono::Duration::hours(1),
    );
    let mut streaming_usage = sample_user_usage_audit(
        "usage-users-me-streaming-1",
        "req-users-me-streaming-1",
        "user-auth-1",
        "gpt-4.1-mini",
        "OpenAI",
        "streaming",
        now - chrono::Duration::minutes(2),
    );
    streaming_usage.request_metadata = Some(json!({
        "rate_multiplier": 0.5,
    }));
    let usage_repository = Arc::new(InMemoryUsageReadRepository::seed(vec![
        sample_user_usage_audit(
            "usage-users-me-pending-1",
            "req-users-me-pending-1",
            "user-auth-1",
            "gpt-4.1",
            "OpenAI",
            "pending",
            now - chrono::Duration::minutes(4),
        ),
        streaming_usage,
        sample_user_usage_audit(
            "usage-users-me-completed-1",
            "req-users-me-completed-1",
            "user-auth-1",
            "gpt-4.1",
            "OpenAI",
            "completed",
            now - chrono::Duration::minutes(30),
        ),
    ]));
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_usage_state(
            user,
            sample_auth_wallet("user-auth-1", now),
            [sample_auth_session(
                "user-auth-1",
                "session-users-me-active-1",
                "device-users-me-active-1",
                "refresh-token-placeholder",
                now,
            )],
            usage_repository,
        )
        .await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/users/me/usage/active"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-active-1")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let requests = payload["requests"].as_array().expect("requests array");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["status"], "streaming");
    assert_eq!(requests[0]["rate_multiplier"], 0.5);
    assert_eq!(requests[0]["cache_creation_ephemeral_5m_input_tokens"], 4);
    assert_eq!(requests[0]["cache_creation_ephemeral_1h_input_tokens"], 6);
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_users_me_usage_interval_timeline_and_heatmap_locally_without_proxying_upstream(
) {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-usage-heatmap-1"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let usage_repository = Arc::new(InMemoryUsageReadRepository::seed(vec![
        sample_user_usage_audit(
            "usage-users-me-heatmap-1",
            "req-users-me-heatmap-1",
            "user-auth-1",
            "gpt-4.1",
            "OpenAI",
            "completed",
            now - chrono::Duration::minutes(25),
        ),
        sample_user_usage_audit(
            "usage-users-me-heatmap-2",
            "req-users-me-heatmap-2",
            "user-auth-1",
            "gpt-4.1",
            "OpenAI",
            "completed",
            now - chrono::Duration::minutes(15),
        ),
        sample_user_usage_audit(
            "usage-users-me-heatmap-3",
            "req-users-me-heatmap-3",
            "user-auth-1",
            "gpt-4.1",
            "OpenAI",
            "completed",
            now - chrono::Duration::days(1) - chrono::Duration::minutes(1),
        ),
    ]));
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_usage_state(
            user,
            sample_auth_wallet("user-auth-1", now),
            [sample_auth_session(
                "user-auth-1",
                "session-users-me-usage-heatmap-1",
                "device-users-me-usage-heatmap-1",
                "refresh-token-placeholder",
                now,
            )],
            usage_repository,
        )
        .await;

    let client = reqwest::Client::new();
    let timeline_response = client
        .get(format!(
            "{gateway_url}/api/users/me/usage/interval-timeline?hours=24&limit=100"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-usage-heatmap-1")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("timeline request should succeed");
    assert_eq!(timeline_response.status(), StatusCode::OK);
    let timeline_payload: serde_json::Value = timeline_response
        .json()
        .await
        .expect("timeline json should parse");
    assert_eq!(timeline_payload["analysis_period_hours"], 24);
    assert_eq!(timeline_payload["total_points"], 1);

    let heatmap_response = client
        .get(format!("{gateway_url}/api/users/me/usage/heatmap"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-usage-heatmap-1")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("heatmap request should succeed");
    assert_eq!(heatmap_response.status(), StatusCode::OK);
    let heatmap_payload: serde_json::Value = heatmap_response
        .json()
        .await
        .expect("heatmap json should parse");
    assert!(
        heatmap_payload["days"]
            .as_array()
            .expect("days array")
            .len()
            >= 2
    );
    assert!(heatmap_payload["max_requests"].as_u64().unwrap_or(0) >= 1);
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_returns_service_unavailable_for_users_me_usage_routes_without_reader() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-usage-unavailable"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(
            user,
            sample_auth_wallet("user-auth-1", now),
            [sample_auth_session(
                "user-auth-1",
                "session-users-me-usage-unavailable",
                "device-users-me-usage-unavailable",
                "refresh-token-placeholder",
                now,
            )],
        )
        .await;

    let client = reqwest::Client::new();
    for path in [
        "/api/users/me/usage",
        "/api/users/me/usage/active",
        "/api/users/me/usage/interval-timeline?hours=24&limit=100",
        "/api/users/me/usage/heatmap",
    ] {
        let response = client
            .get(format!("{gateway_url}{path}"))
            .header("authorization", format!("Bearer {access_token}"))
            .header("x-client-device-id", "device-users-me-usage-unavailable")
            .header("user-agent", "AetherTest/1.0")
            .send()
            .await
            .expect("request should succeed");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{path}");
        let payload: serde_json::Value = response.json().await.expect("json body should parse");
        assert_eq!(payload["detail"], "用户用量数据暂不可用", "{path}");
    }
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_users_me_sessions_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            ("session_id".to_string(), json!("session-users-me-current")),
        ]),
        now + chrono::Duration::hours(1),
    );
    let current = sample_auth_session(
        "user-auth-1",
        "session-users-me-current",
        "device-users-me-current",
        "refresh-token-current",
        now,
    );
    let mut other = sample_auth_session(
        "user-auth-1",
        "session-users-me-other",
        "device-users-me-other",
        "refresh-token-other",
        now - chrono::Duration::hours(6),
    );
    other.ip_address = Some("10.0.0.2".to_string());
    let mut revoked = sample_auth_session(
        "user-auth-1",
        "session-users-me-revoked",
        "device-users-me-revoked",
        "refresh-token-revoked",
        now - chrono::Duration::hours(12),
    );
    revoked.revoked_at = Some(now);
    let mut expired = sample_auth_session(
        "user-auth-1",
        "session-users-me-expired",
        "device-users-me-expired",
        "refresh-token-expired",
        now - chrono::Duration::hours(24),
    );
    expired.expires_at = Some(now - chrono::Duration::minutes(1));

    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(
            user,
            sample_auth_wallet("user-auth-1", now),
            [current, other, revoked, expired],
        )
        .await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/users/me/sessions"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-current")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let sessions = payload.as_array().expect("sessions should be array");
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0]["id"], "session-users-me-current");
    assert_eq!(sessions[0]["is_current"], true);
    assert_eq!(sessions[0]["device_label"], "未知设备");
    assert_eq!(sessions[1]["id"], "session-users-me-other");
    assert_eq!(sessions[1]["is_current"], false);
    assert_eq!(sessions[1]["ip_address"], "10.0.0.2");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_revokes_other_users_me_sessions_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            ("session_id".to_string(), json!("session-users-me-current")),
        ]),
        now + chrono::Duration::hours(1),
    );
    let current = sample_auth_session(
        "user-auth-1",
        "session-users-me-current",
        "device-users-me-current",
        "refresh-token-current",
        now,
    );
    let other = sample_auth_session(
        "user-auth-1",
        "session-users-me-other",
        "device-users-me-other",
        "refresh-token-other",
        now - chrono::Duration::hours(6),
    );

    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(
            user,
            sample_auth_wallet("user-auth-1", now),
            [current, other],
        )
        .await;

    let client = reqwest::Client::new();
    let delete_response = client
        .delete(format!("{gateway_url}/api/users/me/sessions/others"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-current")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(delete_response.status(), StatusCode::OK);
    let delete_payload: serde_json::Value = delete_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(delete_payload["message"], "其他设备已退出登录");
    assert_eq!(delete_payload["revoked_count"], 1);

    let get_response = client
        .get(format!("{gateway_url}/api/users/me/sessions"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-current")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(get_response.status(), StatusCode::OK);
    let get_payload: serde_json::Value = get_response.json().await.expect("json body should parse");
    let sessions = get_payload.as_array().expect("sessions should be array");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], "session-users-me-current");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_revokes_users_me_session_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            ("session_id".to_string(), json!("session-users-me-current")),
        ]),
        now + chrono::Duration::hours(1),
    );
    let current = sample_auth_session(
        "user-auth-1",
        "session-users-me-current",
        "device-users-me-current",
        "refresh-token-current",
        now,
    );
    let other = sample_auth_session(
        "user-auth-1",
        "session-users-me-other",
        "device-users-me-other",
        "refresh-token-other",
        now - chrono::Duration::hours(6),
    );

    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(
            user,
            sample_auth_wallet("user-auth-1", now),
            [current, other],
        )
        .await;

    let client = reqwest::Client::new();
    let delete_response = client
        .delete(format!(
            "{gateway_url}/api/users/me/sessions/session-users-me-other"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-current")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(delete_response.status(), StatusCode::OK);
    let delete_payload: serde_json::Value = delete_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(delete_payload["message"], "设备已退出登录");

    let get_response = client
        .get(format!("{gateway_url}/api/users/me/sessions"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-current")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(get_response.status(), StatusCode::OK);
    let get_payload: serde_json::Value = get_response.json().await.expect("json body should parse");
    let sessions = get_payload.as_array().expect("sessions should be array");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], "session-users-me-current");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_updates_users_me_session_label_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            ("session_id".to_string(), json!("session-users-me-current")),
        ]),
        now + chrono::Duration::hours(1),
    );
    let current = sample_auth_session(
        "user-auth-1",
        "session-users-me-current",
        "device-users-me-current",
        "refresh-token-current",
        now,
    );
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(user, sample_auth_wallet("user-auth-1", now), [current])
            .await;

    let response = reqwest::Client::new()
        .patch(format!(
            "{gateway_url}/api/users/me/sessions/session-users-me-current"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-current")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({ "device_label": "我的 MacBook" }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["id"], "session-users-me-current");
    assert_eq!(payload["device_label"], "我的 MacBook");
    assert_eq!(payload["is_current"], true);
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    let sessions_response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/users/me/sessions"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-current")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(sessions_response.status(), StatusCode::OK);
    let sessions_payload: serde_json::Value = sessions_response
        .json()
        .await
        .expect("json body should parse");
    let sessions = sessions_payload
        .as_array()
        .expect("sessions should be array");
    assert_eq!(sessions[0]["device_label"], "我的 MacBook");

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_rejects_invalid_users_me_session_delete_path_as_local_not_found_without_hitting_upstream(
) {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            ("session_id".to_string(), json!("session-users-me-current")),
        ]),
        now + chrono::Duration::hours(1),
    );
    let current = sample_auth_session(
        "user-auth-1",
        "session-users-me-current",
        "device-users-me-current",
        "refresh-token-current",
        now,
    );

    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(user, sample_auth_wallet("user-auth-1", now), [current])
            .await;

    let response = reqwest::Client::new()
        .delete(format!("{gateway_url}/api/users/me/sessions/"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-current")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_local_route_not_found_response(response).await;
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_rejects_invalid_users_me_session_update_path_as_local_not_found_without_hitting_upstream(
) {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            ("session_id".to_string(), json!("session-users-me-current")),
        ]),
        now + chrono::Duration::hours(1),
    );
    let current = sample_auth_session(
        "user-auth-1",
        "session-users-me-current",
        "device-users-me-current",
        "refresh-token-current",
        now,
    );

    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(user, sample_auth_wallet("user-auth-1", now), [current])
            .await;

    let response = reqwest::Client::new()
        .patch(format!("{gateway_url}/api/users/me/sessions/"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-current")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({ "device_label": "我的 MacBook" }))
        .send()
        .await
        .expect("request should succeed");

    assert_local_route_not_found_response(response).await;
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_users_me_api_keys_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            ("session_id".to_string(), json!("session-users-me-api-keys")),
        ]),
        now + chrono::Duration::hours(1),
    );
    let encrypted = encrypt_python_fernet_plaintext(DEVELOPMENT_ENCRYPTION_KEY, "sk-user-live-1")
        .expect("ciphertext should build");
    let auth_repository = Arc::new(
        InMemoryAuthApiKeySnapshotRepository::seed(vec![(
            Some("hash-user-key-1".to_string()),
            StoredAuthApiKeySnapshot::new(
                "user-auth-1".to_string(),
                "alice".to_string(),
                Some("alice@example.com".to_string()),
                "user".to_string(),
                "local".to_string(),
                true,
                false,
                Some(json!(["openai"])),
                Some(json!(["openai:chat"])),
                Some(json!(["gpt-5"])),
                "user-key-1".to_string(),
                Some("primary".to_string()),
                true,
                false,
                false,
                Some(60),
                Some(5),
                Some(300),
                Some(json!(["openai"])),
                Some(json!(["openai:chat"])),
                Some(json!(["gpt-5"])),
            )
            .expect("auth api key snapshot should build"),
        )])
        .with_export_records(vec![StoredAuthApiKeyExportRecord::new(
            "user-auth-1".to_string(),
            "user-key-1".to_string(),
            "hash-user-key-1".to_string(),
            Some(encrypted),
            Some("primary".to_string()),
            Some(json!(["openai"])),
            Some(json!(["openai:chat"])),
            Some(json!(["gpt-5"])),
            Some(60),
            Some(5),
            Some(json!({"cache_1h": true})),
            true,
            Some(300),
            false,
            9,
            0,
            1.5,
            false,
        )
        .expect("export record should build")
        .with_activity_timestamps(
            Some(1_711_000_102),
            Some(1_711_000_100),
            Some(1_711_000_101),
        )
        .expect("export activity timestamps should build")]),
    );
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));

    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state =
                crate::data::GatewayDataState::with_user_reader_for_tests(user_repository)
                    .with_auth_api_key_reader(auth_repository);
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-api-keys",
                    "device-users-me-api-keys",
                    "refresh-token-users-me-api-keys",
                    now,
                )])
        })
        .await;

    let client = reqwest::Client::new();
    let list_response = client
        .get(format!("{gateway_url}/api/users/me/api-keys"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-keys")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(list_response.status(), StatusCode::OK);
    let list_payload: serde_json::Value =
        list_response.json().await.expect("json body should parse");
    let api_keys = list_payload.as_array().expect("api keys should be array");
    assert_eq!(api_keys.len(), 1);
    assert_eq!(api_keys[0]["id"], "user-key-1");
    assert_eq!(api_keys[0]["name"], "primary");
    assert_eq!(api_keys[0]["key_display"], "sk-user-li...ve-1");
    assert_eq!(api_keys[0]["total_requests"], 9);
    assert_eq!(api_keys[0]["total_cost_usd"], 1.5);
    assert_eq!(api_keys[0]["created_at"], "2024-03-21T05:48:20+00:00");
    assert_eq!(api_keys[0]["last_used_at"], "2024-03-21T05:48:22+00:00");

    let detail_response = client
        .get(format!(
            "{gateway_url}/api/users/me/api-keys/user-key-1?include_key=true"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-keys")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(detail_response.status(), StatusCode::OK);
    let detail_payload: serde_json::Value = detail_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(detail_payload["key"], "sk-user-live-1");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_users_me_client_config_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let _public_base_url_guard =
        set_test_env_var("AETHER_PUBLIC_BASE_URL", "https://aether.example.com/");
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-client-config"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));

    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state =
                crate::data::GatewayDataState::with_user_reader_for_tests(user_repository)
                    .with_system_config_values_for_tests(vec![(
                        "site_name".to_string(),
                        json!("Aether Local"),
                    )]);
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-client-config",
                    "device-users-me-client-config",
                    "refresh-token-users-me-client-config",
                    now,
                )])
        })
        .await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/users/me/client-config"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-client-config")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["base_url"], "https://aether.example.com");
    assert_eq!(payload["site_name"], "Aether Local");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_rejects_invalid_users_me_api_key_detail_path_as_local_not_found_without_hitting_upstream(
) {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            ("session_id".to_string(), json!("session-users-me-api-keys")),
        ]),
        now + chrono::Duration::hours(1),
    );
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(
            user,
            sample_auth_wallet("user-auth-1", now),
            [sample_auth_session(
                "user-auth-1",
                "session-users-me-api-keys",
                "device-users-me-api-keys",
                "refresh-token-users-me-api-keys",
                now,
            )],
        )
        .await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/users/me/api-keys/"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-keys")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_local_route_not_found_response(response).await;
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_rejects_invalid_users_me_api_key_patch_path_as_local_not_found_without_hitting_upstream(
) {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-api-key-writes"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state = crate::data::GatewayDataState::with_auth_api_key_repository_for_tests(
                Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![])),
            )
            .with_user_reader(Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![
                user,
            ])));
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-api-key-writes",
                    "device-users-me-api-key-writes",
                    "refresh-token-users-me-api-key-writes",
                    now,
                )])
        })
        .await;

    let response = reqwest::Client::new()
        .patch(format!("{gateway_url}/api/users/me/api-keys/"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-key-writes")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({ "is_active": false }))
        .send()
        .await
        .expect("request should succeed");

    assert_local_route_not_found_response(response).await;
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_rejects_users_me_api_key_nested_get_path_as_local_not_found_without_hitting_upstream(
) {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            ("session_id".to_string(), json!("session-users-me-api-keys")),
        ]),
        now + chrono::Duration::hours(1),
    );
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(
            user,
            sample_auth_wallet("user-auth-1", now),
            [sample_auth_session(
                "user-auth-1",
                "session-users-me-api-keys",
                "device-users-me-api-keys",
                "refresh-token-users-me-api-keys",
                now,
            )],
        )
        .await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/api/users/me/api-keys/key-123/providers"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-keys")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response
            .headers()
            .get(CONTROL_ROUTE_FAMILY_HEADER)
            .and_then(|value| value.to_str().ok()),
        None
    );
    assert_eq!(
        response
            .headers()
            .get(CONTROL_ROUTE_KIND_HEADER)
            .and_then(|value| value.to_str().ok()),
        None
    );
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["error"]["type"], "http_error");
    assert_eq!(payload["error"]["message"], "Route not found");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_users_me_api_key_writes_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-api-key-writes"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let encrypted =
        encrypt_python_fernet_plaintext(DEVELOPMENT_ENCRYPTION_KEY, "sk-user-existing-1")
            .expect("ciphertext should build");
    let auth_repository = Arc::new(
        InMemoryAuthApiKeySnapshotRepository::seed(vec![(
            Some("hash-user-existing-1".to_string()),
            StoredAuthApiKeySnapshot::new(
                "user-auth-1".to_string(),
                "alice".to_string(),
                Some("alice@example.com".to_string()),
                "user".to_string(),
                "local".to_string(),
                true,
                false,
                Some(json!(["openai"])),
                Some(json!(["openai:chat"])),
                Some(json!(["gpt-5"])),
                "user-existing-1".to_string(),
                Some("existing".to_string()),
                true,
                false,
                false,
                Some(60),
                Some(5),
                Some(300),
                Some(json!(["openai"])),
                Some(json!(["openai:chat"])),
                Some(json!(["gpt-5"])),
            )
            .expect("auth api key snapshot should build"),
        )])
        .with_export_records(vec![StoredAuthApiKeyExportRecord::new(
            "user-auth-1".to_string(),
            "user-existing-1".to_string(),
            "hash-user-existing-1".to_string(),
            Some(encrypted),
            Some("existing".to_string()),
            Some(json!(["openai"])),
            Some(json!(["openai:chat"])),
            Some(json!(["gpt-5"])),
            Some(60),
            Some(5),
            None,
            true,
            Some(300),
            false,
            3,
            0,
            0.5,
            false,
        )
        .expect("export record should build")]),
    );
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));
    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider("provider-openai", "openai", 10)],
        vec![sample_endpoint(
            "endpoint-openai-1",
            "provider-openai",
            "openai:chat",
            "https://api.openai.example",
        )],
        vec![],
    ));

    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state = crate::data::GatewayDataState::with_auth_api_key_repository_for_tests(
                auth_repository,
            )
            .with_user_reader(user_repository)
            .with_provider_catalog_reader(provider_catalog_repository)
            .with_encryption_key_for_tests(DEVELOPMENT_ENCRYPTION_KEY);
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-api-key-writes",
                    "device-users-me-api-key-writes",
                    "refresh-token-users-me-api-key-writes",
                    now,
                )])
        })
        .await;

    let client = reqwest::Client::new();
    let create_response = client
        .post(format!("{gateway_url}/api/users/me/api-keys"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-key-writes")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({
            "name": "writer-key",
            "rate_limit": 120
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(create_response.status(), StatusCode::OK);
    let create_payload: serde_json::Value = create_response
        .json()
        .await
        .expect("json body should parse");
    let created_id = create_payload["id"]
        .as_str()
        .expect("created id should be string")
        .to_string();
    assert_eq!(create_payload["name"], "writer-key");
    assert_eq!(create_payload["rate_limit"], 120);
    assert_eq!(create_payload["concurrent_limit"], serde_json::Value::Null);
    assert_eq!(create_payload["feature_settings"], serde_json::Value::Null);
    assert_eq!(create_payload["message"], "API密钥创建成功");
    let created_at = create_payload["created_at"]
        .as_str()
        .expect("created_at should be string");
    assert!(chrono::DateTime::parse_from_rfc3339(created_at).is_ok());
    assert!(!created_at.starts_with("1970-01-01"));
    assert!(create_payload["key"]
        .as_str()
        .unwrap_or_default()
        .starts_with("sk-"));

    let update_response = client
        .put(format!("{gateway_url}/api/users/me/api-keys/{created_id}"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-key-writes")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({
            "name": "writer-key-renamed",
            "rate_limit": 30,
            "concurrent_limit": 4,
            "feature_settings": {
                "chat_pii_redaction": {
                    "enabled": true
                }
            }
        }))
        .send()
        .await
        .expect("request should succeed");
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_payload: serde_json::Value = update_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(update_payload["name"], "writer-key-renamed");
    assert_eq!(update_payload["rate_limit"], 30);
    assert_eq!(update_payload["concurrent_limit"], 4);
    assert_eq!(
        update_payload["feature_settings"]["chat_pii_redaction"]["enabled"],
        true
    );
    assert_eq!(update_payload["message"], "API密钥已更新");

    let toggle_response = client
        .patch(format!("{gateway_url}/api/users/me/api-keys/{created_id}"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-key-writes")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({ "is_active": false }))
        .send()
        .await
        .expect("request should succeed");
    assert_eq!(toggle_response.status(), StatusCode::OK);
    let toggle_payload: serde_json::Value = toggle_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(toggle_payload["is_active"], false);

    let providers_response = client
        .put(format!(
            "{gateway_url}/api/users/me/api-keys/{created_id}/providers"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-key-writes")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({ "providers": ["openai"] }))
        .send()
        .await
        .expect("request should succeed");
    assert_eq!(providers_response.status(), StatusCode::OK);
    let providers_payload: serde_json::Value = providers_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(providers_payload["message"], "API密钥可用提供商已更新");
    assert_eq!(
        providers_payload["allowed_providers"],
        json!(["provider-openai"])
    );

    let capabilities_response = client
        .put(format!(
            "{gateway_url}/api/users/me/api-keys/{created_id}/capabilities"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-key-writes")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({ "force_capabilities": {} }))
        .send()
        .await
        .expect("request should succeed");
    assert_eq!(capabilities_response.status(), StatusCode::OK);
    let capabilities_payload: serde_json::Value = capabilities_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(capabilities_payload["force_capabilities"], json!({}));

    let detail_response = client
        .get(format!("{gateway_url}/api/users/me/api-keys/{created_id}"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-key-writes")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");
    assert_eq!(detail_response.status(), StatusCode::OK);
    let detail_payload: serde_json::Value = detail_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(detail_payload["id"], created_id);
    assert_eq!(detail_payload["is_active"], false);
    assert_eq!(
        detail_payload["allowed_providers"],
        json!(["provider-openai"])
    );
    assert_eq!(detail_payload["concurrent_limit"], 4);
    assert_eq!(detail_payload["force_capabilities"], json!({}));
    assert_eq!(detail_payload["created_at"], created_at);
    assert_eq!(
        detail_payload["feature_settings"]["chat_pii_redaction"]["enabled"],
        true
    );

    let delete_response = client
        .delete(format!("{gateway_url}/api/users/me/api-keys/{created_id}"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-key-writes")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");
    assert_eq!(delete_response.status(), StatusCode::OK);
    let delete_payload: serde_json::Value = delete_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(delete_payload["message"], "API密钥已删除");

    let missing_response = client
        .get(format!("{gateway_url}/api/users/me/api-keys/{created_id}"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-key-writes")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");
    assert_eq!(missing_response.status(), StatusCode::NOT_FOUND);
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_returns_service_unavailable_for_users_me_api_key_writes_without_writer() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-api-key-readonly"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let encrypted =
        encrypt_python_fernet_plaintext(DEVELOPMENT_ENCRYPTION_KEY, "sk-user-existing-1")
            .expect("ciphertext should build");
    let auth_repository = Arc::new(
        InMemoryAuthApiKeySnapshotRepository::seed(vec![(
            Some("hash-user-existing-1".to_string()),
            StoredAuthApiKeySnapshot::new(
                "user-auth-1".to_string(),
                "alice".to_string(),
                Some("alice@example.com".to_string()),
                "user".to_string(),
                "local".to_string(),
                true,
                false,
                Some(json!(["openai"])),
                Some(json!(["openai:chat"])),
                Some(json!(["gpt-5"])),
                "user-existing-1".to_string(),
                Some("existing".to_string()),
                true,
                false,
                false,
                Some(60),
                Some(5),
                Some(300),
                Some(json!(["openai"])),
                Some(json!(["openai:chat"])),
                Some(json!(["gpt-5"])),
            )
            .expect("auth api key snapshot should build"),
        )])
        .with_export_records(vec![StoredAuthApiKeyExportRecord::new(
            "user-auth-1".to_string(),
            "user-existing-1".to_string(),
            "hash-user-existing-1".to_string(),
            Some(encrypted),
            Some("existing".to_string()),
            Some(json!(["openai"])),
            Some(json!(["openai:chat"])),
            Some(json!(["gpt-5"])),
            Some(60),
            Some(5),
            None,
            true,
            Some(300),
            false,
            3,
            0,
            0.5,
            false,
        )
        .expect("export record should build")]),
    );
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));
    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider("provider-openai", "openai", 10)],
        vec![sample_endpoint(
            "endpoint-openai-1",
            "provider-openai",
            "openai:chat",
            "https://api.openai.example",
        )],
        vec![],
    ));

    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state =
                crate::data::GatewayDataState::with_auth_api_key_reader_for_tests(auth_repository)
                    .with_user_reader(user_repository)
                    .with_provider_catalog_reader(provider_catalog_repository);
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-api-key-readonly",
                    "device-users-me-api-key-readonly",
                    "refresh-token-users-me-api-key-readonly",
                    now,
                )])
        })
        .await;

    let client = reqwest::Client::new();
    let create_response = client
        .post(format!("{gateway_url}/api/users/me/api-keys"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-key-readonly")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({
            "name": "readonly-key",
            "rate_limit": 120
        }))
        .send()
        .await
        .expect("request should succeed");
    assert_eq!(create_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let create_payload: serde_json::Value = create_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(create_payload["detail"], "用户 API 密钥写入暂不可用");

    let update_response = client
        .put(format!(
            "{gateway_url}/api/users/me/api-keys/user-existing-1"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-key-readonly")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({
            "name": "writer-key-renamed",
            "rate_limit": 30
        }))
        .send()
        .await
        .expect("request should succeed");
    assert_eq!(update_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let update_payload: serde_json::Value = update_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(update_payload["detail"], "用户 API 密钥写入暂不可用");

    let toggle_response = client
        .patch(format!(
            "{gateway_url}/api/users/me/api-keys/user-existing-1"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-key-readonly")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({ "is_active": false }))
        .send()
        .await
        .expect("request should succeed");
    assert_eq!(toggle_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let toggle_payload: serde_json::Value = toggle_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(toggle_payload["detail"], "用户 API 密钥写入暂不可用");

    let providers_response = client
        .put(format!(
            "{gateway_url}/api/users/me/api-keys/user-existing-1/providers"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-key-readonly")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({ "providers": ["openai"] }))
        .send()
        .await
        .expect("request should succeed");
    assert_eq!(providers_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let providers_payload: serde_json::Value = providers_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(providers_payload["detail"], "用户 API 密钥写入暂不可用");

    let capabilities_response = client
        .put(format!(
            "{gateway_url}/api/users/me/api-keys/user-existing-1/capabilities"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-key-readonly")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({ "force_capabilities": {} }))
        .send()
        .await
        .expect("request should succeed");
    assert_eq!(
        capabilities_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let capabilities_payload: serde_json::Value = capabilities_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(capabilities_payload["detail"], "用户 API 密钥写入暂不可用");

    let delete_response = client
        .delete(format!(
            "{gateway_url}/api/users/me/api-keys/user-existing-1"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-api-key-readonly")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");
    assert_eq!(delete_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let delete_payload: serde_json::Value = delete_response
        .json()
        .await
        .expect("json body should parse");
    assert_eq!(delete_payload["detail"], "用户 API 密钥写入暂不可用");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_rejects_users_me_management_token_nested_get_path_as_local_not_found_without_hitting_upstream(
) {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-management-token-reads"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let repository = Arc::new(InMemoryManagementTokenRepository::seed(vec![
        sample_users_me_management_token("mt-user-1", "user-auth-1", "alice", true),
    ]));
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));

    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state =
                crate::data::GatewayDataState::with_management_token_repository_for_tests(
                    repository,
                )
                .with_user_reader(user_repository);
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-management-token-reads",
                    "device-users-me-management-token-reads",
                    "refresh-token-users-me-management-token-reads",
                    now,
                )])
        })
        .await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/api/me/management-tokens/mt-user-1/status"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header(
            "x-client-device-id",
            "device-users-me-management-token-reads",
        )
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_local_route_not_found_response(response).await;
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_users_me_providers_locally_without_proxying_upstream() {
    let now = Utc::now();
    let mut user = sample_auth_user(now);
    user.allowed_providers = Some(vec!["claude".to_string()]);
    user.allowed_providers_mode = "specific".to_string();
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-providers"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-openai", "openai", 10),
            sample_provider("provider-claude", "claude", 20),
        ],
        vec![
            sample_endpoint(
                "endpoint-openai-1",
                "provider-openai",
                "openai:chat",
                "https://api.openai.example",
            ),
            sample_endpoint(
                "endpoint-claude-1",
                "provider-claude",
                "claude:messages",
                "https://api.claude.example",
            ),
        ],
        vec![],
    ));
    let global_model_repository = Arc::new(
        InMemoryGlobalModelReadRepository::seed(Vec::<StoredPublicGlobalModel>::new())
            .with_public_catalog_models(vec![
                sample_public_catalog_model(
                    "model-openai-gpt5",
                    "provider-openai",
                    "openai",
                    "gpt-5-preview",
                    "gpt-5",
                    "GPT 5",
                ),
                sample_public_catalog_model(
                    "model-claude-sonnet",
                    "provider-claude",
                    "claude",
                    "claude-sonnet-4-5-20251001",
                    "claude-sonnet-4-5",
                    "Claude Sonnet 4.5",
                ),
            ]),
    );
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));
    let group = user_repository
        .create_user_group(UpsertUserGroupRecord {
            name: "OpenAI only".to_string(),
            description: None,
            priority: 0,
            allowed_providers: Some(vec!["openai".to_string()]),
            allowed_providers_mode: "specific".to_string(),
            allowed_api_formats: None,
            allowed_api_formats_mode: "unrestricted".to_string(),
            allowed_models: None,
            allowed_models_mode: "unrestricted".to_string(),
            rate_limit: None,
            rate_limit_mode: "system".to_string(),
        })
        .await
        .expect("group should create")
        .expect("group should exist");
    user_repository
        .add_user_to_group(&group.id, "user-auth-1")
        .await
        .expect("group membership should create");

    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state = crate::data::GatewayDataState::with_global_model_reader_for_tests(
                global_model_repository,
            )
            .with_provider_catalog_reader(provider_catalog_repository)
            .with_user_reader(user_repository);
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-providers",
                    "device-users-me-providers",
                    "refresh-token-users-me-providers",
                    now,
                )])
        })
        .await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/users/me/providers"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-providers")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let providers = payload.as_array().expect("providers should be array");
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0]["id"], "provider-openai");
    assert!(providers[0].get("name").is_none());
    assert!(providers[0].get("description").is_none());
    assert_eq!(providers[0]["endpoints"][0]["id"], "endpoint-openai-1");
    assert!(providers[0]["endpoints"][0].get("base_url").is_none());
    assert_eq!(providers[0]["models"][0]["name"], "gpt-5");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_auth_login_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(user, sample_auth_wallet("user-auth-1", now), []).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{gateway_url}/api/auth/login"))
        .header("x-client-device-id", "device-auth-login")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({
            "email": "alice@example.com",
            "password": "secret123",
            "auth_type": "local",
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let set_cookie = response
        .headers()
        .get(reqwest::header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
        .expect("set-cookie should exist");
    assert!(set_cookie.contains("aether_refresh_token="));
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let access_token = payload["access_token"]
        .as_str()
        .expect("access token should exist")
        .to_string();
    assert_eq!(payload["user_id"], "user-auth-1");
    assert_eq!(payload["email"], "alice@example.com");
    assert_eq!(payload["username"], "alice");
    assert_eq!(payload["role"], "user");

    let me_response = client
        .get(format!("{gateway_url}/api/auth/me"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-auth-login")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("me request should succeed");

    assert_eq!(me_response.status(), StatusCode::OK);
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_rejects_unsupported_auth_login_type_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(user, sample_auth_wallet("user-auth-1", now), []).await;

    let response = reqwest::Client::new()
        .post(format!("{gateway_url}/api/auth/login"))
        .header("x-client-device-id", "device-auth-login-unsupported")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({
            "email": "alice@example.com",
            "password": "secret123",
            "auth_type": "oauth",
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["detail"], "不支持的认证类型");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

async fn start_turnstile_siteverify_server(
    response_payload: serde_json::Value,
    status: StatusCode,
) -> (
    String,
    Arc<Mutex<Vec<std::collections::HashMap<String, String>>>>,
    tokio::task::JoinHandle<()>,
) {
    start_turnstile_siteverify_server_with_delay(response_payload, status, None).await
}

async fn start_turnstile_siteverify_server_with_delay(
    response_payload: serde_json::Value,
    status: StatusCode,
    delay: Option<Duration>,
) -> (
    String,
    Arc<Mutex<Vec<std::collections::HashMap<String, String>>>>,
    tokio::task::JoinHandle<()>,
) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let requests_clone = Arc::clone(&requests);
    let upstream = Router::new().route(
        "/turnstile/siteverify",
        any(
            move |axum::extract::Form(form): axum::extract::Form<
                std::collections::HashMap<String, String>,
            >| {
                let requests_inner = Arc::clone(&requests_clone);
                let response_payload = response_payload.clone();
                async move {
                    if let Some(delay) = delay {
                        tokio::time::sleep(delay).await;
                    }
                    requests_inner
                        .lock()
                        .expect("turnstile requests should lock")
                        .push(form);
                    (status, Json(response_payload))
                }
            },
        ),
    );
    let (url, handle) = start_server(upstream).await;
    (format!("{url}/turnstile/siteverify"), requests, handle)
}

fn turnstile_enabled_data_state() -> crate::data::GatewayDataState {
    crate::data::GatewayDataState::disabled().with_system_config_values_for_tests(vec![
        ("enable_registration".to_string(), json!(true)),
        ("require_email_verification".to_string(), json!(true)),
        ("smtp_host".to_string(), json!("smtp.example.com")),
        ("smtp_from_email".to_string(), json!("ops@example.com")),
        ("default_user_initial_gift_usd".to_string(), json!(12.5)),
        ("turnstile_enabled".to_string(), json!(true)),
        ("turnstile_site_key".to_string(), json!("site-public-key")),
        (
            "turnstile_secret_key".to_string(),
            json!("secret-private-key"),
        ),
        (
            "turnstile_allowed_hostnames".to_string(),
            json!(["gateway.example.com"]),
        ),
    ])
}

#[tokio::test]
async fn gateway_handles_users_me_available_models_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-available-models"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let global_model_repository = Arc::new(
        InMemoryGlobalModelReadRepository::seed(vec![
            sample_public_global_model("gm-1", "gpt-5", "GPT 5", true),
            sample_public_global_model("gm-2", "claude-sonnet-4-5", "Claude Sonnet 4.5", true),
            sample_public_global_model("gm-3", "disabled-model", "Disabled Model", false),
        ])
        .with_active_global_model_refs(vec![
            StoredProviderActiveGlobalModel::new("provider-openai".to_string(), "gm-1".to_string())
                .expect("active global model ref should build"),
            StoredProviderActiveGlobalModel::new("provider-claude".to_string(), "gm-2".to_string())
                .expect("active global model ref should build"),
        ]),
    );
    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-openai", "openai", 10),
            sample_provider("provider-claude", "claude", 20),
        ],
        vec![],
        vec![],
    ));
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state = crate::data::GatewayDataState::with_global_model_reader_for_tests(
                global_model_repository,
            )
            .with_provider_catalog_reader(provider_catalog_repository)
            .with_user_reader(user_repository);
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-available-models",
                    "device-users-me-available-models",
                    "refresh-token-placeholder",
                    now,
                )])
        })
        .await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/api/users/me/available-models?search=gpt&skip=0&limit=10"
        ))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-available-models")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let models = payload["models"]
        .as_array()
        .expect("models should be an array");
    assert_eq!(payload["total"], 1);
    assert_eq!(models.len(), 1);
    assert_eq!(models[0]["id"], "gm-1");
    assert_eq!(models[0]["name"], "gpt-5");
    assert_eq!(models[0]["display_name"], "GPT 5");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_refreshes_users_me_available_models_after_group_assignment() {
    let now = Utc::now();
    let mut user = sample_auth_user(now);
    user.allowed_providers = None;
    user.allowed_providers_mode = "unrestricted".to_string();
    // Keep the legacy gpt-5 personal policy from sample_auth_user. Group policies are the
    // authority now, so this stale field must not narrow the user catalog.
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-group-models"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let mut allowed_model =
        sample_public_global_model("gm-2", "claude-sonnet-4-5", "Claude Sonnet 4.5", true);
    allowed_model.config = Some(json!({
        "description": "Claude detail",
        "model_mappings": ["claude-upstream"]
    }));
    let mut blocked_model = sample_public_global_model("gm-1", "gpt-5", "GPT 5", true);
    blocked_model.config = Some(json!({
        "description": "GPT detail",
        "model_mappings": ["gpt-upstream"]
    }));
    let global_model_repository = Arc::new(InMemoryGlobalModelReadRepository::seed(vec![
        blocked_model,
        allowed_model,
    ]));
    let user_repository: Arc<dyn UserReadRepository> =
        Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));
    let group = user_repository
        .create_user_group(UpsertUserGroupRecord {
            name: "Claude only".to_string(),
            description: None,
            priority: 0,
            allowed_providers: None,
            allowed_providers_mode: "unrestricted".to_string(),
            allowed_api_formats: None,
            allowed_api_formats_mode: "unrestricted".to_string(),
            allowed_models: Some(vec!["claude-sonnet-4-5".to_string()]),
            allowed_models_mode: "specific".to_string(),
            rate_limit: None,
            rate_limit_mode: "system".to_string(),
        })
        .await
        .expect("group should create")
        .expect("group should exist");
    let data_state =
        crate::data::GatewayDataState::with_global_model_reader_for_tests(global_model_repository)
            .with_user_reader(Arc::clone(&user_repository));
    let state = AppState::new()
        .expect("gateway should build")
        .with_data_state_for_tests(data_state)
        .with_auth_sessions_for_tests([sample_auth_session(
            "user-auth-1",
            "session-users-me-group-models",
            "device-users-me-group-models",
            "refresh-token-placeholder",
            now,
        )]);
    let mutation_state = state.clone();
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| state).await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{gateway_url}/api/users/me/available-models"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-group-models")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["total"], 2);

    mutation_state
        .replace_user_groups_for_user("user-auth-1", std::slice::from_ref(&group.id))
        .await
        .expect("group membership should create");

    let response = client
        .get(format!("{gateway_url}/api/users/me/available-models"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-group-models")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed after group assignment");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let models = payload["models"]
        .as_array()
        .expect("models should be an array");
    assert_eq!(payload["total"], 1);
    assert_eq!(models.len(), 1);
    assert_eq!(models[0]["name"], "claude-sonnet-4-5");
    assert_eq!(models[0]["config"]["description"], "Claude detail");
    assert!(models[0]["config"].get("model_mappings").is_none());
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_returns_no_users_me_available_models_when_group_denies_all_models() {
    let now = Utc::now();
    let mut user = sample_auth_user(now);
    user.allowed_providers = None;
    user.allowed_providers_mode = "unrestricted".to_string();
    user.allowed_models = None;
    user.allowed_models_mode = "unrestricted".to_string();
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-deny-all-models"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let global_model_repository = Arc::new(InMemoryGlobalModelReadRepository::seed(vec![
        sample_public_global_model("gm-1", "gpt-5", "GPT 5", true),
        sample_public_global_model("gm-2", "claude-sonnet-4-5", "Claude Sonnet 4.5", true),
    ]));
    let user_repository: Arc<dyn UserReadRepository> =
        Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));
    let group = user_repository
        .create_user_group(UpsertUserGroupRecord {
            name: "No models".to_string(),
            description: None,
            priority: 0,
            allowed_providers: None,
            allowed_providers_mode: "unrestricted".to_string(),
            allowed_api_formats: None,
            allowed_api_formats_mode: "unrestricted".to_string(),
            allowed_models: None,
            allowed_models_mode: "deny_all".to_string(),
            rate_limit: None,
            rate_limit_mode: "system".to_string(),
        })
        .await
        .expect("group should create")
        .expect("group should exist");
    user_repository
        .add_user_to_group(&group.id, "user-auth-1")
        .await
        .expect("group membership should create");

    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state = crate::data::GatewayDataState::with_global_model_reader_for_tests(
                global_model_repository,
            )
            .with_user_reader(Arc::clone(&user_repository));
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-deny-all-models",
                    "device-users-me-deny-all-models",
                    "refresh-token-placeholder",
                    now,
                )])
        })
        .await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/users/me/available-models"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-deny-all-models")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["total"], 0);
    assert_eq!(payload["models"].as_array().map(Vec::len), Some(0));
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_returns_service_unavailable_for_users_me_available_models_without_provider_catalog(
) {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-available-models-unavailable"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let global_model_repository = Arc::new(
        InMemoryGlobalModelReadRepository::seed(vec![
            sample_public_global_model("gm-1", "gpt-5", "GPT 5", true),
            sample_public_global_model("gm-2", "claude-sonnet-4-5", "Claude Sonnet 4.5", true),
        ])
        .with_active_global_model_refs(vec![StoredProviderActiveGlobalModel::new(
            "provider-openai".to_string(),
            "gm-1".to_string(),
        )
        .expect("active global model ref should build")]),
    );
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));
    let group = user_repository
        .create_user_group(UpsertUserGroupRecord {
            name: "OpenAI provider only".to_string(),
            description: None,
            priority: 0,
            allowed_providers: Some(vec!["openai".to_string()]),
            allowed_providers_mode: "specific".to_string(),
            allowed_api_formats: None,
            allowed_api_formats_mode: "unrestricted".to_string(),
            allowed_models: None,
            allowed_models_mode: "unrestricted".to_string(),
            rate_limit: None,
            rate_limit_mode: "system".to_string(),
        })
        .await
        .expect("group should create")
        .expect("group should exist");
    user_repository
        .add_user_to_group(&group.id, "user-auth-1")
        .await
        .expect("group membership should create");
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state = crate::data::GatewayDataState::with_global_model_reader_for_tests(
                global_model_repository,
            )
            .with_user_reader(user_repository);
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-available-models-unavailable",
                    "device-users-me-available-models-unavailable",
                    "refresh-token-placeholder",
                    now,
                )])
        })
        .await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/users/me/available-models"))
        .header("authorization", format!("Bearer {access_token}"))
        .header(
            "x-client-device-id",
            "device-users-me-available-models-unavailable",
        )
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["detail"], "用户提供商目录暂不可用");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_users_me_model_capabilities_get_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-model-cap-get"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state =
                crate::data::GatewayDataState::with_user_reader_for_tests(user_repository);
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-model-cap-get",
                    "device-users-me-model-cap-get",
                    "refresh-token-placeholder",
                    now,
                )])
                .with_auth_user_model_capability_settings_for_tests(
                    "user-auth-1",
                    json!({"gpt-5": {"cache_1h": true}}),
                )
        })
        .await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/api/users/me/model-capabilities"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-model-cap-get")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(
        payload["model_capability_settings"],
        json!({"gpt-5": {"cache_1h": true}})
    );
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_updates_users_me_model_capabilities_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-model-cap-put"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let user_repository = Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]));
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state =
                crate::data::GatewayDataState::with_user_reader_for_tests(user_repository);
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-model-cap-put",
                    "device-users-me-model-cap-put",
                    "refresh-token-placeholder",
                    now,
                )])
                .with_auth_user_model_capability_settings_for_tests(
                    "user-auth-1",
                    json!({"disabled-model": {"disabled_flag": true}}),
                )
        })
        .await;

    let client = reqwest::Client::new();
    let put_response = client
        .put(format!("{gateway_url}/api/users/me/model-capabilities"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-model-cap-put")
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({
            "model_capability_settings": {}
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(put_response.status(), StatusCode::OK);
    let put_payload: serde_json::Value = put_response.json().await.expect("json body should parse");
    assert_eq!(put_payload["message"], "模型能力配置已更新");
    assert_eq!(put_payload["model_capability_settings"], json!({}));

    let get_response = client
        .get(format!("{gateway_url}/api/users/me/model-capabilities"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-users-me-model-cap-put")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(get_response.status(), StatusCode::OK);
    let get_payload: serde_json::Value = get_response.json().await.expect("json body should parse");
    assert_eq!(get_payload["model_capability_settings"], json!({}));
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_returns_service_unavailable_for_users_me_model_capabilities_update_without_storage(
) {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            (
                "session_id".to_string(),
                json!("session-users-me-model-cap-unavailable"),
            ),
        ]),
        now + chrono::Duration::hours(1),
    );
    let user_repository =
        Arc::new(InMemoryUserReadRepository::seed_auth_users(vec![user]).read_only());
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_builder(|| {
            let data_state =
                crate::data::GatewayDataState::with_user_reader_for_tests(user_repository);
            AppState::new()
                .expect("gateway should build")
                .with_data_state_for_tests(data_state)
                .with_auth_sessions_for_tests([sample_auth_session(
                    "user-auth-1",
                    "session-users-me-model-cap-unavailable",
                    "device-users-me-model-cap-unavailable",
                    "refresh-token-placeholder",
                    now,
                )])
                .without_auth_user_model_capability_store_for_tests()
        })
        .await;

    let response = reqwest::Client::new()
        .put(format!("{gateway_url}/api/users/me/model-capabilities"))
        .header("authorization", format!("Bearer {access_token}"))
        .header(
            "x-client-device-id",
            "device-users-me-model-cap-unavailable",
        )
        .header("user-agent", "AetherTest/1.0")
        .json(&json!({
            "model_capability_settings": {}
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["detail"], "用户模型能力配置存储暂不可用");
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_auth_refresh_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let refresh_token = build_test_auth_token(
        "refresh",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            ("session_id".to_string(), json!("session-auth-2")),
            ("jti".to_string(), json!("jti-auth-1")),
        ]),
        now + chrono::Duration::days(7),
    );
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(
            user,
            sample_auth_wallet("user-auth-1", now),
            [sample_auth_session(
                "user-auth-1",
                "session-auth-2",
                "device-auth-2",
                &refresh_token,
                now,
            )],
        )
        .await;

    let response = reqwest::Client::new()
        .post(format!("{gateway_url}/api/auth/refresh"))
        .header("cookie", format!("aether_refresh_token={refresh_token}"))
        .header("x-client-device-id", "device-auth-2")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let set_cookie = response
        .headers()
        .get(http::header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .expect("set-cookie should exist")
        .to_string();
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    let access_token = payload["access_token"]
        .as_str()
        .expect("access token should be string");
    assert_eq!(payload["token_type"], "bearer");
    assert!(payload["expires_in"].as_i64().unwrap_or_default() > 0);
    assert_eq!(access_token.split('.').count(), 3);
    assert!(set_cookie.contains("aether_refresh_token="));
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("Path=/api/auth"));
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_handles_auth_logout_locally_without_proxying_upstream() {
    let now = Utc::now();
    let user = sample_auth_user(now);
    let access_token = build_test_auth_token(
        "access",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!(user.id)),
            ("role".to_string(), json!(user.role)),
            (
                "created_at".to_string(),
                json!(user.created_at.map(|value| value.to_rfc3339())),
            ),
            ("session_id".to_string(), json!("session-auth-3")),
        ]),
        now + chrono::Duration::hours(1),
    );
    let refresh_token = build_test_auth_token(
        "refresh",
        serde_json::Map::from_iter([
            ("user_id".to_string(), json!("user-auth-1")),
            ("created_at".to_string(), json!(Some(now.to_rfc3339()))),
            ("session_id".to_string(), json!("session-auth-3")),
            ("jti".to_string(), json!("jti-auth-3")),
        ]),
        now + chrono::Duration::days(7),
    );
    let (gateway_url, upstream_hits, gateway_handle, upstream_handle) =
        start_auth_gateway_with_state(
            user,
            sample_auth_wallet("user-auth-1", now),
            [sample_auth_session(
                "user-auth-1",
                "session-auth-3",
                "device-auth-3",
                &refresh_token,
                now,
            )],
        )
        .await;

    let response = reqwest::Client::new()
        .post(format!("{gateway_url}/api/auth/logout"))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-client-device-id", "device-auth-3")
        .header("user-agent", "AetherTest/1.0")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let set_cookie = response
        .headers()
        .get(http::header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .expect("set-cookie should exist")
        .to_string();
    let payload: serde_json::Value = response.json().await.expect("json body should parse");
    assert_eq!(payload["success"], true);
    assert_eq!(payload["message"], "登出成功");
    assert!(set_cookie.contains("aether_refresh_token="));
    assert!(set_cookie.contains("Max-Age=0"));
    assert_eq!(*upstream_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}
