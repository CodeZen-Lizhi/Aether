use std::collections::BTreeSet;

use serde_json::json;

use super::{
    build_import_plan, copy_database_records, decode_jsonl, encode_jsonl, export_sqlite_core_jsonl,
    export_sqlite_jsonl, filter_import_payload, import_sqlite_jsonl, normalize_imported_binary,
    normalize_imported_integer_timestamp, sqlite_core_export_domains, DataCopyOptions,
    DataExportManifest, DataExportRecord, DataImportPlan, ExportDomain, ExportRow,
    AUXILIARY_TABLES,
};
use crate::lifecycle::migrate::run_sqlite_migrations;
use crate::{DatabaseDriver, SqlDatabaseConfig};

#[test]
fn jsonl_round_trips_manifest_and_domain_rows() {
    let records = vec![
        DataExportRecord::manifest(DataExportManifest::new(
            1_700_000_000,
            Some(DatabaseDriver::Postgres),
            vec![ExportDomain::Users, ExportDomain::ApiKeys],
        )),
        DataExportRecord::row(
            ExportDomain::Users,
            "user-1",
            json!({
                "id": "user-1",
                "email": "owner@example.com"
            }),
        ),
        DataExportRecord::row(
            ExportDomain::ApiKeys,
            "api-key-1",
            json!({
                "id": "api-key-1",
                "key_hash": "ciphertext-preserved"
            }),
        ),
    ];

    let encoded = encode_jsonl(&records).expect("records should encode");
    assert_eq!(encoded.lines().count(), 3);

    let decoded = decode_jsonl(&encoded).expect("records should decode");
    assert_eq!(decoded, records);

    let import_plan = build_import_plan(&encoded).expect("import plan should build");
    assert_eq!(
        import_plan.manifest.source_driver,
        Some(DatabaseDriver::Postgres)
    );
    assert_eq!(import_plan.rows(ExportDomain::Users).len(), 1);
    assert_eq!(
        import_plan.rows(ExportDomain::ApiKeys)[0].payload["key_hash"],
        "ciphertext-preserved"
    );
}

#[test]
fn core_export_domains_only_include_current_sqlite_domains() {
    for domain in [
        ExportDomain::OAuthProviders,
        ExportDomain::UserOAuthLinks,
        ExportDomain::UserGroups,
        ExportDomain::UserGroupMembers,
    ] {
        assert!(!sqlite_core_export_domains().contains(&domain));
    }
    assert!(sqlite_core_export_domains().contains(&ExportDomain::Auxiliary));
}

#[tokio::test]
async fn sqlite_core_export_covers_every_portable_table() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite pool should connect");
    run_sqlite_migrations(&pool)
        .await
        .expect("sqlite migrations should run");

    let schema_tables = sqlx::query_scalar::<_, String>(
        r#"
SELECT name
FROM sqlite_master
WHERE type = 'table'
  AND name NOT LIKE 'sqlite_%'
  AND name NOT IN ('_sqlx_migrations', 'schema_backfills')
ORDER BY name
"#,
    )
    .fetch_all(&pool)
    .await
    .expect("sqlite schema tables should load")
    .into_iter()
    .collect::<BTreeSet<_>>();

    let mut exported_tables = [
        "users",
        "api_keys",
        "providers",
        "provider_api_keys",
        "provider_endpoints",
        "global_models",
        "models",
        "auth_modules",
        "proxy_nodes",
        "system_configs",
        "usage",
        "wallets",
        "wallet_transactions",
        "wallet_daily_usage_ledgers",
        "payment_orders",
        "payment_callbacks",
        "refund_requests",
        "redeem_code_batches",
        "redeem_codes",
        "billing_rules",
        "dimension_collectors",
        "usage_settlement_snapshots",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<BTreeSet<_>>();
    exported_tables.extend(AUXILIARY_TABLES.iter().map(|table| table.name.to_string()));

    assert_eq!(schema_tables, exported_tables);
    let encoded = export_sqlite_core_jsonl(&pool, 1)
        .await
        .expect("default export must read every current table");
    let plan = build_import_plan(&encoded).expect("default export should decode");
    assert_eq!(plan.manifest.domains, sqlite_core_export_domains());
}

