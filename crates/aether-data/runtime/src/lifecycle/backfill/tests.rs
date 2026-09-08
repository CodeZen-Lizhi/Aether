use sqlx::{query, query_as, query_scalar};

use super::sqlite::{pending_backfills_from_applied, AppliedBackfill};
use super::{pending_sqlite_backfills, run_sqlite_backfills};
use crate::lifecycle::migrate::run_sqlite_migrations;

const LEGACY_SYNC_ENABLED_ACTIVE_FLAGS_VERSION: i64 = 20260517012000;
const LEGACY_SYNC_ENABLED_ACTIVE_FLAGS_SQL: &str =
    include_str!("../../../backfills/sqlite/20260517012000_sync_legacy_enabled_active_flags.sql");

#[test]
fn legacy_enabled_backfill_preserves_canonical_active_flags() {
    for table in ["providers", "provider_endpoints", "models"] {
        assert!(
            LEGACY_SYNC_ENABLED_ACTIVE_FLAGS_SQL
                .contains(&format!("UPDATE {table}\nSET enabled = is_active")),
            "legacy {table}.enabled should be initialized from canonical is_active"
        );
    }
    assert!(
        !LEGACY_SYNC_ENABLED_ACTIVE_FLAGS_SQL.contains("is_active = enabled"),
        "the corrected script must not enable canonically disabled records"
    );
}

#[test]
fn pending_backfills_from_applied_returns_all_versions_when_none_applied() {
    let versions = pending_backfills_from_applied(&[])
        .into_iter()
        .map(|item| item.version)
        .collect::<Vec<_>>();
    assert_eq!(
        versions,
        vec![
            20260422120000,
            20260505120000,
            20260517012000,
            20260716010000,
        ]
    );
}

#[test]
fn pending_backfills_from_applied_skips_versions_already_applied() {
    let versions = pending_backfills_from_applied(&[AppliedBackfill {
        version: 20260422120000,
        checksum: Vec::new(),
    }])
    .into_iter()
    .map(|item| item.version)
    .collect::<Vec<_>>();
    assert_eq!(
        versions,
        vec![20260505120000, 20260517012000, 20260716010000,]
    );
}

#[test]
fn corrected_legacy_backfill_is_not_requeued_after_application() {
    let pending_versions = pending_backfills_from_applied(&[AppliedBackfill {
        version: LEGACY_SYNC_ENABLED_ACTIVE_FLAGS_VERSION,
        checksum: Vec::new(),
    }])
    .into_iter()
    .map(|item| item.version)
    .collect::<Vec<_>>();

    assert!(!pending_versions.contains(&LEGACY_SYNC_ENABLED_ACTIVE_FLAGS_VERSION));
}

