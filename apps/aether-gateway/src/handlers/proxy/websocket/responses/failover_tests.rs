//! Loopback WS evidence for the shared logical-turn policy and real binding.
//! Full authenticated route/health-store integration is tested separately.

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use aether_contracts::ExecutionTimeouts;
use axum::{
    extract::{
        ws::{Message, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};

use super::adapter::{resolve_responses_websocket_adapter, ResponsesWebSocketRebindSafety};
use super::frame::{frame_is_replayable_rejection, ParsedResponsesWebSocketFrame};
use super::state::BoundResponsesConnection;
use super::turn_state::LogicalTurn;
use super::upstream::bind_responses_upstream_before;
use crate::ai_serving::{AiExecutionDecision, ResponsesWebSocketBodyNormalization};
use crate::orchestration::{classify_chat_failure, ChatFailureSource, ResponsesWebSocketAdapter};

struct Script {
    frames: Vec<(Duration, Value)>,
    requests: AtomicUsize,
}

struct LocalUpstream {
    url: String,
    script: Arc<Script>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for LocalUpstream {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn scripted_socket(
    ws: WebSocketUpgrade,
    State(script): State<Arc<Script>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |mut socket| async move {
        let Some(Ok(Message::Text(request))) = socket.next().await else {
            return;
        };
        let request: Value = serde_json::from_str(request.as_str()).unwrap();
        assert_eq!(request["type"], "response.create");
        script.requests.fetch_add(1, Ordering::SeqCst);
        for (delay, event) in &script.frames {
            tokio::time::sleep(*delay).await;
            if socket
                .send(Message::Text(event.to_string().into()))
                .await
                .is_err()
            {
                break;
            }
        }
    })
}

async fn upstream(frames: Vec<(Duration, Value)>) -> LocalUpstream {
    let script = Arc::new(Script {
        frames,
        requests: AtomicUsize::new(0),
    });
    let app = Router::new()
        .route("/v1/responses", get(scripted_socket))
        .with_state(script.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1/responses", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    LocalUpstream {
        url,
        script,
        server,
    }
}

fn request() -> Value {
    json!({"type":"response.create", "model":"test-model"})
}

async fn bind(server: &LocalUpstream, logical: &LogicalTurn) -> BoundResponsesConnection {
    let decision: AiExecutionDecision = serde_json::from_value(json!({
        "action":"local", "upstream_url":server.url, "mapped_model":"test-model",
        "provider_api_format":"openai:responses", "provider_request_body":{"model":"test-model"}
    }))
    .unwrap();
    bind_responses_upstream_before(
        &decision,
        ResponsesWebSocketBodyNormalization::for_tests("test-model"),
        &logical.client_event,
        resolve_responses_websocket_adapter(ResponsesWebSocketAdapter::Standard),
        logical.first_output_deadline.unwrap(),
    )
    .await
    .unwrap()
}

async fn receive(
    bound: &mut BoundResponsesConnection,
    deadline: Option<Instant>,
) -> Result<String, tokio::time::error::Elapsed> {
    let message = tokio::time::timeout_at(
        deadline
            .unwrap_or_else(|| Instant::now() + Duration::from_secs(2))
            .into(),
        bound.upstream.as_mut().unwrap().recv(),
    )
    .await?
    .unwrap()
    .unwrap();
    match message {
        wreq::ws::message::Message::Text(text) => Ok(text.to_string()),
        other => panic!("unexpected WS frame: {other:?}"),
    }
}

#[tokio::test]
async fn local_ws_rejections_use_two_attempts_then_backup() {
    let failed = upstream(vec![(
        Duration::ZERO,
        json!({"type":"error", "status_code":500, "error":{"code":"server_error"}}),
    )])
    .await;
    let backup = upstream(vec![(
        Duration::ZERO,
        json!({"type":"response.completed", "response":{"id":"resp_backup", "status":"completed"}}),
    )])
    .await;
    let mut logical = LogicalTurn::new(request(), 1, "logical".into()).with_failover_budget(
        Instant::now(),
        None,
        Some("K1"),
    );
    let mut order = Vec::new();
    loop {
        let mut bound = bind(&failed, &logical).await;
        order.push("K1");
        let text = receive(&mut bound, logical.first_output_deadline)
            .await
            .unwrap();
        let frame = ParsedResponsesWebSocketFrame::parse(&text).unwrap();
        assert!(frame_is_replayable_rejection(&frame));
        assert!(classify_chat_failure(
            frame.status().unwrap(),
            Some(&text),
            ChatFailureSource::UpstreamResponse,
            false
        )
        .retryable());
        if !logical.reserve_same_key_retry("K1", 2, Duration::from_millis(500)) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        logical.record_attempt("K1");
    }
    assert!(logical.excluded_keys.contains("K1"));
    let mut bound = bind(&backup, &logical).await;
    order.push("K2");
    let text = receive(&mut bound, logical.first_output_deadline)
        .await
        .unwrap();
    logical.observe_effective_output(&ParsedResponsesWebSocketFrame::parse(&text).unwrap());
    assert_eq!(order, ["K1", "K1", "K2"]);
    assert_eq!(failed.script.requests.load(Ordering::SeqCst), 2);
    assert_eq!(backup.script.requests.load(Ordering::SeqCst), 1);
    assert_eq!(logical.first_output_deadline, None);
}

#[tokio::test]
async fn local_ws_retry_preserves_first_output_deadline_and_created_is_not_output() {
    let server = upstream(vec![
        (Duration::ZERO, json!({"type":"response.created"})),
        (
            Duration::from_millis(500),
            json!({"type":"response.output_text.delta", "delta":"late"}),
        ),
    ])
    .await;
    let timeouts = ExecutionTimeouts {
        stream_failover_budget_ms: Some(150),
        ..Default::default()
    };
    let mut logical = LogicalTurn::new(request(), 1, "logical".into()).with_failover_budget(
        Instant::now(),
        Some(&timeouts),
        Some("K1"),
    );
    let original = logical.first_output_deadline;
    let mut first = bind(&server, &logical).await;
    let text = receive(&mut first, original).await.unwrap();
    logical.observe_effective_output(&ParsedResponsesWebSocketFrame::parse(&text).unwrap());
    assert_eq!(logical.first_output_deadline, original);
    // Physical replacement cannot restart a logical deadline.
    let mut second = bind(&server, &logical).await;
    receive(&mut second, original).await.unwrap();
    assert!(receive(&mut second, logical.first_output_deadline)
        .await
        .is_err());
    assert_eq!(
        logical.retry_block_reason(),
        Some("stream_failover_budget_exhausted")
    );
}

#[tokio::test]
async fn local_ws_valid_output_releases_only_the_first_output_budget() {
    let server = upstream(vec![(Duration::ZERO, json!({"type":"response.output_text.delta", "delta":"hello"})),
        (Duration::from_millis(200), json!({"type":"response.completed", "response":{"id":"resp_long", "status":"completed"}}))]).await;
    let timeouts = ExecutionTimeouts {
        stream_failover_budget_ms: Some(150),
        ..Default::default()
    };
    let mut logical = LogicalTurn::new(request(), 1, "logical".into()).with_failover_budget(
        Instant::now(),
        Some(&timeouts),
        Some("K1"),
    );
    let original = logical.first_output_deadline.unwrap();
    let mut bound = bind(&server, &logical).await;
    let first = receive(&mut bound, logical.first_output_deadline)
        .await
        .unwrap();
    logical.observe_effective_output(&ParsedResponsesWebSocketFrame::parse(&first).unwrap());
    assert_eq!(logical.first_output_deadline, None);
    let terminal = receive(&mut bound, logical.first_output_deadline)
        .await
        .unwrap();
    assert!(ParsedResponsesWebSocketFrame::parse(&terminal)
        .unwrap()
        .is_terminal());
    assert!(Instant::now() > original);
}

#[tokio::test]
async fn local_ws_pinned_and_delivered_tool_state_block_replay() {
    let server = upstream(vec![(Duration::ZERO, json!({"type":"response.output_item.added", "item":{"type":"function_call", "id":"item_1", "call_id":"call_1"}})),
        (Duration::ZERO, json!({"type":"error", "status_code":500}))]).await;
    let mut logical = LogicalTurn::new(request(), 1, "tool".into());
    let mut bound = bind(&server, &logical).await;
    let tool = receive(&mut bound, logical.first_output_deadline)
        .await
        .unwrap();
    let frame = ParsedResponsesWebSocketFrame::parse(&tool).unwrap();
    if let ResponsesWebSocketRebindSafety::Unsafe { reason } = bound
        .adapter
        .rebind_safety_for_upstream_event(frame.event())
    {
        logical.mark_retry_unsafe(reason);
    }
    let failure = receive(&mut bound, logical.first_output_deadline)
        .await
        .unwrap();
    assert!(frame_is_replayable_rejection(
        &ParsedResponsesWebSocketFrame::parse(&failure).unwrap()
    ));
    assert!(!logical.reserve_same_key_retry("K1", 2, Duration::ZERO));
    let pinned = LogicalTurn::new(
        json!({"type":"response.create", "model":"test-model", "previous_response_id":"resp_original"}),
        2,
        "pinned".into(),
    );
    assert_eq!(pinned.retry_block_reason(), Some("previous_response_id"));
    assert_eq!(server.script.requests.load(Ordering::SeqCst), 1);
}

#[test]
fn same_key_wait_is_cumulative_and_long_retry_after_is_not_shortened() {
    let mut logical = LogicalTurn::new(request(), 1, "logical".into());
    assert!(logical.reserve_same_key_retry("K1", 99, Duration::from_millis(750)));
    logical.record_attempt("K1");
    assert!(logical.reserve_same_key_retry("K1", 99, Duration::from_millis(1_000)));
    logical.record_attempt("K1");
    assert!(!logical.reserve_same_key_retry("K1", 99, Duration::from_millis(500)));
    assert!(!logical.reserve_same_key_retry("K2", 99, Duration::from_secs(30)));
    assert_eq!(
        logical.retry_wait_by_key["K1"],
        Duration::from_millis(1_750)
    );
    assert_eq!(logical.retry_wait_by_key["K2"], Duration::ZERO);
}