#[tokio::test]
async fn version_one_exports_remain_importable_after_full_export_expansion() {
    let records = decode_jsonl(
        r#"{"record_type":"manifest","manifest":{"format_version":1,"created_at_unix_secs":1,"source_driver":null,"domains":["users"]}}
{"record_type":"row","domain":"users","id":"user-1","payload":{"id":"user-1"}}"#,
    )
    .expect("version one exports should remain supported");

    assert_eq!(records.len(), 2);

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite pool should connect");
    run_sqlite_migrations(&pool)
        .await
        .expect("migrations should run");
    for domain in [
        ExportDomain::OAuthProviders,
        ExportDomain::UserOAuthLinks,
        ExportDomain::UserGroups,
        ExportDomain::UserGroupMembers,
    ] {
        let error = export_sqlite_jsonl(&pool, vec![domain], 1)
            .await
            .expect_err("retired domains cannot be exported");
        assert!(error.to_string().contains("retired"));

        let mut manifest = DataExportManifest::new(
            1,
            Some(DatabaseDriver::Postgres),
            vec![ExportDomain::GlobalModels, domain],
        );
        manifest.format_version = 1;
        let mut records = vec![
            DataExportRecord::manifest(manifest),
            DataExportRecord::row(
                ExportDomain::GlobalModels,
                "legacy-model",
                json!({
                    "id": "legacy-model", "name": "legacy-model", "created_at": 1, "updated_at": 1
                }),
            ),
        ];
        let encoded = encode_jsonl(&records).expect("legacy backup should encode");
        assert_eq!(
            import_sqlite_jsonl(&pool, &encoded)
                .await
                .expect("empty retired domains remain importable"),
            1
        );

        if let DataExportRecord::Row { payload, .. } = &mut records[1] {
            payload["name"] = json!("must-rollback");
        }
        records.push(DataExportRecord::row(
            domain,
            "retired-row",
            json!({"id": "retired-row"}),
        ));
        let encoded = encode_jsonl(&records).expect("legacy backup should encode");
        let error = import_sqlite_jsonl(&pool, &encoded)
            .await
            .expect_err("retired rows must not be silently discarded");
        assert!(error.to_string().contains("retired"));
        let name: String =
            sqlx::query_scalar("SELECT name FROM global_models WHERE id = 'legacy-model'")
                .fetch_one(&pool)
                .await
                .expect("model should remain");
        assert_eq!(name, "legacy-model");
    }
}

