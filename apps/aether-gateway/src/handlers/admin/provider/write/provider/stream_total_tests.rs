use super::{
    build_admin_create_provider_record_from_existing,
    build_admin_update_provider_record_from_existing,
};
use crate::handlers::admin::provider::{
    shared::payloads::{AdminProviderCreateRequest, AdminProviderUpdatePatch},
    summary::build_admin_provider_summary_value,
};
use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogProvider;
use serde_json::{json, Value};

fn create(value: Value) -> Result<StoredProviderCatalogProvider, String> {
    let payload: AdminProviderCreateRequest =
        serde_json::from_value(value).map_err(|err| err.to_string())?;
    build_admin_create_provider_record_from_existing(&[], payload).map(|(provider, _)| provider)
}

fn patch(
    provider: &StoredProviderCatalogProvider,
    value: Value,
) -> Result<StoredProviderCatalogProvider, String> {
    let patch = AdminProviderUpdatePatch::from_object(value.as_object().unwrap().clone())
        .map_err(|err| err.to_string())?;
    build_admin_update_provider_record_from_existing(&[], provider, patch)
}

fn summary(provider: &StoredProviderCatalogProvider) -> Value {
    build_admin_provider_summary_value(provider, &[], &[], None, None, vec![], 0)
}

#[test]
fn stream_total_timeout_create_patch_and_readback_keep_legacy_config() {
    let original =
        json!({"future": {"enabled": true}, "failover_rules": {"stream_failover_budget_ms": 1800}});
    let provider =
        create(json!({"name": "timeout", "request_timeout": 12, "config": original})).unwrap();
    let defaults = summary(&provider);
    assert_eq!(defaults["stream_total_timeout"], Value::Null);
    assert_eq!(defaults["effective_stream_total_timeout"], 900.0);
    assert_eq!(defaults["effective_stream_total_timeout_source"], "default");
    let saved = patch(&provider, json!({"stream_total_timeout": 5.001})).unwrap();
    let readback = summary(&saved);
    assert_eq!(readback["stream_total_timeout"], 5.001);
    assert_eq!(readback["effective_stream_total_timeout"], 5.001);
    assert_eq!(
        readback["effective_stream_total_timeout_source"],
        "config.stream_total_timeout_ms"
    );
    assert_eq!(readback["request_timeout"], 12.0);
    assert_eq!(readback["stream_failover_budget_ms"], 1800);
    assert_eq!(saved.config.as_ref().unwrap()["future"], original["future"]);
    let renamed = patch(&saved, json!({"name": "renamed"})).unwrap();
    assert_eq!(summary(&renamed)["stream_total_timeout"], 5.001);
    for clear in [
        json!({"stream_total_timeout": null}),
        json!({"config": {"stream_total_timeout_ms": null}}),
    ] {
        let cleared = patch(&renamed, clear).unwrap();
        assert_eq!(summary(&cleared)["stream_total_timeout"], Value::Null);
        assert_eq!(summary(&cleared)["effective_stream_total_timeout"], 900.0);
        assert_eq!(
            summary(&cleared)["effective_stream_total_timeout_source"],
            "default"
        );
        assert!(!cleared
            .config
            .as_ref()
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("stream_total_timeout_ms"));
        assert_eq!(
            cleared.config.as_ref().unwrap()["future"],
            original["future"]
        );
        assert_eq!(summary(&cleared)["stream_failover_budget_ms"], 1800);
    }
}

#[test]
fn stream_total_timeout_create_and_direct_config_writes_share_validation() {
    let provider = create(json!({"name": "timeout", "stream_total_timeout": 1.001})).unwrap();
    assert_eq!(
        provider.config.as_ref().unwrap()["stream_total_timeout_ms"],
        1001
    );
    let direct =
        create(json!({"name": "direct", "config": {"stream_total_timeout_ms": 1200000}})).unwrap();
    assert_eq!(summary(&direct)["effective_stream_total_timeout"], 1200.0);
    let cleared_on_create = create(json!({"name": "clear", "stream_total_timeout": null, "config": {"stream_total_timeout_ms": 5000}})).unwrap();
    assert_eq!(
        summary(&cleared_on_create)["effective_stream_total_timeout"],
        900.0
    );
    let direct = patch(
        &provider,
        json!({"config": {"stream_total_timeout_ms": 1200000}}),
    )
    .unwrap();
    assert_eq!(summary(&direct)["stream_total_timeout"], 1200.0);

    for value in [
        json!(0),
        json!(0.999),
        json!(1200.001),
        json!(1.0001),
        json!("5"),
        json!(true),
    ] {
        assert!(create(json!({"name": "invalid", "stream_total_timeout": value})).is_err());
        assert!(patch(&provider, json!({"stream_total_timeout": value})).is_err());
    }
    for value in [
        json!(999),
        json!(1200001),
        json!(1000.5),
        json!("5000"),
        json!(true),
    ] {
        assert!(
            create(json!({"name": "invalid", "config": {"stream_total_timeout_ms": value}}))
                .is_err()
        );
        assert!(patch(
            &provider,
            json!({"config": {"stream_total_timeout_ms": value}})
        )
        .is_err());
    }
    assert_eq!(summary(&provider)["stream_total_timeout"], 1.001);
}
