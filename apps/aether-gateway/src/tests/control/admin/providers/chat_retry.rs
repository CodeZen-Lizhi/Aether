use super::*;

async fn request_json(
    state: &AppState,
    method: http::Method,
    path: &str,
    body: Option<serde_json::Value>,
    expected_status: StatusCode,
) -> serde_json::Value {
    let response = local_admin_providers_response(state, method, path, body).await;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    assert_eq!(
        status,
        expected_status,
        "{}",
        String::from_utf8_lossy(&body)
    );
    serde_json::from_slice(&body).unwrap()
}

fn state_with_retry_config(
    provider_attempts: Option<i32>,
    endpoint_attempts: Option<i32>,
) -> (AppState, Arc<InMemoryProviderCatalogReadRepository>) {
    let mut provider = sample_provider("provider-retry", "retry", 10);
    provider.max_retries = provider_attempts;
    provider.config = Some(json!({"failover_rules": {
        "stop_status_codes": [400], "future_rule": {"preserved": true}
    }}));
    let mut endpoint = sample_endpoint(
        "endpoint-retry",
        "provider-retry",
        "openai:chat",
        "https://fixture.invalid",
    );
    endpoint.max_retries = endpoint_attempts;
    let key = sample_key("key-retry", "provider-retry", "openai:chat", "fixture-key");
    let repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![provider],
        vec![endpoint],
        vec![key],
    ));
    let state = AppState::new().unwrap().with_data_state_for_tests(
        GatewayDataState::with_provider_catalog_repository_for_tests(repository.clone())
            .with_encryption_key_for_tests(DEVELOPMENT_ENCRYPTION_KEY),
    );
    (state, repository)
}

#[tokio::test]
async fn chat_retry_admin_create_refresh_preserves_absence_and_explicit_values() {
    let (state, repository) = state_with_retry_config(None, None);
    for (payload, expected_raw, attempts, source, budget) in [
        (json!({"name": "retry-absent"}), None, 1, "default", 90_000),
        (
            json!({"name": "retry-explicit", "max_retries": 2}),
            Some(2),
            2,
            "provider.max_attempts",
            90_000,
        ),
        (
            json!({"name": "retry-canonical", "failover_rules": {
                "max_attempts": 3, "stream_failover_budget_ms": 45000,
                "future_rule": {"preserved": true}
            }}),
            None,
            3,
            "failover_rules.max_attempts",
            45_000,
        ),
    ] {
        let created = request_json(
            &state,
            http::Method::POST,
            "/api/admin/providers/",
            Some(payload),
            StatusCode::OK,
        )
        .await;
        let provider_id = created["id"].as_str().unwrap();
        let summary_path = format!("/api/admin/providers/{provider_id}/summary");
        for _ in 0..2 {
            let summary = request_json(
                &state,
                http::Method::GET,
                &summary_path,
                None,
                StatusCode::OK,
            )
            .await;
            assert_eq!(summary["max_retries"], json!(expected_raw));
            assert_eq!(summary["effective_max_attempts"], attempts);
            assert_eq!(summary["effective_max_attempts_source"], source);
            assert_eq!(summary["stream_failover_budget_ms"], budget);
        }
        let endpoint = request_json(
            &state,
            http::Method::POST,
            &format!("/api/admin/endpoints/providers/{provider_id}/endpoints"),
            Some(json!({
                "provider_id": provider_id, "api_format": "openai:chat",
                "base_url": "https://fixture.invalid"
            })),
            StatusCode::OK,
        )
        .await;
        let refreshed_endpoint = request_json(
            &state,
            http::Method::GET,
            &format!("/api/admin/endpoints/{}", endpoint["id"].as_str().unwrap()),
            None,
            StatusCode::OK,
        )
        .await;
        assert_eq!(refreshed_endpoint["max_retries"], serde_json::Value::Null);
        assert_eq!(refreshed_endpoint["effective_max_attempts"], attempts);
        assert_eq!(refreshed_endpoint["effective_max_attempts_source"], source);
        let stored = repository
            .list_providers(false)
            .await
            .unwrap()
            .into_iter()
            .find(|provider| provider.id == provider_id)
            .unwrap();
        assert_eq!(stored.max_retries, expected_raw);
        if source == "failover_rules.max_attempts" {
            let rules = &stored.config.as_ref().unwrap()["failover_rules"];
            assert_eq!(rules["max_attempts"], attempts);
            assert_eq!(rules["stream_failover_budget_ms"], budget);
            assert_eq!(rules["future_rule"], json!({"preserved": true}));
        }
    }
}