#[test]
fn jsonl_rejects_missing_manifest() {
    let err = decode_jsonl(r#"{"record_type":"row","domain":"users","id":"user-1","payload":{}}"#)
        .expect_err("missing manifest should fail");

    assert!(err.to_string().contains("must start with a manifest"));
}

#[test]
fn jsonl_rejects_rows_outside_manifest_domains() {
    let records = vec![
        DataExportRecord::manifest(DataExportManifest::new(
            1_700_000_000,
            Some(DatabaseDriver::Sqlite),
            vec![ExportDomain::Users],
        )),
        DataExportRecord::row(
            ExportDomain::Wallets,
            "wallet-1",
            json!({ "id": "wallet-1" }),
        ),
    ];

    let err = encode_jsonl(&records).expect_err("undeclared domain should fail");
    assert!(err.to_string().contains("not declared in manifest"));
}

#[test]
fn jsonl_rejects_bad_json_with_line_number() {
    let err = decode_jsonl(
            r#"{"record_type":"manifest","manifest":{"format_version":1,"created_at_unix_secs":1,"source_driver":null,"domains":["users"]}}
not-json"#,
        )
        .expect_err("bad json should fail");

    assert!(err.to_string().contains("line 2"));
}

#[test]
fn jsonl_rejects_duplicate_domain_ids() {
    let records = vec![
        DataExportRecord::manifest(DataExportManifest::new(
            1_700_000_000,
            None,
            vec![ExportDomain::Users],
        )),
        DataExportRecord::row(ExportDomain::Users, "user-1", json!({ "id": "user-1" })),
        DataExportRecord::row(ExportDomain::Users, "user-1", json!({ "id": "user-1" })),
    ];

    let err = encode_jsonl(&records).expect_err("duplicate id should fail");
    assert!(err.to_string().contains("duplicate"));
}

#[test]
fn cross_driver_timestamp_normalization_preserves_usage_second_contract() {
    assert_eq!(
        normalize_imported_integer_timestamp(
            "sqlite",
            r#""usage""#,
            "created_at_unix_ms",
            &json!("1970-01-01T00:00:01.234900Z"),
        )
        .expect("usage timestamp should normalize"),
        Some(1),
    );
    assert_eq!(
        normalize_imported_integer_timestamp(
            "sqlite",
            "request_candidates",
            "created_at_unix_ms",
            &json!("1970-01-01T00:00:01.234900Z"),
        )
        .expect("millisecond timestamp should normalize"),
        Some(1_234),
    );
}

#[test]
fn cross_driver_binary_normalization_preserves_raw_bytes() {
    assert_eq!(
        normalize_imported_binary("sqlite", "payload_gzip", &json!([0, 1, 127, 255]))
            .expect("byte array should normalize"),
        Some(vec![0, 1, 127, 255]),
    );
    assert_eq!(
        normalize_imported_binary("sqlite", "payload_gzip", &json!("\\x00017fff"))
            .expect("postgres hex should normalize"),
        Some(vec![0, 1, 127, 255]),
    );
    assert!(normalize_imported_binary("sqlite", "payload_gzip", &json!([256])).is_err());
    assert!(normalize_imported_binary("sqlite", "payload_gzip", &json!("\\x€0")).is_err());
}

#[test]
fn sqlite_import_payloads_reject_non_null_unknown_columns() {
    let target_columns = BTreeSet::from(["id".to_string()]);
    let row = ExportRow {
        id: "user-1".to_string(),
        payload: json!({
            "id": "user-1",
            "legacy_nullable": null,
            "unexpected_column": "value"
        }),
    };

    for driver_name in ["sqlite"] {
        let err = filter_import_payload(
            driver_name,
            "users",
            ExportDomain::Users,
            &row,
            &target_columns,
        )
        .expect_err("non-null unknown columns should fail");

        assert!(err.to_string().contains("unexpected_column"));
        assert!(err.to_string().contains("does not exist"));
        assert!(err.to_string().contains(driver_name));
    }
}

#[test]
fn sqlite_import_payloads_ignore_unknown_null_columns() {
    let target_columns = BTreeSet::from(["id".to_string()]);
    let row = ExportRow {
        id: "user-1".to_string(),
        payload: json!({
            "id": "user-1",
            "legacy_nullable": null
        }),
    };

    let filtered = filter_import_payload(
        "sqlite",
        "users",
        ExportDomain::Users,
        &row,
        &target_columns,
    )
    .expect("unknown null columns should remain backward compatible");

    assert_eq!(
        filtered,
        serde_json::Map::from_iter([("id".to_string(), json!("user-1"))])
    );
}

#[tokio::test]
async fn sqlite_import_rejects_non_integer_timestamp_values() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite pool should connect");
    run_sqlite_migrations(&pool)
        .await
        .expect("sqlite migrations should run");

    for invalid_value in [
        json!("not-a-timestamp"),
        json!(1.5),
        json!(true),
        json!({"unexpected": "object"}),
    ] {
        let encoded = encode_jsonl(&[
            DataExportRecord::manifest(DataExportManifest::new(
                1_700_000_000,
                Some(DatabaseDriver::Postgres),
                vec![ExportDomain::GlobalModels],
            )),
            DataExportRecord::row(
                ExportDomain::GlobalModels,
                "invalid-timestamp",
                json!({
                    "id": "invalid-timestamp",
                    "name": "invalid-timestamp",
                    "created_at": invalid_value,
                    "updated_at": 1
                }),
            ),
        ])
        .expect("invalid timestamp fixture should encode");

        let err = import_sqlite_jsonl(&pool, &encoded)
            .await
            .expect_err("non-integer timestamp should be rejected");
        assert!(err.to_string().contains(
            "timestamp column 'created_at' must contain an integer or supported datetime"
        ));
    }
}

