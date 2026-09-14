//! Authenticated gateway WS routing and persisted per-attempt health smoke test.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use aether_crypto::{encrypt_python_fernet_plaintext, DEVELOPMENT_ENCRYPTION_KEY};
use aether_data::repository::auth::{
    InMemoryAuthApiKeySnapshotRepository, StoredAuthApiKeySnapshot,
};
use aether_data::repository::candidate_selection::InMemoryMinimalCandidateSelectionReadRepository;
use aether_data::repository::candidates::InMemoryRequestCandidateRepository;
use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
use aether_data::repository::routing_profiles::InMemoryRoutingGroupRepository;
use aether_data::repository::usage::InMemoryUsageReadRepository;
use aether_data_contracts::repository::candidate_selection::StoredMinimalCandidateSelectionRow;
use aether_data_contracts::repository::provider_catalog::{
    ProviderCatalogReadRepository, StoredProviderCatalogEndpoint, StoredProviderCatalogKey,
    StoredProviderCatalogProvider,
};
use aether_data_contracts::repository::routing_profiles::StoredRoutingGroup;
use axum::extract::ws::{Message, WebSocketUpgrade};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use wreq::ws::message::Message as UpstreamMessage;

use crate::data::GatewayDataState;
use crate::{build_router_with_state, AppState};

const FORMAT: &str = "openai:responses";
const CLIENT_KEY: &str = "sk-local-ws-smoke";

struct Servers(Vec<tokio::task::JoinHandle<()>>);

impl Drop for Servers {
    fn drop(&mut self) {
        for server in &self.0 {
            server.abort();
        }
    }
}

async fn upstream(
    ws: WebSocketUpgrade,
    State(calls): State<Arc<Mutex<Vec<String>>>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let key = headers[http::header::AUTHORIZATION]
        .to_str()
        .unwrap()
        .to_string();
    ws.on_upgrade(move |mut socket| async move {
        while let Some(Ok(Message::Text(text))) = socket.next().await {
            let request: Value = serde_json::from_str(text.as_str()).unwrap();
            assert_eq!(request["type"], "response.create");
            assert_eq!(request["model"], "local-model");
            assert!(request.get("previous_response_id").is_none());
            calls.lock().unwrap().push(key.clone());
            let frames = if key == "Bearer sk-upstream-0" {
                vec![json!({"type":"error", "status_code":500,
                    "error":{"type":"server_error", "message":"scripted local failure"}})]
            } else {
                assert_eq!(key, "Bearer sk-upstream-1");
                vec![
                    json!({"type":"response.output_text.delta", "response_id":"resp_smoke",
                        "item_id":"msg_smoke", "output_index":0, "content_index":0, "delta":"local success"}),
                    json!({"type":"response.completed", "response":{
                        "id":"resp_smoke", "object":"response", "status":"completed", "model":"local-model",
                        "output":[{"id":"msg_smoke", "type":"message", "role":"assistant", "status":"completed",
                            "content":[{"type":"output_text", "text":"local success", "annotations":[]}]}],
                        "usage":{"input_tokens":1, "output_tokens":2, "total_tokens":3}}}),
                ]
            };
            for frame in frames {
                if socket.send(Message::Text(frame.to_string().into())).await.is_err() {
                    return;
                }
            }
        }
    })
}

