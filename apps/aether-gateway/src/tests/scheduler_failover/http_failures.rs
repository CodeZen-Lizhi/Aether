use aether_data_contracts::repository::provider_catalog::{
    ProviderCatalogReadRepository, ProviderCatalogWriteRepository,
};
use http::StatusCode;
use serde_json::json;
use std::time::Duration;

use super::{
    run,
    support::{assert_success, Fixture, Reply},
};

#[test]
fn service_unavailable_retry_after_is_shared_with_later_requests() {
    run("503-shared-retry-after", async {
        let fixture = Fixture::new(
            "fixed_order",
            2,
            false,
            [1.0, 1.0],
            90_000,
            [
                vec![
                    Reply::Error(StatusCode::SERVICE_UNAVAILABLE, Some("30")),
                    Reply::Success,
                ],
                vec![Reply::Success, Reply::Success],
            ],
        )
        .await;
        let before = crate::clock::current_unix_secs();
        assert_success(fixture.request(false).await).await;
        assert_eq!(fixture.targets(), [0, 1]);
        fixture.assert_health(0, 0.9).await;
        let key = fixture.key(0).await;
        let until = key.health_by_format.as_ref().unwrap()["openai:chat"]
            ["rate_limit_cooldown_until_unix_secs"]
            .as_u64()
            .unwrap();
        assert!(until >= before + 30);
        tokio::time::sleep(Duration::from_millis(2100)).await;
        assert_success(fixture.request(false).await).await;
        assert_eq!(fixture.targets(), [0, 1, 1]);
        fixture.assert_health(0, 0.9).await;
    });
}

#[test]
fn http_200_error_envelope_retries_and_never_restores_health() {
    run("http-200-error-envelope", async {
        let fixture = Fixture::new(
            "cache_affinity",
            2,
            false,
            [1.0, 1.0],
            90_000,
            [
                vec![
                    Reply::Error(StatusCode::OK, None),
                    Reply::Error(StatusCode::OK, None),
                ],
                vec![Reply::Success, Reply::Success],
            ],
        )
        .await;
        assert_success(fixture.request(false).await).await;
        assert_eq!(fixture.targets(), [0, 0, 1]);
        fixture.assert_health(0, 0.6).await;
        fixture.assert_health(1, 1.0).await;
        assert_success(fixture.request(false).await).await;
        assert_eq!(fixture.targets(), [0, 0, 1, 1]);
        fixture.assert_health(0, 0.6).await;
    });
}

#[test]
fn non_json_http_200_is_a_protocol_failure_and_fails_over() {
    run("non-json-success-rejected", async {
        for (content_type, body) in [
            ("text/html", "<html>maintenance</html>"),
            ("text/plain", "upstream unavailable"),
            ("application/json", ""),
        ] {
            let fixture = Fixture::new(
                "fixed_order",
                2,
                false,
                [1.0, 1.0],
                90_000,
                [
                    vec![
                        Reply::RawSuccess(content_type, body),
                        Reply::RawSuccess(content_type, body),
                    ],
                    vec![Reply::Success],
                ],
            )
            .await;
            assert_success(fixture.request(false).await).await;
            assert_eq!(fixture.targets(), [0, 0, 1]);
            fixture.assert_health(0, 0.6).await;
            fixture.assert_health(1, 1.0).await;
        }
    });
}

#[test]
fn valid_forced_upstream_sse_is_still_accepted_for_a_sync_client() {
    run("valid-forced-sse-sync", async {
        let fixture = Fixture::new(
            "fixed_order",
            2,
            false,
            [1.0, 1.0],
            90_000,
            [vec![Reply::DelayedSse(Duration::ZERO)], vec![]],
        )
        .await;
        let mut endpoint = fixture
            .catalog
            .list_endpoints_by_ids(&["endpoint-0".into()])
            .await
            .unwrap()
            .pop()
            .unwrap();
        endpoint.config = Some(json!({"upstream_stream_policy":"force_stream"}));
        fixture.catalog.update_endpoint(&endpoint).await.unwrap();
        let response = fixture.request(false).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(
            body["choices"][0]["message"]["content"],
            super::support::FIRST_TEXT
        );
        assert_eq!(body["choices"][0]["finish_reason"], "stop");
        assert_eq!(fixture.targets(), [0]);
        fixture.assert_health(0, 1.0).await;
    });
}

#[test]
fn transport_first_byte_timeout_scores_one_from_the_real_sync_route() {
    run("typed-first-byte-timeout", async {
        let fixture = Fixture::new(
            "fixed_order",
            1,
            false,
            [1.0, 1.0],
            90_000,
            [
                vec![Reply::DelayedSse(Duration::from_secs(1))],
                vec![Reply::Success],
            ],
        )
        .await;
        let mut provider = fixture
            .catalog
            .list_providers_by_ids(&["provider-0".into()])
            .await
            .unwrap()
            .pop()
            .unwrap();
        provider.stream_first_byte_timeout_secs = Some(0.05);
        fixture.catalog.update_provider(&provider).await.unwrap();
        let mut endpoint = fixture
            .catalog
            .list_endpoints_by_ids(&["endpoint-0".into()])
            .await
            .unwrap()
            .pop()
            .unwrap();
        endpoint.config = Some(json!({"upstream_stream_policy":"force_stream"}));
        fixture.catalog.update_endpoint(&endpoint).await.unwrap();
        assert_success(fixture.request(false).await).await;
        assert_eq!(fixture.targets(), [0, 1]);
        fixture.assert_health(0, 0.9).await;
        let key = fixture.key(0).await;
        assert_eq!(
            key.health_by_format.unwrap()["openai:chat"]["consecutive_failures"],
            1
        );
    });
}

