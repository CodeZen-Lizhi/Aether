use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aether_crypto::{encrypt_python_fernet_plaintext, DEVELOPMENT_ENCRYPTION_KEY};
use aether_data::repository::auth::{
    InMemoryAuthApiKeySnapshotRepository, StoredAuthApiKeySnapshot,
};
use aether_data::repository::candidate_selection::InMemoryMinimalCandidateSelectionReadRepository;
use aether_data::repository::candidates::InMemoryRequestCandidateRepository;
use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
use aether_data::repository::routing_profiles::InMemoryRoutingGroupRepository;
use aether_data_contracts::repository::candidate_selection::StoredMinimalCandidateSelectionRow;
use aether_data_contracts::repository::candidates::RequestCandidateReadRepository;
use aether_data_contracts::repository::provider_catalog::{
    ProviderCatalogReadRepository, StoredProviderCatalogEndpoint, StoredProviderCatalogKey,
    StoredProviderCatalogProvider,
};
use aether_data_contracts::repository::routing_profiles::StoredRoutingGroup;
use axum::body::{to_bytes, Body, Bytes};
use axum::extract::Request;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use http::StatusCode;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::Notify;

use crate::data::GatewayDataState;
use crate::tests::{build_router_with_state, start_server, AppState};

pub(super) const CLIENT_KEY: &str = "sk-local-failover-client";
pub(super) const FIRST_TEXT: &str = "first-target-visible-content";