fn state(base_url: &str) -> (AppState, Arc<InMemoryProviderCatalogReadRepository>) {
    let provider = StoredProviderCatalogProvider::new(
        "provider-ws".into(), "local-ws".into(), None, "custom".into(),
    ).unwrap().with_transport_fields(true, false, None, None, None, Some(10.0), Some(5.0),
        Some(json!({"responses_websocket":{"enabled":true},
            "failover_rules":{"chat_policy_version":1, "max_attempts":2, "stream_failover_budget_ms":90000}})));
    let endpoint = StoredProviderCatalogEndpoint::new(
        "endpoint-ws".into(),
        "provider-ws".into(),
        FORMAT.into(),
        Some("openai".into()),
        Some("responses".into()),
        true,
    )
    .unwrap()
    .with_transport_fields(
        format!("{base_url}/v1"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    let mut keys = Vec::new();
    let mut candidates = Vec::new();
    for index in 0..2 {
        let key_id = format!("key-ws-{index}");
        keys.push(StoredProviderCatalogKey::new(
            key_id.clone(), "provider-ws".into(), key_id.clone(), "api_key".into(), None, true,
        ).unwrap().with_transport_fields(Some(json!([FORMAT])),
            encrypt_python_fernet_plaintext(DEVELOPMENT_ENCRYPTION_KEY, &format!("sk-upstream-{index}")).unwrap(),
            None, None, None, None, None, None,
        ).unwrap().with_internal_priority(index).with_health_fields(
            Some(json!({FORMAT:{"health_score":if index == 0 {1.0} else {0.6}, "consecutive_failures":0}})), None,
        ));
        candidates.push(StoredMinimalCandidateSelectionRow {
            provider_id: "provider-ws".into(),
            provider_name: "local-ws".into(),
            provider_type: "custom".into(),
            provider_is_active: true,
            endpoint_id: "endpoint-ws".into(),
            endpoint_api_format: FORMAT.into(),
            endpoint_api_family: Some("openai".into()),
            endpoint_kind: Some("responses".into()),
            endpoint_is_active: true,
            key_id: key_id.clone(),
            key_name: key_id,
            key_auth_type: "api_key".into(),
            key_is_active: true,
            key_api_formats: Some(vec![FORMAT.into()]),
            key_allowed_models: None,
            key_capabilities: None,
            model_id: "model-ws".into(),
            global_model_id: "global-ws".into(),
            global_model_name: "local-model".into(),
            global_model_supports_streaming: Some(true),
            model_provider_model_name: "local-model".into(),
            model_provider_model_mappings: None,
            model_supports_streaming: Some(true),
            model_is_active: true,
            model_is_available: true,
        });
    }
    let auth = StoredAuthApiKeySnapshot::new(
        "user-ws".into(),
        "local".into(),
        None,
        "user".into(),
        "local".into(),
        true,
        false,
        None,
        Some(json!([FORMAT])),
        Some(json!(["local-model"])),
        "client-key-ws".into(),
        Some("default".into()),
        true,
        false,
        false,
        Some(6000),
        Some(100),
        Some(4_102_444_800),
        None,
        Some(json!([FORMAT])),
        Some(json!(["local-model"])),
    )
    .unwrap();
    let catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![provider],
        vec![endpoint],
        keys,
    ));
    let routing = Arc::new(InMemoryRoutingGroupRepository::seed(
        vec![StoredRoutingGroup {
            id: "routing-ws".into(),
            name: "default".into(),
            description: None,
            enabled: true,
            is_system_default: true,
            config_json: json!({"default_policy":{"scheduling_mode":"fixed_order"}, "rules":[]}),
            version: 1,
            created_at: 1,
            updated_at: 1,
            published_at: None,
        }],
        vec![],
        vec![],
    ));
    let data = GatewayDataState::with_auth_candidate_selection_provider_catalog_request_candidates_and_usage_for_tests(
        Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(Some(format!("{:x}", Sha256::digest(CLIENT_KEY))), auth)])),
        Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(candidates)), catalog.clone(),
        Arc::new(InMemoryRequestCandidateRepository::default()), Arc::new(InMemoryUsageReadRepository::default()),
        DEVELOPMENT_ENCRYPTION_KEY,
    ).with_routing_group_repository_for_tests(routing);
    (
        AppState::new().unwrap().with_data_state_for_tests(data),
        catalog,
    )
}

#[test]
fn authenticated_ws_gateway_retries_then_backup_and_settles_each_key() {
    // Match the real-router suite's stack size; its nested planner futures are large.
    let thread = std::thread::Builder::new()
        .name("ws-route-health-smoke".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async { tokio::time::timeout(Duration::from_secs(15), smoke()).await })
                .expect("WS gateway smoke must terminate within its client deadline");
        })
        .unwrap();
    if let Err(panic) = thread.join() {
        std::panic::resume_unwind(panic);
    }
}

async fn smoke() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let (upstream_url, upstream_server) = crate::tests::start_server(
        Router::new()
            .route("/v1/responses", get(upstream))
            .with_state(calls.clone()),
    )
    .await;
    let mut servers = Servers(vec![upstream_server]);
    let (state, catalog) = state(&upstream_url);
    let (gateway_url, gateway_server) =
        crate::tests::start_server(build_router_with_state(state)).await;
    servers.0.push(gateway_server);
    let response = wreq::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .websocket(format!(
            "{}/v1/responses",
            gateway_url.replacen("http://", "ws://", 1)
        ))
        .header(http::header::AUTHORIZATION, format!("Bearer {CLIENT_KEY}"))
        .header("x-aether-session-id", "local-ws-smoke-session")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 101);
    let mut socket = response.into_websocket().await.unwrap();
    socket
        .send(UpstreamMessage::Text(
            json!({"type":"response.create", "model":"local-model",
        "input":[{"role":"user", "content":"local fixture"}]})
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
    let mut visible = Vec::new();
    loop {
        let Some(Ok(UpstreamMessage::Text(text))) = socket.recv().await else {
            panic!("gateway closed before the successful terminal; observed {visible:?}");
        };
        let event: Value = serde_json::from_str(text.as_str()).unwrap();
        assert_ne!(
            event["type"], "error",
            "retryable upstream error leaked: {event}"
        );
        let completed = event["type"] == "response.completed";
        visible.push(event);
        if completed {
            break;
        }
    }
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        [
            "Bearer sk-upstream-0",
            "Bearer sk-upstream-0",
            "Bearer sk-upstream-1"
        ]
    );
    assert!(visible
        .iter()
        .any(|event| event["delta"] == "local success"));
    // Client delivery precedes bounded asynchronous terminal persistence.
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let keys = catalog
                .list_keys_by_ids(&["key-ws-0".into(), "key-ws-1".into()])
                .await
                .unwrap();
            let score = |id: &str| {
                keys.iter()
                    .find(|key| key.id == id)
                    .unwrap()
                    .health_by_format
                    .as_ref()
                    .and_then(|health| health[FORMAT]["health_score"].as_f64())
                    .unwrap()
            };
            if (score("key-ws-0") - 0.6).abs() < 1e-9 && (score("key-ws-1") - 0.7).abs() < 1e-9 {
                let first = keys.iter().find(|key| key.id == "key-ws-0").unwrap();
                assert_eq!(
                    first.health_by_format.as_ref().unwrap()[FORMAT]["consecutive_failures"],
                    2
                );
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("K1 must persist two failures (0.6), K2 one complete success (0.7)");
    let _ = socket.send(UpstreamMessage::Close(None)).await;
}