#[tokio::test]
async fn chat_retry_admin_reads_preview_legacy_two_without_rewriting_storage() {
    let (state, repository) = state_with_retry_config(Some(2), Some(2));
    for _ in 0..2 {
        let summary = request_json(
            &state,
            http::Method::GET,
            "/api/admin/providers/provider-retry/summary",
            None,
            StatusCode::OK,
        )
        .await;
        assert_eq!(summary["effective_max_attempts"], 2);
        assert_eq!(summary["legacy_effective_max_attempts"], 1);
        assert_eq!(
            summary["effective_max_attempts_source"],
            "provider.max_retries"
        );
        assert_eq!(summary["stream_failover_budget_ms"], 90_000);
    }
    let stored = repository.list_providers(false).await.unwrap().remove(0);
    assert_eq!(stored.max_retries, Some(2));
    assert!(stored.config.unwrap()["failover_rules"]
        .get("max_attempts")
        .is_none());
}

#[tokio::test]
async fn chat_retry_admin_endpoint_save_refresh_preserves_explicit_two_and_inheritance() {
    let (state, _) = state_with_retry_config(Some(5), Some(2));
    let path = "/api/admin/endpoints/endpoint-retry";
    let before = request_json(&state, http::Method::GET, path, None, StatusCode::OK).await;
    assert_eq!(before["effective_max_attempts"], 5);
    assert_eq!(
        before["effective_max_attempts_source"],
        "provider.max_retries"
    );
    let saved = request_json(
        &state,
        http::Method::PUT,
        path,
        Some(json!({"max_retries": 2})),
        StatusCode::OK,
    )
    .await;
    let refreshed = request_json(&state, http::Method::GET, path, None, StatusCode::OK).await;
    assert_eq!(saved["effective_max_attempts"], 2);
    assert_eq!(refreshed["effective_max_attempts"], 2);
    assert_eq!(
        refreshed["effective_max_attempts_source"],
        "endpoint.max_attempts"
    );
    request_json(
        &state,
        http::Method::PUT,
        path,
        Some(json!({"max_retries": null})),
        StatusCode::OK,
    )
    .await;
    let inherited = request_json(&state, http::Method::GET, path, None, StatusCode::OK).await;
    assert_eq!(inherited["max_retries"], serde_json::Value::Null);
    assert_eq!(inherited["effective_max_attempts"], 5);
}

#[tokio::test]
async fn chat_retry_admin_budget_save_refresh_invalidates_transport_snapshot() {
    let (state, repository) = state_with_retry_config(Some(2), Some(2));
    let before = state
        .read_provider_transport_snapshot("provider-retry", "endpoint-retry", "key-retry")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        crate::provider_transport::resolve_transport_execution_timeouts(&before)
            .unwrap()
            .stream_failover_budget_ms,
        Some(90_000)
    );
    let patch = json!({"failover_rules": {"max_attempts": 3, "stream_failover_budget_ms": 1234}});
    for _ in 0..2 {
        let saved = request_json(
            &state,
            http::Method::PATCH,
            "/api/admin/providers/provider-retry",
            Some(patch.clone()),
            StatusCode::OK,
        )
        .await;
        let refreshed = request_json(
            &state,
            http::Method::GET,
            "/api/admin/providers/provider-retry/summary",
            None,
            StatusCode::OK,
        )
        .await;
        assert_eq!(saved["effective_max_attempts"], 3);
        assert_eq!(refreshed["effective_max_attempts"], 3);
        assert_eq!(
            refreshed["effective_max_attempts_source"],
            "failover_rules.max_attempts"
        );
        assert_eq!(refreshed["stream_failover_budget_ms"], 1234);
        assert_eq!(
            refreshed["failover_rules"]["stop_status_codes"],
            json!([400])
        );
        assert_eq!(
            refreshed["failover_rules"]["future_rule"],
            json!({"preserved": true})
        );
    }
    let after = state
        .read_provider_transport_snapshot("provider-retry", "endpoint-retry", "key-retry")
        .await
        .unwrap()
        .unwrap();
    let timeouts = crate::provider_transport::resolve_transport_execution_timeouts(&after).unwrap();
    assert_eq!(timeouts.stream_failover_budget_ms, Some(1234));
    assert_eq!(timeouts.first_byte_ms, Some(30_000));
    assert_eq!(timeouts.total_ms, None);
    let stored = repository.list_providers(false).await.unwrap().remove(0);
    assert_eq!(
        stored.config.unwrap()["failover_rules"]["stream_failover_budget_ms"],
        1234
    );
}