#[tokio::test]
async fn sqlite_import_updates_parent_without_cascading_child_rows() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite pool should connect");
    run_sqlite_migrations(&pool)
        .await
        .expect("sqlite migrations should run");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("foreign keys should be enabled");
    sqlx::raw_sql(
        r#"
INSERT INTO users (id, email, username, created_at, updated_at)
VALUES ('import-user', 'import@example.test', 'import-user', 1, 1);
INSERT INTO referral_rewards (id, referral_id, inviter_user_id, invitee_user_id, reward_type, trigger_point, idempotency_key, amount_usd, created_at, updated_at)
VALUES ('import-reward', 'historical-referral', 'import-user', 'import-user', 'gift', 'payment', 'import-reward-once', 2.5, 1, 1);
"#,
    )
    .execute(&pool)
    .await
    .expect("parent and child fixtures should insert");

    let encoded = encode_jsonl(&[
        DataExportRecord::manifest(DataExportManifest::new(
            1,
            Some(DatabaseDriver::Postgres),
            vec![ExportDomain::Users],
        )),
        DataExportRecord::row(
            ExportDomain::Users,
            "import-user",
            json!({
                "id": "import-user", "username": "After", "created_at": 1, "updated_at": 2
            }),
        ),
    ])
    .expect("user export should encode");
    assert_eq!(
        import_sqlite_jsonl(&pool, &encoded)
            .await
            .expect("user import should update in place"),
        1
    );
    let user: (String, String) =
        sqlx::query_as("SELECT username, email FROM users WHERE id = 'import-user'")
            .fetch_one(&pool)
            .await
            .expect("updated user should load");
    assert_eq!(
        user,
        ("After".to_string(), "import@example.test".to_string())
    );
    let reward_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM referral_rewards WHERE inviter_user_id = 'import-user'",
    )
    .fetch_one(&pool)
    .await
    .expect("child reward count should load");
    assert_eq!(reward_count, 1);
}