#[tokio::test]
async fn sqlite_backfills_apply_portable_repairs_and_record_versions() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite backfill test pool should connect");
    run_sqlite_migrations(&pool)
        .await
        .expect("sqlite schema should migrate");

    query(
        r#"
INSERT INTO api_keys (
    id, user_id, key_hash, total_requests, total_tokens, total_cost_usd, created_at, updated_at
) VALUES (
    'sqlite-backfill-api-key', 'sqlite-backfill-user', 'sqlite-backfill-hash',
    77, 7777, 77.0, 1, 1
)
"#,
    )
    .execute(&pool)
    .await
    .expect("sqlite api key fixture should insert");
    query(
        r#"
INSERT INTO provider_api_keys (
    id, provider_id, name, total_tokens, created_at, updated_at
) VALUES (
    'sqlite-backfill-provider-key', 'sqlite-backfill-provider', 'Portable key', 7777, 1, 1
)
"#,
    )
    .execute(&pool)
    .await
    .expect("sqlite provider key fixture should insert");
    query(
        r#"
INSERT INTO global_models (
    id, name, display_name, usage_count, created_at, updated_at
) VALUES (
    'sqlite-backfill-model', 'gpt-portable', 'GPT Portable', 77, 1, 1
)
"#,
    )
    .execute(&pool)
    .await
    .expect("sqlite global model fixture should insert");
    query(
        r#"
INSERT INTO providers (
    id, name, provider_type, enabled, is_active, created_at, updated_at
) VALUES (
    'sqlite-backfill-provider', 'SQLite Backfill Provider', 'openai', 1, 0, 1, 1
)
"#,
    )
    .execute(&pool)
    .await
    .expect("sqlite provider flag fixture should insert");
    query(
        r#"
INSERT INTO provider_endpoints (
    id, provider_id, name, base_url, enabled, is_active, created_at, updated_at
) VALUES (
    'sqlite-backfill-endpoint', 'sqlite-backfill-provider', 'Default',
    'https://example.invalid', 1, 0, 1, 1
)
"#,
    )
    .execute(&pool)
    .await
    .expect("sqlite provider endpoint flag fixture should insert");
    query(
        r#"
INSERT INTO models (
    id, provider_id, provider_model_name, enabled, is_active, created_at, updated_at
) VALUES (
    'sqlite-backfill-provider-model', 'sqlite-backfill-provider', 'gpt-portable',
    1, 0, 1, 1
)
"#,
    )
    .execute(&pool)
    .await
    .expect("sqlite model flag fixture should insert");
    query(
        r#"
INSERT INTO "usage" (
    request_id,
    api_key_id,
    provider_api_key_id,
    model,
    status,
    total_tokens,
    input_tokens,
    output_tokens,
    cache_read_input_tokens,
    api_format,
    total_cost_usd,
    created_at,
    created_at_unix_ms,
    updated_at_unix_secs
) VALUES
    (
        'sqlite-backfill-completed',
        'sqlite-backfill-api-key',
        'sqlite-backfill-provider-key',
        'gpt-portable',
        'completed',
        0,
        120,
        30,
        20,
        'openai',
        1.25,
        1714979289,
        1714979289,
        1714979289
    ),
    (
        'sqlite-backfill-pending',
        'sqlite-backfill-api-key',
        'sqlite-backfill-provider-key',
        'gpt-portable',
        'pending',
        777,
        700,
        77,
        0,
        'openai',
        0.25,
        1714979349,
        1714979349,
        1714979349
    )
"#,
    )
    .execute(&pool)
    .await
    .expect("sqlite usage fixtures should insert");
    query(
        r#"
INSERT INTO usage_settlement_snapshots (
    request_id,
    billing_status,
    billing_effective_input_tokens,
    billing_output_tokens,
    billing_cache_creation_tokens,
    billing_cache_read_tokens,
    created_at,
    updated_at
) VALUES (
    'sqlite-backfill-completed', 'settled', 100, 30, 10, 20, 1, 1
)
"#,
    )
    .execute(&pool)
    .await
    .expect("sqlite settlement fixture should insert");

    let pending_versions = pending_sqlite_backfills(&pool)
        .await
        .expect("sqlite pending backfills should load")
        .into_iter()
        .map(|item| item.version)
        .collect::<Vec<_>>();
    assert_eq!(
        pending_versions,
        vec![
            20260422120000,
            20260505120000,
            20260517012000,
            20260716010000
        ]
    );

    run_sqlite_backfills(&pool)
        .await
        .expect("sqlite backfills should apply");
    assert!(pending_sqlite_backfills(&pool)
        .await
        .expect("sqlite pending backfills should reload")
        .is_empty());

    let applied_versions: Vec<i64> =
        query_scalar("SELECT version FROM schema_backfills ORDER BY version")
            .fetch_all(&pool)
            .await
            .expect("sqlite applied backfill versions should load");
    assert_eq!(
        applied_versions,
        vec![
            20260422120000,
            20260505120000,
            20260517012000,
            20260716010000
        ]
    );
    let api_key_stats: (i64, i64, f64, Option<i64>) = query_as(
        "SELECT total_requests, total_tokens, total_cost_usd, last_used_at FROM api_keys WHERE id = 'sqlite-backfill-api-key'",
    )
    .fetch_one(&pool)
    .await
    .expect("sqlite api key backfill result should load");
    assert_eq!(api_key_stats, (2, 160, 1.5, Some(1714979349)));
    let provider_total_tokens: i64 = query_scalar(
        "SELECT total_tokens FROM provider_api_keys WHERE id = 'sqlite-backfill-provider-key'",
    )
    .fetch_one(&pool)
    .await
    .expect("sqlite provider key total should load");
    assert_eq!(provider_total_tokens, 160);
    let global_usage_count: i64 =
        query_scalar("SELECT usage_count FROM global_models WHERE id = 'sqlite-backfill-model'")
            .fetch_one(&pool)
            .await
            .expect("sqlite global model count should load");
    assert_eq!(global_usage_count, 1);
    for table in ["providers", "provider_endpoints", "models"] {
        let enabled: i64 =
            query_scalar(&format!("SELECT enabled FROM {table} WHERE is_active = 0"))
                .fetch_one(&pool)
                .await
                .unwrap_or_else(|error| panic!("sqlite {table} legacy flag should load: {error}"));
        assert_eq!(enabled, 0, "sqlite {table}.enabled should follow is_active");
    }

    run_sqlite_backfills(&pool)
        .await
        .expect("sqlite backfills should be idempotent");
    let applied_count: i64 = query_scalar("SELECT COUNT(*) FROM schema_backfills")
        .fetch_one(&pool)
        .await
        .expect("sqlite applied backfill count should load");
    assert_eq!(applied_count, 4);

    query("UPDATE schema_backfills SET checksum = X'00' WHERE version = 20260422120000")
        .execute(&pool)
        .await
        .expect("sqlite checksum compatibility fixture should update");
    assert!(pending_sqlite_backfills(&pool)
        .await
        .expect("checksum drift should retain the historical compatibility policy")
        .is_empty());

    query(
        r#"
INSERT INTO schema_backfills (
    version, description, success, checksum, execution_time
) VALUES (
    99999999999999, 'missing embedded backfill', 1, X'', 0
)
"#,
    )
    .execute(&pool)
    .await
    .expect("unknown sqlite backfill fixture should insert");
    let error = pending_sqlite_backfills(&pool)
        .await
        .expect_err("unknown applied sqlite backfill should fail validation");
    assert!(matches!(
        error,
        sqlx::migrate::MigrateError::VersionMissing(99999999999999)
    ));
}