#[tokio::test]
async fn chat_retry_admin_clearing_budget_restores_default_in_storage_and_snapshot() {
    let (state, repository) = state_with_retry_config(Some(2), Some(2));
    request_json(
        &state,
        http::Method::PATCH,
        "/api/admin/providers/provider-retry",
        Some(json!({"failover_rules": {"stream_failover_budget_ms": 1234}})),
        StatusCode::OK,
    )
    .await;
    let configured = state
        .read_provider_transport_snapshot("provider-retry", "endpoint-retry", "key-retry")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        crate::provider_transport::resolve_transport_execution_timeouts(&configured)
            .unwrap()
            .stream_failover_budget_ms,
        Some(1234)
    );
    for _ in 0..2 {
        request_json(
            &state,
            http::Method::PATCH,
            "/api/admin/providers/provider-retry",
            Some(json!({"failover_rules": {"stream_failover_budget_ms": null}})),
            StatusCode::OK,
        )
        .await;
        let refreshed = request_json(
            &state,
            http::Method::GET,
            "/api/admin/providers/provider-retry/summary",
            None,
            StatusCode::OK,
        )
        .await;
        assert_eq!(refreshed["stream_failover_budget_ms"], 90_000);
        assert!(refreshed["failover_rules"]
            .get("stream_failover_budget_ms")
            .is_none());
        let snapshot = state
            .read_provider_transport_snapshot("provider-retry", "endpoint-retry", "key-retry")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            crate::provider_transport::resolve_transport_execution_timeouts(&snapshot)
                .unwrap()
                .stream_failover_budget_ms,
            Some(90_000)
        );
    }
    let stored = repository.list_providers(false).await.unwrap().remove(0);
    let rules = &stored.config.as_ref().unwrap()["failover_rules"];
    assert!(rules.get("stream_failover_budget_ms").is_none());
    assert_eq!(rules["future_rule"], json!({"preserved": true}));
    assert_eq!(rules["stop_status_codes"], json!([400]));
}

#[tokio::test]
async fn chat_retry_admin_non_chat_endpoint_keeps_legacy_attempts_after_chat_edit() {
    let (state, repository) = state_with_retry_config(Some(5), Some(2));
    let created = request_json(
        &state,
        http::Method::POST,
        "/api/admin/endpoints/providers/provider-retry/endpoints",
        Some(json!({
            "provider_id": "provider-retry",
            "api_format": "openai:embedding",
            "base_url": "https://fixture.invalid",
            "max_retries": 2
        })),
        StatusCode::OK,
    )
    .await;
    assert_eq!(created["max_retries"], 2);
    assert_eq!(created["effective_max_attempts"], 5);
    let path = format!("/api/admin/endpoints/{}", created["id"].as_str().unwrap());
    request_json(
        &state,
        http::Method::PATCH,
        "/api/admin/providers/provider-retry",
        Some(json!({"failover_rules": {"max_attempts": 3}})),
        StatusCode::OK,
    )
    .await;
    let chat = request_json(
        &state,
        http::Method::GET,
        "/api/admin/endpoints/endpoint-retry",
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(chat["effective_max_attempts"], 3);
    for _ in 0..2 {
        let refreshed = request_json(&state, http::Method::GET, &path, None, StatusCode::OK).await;
        assert_eq!(refreshed["max_retries"], 2);
        assert_eq!(refreshed["effective_max_attempts"], 5);
        assert_eq!(
            refreshed["effective_max_attempts_source"],
            "provider.max_retries"
        );
        assert!(refreshed.get("chat_policy_version").is_none());
        assert!(refreshed.get("legacy_effective_max_attempts").is_none());
    }
    let stored = repository.list_providers(false).await.unwrap().remove(0);
    assert_eq!(stored.max_retries, Some(5));
    assert_eq!(stored.config.unwrap()["failover_rules"]["max_attempts"], 3);
}

#[tokio::test]
async fn chat_retry_admin_rejects_invalid_and_conflicting_writes_without_mutation() {
    let (state, repository) = state_with_retry_config(Some(2), None);
    let before = repository.list_providers(false).await.unwrap().remove(0);
    for rules in [
        json!({"max_attempts": 0}),
        json!({"max_attempts": 100}),
        json!({"max_attempts": 2.5}),
        json!({"max_attempts": 2, "max_retries": 3}),
        json!({"stream_failover_budget_ms": 0}),
        json!({"stream_failover_budget_ms": 1200001}),
    ] {
        request_json(
            &state,
            http::Method::PATCH,
            "/api/admin/providers/provider-retry",
            Some(json!({"failover_rules": rules})),
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    let after = repository.list_providers(false).await.unwrap().remove(0);
    assert_eq!(after.config, before.config);
    assert_eq!(after.max_retries, before.max_retries);
}
