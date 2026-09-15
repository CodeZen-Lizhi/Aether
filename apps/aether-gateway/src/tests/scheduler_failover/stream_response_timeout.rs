use std::time::{Duration, Instant};

use aether_data_contracts::repository::candidates::{
    RequestCandidateReadRepository, RequestCandidateStatus,
};
use aether_data_contracts::repository::usage::{StoredRequestUsageAudit, UsageReadRepository};
use http::StatusCode;

use super::run;
use super::support::{assert_success, first_sse_frame, Fixture, Reply, FIRST_TEXT};

fn completed_sse() -> String {
    format!("{}data: {{\"id\":\"chatcmpl-local\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"gpt-5\",\"choices\":[{{\"index\":0,\"delta\":{{}},\"finish_reason\":\"stop\"}}]}}\n\ndata: [DONE]\n\n", first_sse_frame())
}

fn timed_sse(chunks: Vec<(Duration, String)>) -> Reply {
    Reply::TimedSse {
        header_delay: Duration::from_millis(100),
        chunks,
    }
}

async fn fixture(reply: Reply, total_ms: u64) -> Fixture {
    let fixture = Fixture::with_target_limit(
        "fixed_order",
        1,
        false,
        [1.0, 1.0],
        1_800,
        [
            vec![reply, Reply::Success],
            vec![Reply::DelayedSse(Duration::ZERO)],
        ],
        Some(1),
    )
    .await;
    fixture.configure_stream_timeouts(0, 1.0, total_ms).await;
    fixture.configure_stream_timeouts(1, 1.0, 5_000).await;
    fixture
}