#[tokio::test]
async fn sqlite_backfill_sql_and_version_record_commit_atomically() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite backfill transaction test pool should connect");
    run_sqlite_migrations(&pool)
        .await
        .expect("sqlite schema should migrate");
    query(
        r#"
INSERT INTO global_models (
    id, name, display_name, usage_count, created_at, updated_at
) VALUES (
    'sqlite-backfill-rollback-model', 'rollback-model', 'Rollback Model', 77, 1, 1
)
"#,
    )
    .execute(&pool)
    .await
    .expect("sqlite rollback global model fixture should insert");
    query(
        r#"
CREATE TRIGGER reject_global_model_backfill
BEFORE UPDATE OF usage_count ON global_models
BEGIN
    SELECT RAISE(ABORT, 'forced global model backfill failure');
END
"#,
    )
    .execute(&pool)
    .await
    .expect("sqlite rollback trigger should create");

    run_sqlite_backfills(&pool)
        .await
        .expect_err("forced sqlite backfill failure should propagate");
    let applied_versions: Vec<i64> =
        query_scalar("SELECT version FROM schema_backfills ORDER BY version")
            .fetch_all(&pool)
            .await
            .expect("sqlite partial applied versions should load");
    assert_eq!(applied_versions, vec![20260422120000]);
    let usage_count: i64 = query_scalar(
        "SELECT usage_count FROM global_models WHERE id = 'sqlite-backfill-rollback-model'",
    )
    .fetch_one(&pool)
    .await
    .expect("sqlite rolled back global model should load");
    assert_eq!(usage_count, 77);

    query("DROP TRIGGER reject_global_model_backfill")
        .execute(&pool)
        .await
        .expect("sqlite rollback trigger should drop");
    run_sqlite_backfills(&pool)
        .await
        .expect("sqlite backfills should resume after the failed transaction");
    let applied_count: i64 = query_scalar("SELECT COUNT(*) FROM schema_backfills")
        .fetch_one(&pool)
        .await
        .expect("sqlite resumed applied backfill count should load");
    assert_eq!(applied_count, 4);
}