#[tokio::test]
async fn sqlite_import_rolls_back_rows_after_late_failure() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite pool should connect");
    run_sqlite_migrations(&pool)
        .await
        .expect("sqlite migrations should run");
    let encoded = encode_jsonl(&[
        DataExportRecord::manifest(DataExportManifest::new(
            1_700_000_000,
            Some(DatabaseDriver::Postgres),
            vec![ExportDomain::GlobalModels],
        )),
        DataExportRecord::row(
            ExportDomain::GlobalModels,
            "rollback-valid",
            json!({
                "id": "rollback-valid",
                "name": "rollback-valid",
                "created_at": 1,
                "updated_at": 1
            }),
        ),
        DataExportRecord::row(
            ExportDomain::GlobalModels,
            "rollback-invalid",
            json!({
                "id": "rollback-invalid",
                "name": "rollback-invalid",
                "created_at": "invalid-timestamp",
                "updated_at": 1
            }),
        ),
    ])
    .expect("rollback fixture should encode");

    import_sqlite_jsonl(&pool, &encoded)
        .await
        .expect_err("late invalid row should fail the import");
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM global_models WHERE id LIKE 'rollback-%'")
            .fetch_one(&pool)
            .await
            .expect("rolled back row count should load");
    assert_eq!(count, 0);

    let encoded = encode_jsonl(&[
        DataExportRecord::manifest(DataExportManifest::new(
            1,
            Some(DatabaseDriver::Sqlite),
            vec![ExportDomain::GlobalModels, ExportDomain::Auxiliary],
        )),
        DataExportRecord::row(
            ExportDomain::GlobalModels,
            "rollback-parent",
            json!({
                "id": "rollback-parent", "name": "rollback-parent", "created_at": 1, "updated_at": 1
            }),
        ),
        DataExportRecord::row(
            ExportDomain::Auxiliary,
            "background_task_events:rollback-child",
            json!({
                "__table": "background_task_events", "id": "rollback-child",
                "run_id": "missing-run", "event_type": "created", "message": "rollback",
                "created_at_unix_secs": 1
            }),
        ),
    ])
    .expect("foreign key fixture should encode");
    let error = import_sqlite_jsonl(&pool, &encoded)
        .await
        .expect_err("deferred foreign keys must still be enforced at commit");
    assert!(error.to_string().contains("FOREIGN KEY constraint failed"));
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM global_models WHERE id LIKE 'rollback-%'")
            .fetch_one(&pool)
            .await
            .expect("rolled back parents should load");
    assert_eq!(count, 0);
    let deferred: i64 = sqlx::query_scalar("PRAGMA defer_foreign_keys")
        .fetch_one(&pool)
        .await
        .expect("foreign key mode should load");
    assert_eq!(deferred, 0);
}