async fn terminal(fixture: &Fixture, request_id: &str) -> StoredRequestUsageAudit {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(record) = fixture.usage.find_by_request_id(request_id).await.unwrap() {
                if matches!(record.status.as_str(), "completed" | "failed" | "cancelled") {
                    return record;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("request must reach an authoritative terminal")
}

fn request_id(response: &reqwest::Response) -> String {
    response.headers()[crate::constants::TRACE_ID_HEADER]
        .to_str()
        .unwrap()
        .to_string()
}

#[test]
fn stream_response_timeout_headers_allow_delayed_body_and_heartbeat_then_silence() {
    run("headers-delayed-body", async {
        for heartbeat in [false, true] {
            let mut chunks = Vec::new();
            if heartbeat {
                chunks.push((Duration::ZERO, ": heartbeat\n\n".to_string()));
            }
            chunks.push((Duration::from_secs(3), completed_sse()));
            let fixture = fixture(timed_sse(chunks), 5_000).await;
            let started = Instant::now();
            let response = fixture.request(true).await;
            let id = request_id(&response);
            assert_eq!(response.status(), StatusCode::OK);
            let body = response.text().await.unwrap();
            assert!(body.contains(FIRST_TEXT), "{body}");
            assert!(body.contains("[DONE]"), "{body}");
            assert!(started.elapsed() >= Duration::from_secs(3));
            let usage = terminal(&fixture, &id).await;
            assert_eq!(usage.status, "completed");
            assert_eq!(fixture.targets(), [0]);
            assert_eq!(fixture.target_in_flight(0).await, 0);
        }
    });
}

#[test]
fn stream_response_timeout_headers_without_body_expire_only_at_total_deadline() {
    run("headers-total-timeout", async {
        for heartbeat in [false, true] {
            let mut chunks = Vec::new();
            if heartbeat {
                chunks.push((Duration::from_millis(150), ": heartbeat\n\n".to_string()));
            }
            chunks.push((Duration::from_secs(5), completed_sse()));
            let fixture = fixture(timed_sse(chunks), 1_800).await;
            let started = Instant::now();
            let response = fixture.request(true).await;
            let id = request_id(&response);
            assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
            let body = response.text().await.unwrap();
            assert!(body.contains("request total timeout exceeded"), "{body}");
            assert!(started.elapsed() >= Duration::from_millis(1_700));
            assert!(started.elapsed() < Duration::from_secs(3));
            let usage = terminal(&fixture, &id).await;
            assert_eq!(usage.status, "failed");
            assert_eq!(usage.status_code, Some(504));
            let metadata = usage
                .request_metadata
                .as_ref()
                .expect("timeout metadata must persist");
            let timing = metadata
                .get("stream_timing")
                .expect("timeout must retain observed stages");
            let headers_ms = timing["response_headers_elapsed_ms"]
                .as_u64()
                .expect("successful headers were observed");
            assert!(headers_ms > 0 && headers_ms < 1_700, "{timing}");
            assert!(
                timing["first_effective_output_elapsed_ms"].is_null(),
                "heartbeat is not effective output: {timing}"
            );
            if heartbeat {
                assert!(
                    usage
                        .first_byte_time_ms
                        .is_some_and(|ms| ms >= 150 && ms < 1_000),
                    "precommit heartbeat body time must survive total timeout"
                );
                let first_body_ms = timing["first_body_elapsed_ms"]
                    .as_u64()
                    .expect("precommit body was observed");
                assert!(
                    first_body_ms >= headers_ms && first_body_ms < 1_700,
                    "{timing}"
                );
                assert_eq!(
                    metadata["end_to_end_first_byte_time_ms"].as_u64(),
                    Some(first_body_ms)
                );
            } else {
                assert!(
                    usage.first_byte_time_ms.is_none(),
                    "headers must not become first body time"
                );
                assert!(timing["first_body_elapsed_ms"].is_null(), "{timing}");
            }
            assert!(metadata["end_to_end_time_ms"]
                .as_u64()
                .is_some_and(|ms| ms >= 1_700));
            assert!(usage.response_time_ms.is_some_and(|ms| ms >= 1_700));
            let candidates = fixture
                .request_candidates
                .list_by_request_id(&id)
                .await
                .unwrap();
            let attempt = candidates
                .iter()
                .find(|candidate| candidate.error_type.as_deref() == Some("stream_total_timeout"))
                .unwrap();
            assert_eq!(attempt.status, RequestCandidateStatus::Failed);
            assert_eq!(attempt.status_code, Some(504));
            assert_eq!(fixture.targets(), [0]);
            assert_eq!(fixture.target_in_flight(0).await, 0);
            fixture.assert_health(0, 1.0).await;
            assert_success(fixture.request(false).await).await;
        }
    });
}

#[test]
fn stream_response_timeout_continuous_output_expires_after_response_return() {
    run("body-total-timeout", async {
        let fixture = fixture(
            timed_sse(
                (0..30)
                    .map(|_| (Duration::from_millis(100), first_sse_frame()))
                    .collect(),
            ),
            1_500,
        )
        .await;
        let started = Instant::now();
        let response = fixture.request(true).await;
        let id = request_id(&response);
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "response must return before total deadline"
        );
        let body = response.text().await.unwrap();
        assert!(body.contains(FIRST_TEXT));
        assert!(body.contains("request total timeout exceeded"), "{body}");
        assert!(started.elapsed() >= Duration::from_millis(1_400));
        assert!(started.elapsed() < Duration::from_millis(2_500));
        let usage = terminal(&fixture, &id).await;
        assert_eq!(usage.status, "failed");
        assert_eq!(usage.status_code, Some(504));
        assert!(
            usage
                .first_byte_time_ms
                .is_some_and(|ms| ms > 0 && ms < 1_000),
            "total timeout must retain observed first-body timing"
        );
        assert_eq!(fixture.targets(), [0], "committed stream must not replay");
        let candidates = fixture
            .request_candidates
            .list_by_request_id(&id)
            .await
            .unwrap();
        assert!(candidates
            .iter()
            .any(|candidate| candidate.error_type.as_deref() == Some("stream_total_timeout")));
        assert_eq!(fixture.target_in_flight(0).await, 0);
        fixture.assert_health(0, 1.0).await;
        assert_success(fixture.request(false).await).await;
    });
}

#[test]
fn stream_response_timeout_early_error_retries_but_committed_error_and_eof_do_not() {
    run("headers-error-boundary", async {
        for (committed, error) in [(false, true), (true, true), (true, false)] {
            let mut chunks = Vec::new();
            if committed {
                chunks.push((Duration::ZERO, first_sse_frame()));
            }
            if error {
                chunks.push((Duration::from_millis(100), "data: {\"error\":{\"type\":\"server_error\",\"message\":\"scripted stream failure\"}}\n\n".to_string()));
            }
            let fixture = fixture(timed_sse(chunks), 5_000).await;
            let response = fixture.request(true).await;
            let id = request_id(&response);
            assert_eq!(response.status(), StatusCode::OK);
            let body = response.text().await.unwrap();
            let usage = terminal(&fixture, &id).await;
            if committed {
                assert_eq!(usage.status, "failed", "error={error}");
                assert_eq!(fixture.targets(), [0]);
                if !error {
                    assert!(body.contains("stream_missing_terminal_event"), "{body}");
                }
            } else {
                assert_eq!(usage.status, "completed");
                assert_eq!(fixture.targets(), [0, 1]);
            }
            assert_eq!(fixture.target_in_flight(0).await, 0);
        }
    });
}
