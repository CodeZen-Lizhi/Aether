mod http_failures;
mod probe_lifetime;
mod support;
mod target_admission;

use std::sync::Arc;
use std::time::{Duration, Instant};

use http::StatusCode;
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
                    Reply::Error(StatusCode::TOO_MANY_REQUESTS, Some("30")),
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

#[test]
fn configured_first_output_budget_is_shared_across_real_candidates() {
    run("shared-output-budget", async {
        let fixture = Fixture::new(
            "fixed_order",
            1,
            false,
            [1.0, 1.0],
            1100,
            [
                vec![Reply::DelayedError(Duration::from_millis(700))],
                vec![Reply::DelayedSse(Duration::from_millis(900))],
            ],
        )
        .await;
        let started = Instant::now();
        let response = fixture.request(true).await;
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
            elapsed < Duration::from_millis(1450),
            "budget reset or was ignored: {elapsed:?}"
        );
        fixture.assert_health(0, 0.8).await;
        tokio::time::sleep(Duration::from_millis(600)).await;
        fixture.assert_health(1, 1.0).await;
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
