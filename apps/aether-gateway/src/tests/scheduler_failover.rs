mod http_failures;
mod probe_lifetime;
mod stream_response_timeout;
mod support;
mod target_admission;

use std::sync::Arc;
use std::time::{Duration, Instant};

use aether_data_contracts::repository::candidates::{
    RequestCandidateReadRepository, RequestCandidateStatus,
};
use aether_data_contracts::repository::provider_catalog::ProviderCatalogWriteRepository;
use aether_data_contracts::repository::usage::UsageReadRepository;
use http::StatusCode;
use serde_json::json;
use tokio::sync::Notify;

use support::{assert_success, Fixture, Reply, FIRST_TEXT};

// Real router futures need the same larger stack as the existing chat route suite.
fn run(name: &'static str, future: impl std::future::Future<Output = ()> + Send + 'static) {
    let thread = std::thread::Builder::new()
        .name(name.into())
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(future)
        })
        .unwrap();
    if let Err(panic) = thread.join() {
        std::panic::resume_unwind(panic);
    }
}

async fn failed_usage(
    fixture: &Fixture,
    request_id: &str,
) -> aether_data_contracts::repository::usage::StoredRequestUsageAudit {
    // Terminal seeds use the existing ordered background persistence dispatcher.
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(usage) = fixture.usage.find_by_request_id(request_id).await.unwrap() {
                if usage.status == "failed" {
                    return usage;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timeout usage must promptly leave pending, without maintenance cleanup")
}

fn server_error() -> Reply {
    Reply::Error(StatusCode::INTERNAL_SERVER_ERROR, None)
}

#[test]
fn explicit_and_legacy_two_attempts_reach_real_upstreams_and_persist_six_points() {
    run("two-attempts", async {
        for legacy in [false, true] {
            let fixture = Fixture::new(
                "fixed_order",
                2,
                legacy,
                [1.0, 1.0],
                90_000,
                [vec![server_error(), server_error()], vec![Reply::Success]],
            )
            .await;
            assert_success(fixture.request(false).await).await;
            assert_eq!(fixture.targets(), [0, 0, 1], "legacy={legacy}");
            fixture.assert_health(0, 0.6).await;
            fixture.assert_health(1, 1.0).await;
            let key = fixture.key(0).await;
            assert_eq!(
                key.health_by_format.unwrap()["openai:chat"]["consecutive_failures"],
                2
            );
            let receipts = fixture.receipts.lock().unwrap();
            for receipt in receipts.iter() {
                assert_eq!(receipt.body["model"], "gpt-5");
                assert_eq!(receipt.body["messages"][0]["content"], "local fixture");
            }
        }
    });
}

#[test]
fn routing_modes_control_next_request_after_backup_success() {
    run("routing-modes", async {
        for (mode, expected_next) in [("fixed_order", 0), ("cache_affinity", 1), ("cost_based", 1)]
        {
            let fixture = Fixture::new(
                mode,
                2,
                false,
                [1.0, 1.0],
                90_000,
                [
                    vec![server_error(), server_error(), Reply::Success],
                    vec![Reply::Success, Reply::Success],
                ],
            )
            .await;
            assert_success(fixture.request(false).await).await;
            assert_eq!(fixture.targets(), [0, 0, 1], "first request: {mode}");
            fixture.assert_health(0, 0.6).await;
            assert_success(fixture.request(false).await).await;
            assert_eq!(
                fixture.targets(),
                [0, 0, 1, expected_next],
                "next request: {mode}"
            );
            fixture
                .assert_health(0, if expected_next == 0 { 0.7 } else { 0.6 })
                .await;
        }
    });
}

#[test]
fn cost_mode_prefers_lower_multiplier_over_manual_order_and_backup_affinity() {
    run("cost-order", async {
        let cheaper_backup = Fixture::new(
            "cost_based",
            2,
            false,
            [2.0, 1.0],
            90_000,
            [vec![Reply::Success], vec![Reply::Success]],
        )
        .await;
        assert_success(cheaper_backup.request(false).await).await;
        assert_eq!(cheaper_backup.targets(), [1]);

        let fixture = Fixture::new(
            "cost_based",
            2,
            false,
            [1.0, 2.0],
            90_000,
            [
                vec![server_error(), server_error(), Reply::Success],
                vec![Reply::Success, Reply::Success],
            ],
        )
        .await;
        assert_success(fixture.request(false).await).await;
        assert_eq!(fixture.targets(), [0, 0, 1]);
        assert_success(fixture.request(false).await).await;
        assert_eq!(
            fixture.targets(),
            [0, 0, 1, 0],
            "cheaper K must outrank successful backup affinity"
        );
    });
}

#[test]
fn short_retry_after_retries_same_key_and_long_cooldown_survives_next_request() {
    run("retry-after", async {
        let short = Fixture::new(
            "fixed_order",
            2,
            false,
            [1.0, 1.0],
            90_000,
            [
                vec![
                    Reply::Error(StatusCode::TOO_MANY_REQUESTS, Some("1")),
                    Reply::Success,
                ],
                vec![Reply::Success],
            ],
        )
        .await;
        assert_success(short.request(false).await).await;
        assert_eq!(short.targets(), [0, 0]);
        short.assert_health(0, 1.0).await;
        {
            let receipts = short.receipts.lock().unwrap();
            let wait = receipts[1].at.duration_since(receipts[0].at);
            assert!(
                wait >= Duration::from_millis(900),
                "retry shortened upstream Retry-After: {wait:?}"
            );
            assert!(
                wait < Duration::from_secs(3),
                "short cooldown exceeded request wait budget: {wait:?}"
            );
        }

        let long = Fixture::new(
            "fixed_order",
            2,
            false,
            [1.0, 1.0],
            90_000,
            [
                vec![
                    Reply::Error(StatusCode::TOO_MANY_REQUESTS, Some("3600")),
                    Reply::Success,
                ],
                vec![Reply::Success, Reply::Success],
            ],
        )
        .await;
        let started = Instant::now();
        assert_success(long.request(false).await).await;
        assert!(started.elapsed() < Duration::from_secs(3));
        assert_eq!(long.targets(), [0, 1]);
        long.assert_health(0, 0.9).await;
        let key = long.key(0).await;
        let cooldown_until = key.health_by_format.as_ref().unwrap()["openai:chat"]
            ["rate_limit_cooldown_until_unix_secs"]
            .as_u64()
            .unwrap();
        let last_failure_at = chrono::DateTime::parse_from_rfc3339(
            key.health_by_format.as_ref().unwrap()["openai:chat"]["last_failure_at"]
                .as_str()
                .unwrap(),
        )
        .unwrap()
        .timestamp() as u64;
        assert!(cooldown_until <= last_failure_at + 600);
        assert!(cooldown_until > last_failure_at);
        tokio::time::sleep(Duration::from_millis(2100)).await;
        assert_success(long.request(false).await).await;
        assert_eq!(
            long.targets(),
            [0, 1, 1],
            "long cooldown must not be clipped to two seconds"
        );
        long.assert_health(0, 0.9).await;
    });
}

/// 真实上游 429 之后，下一次请求只命本地冷却时不应伪造上游响应、503 或上游耗时。
#[test]
fn rate_limit_cooldown_rejects_locally_without_upstream_attempt_or_response() {
    run("rate-limit-no-attempt", async {
        let fixture = Fixture::new(
            "fixed_order",
            2,
            false,
            [1.0, 1.0],
            90_000,
            [
                vec![Reply::Error(StatusCode::TOO_MANY_REQUESTS, Some("600"))],
                vec![Reply::Success],
            ],
        )
        .await;

        let first_request_id = "trace-rate-limit-first";
        let first = fixture
            .request_body_with_trace_id(
                "/v1/chat/completions",
                json!({"model":"gpt-5","messages":[]}),
                first_request_id,
            )
            .await;
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(fixture.targets(), [0, 1]);
        fixture.assert_health(0, 0.9).await;

        let mut primary = fixture.key(0).await;
        let mut backup = fixture.key(1).await;
        let cooldown_until_unix_secs = crate::clock::current_unix_secs() + 600;
        primary.health_by_format = Some(json!({
            "openai:chat": {
                "health_score": 0.9,
                "consecutive_failures": 1,
                "last_failure_at": "2026-10-26T12:32:27Z",
                "rate_limit_cooldown_started_at_unix_secs": cooldown_until_unix_secs - 600,
                "rate_limit_cooldown_until_unix_secs": cooldown_until_unix_secs,
                "consecutive_rate_limits": 1
            }
        }));
        backup.health_by_format = primary.health_by_format.clone();
        assert!(fixture
            .catalog
            .update_key_health_state(
                primary.id.as_str(),
                primary.is_active,
                primary.health_by_format.as_ref(),
                primary.circuit_breaker_by_format.as_ref(),
            )
            .await
            .expect("primary health should update"));
        assert!(fixture
            .catalog
            .update_key_health_state(
                backup.id.as_str(),
                backup.is_active,
                backup.health_by_format.as_ref(),
                backup.circuit_breaker_by_format.as_ref(),
            )
            .await
            .expect("backup health should update"));

        let second_request_id = "trace-rate-limit-second";
        let started = Instant::now();
        let second = fixture
            .request_body_with_trace_id(
                "/v1/chat/completions",
                json!({"model":"gpt-5","messages":[]}),
                second_request_id,
            )
            .await;
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            fixture.targets(),
            [0, 1],
            "cooling keys must not reach upstream"
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        let retry_after = second
            .headers()
            .get(http::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .expect("local cooldown must expose retry-after");
        assert!((1..=600).contains(&retry_after));
        let response_body: serde_json::Value = second
            .json()
            .await
            .expect("local cooldown body should parse");
        assert_eq!(response_body["error"]["type"], "rate_limit_exceeded");

        let candidates = fixture
            .request_candidates
            .list_by_request_id(second_request_id)
            .await
            .expect("second request candidates should read");
        assert!(!candidates.is_empty());
        assert!(candidates.iter().all(|candidate| {
            candidate.status == RequestCandidateStatus::Skipped
                && candidate.skip_reason.as_deref() == Some("key_rate_limit_cooldown")
                && candidate.status_code.is_none()
        }));
        assert!(candidates.iter().all(|candidate| {
            candidate
                .extra_data
                .as_ref()
                .and_then(|value| value.get("rate_limit_cooldown_until_unix_secs"))
                .and_then(serde_json::Value::as_u64)
                .is_some()
        }));

        let usage = failed_usage(&fixture, second_request_id).await;
        assert_eq!(
            usage.status_code,
            Some(StatusCode::TOO_MANY_REQUESTS.as_u16())
        );
        assert_eq!(
            usage.routing_local_execution_runtime_miss_reason(),
            Some("key_rate_limit_cooldown")
        );
        assert!(usage.response_headers.is_none());
        assert!(usage.response_body.is_none());
        fixture.assert_health(0, 0.9).await;
        fixture.assert_health(1, 0.9).await;
        assert_eq!(
            usage
                .request_metadata
                .as_ref()
                .and_then(|metadata| metadata.get("rate_limit_cooldown_until_unix_secs"))
                .and_then(serde_json::Value::as_u64),
            Some(cooldown_until_unix_secs)
        );
        assert!(usage.response_time_ms.is_some_and(|ms| ms < 2_000));
    });
}

#[test]
fn stream_response_timeout_total_deadline_is_shared_across_real_candidates() {
    run("shared-output-budget", async {
        let fixture = Fixture::new(
            "fixed_order",
            1,
            false,
            [1.0, 1.0],
            3000,
            [
                vec![Reply::DelayedError(Duration::from_millis(1400))],
                vec![Reply::DelayedSse(Duration::from_millis(2000))],
            ],
        )
        .await;
        fixture.configure_stream_timeouts(0, 5.0, 3_000).await;
        fixture.configure_stream_timeouts(1, 5.0, 5_000).await;
        let started = Instant::now();
        let response = fixture.request(true).await;
        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
        let request_id = response.headers()[crate::constants::TRACE_ID_HEADER]
            .to_str()
            .unwrap()
            .to_string();
        let body = response.text().await.unwrap_or_default();
        let elapsed = started.elapsed();
        assert_eq!(
            fixture.targets(),
            [0, 1],
            "both upstreams must actually receive a request"
        );
        assert!(
            !body.contains(FIRST_TEXT),
            "K2 output arrived beyond the original shared deadline: {body}"
        );
        assert!(
            elapsed < Duration::from_millis(3800),
            "budget reset or was ignored: {elapsed:?}"
        );
        assert!(body.contains("request total timeout exceeded"), "{body}");
        let usage = failed_usage(&fixture, &request_id).await;
        assert_eq!(usage.status, "failed");
        assert_eq!(usage.status_code, Some(504));
        assert!(usage
            .error_message
            .as_deref()
            .unwrap()
            .contains("request total timeout exceeded"));
        assert!(usage
            .response_time_ms
            .is_some_and(|ms| (2900..3800).contains(&ms)));
        let candidates = fixture
            .request_candidates
            .list_by_request_id(&request_id)
            .await
            .unwrap();
        let cancelled = candidates
            .iter()
            .find(|candidate| candidate.error_type.as_deref() == Some("stream_total_timeout"))
            .unwrap();
        assert_eq!(cancelled.status, RequestCandidateStatus::Failed);
        assert_eq!(cancelled.status_code, Some(504));
        assert_eq!(fixture.target_in_flight(1).await, 0);
        fixture.assert_health(0, 0.8).await;
        tokio::time::sleep(Duration::from_millis(600)).await;
        fixture.assert_health(1, 1.0).await;
        let later = fixture
            .usage
            .find_by_request_id(&request_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(later.status, "failed");
        assert_eq!(later.response_time_ms, usage.response_time_ms);
    });
}

#[test]
fn stream_response_timeout_without_headers_keeps_timeout_and_releases_admission() {
    run("exhausted-output-timeout", async {
        use aether_data_contracts::repository::provider_catalog::{
            ProviderCatalogReadRepository, ProviderCatalogWriteRepository,
        };
        let fixture = Fixture::new(
            "fixed_order",
            1,
            false,
            [1.0, 1.0],
            10_000,
            [
                vec![Reply::DelayedSse(Duration::from_secs(2)), Reply::Success],
                vec![Reply::DelayedSse(Duration::from_secs(2)), Reply::Success],
            ],
        )
        .await;
        for target in 0..2 {
            let mut provider = fixture
                .catalog
                .list_providers_by_ids(&[format!("provider-{target}")])
                .await
                .unwrap()
                .pop()
                .unwrap();
            provider.stream_first_byte_timeout_secs = Some(0.15);
            fixture.catalog.update_provider(&provider).await.unwrap();
        }
        let response = fixture.request(true).await;
        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
        let request_id = response.headers()[crate::constants::TRACE_ID_HEADER]
            .to_str()
            .unwrap()
            .to_string();
        let body = response.text().await.unwrap();
        assert!(body.contains("first response timeout"), "{body}");
        assert!(!body.contains("no_local_stream_plans"), "{body}");
        assert_eq!(fixture.targets(), [0, 1]);
        let usage = failed_usage(&fixture, &request_id).await;
        assert_eq!(usage.status, "failed");
        assert_eq!(usage.status_code, Some(504));
        assert!(usage
            .error_message
            .as_deref()
            .unwrap()
            .contains("first response timeout"));
        fixture.assert_health(0, 0.9).await;
        fixture.assert_health(1, 0.9).await;
        assert_eq!(fixture.target_in_flight(0).await, 0);
        assert_eq!(fixture.target_in_flight(1).await, 0);
        assert_success(fixture.request(false).await).await;
    });
}

#[test]
fn delivered_sse_text_is_not_replayed_after_upstream_disconnect() {
    run("committed-sse", async {
        let release = Arc::new(Notify::new());
        let fixture = Fixture::new(
            "fixed_order",
            2,
            false,
            [1.0, 1.0],
            90_000,
            [
                vec![Reply::BrokenSse(Arc::clone(&release)), Reply::Success],
                vec![Reply::Success],
            ],
        )
        .await;
        let mut response = fixture.request(true).await;
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = Vec::new();
        loop {
            let chunk = response
                .chunk()
                .await
                .expect("pre-fault chunk must read")
                .expect("SSE text must arrive");
            body.extend_from_slice(&chunk);
            if String::from_utf8_lossy(&body).contains(FIRST_TEXT) {
                break;
            }
        }
        release.notify_one();
        while let Ok(Some(chunk)) = response.chunk().await {
            body.extend_from_slice(&chunk);
        }
        let body = String::from_utf8_lossy(&body);
        assert_eq!(body.matches(FIRST_TEXT).count(), 1);
        assert!(!body.contains("local-success"));
        fixture.assert_health(0, 0.8).await;
        assert_eq!(
            fixture.targets(),
            [0],
            "committed output must prevent same-K retry and cross-K replay"
        );
    });
}
