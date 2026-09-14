use std::sync::Arc;
use std::time::Duration;

use aether_data_contracts::repository::provider_catalog::{
    ProviderCatalogReadRepository, ProviderCatalogWriteRepository,
};
use http::StatusCode;
use serde_json::json;
use tokio::sync::Notify;

use super::{
    run,
    support::{assert_success, Fixture, Reply, FIRST_TEXT},
};

#[test]
fn long_sse_probe_renews_then_stops_on_owner_replacement_without_replay() {
    run("long-sse-probe-owner", async {
        let fixture = Fixture::new(
            "fixed_order",
            2,
            false,
            [1.0, 1.0],
            2_000,
            [
                vec![Reply::HeldSse(Arc::new(Notify::new()))],
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
        provider.request_timeout_secs = Some(120.0);
        fixture.catalog.update_provider(&provider).await.unwrap();
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
                key.circuit_breaker_by_format.as_ref(),
            )
            .await
            .unwrap());

        // Only this deliberate long-running scenario extends the fixture's
        // client deadline; all requests still enter the authenticated router.
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap();
        let mut response = client
            .post(format!("{}/v1/chat/completions", fixture.url))
            .bearer_auth(super::support::CLIENT_KEY)
            .json(&json!({"model":"gpt-5","stream":true,
                "messages":[{"role":"user","content":"long local probe"}]}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut text = String::new();
        while !text.contains(FIRST_TEXT) {
            text.push_str(&String::from_utf8_lossy(
                &response.chunk().await.unwrap().unwrap(),
            ));
        }
        let initial = fixture.key(0).await.circuit_breaker_by_format.unwrap();
        let original_owner = initial["openai:chat"]["half_open_lease"].clone();
        let original_expiry = initial["openai:chat"]["half_open_until_unix_secs"]
            .as_u64()
            .unwrap();
        assert!(original_owner.is_object());

        // Cross the actual initial 60-second lease, not a mocked projection.
        // Effective output also keeps the stream alive past its 2-second budget.
        tokio::time::sleep(Duration::from_secs(62)).await;
        let mut key = fixture.key(0).await;
        let circuit = &mut key.circuit_breaker_by_format.as_mut().unwrap()["openai:chat"];
        assert_eq!(circuit["half_open_lease"], original_owner);
        let renewed_expiry = circuit["half_open_until_unix_secs"].as_u64().unwrap();
        assert!(crate::clock::current_unix_secs() >= original_expiry);
        assert!(renewed_expiry > original_expiry);
        assert!(renewed_expiry > crate::clock::current_unix_secs());
        assert_success(fixture.request(false).await).await;
        assert_eq!(fixture.targets(), [0, 1]);

        // Replace the generation while the old upstream is still pending.
        // Its next renewal must stop that stream without scoring or replaying it.
        circuit["circuit_epoch"] = json!(2);
        circuit["half_open_lease"]["circuit_epoch"] = json!(2);
        circuit["half_open_lease"]["owner_token"] = json!("replacement-owner");
        let replacement = circuit["half_open_lease"].clone();
        assert!(fixture
            .catalog
            .update_key_health_state(
                &key.id,
                key.is_active,
                key.health_by_format.as_ref(),
                key.circuit_breaker_by_format.as_ref(),
            )
            .await
            .unwrap());
        let tail = tokio::time::timeout(Duration::from_secs(30), response.text())
            .await
            .expect("lease loss must stop the live response within one renewal interval");
        assert!(
            tail.is_err(),
            "lost ownership must not look like a valid EOF"
        );
        fixture.assert_health(0, 0.0).await;
        assert_eq!(
            fixture.targets(),
            [0, 1],
            "delivered output cannot be replayed"
        );
        assert_eq!(
            fixture.key(0).await.circuit_breaker_by_format.unwrap()["openai:chat"]
                ["half_open_lease"],
            replacement,
            "the old owner's cleanup must preserve the new owner",
        );
    });
}