#[tokio::test]
async fn sqlite_core_export_reads_migrated_database_rows() {
    let temp_dir = std::env::temp_dir().join(format!("aether-export-{}", unique_suffix()));
    std::fs::create_dir(&temp_dir).expect("temporary export directory should exist");
    let source_config = SqlDatabaseConfig {
        url: format!("sqlite://{}", temp_dir.join("source.db").display()),
        ..SqlDatabaseConfig::sqlite_default()
    };
    let pool = crate::driver::sqlite::SqlitePoolFactory::new(source_config.clone())
        .expect("sqlite factory should build")
        .connect_lazy()
        .expect("sqlite pool should connect");
    run_sqlite_migrations(&pool)
        .await
        .expect("sqlite migrations should run");

    sqlx::query(
            r#"
INSERT INTO users (id, email, username, auth_source, created_at, updated_at)
VALUES ('user-1', 'owner@example.com', 'owner', 'local', '1970-01-01T00:00:01Z', '1970-01-01T00:00:02Z');
INSERT INTO api_keys (id, user_id, key_hash, key_encrypted, name, created_at, updated_at)
VALUES ('api-key-1', 'user-1', 'hash-1', 'ciphertext-1', 'Default', '1970-01-01T00:00:01Z', '1970-01-01T00:00:02Z');
INSERT INTO providers (id, name, provider_type, created_at, updated_at)
VALUES ('provider-1', 'Provider One', 'openai', '1970-01-01T00:00:01Z', '1970-01-01T00:00:02Z');
INSERT INTO provider_api_keys (id, provider_id, name, encrypted_key, created_at, updated_at)
VALUES ('provider-key-1', 'provider-1', 'Provider Key', 'ciphertext-provider', '1970-01-01T00:00:01Z', '1970-01-01T00:00:02Z');
INSERT INTO provider_endpoints (id, provider_id, name, base_url, created_at, updated_at)
VALUES ('endpoint-1', 'provider-1', 'Primary', 'https://example.test', '1970-01-01T00:00:01Z', '1970-01-01T00:00:02Z');
INSERT INTO global_models (id, name, created_at, updated_at)
VALUES ('global-model-1', 'gpt-test', '1970-01-01T00:00:01Z', '1970-01-01 00:00:02.123456');
INSERT INTO models (id, provider_id, global_model_id, provider_model_name, created_at, updated_at)
VALUES ('model-1', 'provider-1', 'global-model-1', 'gpt-test', '1970-01-01T00:00:01Z', '1970-01-01T00:00:02Z');
INSERT INTO billing_rules (id, global_model_id, name, task_type, expression, variables, dimension_mappings, is_enabled, created_at, updated_at)
VALUES ('billing-rule-1', 'global-model-1', 'Rule One', 'chat', 'input_tokens * 0.01', '{}', '{"input":"input_tokens"}', 1, '1970-01-01T00:00:01Z', '1970-01-01T00:00:02Z');
INSERT INTO dimension_collectors (id, api_format, task_type, dimension_name, source_type, value_type, transform_expression, priority, is_enabled, created_at, updated_at)
VALUES ('collector-1', 'openai', 'chat', 'input_tokens', 'computed', 'float', 'usage.input_tokens', 10, 1, '1970-01-01T00:00:01Z', '1970-01-01T00:00:02Z');
INSERT INTO system_configs (id, key, value, created_at, updated_at)
VALUES ('config-1', 'billing.enabled', 'true', '1970-01-01T00:00:01Z', '1970-01-01T00:00:02Z');
INSERT INTO payment_gateway_configs (provider, endpoint_url, merchant_id, merchant_key_encrypted, created_at, updated_at)
VALUES ('legacy-gateway', 'https://example.test/pay', 'merchant-1', 'ciphertext-merchant', 1, 2);
INSERT INTO referral_rewards (id, referral_id, inviter_user_id, invitee_user_id, reward_type, trigger_point, idempotency_key, amount_usd, created_at, updated_at)
VALUES ('reward-1', 'historical-referral', 'user-1', 'user-1', 'gift', 'payment', 'reward-once', 2.5, 1, 2);
INSERT INTO wallets (id, user_id, created_at, updated_at)
VALUES ('wallet-1', 'user-1', '1970-01-01T00:00:01Z', '1970-01-01T00:00:02Z');
INSERT INTO "usage" (request_id, id, user_id, provider_name, model, status, billing_status, created_at_unix_ms, updated_at_unix_secs)
VALUES ('request-1', 'request-1', 'user-1', 'Provider One', 'gpt-test', 'completed', 'settled', '1970-01-01T00:00:01.234900Z', 2);
INSERT INTO audit_logs (id, event_type, description, request_id, created_at)
VALUES ('audit-1', 'request.completed', 'Exported audit', 'request-1', '1970-01-01T00:00:02Z');
INSERT INTO usage_body_blobs (body_ref, request_id, body_field, payload_gzip, created_at, updated_at)
VALUES ('body-ref-1', 'request-1', 'request', X'00117FFF', '1970-01-01T00:00:01Z', '1970-01-01T00:00:02Z');
INSERT INTO usage_http_audits (request_id, request_body_ref, request_body_state, body_capture_mode, created_at, updated_at)
VALUES ('request-1', 'body-ref-1', 'captured', 'full', '1970-01-01T00:00:01Z', '1970-01-01T00:00:02Z');
INSERT INTO usage_routing_snapshots (
  request_id, candidate_id, candidate_index, selected_provider_id,
  selected_endpoint_id, selected_provider_api_key_id, created_at, updated_at
)
VALUES (
  'request-1', 'candidate-1', 2, 'provider-1',
  'endpoint-1', 'provider-key-1', '1970-01-01T00:00:01Z', '1970-01-01T00:00:02Z'
);
"#,
        )
        .execute(&pool)
        .await
        .expect("sqlite export rows should seed");

    let encoded = export_sqlite_core_jsonl(&pool, 1_700_000_000)
        .await
        .expect("sqlite export should encode");
    let import_plan = build_import_plan(&encoded).expect("sqlite export should decode");

    assert_eq!(
        import_plan.manifest.source_driver,
        Some(DatabaseDriver::Sqlite)
    );
    assert_eq!(import_plan.manifest.domains, sqlite_core_export_domains());
    assert_eq!(
        import_plan.rows(ExportDomain::Users)[0].payload["email"],
        "owner@example.com"
    );
    assert_eq!(
        import_plan.rows(ExportDomain::ApiKeys)[0].payload["key_encrypted"],
        "ciphertext-1"
    );
    assert_eq!(
        import_plan.rows(ExportDomain::ProviderKeys)[0].payload["encrypted_key"],
        "ciphertext-provider"
    );
    assert_eq!(import_plan.rows(ExportDomain::Usage)[0].id, "request-1");
    assert_eq!(import_plan.rows(ExportDomain::Billing).len(), 2);
    assert_eq!(
        import_plan.rows(ExportDomain::Billing)[0].payload["__table"],
        "billing_rules"
    );
    assert_eq!(
        import_plan.rows(ExportDomain::Billing)[0].payload["dimension_mappings"]["input"],
        "input_tokens"
    );
    assert!(import_plan
        .rows(ExportDomain::Auxiliary)
        .iter()
        .any(|row| row.payload["__table"] == "audit_logs" && row.payload["id"] == "audit-1"));
    assert!(import_plan
        .rows(ExportDomain::Auxiliary)
        .iter()
        .any(|row| row.payload["__table"] == "usage_body_blobs"
            && row.payload["payload_gzip"] == json!([0, 17, 127, 255])));
    assert!(import_plan
        .rows(ExportDomain::Auxiliary)
        .iter()
        .any(|row| row.payload["__table"] == "usage_routing_snapshots"
            && row.payload["candidate_id"] == "candidate-1"
            && row.payload["selected_provider_id"] == "provider-1"));
    assert!(import_plan
        .rows(ExportDomain::Auxiliary)
        .iter()
        .any(|row| row.payload["__table"] == "payment_gateway_configs"
            && row.payload["merchant_key_encrypted"] == "ciphertext-merchant"));
    assert!(import_plan
        .rows(ExportDomain::Auxiliary)
        .iter()
        .any(
            |row| row.payload["__table"] == "referral_rewards" && row.payload["amount_usd"] == 2.5
        ));

    let target_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("target sqlite pool should connect");
    run_sqlite_migrations(&target_pool)
        .await
        .expect("target sqlite migrations should run");
    let imported = import_sqlite_jsonl(&target_pool, &encoded)
        .await
        .expect("sqlite import should load exported rows");
    assert_eq!(imported, import_plan_row_count(&import_plan));

    let imported_api_key =
        sqlx::query_as::<_, (String,)>("SELECT key_encrypted FROM api_keys WHERE id = 'api-key-1'")
            .fetch_one(&target_pool)
            .await
            .expect("imported api key should load");
    assert_eq!(imported_api_key.0, "ciphertext-1");

    let imported_usage = sqlx::query_as::<_, (String, i64, String)>(
        "SELECT request_id, created_at_unix_ms, typeof(created_at_unix_ms) FROM \"usage\" WHERE request_id = 'request-1'",
    )
    .fetch_one(&target_pool)
    .await
    .expect("imported usage should load");
    assert_eq!(
        imported_usage,
        ("request-1".to_string(), 1, "integer".to_string())
    );

    let imported_global_model_timestamps = sqlx::query_as::<_, (i64, i64, String, String)>(
        r#"
SELECT created_at, updated_at, typeof(created_at), typeof(updated_at)
FROM global_models
WHERE id = 'global-model-1'
"#,
    )
    .fetch_one(&target_pool)
    .await
    .expect("imported global model timestamps should decode as integers");
    assert_eq!(
        imported_global_model_timestamps,
        (1, 2, "integer".to_string(), "integer".to_string())
    );

    let imported_billing_rule = sqlx::query_as::<_, (String,)>(
        "SELECT expression FROM billing_rules WHERE id = 'billing-rule-1'",
    )
    .fetch_one(&target_pool)
    .await
    .expect("imported billing rule should load");
    assert_eq!(imported_billing_rule.0, "input_tokens * 0.01");

    let imported_body: Vec<u8> = sqlx::query_scalar(
        "SELECT payload_gzip FROM usage_body_blobs WHERE body_ref = 'body-ref-1'",
    )
    .fetch_one(&target_pool)
    .await
    .expect("imported body blob should load");
    assert_eq!(imported_body, vec![0, 17, 127, 255]);

    let imported_routing = sqlx::query_as::<_, (String, i64, String)>(
        r#"
SELECT candidate_id, candidate_index, selected_provider_id
FROM usage_routing_snapshots
WHERE request_id = 'request-1'
"#,
    )
    .fetch_one(&target_pool)
    .await
    .expect("imported routing snapshot should load");
    assert_eq!(
        imported_routing,
        ("candidate-1".to_string(), 2, "provider-1".to_string())
    );

    for omit_request_body_details in [false, true] {
        let target_config = SqlDatabaseConfig {
            url: format!(
                "sqlite://{}",
                temp_dir
                    .join(format!("copy-{omit_request_body_details}.db"))
                    .display()
            ),
            ..SqlDatabaseConfig::sqlite_default()
        };
        let copied_pool = crate::driver::sqlite::SqlitePoolFactory::new(target_config.clone())
            .expect("copy factory should build")
            .connect_lazy()
            .expect("copy pool should connect");
        run_sqlite_migrations(&copied_pool)
            .await
            .expect("copy migrations should run");
        let count = copy_database_records(
            source_config.clone(),
            target_config,
            vec![],
            1,
            DataCopyOptions {
                omit_request_body_details,
            },
        )
        .await
        .expect("default copy should succeed");
        assert_eq!(count, imported - usize::from(omit_request_body_details));
        let merchant_key: String = sqlx::query_scalar("SELECT merchant_key_encrypted FROM payment_gateway_configs WHERE provider = 'legacy-gateway'")
            .fetch_one(&copied_pool).await.expect("gateway config should be copied");
        assert_eq!(merchant_key, "ciphertext-merchant");
        let reward: f64 =
            sqlx::query_scalar("SELECT amount_usd FROM referral_rewards WHERE id = 'reward-1'")
                .fetch_one(&copied_pool)
                .await
                .expect("referral reward should be copied");
        assert_eq!(reward, 2.5);
        let body_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_body_blobs")
            .fetch_one(&copied_pool)
            .await
            .expect("body count should load");
        assert_eq!(body_count, i64::from(!omit_request_body_details));
        let body_ref: Option<String> = sqlx::query_scalar(
            "SELECT request_body_ref FROM usage_http_audits WHERE request_id = 'request-1'",
        )
        .fetch_one(&copied_pool)
        .await
        .expect("audit should be copied");
        assert_eq!(
            body_ref.as_deref(),
            if omit_request_body_details {
                None
            } else {
                Some("body-ref-1")
            }
        );
        let routing: String = sqlx::query_scalar("SELECT selected_provider_id FROM usage_routing_snapshots WHERE request_id = 'request-1'")
            .fetch_one(&copied_pool).await.expect("routing snapshot should be retained");
        assert_eq!(routing, "provider-1");
        copied_pool.close().await;
    }
    pool.close().await;
    std::fs::remove_dir_all(temp_dir).expect("temporary export directory should be removed");
}

fn unique_suffix() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let counter = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{:016x}", nanos ^ counter.rotate_left(17))
}

fn import_plan_row_count(plan: &DataImportPlan) -> usize {
    plan.manifest
        .domains
        .iter()
        .map(|domain| plan.rows(*domain).len())
        .sum()
}