pub(super) enum Reply {
    Error(StatusCode, Option<&'static str>),
    Success,
    RawSuccess(&'static str, &'static str),
    DelayedError(Duration),
    DelayedSse(Duration),
    BrokenSse(Arc<Notify>),
    HeldSse(Arc<Notify>),
}

impl Reply {
    async fn response(self) -> Response {
        match self {
            Self::RawSuccess(content_type, body) => {
                ([(http::header::CONTENT_TYPE, content_type)], body).into_response()
            }
            Self::Error(status, retry_after) => {
                let mut response = (
                    status,
                    Json(json!({"error": {
                        "type": "server_error", "message": "scripted upstream failure"
                    }})),
                )
                    .into_response();
                if let Some(value) = retry_after {
                    response
                        .headers_mut()
                        .insert(http::header::RETRY_AFTER, value.parse().unwrap());
                }
                response
            }
            Self::Success => Json(json!({
                "id": "chatcmpl-local", "object": "chat.completion", "created": 1,
                "model": "gpt-5", "choices": [{"index": 0,
                    "message": {"role": "assistant", "content": "local-success"},
                    "finish_reason": "stop"}],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
            }))
            .into_response(),
            Self::DelayedError(delay) => {
                tokio::time::sleep(delay).await;
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": {
                        "type": "server_error", "message": "delayed scripted failure"
                    }})),
                )
                    .into_response()
            }
            Self::DelayedSse(delay) => {
                tokio::time::sleep(delay).await;
                let terminal = json!({"id":"chatcmpl-local","object":"chat.completion.chunk",
                    "created":1,"model":"gpt-5","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]});
                (
                    [(http::header::CONTENT_TYPE, "text/event-stream")],
                    format!("{}data: {terminal}\n\ndata: [DONE]\n\n", first_sse_frame()),
                )
                    .into_response()
            }
            Self::BrokenSse(release) => {
                // The client releases the fault only after observing the text delta.
                let stream =
                    futures_util::stream::unfold((0, release), |(step, release)| async move {
                        match step {
                            0 => Some((
                                Ok::<_, std::io::Error>(Bytes::from(first_sse_frame())),
                                (1, release),
                            )),
                            1 => {
                                release.notified().await;
                                Some((
                                    Err(std::io::Error::other("scripted upstream disconnect")),
                                    (2, release),
                                ))
                            }
                            _ => None,
                        }
                    });
                (
                    [(http::header::CONTENT_TYPE, "text/event-stream")],
                    Body::from_stream(stream),
                )
                    .into_response()
            }
            Self::HeldSse(release) => {
                let stream = async_stream::stream! {
                    yield Ok::<_, std::io::Error>(Bytes::from(first_sse_frame()));
                    release.notified().await;
                    let terminal = json!({"id":"chatcmpl-local","object":"chat.completion.chunk",
                        "created":1,"model":"gpt-5","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]});
                    yield Ok(Bytes::from(format!("data: {terminal}\n\ndata: [DONE]\n\n")));
                };
                (
                    [(http::header::CONTENT_TYPE, "text/event-stream")],
                    Body::from_stream(stream),
                )
                    .into_response()
            }
        }
    }
}

fn first_sse_frame() -> String {
    let delta = json!({"id":"chatcmpl-local","object":"chat.completion.chunk",
        "created":1,"model":"gpt-5","choices":[{"index":0,
        "delta":{"role":"assistant","content":FIRST_TEXT},"finish_reason":null}]});
    format!("data: {delta}\n\n")
}

#[derive(Clone, Debug)]
pub(super) struct Receipt {
    pub target: usize,
    pub at: Instant,
    pub body: Value,
}

#[derive(Default)]
struct Servers(Vec<tokio::task::JoinHandle<()>>);

impl Drop for Servers {
    fn drop(&mut self) {
        for handle in &self.0 {
            handle.abort();
        }
    }
}

pub(super) struct Fixture {
    pub catalog: Arc<InMemoryProviderCatalogReadRepository>,
    pub receipts: Arc<Mutex<Vec<Receipt>>>,
    pub url: String,
    client: reqwest::Client,
    request_candidates: Arc<InMemoryRequestCandidateRepository>,
    target_admission: Arc<crate::upstream_admission::UpstreamTargetAdmission>,
    _servers: Servers,
}

impl Fixture {
    pub async fn new(
        mode: &str,
        attempts: u32,
        legacy: bool,
        rates: [f64; 2],
        budget_ms: u64,
        scripts: [Vec<Reply>; 2],
    ) -> Self {
        Self::with_target_limit(mode, attempts, legacy, rates, budget_ms, scripts, None).await
    }

    pub async fn with_target_limit(
        mode: &str,
        attempts: u32,
        legacy: bool,
        rates: [f64; 2],
        budget_ms: u64,
        scripts: [Vec<Reply>; 2],
        target_limit: Option<usize>,
    ) -> Self {
        let mut servers = Servers::default();
        let receipts = Arc::new(Mutex::new(Vec::new()));
        let mut urls = Vec::new();
        for (target, replies) in scripts.into_iter().enumerate() {
            let replies = Arc::new(Mutex::new(VecDeque::from(replies)));
            let receipts = Arc::clone(&receipts);
            let app = Router::new().route(
                "/v1/chat/completions",
                post(move |request: Request| {
                    let replies = Arc::clone(&replies);
                    let receipts = Arc::clone(&receipts);
                    async move {
                        assert_eq!(
                            request.headers()[http::header::AUTHORIZATION],
                            format!("Bearer sk-local-upstream-{target}")
                        );
                        let bytes = to_bytes(request.into_body(), 1024 * 1024).await.unwrap();
                        let body: Value = serde_json::from_slice(&bytes).unwrap();
                        receipts.lock().unwrap().push(Receipt {
                            target,
                            at: Instant::now(),
                            body,
                        });
                        let reply = replies.lock().unwrap().pop_front();
                        match reply {
                            Some(reply) => reply.response().await,
                            None => (StatusCode::IM_A_TEAPOT, "unexpected extra upstream attempt")
                                .into_response(),
                        }
                    }
                }),
            );
            let (url, handle) = start_server(app).await;
            urls.push(url);
            servers.0.push(handle);
        }

        let mut providers = Vec::new();
        let mut endpoints = Vec::new();
        let mut keys = Vec::new();
        let mut candidates = Vec::new();
        for target in 0..2 {
            let provider_id = format!("provider-{target}");
            let endpoint_id = format!("endpoint-{target}");
            let key_id = format!("key-{target}");
            let mut config = json!({"failover_rules": {"stream_failover_budget_ms": budget_ms}});
            if !legacy {
                config["failover_rules"]["max_attempts"] = json!(attempts);
                config["failover_rules"]["chat_policy_version"] = json!(1);
            }
            providers.push(
                StoredProviderCatalogProvider::new(
                    provider_id.clone(),
                    format!("local-{target}"),
                    None,
                    "custom".into(),
                )
                .unwrap()
                .with_transport_fields(
                    true,
                    true,
                    None,
                    legacy.then_some(attempts as i32),
                    None,
                    Some(10.0),
                    Some(5.0),
                    Some(config),
                ),
            );
            endpoints.push(
                StoredProviderCatalogEndpoint::new(
                    endpoint_id.clone(),
                    provider_id.clone(),
                    "openai:chat".into(),
                    Some("openai".into()),
                    Some("chat".into()),
                    true,
                )
                .unwrap()
                .with_transport_fields(
                    format!("{}/v1", urls[target]),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap(),
            );
            keys.push(
                StoredProviderCatalogKey::new(
                    key_id.clone(),
                    provider_id.clone(),
                    format!("local-key-{target}"),
                    "api_key".into(),
                    None,
                    true,
                )
                .unwrap()
                .with_transport_fields(
                    Some(json!(["openai:chat"])),
                    encrypt_python_fernet_plaintext(
                        DEVELOPMENT_ENCRYPTION_KEY,
                        &format!("sk-local-upstream-{target}"),
                    )
                    .unwrap(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap()
                .with_internal_priority(target as i32)
                .with_default_rate_multiplier(rates[target]),
            );
            candidates.push(StoredMinimalCandidateSelectionRow {
                provider_id,
                provider_name: format!("local-{target}"),
                provider_type: "custom".into(),
                provider_is_active: true,
                endpoint_id,
                endpoint_api_format: "openai:chat".into(),
                endpoint_api_family: Some("openai".into()),
                endpoint_kind: Some("chat".into()),
                endpoint_is_active: true,
                key_id,
                key_name: format!("local-key-{target}"),
                key_auth_type: "api_key".into(),
                key_is_active: true,
                key_api_formats: Some(vec!["openai:chat".into()]),
                key_allowed_models: None,
                key_capabilities: None,
                model_id: format!("model-{target}"),
                global_model_id: "global-local-model".into(),
                global_model_name: "gpt-5".into(),
                global_model_supports_streaming: Some(true),
                model_provider_model_name: "gpt-5".into(),
                model_provider_model_mappings: None,
                model_supports_streaming: Some(true),
                model_is_active: true,
                model_is_available: true,
            });
        }
        let auth = StoredAuthApiKeySnapshot::new(
            "local-user".into(),
            "local".into(),
            None,
            "user".into(),
            "local".into(),
            true,
            false,
            None,
            Some(json!(["openai:chat", "openai:responses"])),
            Some(json!(["gpt-5"])),
            "local-api-key".into(),
            Some("default".into()),
            true,
            false,
            false,
            Some(6000),
            Some(100),
            Some(4_102_444_800),
            None,
            Some(json!(["openai:chat", "openai:responses"])),
            Some(json!(["gpt-5"])),
        )
        .unwrap();
        let catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
            providers, endpoints, keys,
        ));
        let routing = Arc::new(InMemoryRoutingGroupRepository::seed(
            vec![StoredRoutingGroup {
                id: "local-routing".into(),
                name: "default".into(),
                description: None,
                enabled: true,
                is_system_default: true,
                config_json: json!({
                "default_policy": {"scheduling_mode":mode},
                "rules":[{"id":"ui_provider_priority","actions":[
                    {"type":"set_provider_priority","provider_id":"provider-0","priority":1},
                    {"type":"set_provider_priority","provider_id":"provider-1","priority":2}
                ]}]}),
                version: 1,
                created_at: 1,
                updated_at: 1,
                published_at: None,
            }],
            vec![],
            vec![],
        ));
        let request_candidates = Arc::new(InMemoryRequestCandidateRepository::default());
        let data = GatewayDataState::with_auth_candidate_selection_provider_catalog_and_request_candidate_repository_for_tests(
            Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(Some(format!("{:x}", Sha256::digest(CLIENT_KEY))), auth)])),
            Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(candidates)),
            Arc::clone(&catalog), Arc::clone(&request_candidates), DEVELOPMENT_ENCRYPTION_KEY,
        ).with_routing_group_repository_for_tests(routing);
        let mut state = AppState::new().unwrap().with_data_state_for_tests(data);
        if let Some(limit) = target_limit {
            state.upstream_target_admission =
                Arc::new(crate::upstream_admission::UpstreamTargetAdmission::new(
                    Some(limit),
                    Duration::from_millis(10),
                ));
        }
        let target_admission = Arc::clone(&state.upstream_target_admission);
        let (url, handle) = start_server(build_router_with_state(state)).await;
        servers.0.push(handle);
        Self {
            catalog,
            receipts,
            request_candidates,
            target_admission,
            url,
            client: reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            _servers: servers,
        }
    }

    pub async fn request(&self, stream: bool) -> reqwest::Response {
        self.request_body(
            "/v1/chat/completions",
            json!({"model":"gpt-5","stream":stream,
                "messages":[{"role":"user","content":"local fixture"}]}),
        )
        .await
    }

    pub async fn request_body(&self, path: &str, body: Value) -> reqwest::Response {
        let trace_id = uuid::Uuid::new_v4().to_string();
        let response = self
            .client
            .post(format!("{}{path}", self.url))
            .bearer_auth(CLIENT_KEY)
            .header(crate::constants::TRACE_ID_HEADER, &trace_id)
            .header("x-aether-session-id", "local-stable-session")
            .json(&body)
            .send()
            .await
            .expect("local gateway must respond before client deadline");
        if !response.status().is_success() {
            eprintln!(
                "request candidates: {:#?}",
                self.request_candidates
                    .list_by_request_id(&trace_id)
                    .await
                    .unwrap()
            );
        }
        response
    }

    pub fn targets(&self) -> Vec<usize> {
        self.receipts
            .lock()
            .unwrap()
            .iter()
            .map(|receipt| receipt.target)
            .collect()
    }

    pub async fn target_in_flight(&self, target: usize) -> usize {
        let endpoint = self
            .catalog
            .list_endpoints_by_ids(&[format!("endpoint-{target}")])
            .await
            .unwrap()
            .pop()
            .unwrap();
        let target_key =
            crate::upstream_admission::upstream_target_key_from_url(&endpoint.base_url, None)
                .unwrap();
        self.target_admission
            .snapshot_for_target_key(&target_key)
            .map(|snapshot| snapshot.in_flight)
            .unwrap_or(0)
    }

    pub async fn key(&self, target: usize) -> StoredProviderCatalogKey {
        self.catalog
            .list_keys_by_ids(&[format!("key-{target}")])
            .await
            .unwrap()
            .pop()
            .unwrap()
    }

    pub async fn assert_health(&self, target: usize, expected: f64) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let key = self.key(target).await;
            let score = key
                .health_by_format
                .as_ref()
                .and_then(|health| health["openai:chat"]["health_score"].as_f64())
                .unwrap_or(1.0);
            if (score - expected).abs() < 0.000001 {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "K{target}: expected {expected}, got {score}: {:?}",
                key.health_by_format
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

pub(super) async fn assert_success(response: reqwest::Response) {
    let status = response.status();
    let body = response.text().await.unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(body["choices"][0]["message"]["content"], "local-success");
}
