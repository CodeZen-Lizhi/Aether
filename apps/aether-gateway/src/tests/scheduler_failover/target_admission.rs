use std::sync::Arc;
use std::time::Duration;

use http::StatusCode;
use tokio::sync::Notify;

use super::{
    run,
    support::{assert_success, Fixture, Reply, FIRST_TEXT},
};

#[test]
fn active_sse_holds_target_capacity_until_terminal_or_client_cancel() {
    run("sse-target-capacity", async {
        for cancel in [false, true] {
            let release = Arc::new(Notify::new());
            let fixture = Fixture::with_target_limit(
                "fixed_order",
                1,
                false,
                [1.0, 1.0],
                90_000,
                [
                    vec![Reply::HeldSse(release.clone()), Reply::Success],
                    vec![Reply::DelayedSse(Duration::ZERO)],
                ],
                Some(1),
            )
            .await;
            let mut response = fixture.request(true).await;
            assert_eq!(response.status(), StatusCode::OK);
            let mut text = String::new();
            while !text.contains(FIRST_TEXT) {
                text.push_str(&String::from_utf8_lossy(
                    &response.chunk().await.unwrap().unwrap(),
                ));
            }
            assert_eq!(
                fixture.target_in_flight(0).await,
                1,
                "target capacity must remain held after delivered content"
            );
            let fallback = fixture.request(true).await;
            assert_eq!(fallback.status(), StatusCode::OK);
            assert!(fallback.text().await.unwrap().contains("[DONE]"));
            assert_eq!(
                fixture.targets(),
                [0, 1],
                "a second stream must use the backup instead of the occupied target"
            );
            assert_eq!(fixture.target_in_flight(0).await, 1);
            fixture.assert_health(0, 1.0).await;

            if cancel {
                drop(response);
            } else {
                release.notify_one();
                let tail = response.text().await.unwrap();
                assert!(tail.contains("[DONE]"));
            }
            let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
            while fixture.target_in_flight(0).await != 0 {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "terminal/cancel must release target capacity"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert_success(fixture.request(false).await).await;
            assert_eq!(
                fixture.targets(),
                [0, 1, 0],
                "released target must accept the next request"
            );
            fixture.assert_health(0, 1.0).await;
        }
    });
}