#[test]
fn upstream_connection_refusal_scores_two_but_local_transport_configuration_is_neutral() {
    run("typed-transport-attribution", async {
        for local_config in [false, true] {
            let fixture = Fixture::new(
                "fixed_order",
                1,
                false,
                [1.0, 1.0],
                90_000,
                [vec![], vec![Reply::Success]],
            )
            .await;
            let listener = crate::test_support::bind_loopback_listener().await.unwrap();
            let unused = listener.local_addr().unwrap();
            drop(listener);
            let mut endpoint = fixture
                .catalog
                .list_endpoints_by_ids(&["endpoint-0".into()])
                .await
                .unwrap()
                .pop()
                .unwrap();
            if local_config {
                // A malformed saved URL is a request-builder configuration failure.
                endpoint.base_url = "http://[invalid".into();
            } else {
                endpoint.base_url = format!("http://{unused}/v1");
            }
            fixture.catalog.update_endpoint(&endpoint).await.unwrap();
            let response = fixture.request(false).await;
            if !local_config {
                assert_success(response).await;
            } else {
                let _ = response.text().await;
            }
            assert!(!fixture.targets().contains(&0));
            fixture
                .assert_health(0, if local_config { 1.0 } else { 0.8 })
                .await;
        }
    });
}

#[test]
fn http_response_reference_is_rejected_before_cross_format_history_expansion() {
    run("http-response-reference", async {
        let fixture = Fixture::new(
            "fixed_order",
            2,
            false,
            [1.0, 1.0],
            90_000,
            [vec![], vec![]],
        )
        .await;
        let response = fixture
            .request_body(
                "/v1/responses",
                json!({
                    "model":"gpt-5", "input":"continue", "previous_response_id":"resp-original-k1"
                }),
            )
            .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert!(response
            .text()
            .await
            .unwrap()
            .contains("original upstream binding"));
        assert!(fixture.targets().is_empty());
        fixture.assert_health(0, 1.0).await;
        fixture.assert_health(1, 1.0).await;
    });
}

#[test]
fn http_due_probe_claims_once_and_complete_success_recovers_one_point() {
    run("http-due-probe", async {
        let fixture = Fixture::new(
            "fixed_order",
            2,
            false,
            [1.0, 1.0],
            90_000,
            [vec![Reply::Success], vec![Reply::Success]],
        )
        .await;
        let mut key = fixture.key(0).await;
        key.health_by_format = Some(json!({"openai:chat":{
            "health_score":0.0, "consecutive_failures":6, "health_policy_version":2
        }}));
        key.circuit_breaker_by_format = Some(json!({"openai:chat":{
            "open":true, "next_probe_at_unix_secs":1, "circuit_epoch":1
        }}));
        assert!(fixture
            .catalog
            .update_key_health_state(
                &key.id,
                key.is_active,
                key.health_by_format.as_ref(),
                key.circuit_breaker_by_format.as_ref()
            )
            .await
            .unwrap());
        assert_eq!(
            fixture.key(0).await.health_by_format.as_ref().unwrap()["openai:chat"]["health_score"],
            0.0
        );
        assert_success(fixture.request(false).await).await;
        assert_eq!(fixture.targets(), [0]);
        fixture.assert_health(0, 0.1).await;
        assert!(fixture
            .key(0)
            .await
            .circuit_breaker_by_format
            .as_ref()
            .and_then(|value| value["openai:chat"].get("half_open_lease"))
            .is_none());
    });
}

#[test]
fn http_sse_probe_keeps_owner_after_headers_and_releases_on_client_cancel() {
    run("http-sse-probe-cancel", async {
        let release = std::sync::Arc::new(tokio::sync::Notify::new());
        let fixture = Fixture::new(
            "fixed_order",
            2,
            false,
            [1.0, 1.0],
            90_000,
            [vec![Reply::BrokenSse(release)], vec![Reply::Success]],
        )
        .await;
        let mut key = fixture.key(0).await;
        key.health_by_format = Some(json!({"openai:chat":{
            "health_score":0.0, "consecutive_failures":6, "health_policy_version":2
        }}));
        key.circuit_breaker_by_format = Some(json!({"openai:chat":{
            "open":true, "next_probe_at_unix_secs":1, "circuit_epoch":1
        }}));
        assert!(fixture
            .catalog
            .update_key_health_state(
                &key.id,
                key.is_active,
                key.health_by_format.as_ref(),
                key.circuit_breaker_by_format.as_ref()
            )
            .await
            .unwrap());
        assert_eq!(
            fixture.key(0).await.health_by_format.as_ref().unwrap()["openai:chat"]["health_score"],
            0.0
        );
        let mut response = fixture.request(true).await;
        assert_eq!(response.status(), StatusCode::OK);
        let mut text = String::new();
        while !text.contains(super::support::FIRST_TEXT) {
            text.push_str(&String::from_utf8_lossy(
                &response.chunk().await.unwrap().unwrap(),
            ));
        }
        assert!(fixture
            .key(0)
            .await
            .circuit_breaker_by_format
            .as_ref()
            .and_then(|value| value["openai:chat"].get("half_open_lease"))
            .is_some());
        assert_success(fixture.request(false).await).await;
        assert_eq!(fixture.targets(), [0, 1]);
        drop(response);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            let key = fixture.key(0).await;
            if key
                .circuit_breaker_by_format
                .as_ref()
                .and_then(|value| value["openai:chat"].get("half_open_lease"))
                .is_none()
            {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "cancelled SSE must release probe owner"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        fixture.assert_health(0, 0.0).await;
    });
}
