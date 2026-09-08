use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use sqlx::{query, query_scalar, SqlitePool};

const RETIRED_TABLES: &[&str] = &[
    "pool_member_scores",
    "user_group_members",
    "user_groups",
    "user_invite_codes",
    "user_referrals",
    "user_oauth_links",
    "oauth_providers",
    "ldap_configs",
];

#[tokio::test]
async fn migrated_sqlite_columns_match_the_generated_logical_schema() {
    const GENERATED_SQLITE_SCHEMA: &[&str] = &[
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/generated/sqlite/baseline/001_identity.sql"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/generated/sqlite/baseline/002_provider_catalog.sql"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/generated/sqlite/baseline/003_auth_config.sql"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/generated/sqlite/baseline/004_proxy_nodes.sql"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/generated/sqlite/baseline/005_wallet_billing.sql"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/generated/sqlite/baseline/006_usage.sql"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/generated/sqlite/baseline/007_stats.sql"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/generated/sqlite/baseline/008_background_tasks.sql"
        )),
    ];

    let migrated = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("migrated sqlite pool should connect");
    super::run_sqlite_migrations(&migrated)
        .await
        .expect("sqlite migrations should run");

    let generated = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("generated sqlite pool should connect");
    for source in GENERATED_SQLITE_SCHEMA {
        sqlx::raw_sql(source)
            .execute(&generated)
            .await
            .expect("generated sqlite schema fragment should run");
    }

    let migrated_tables = sqlite_portable_table_names(&migrated).await;
    let mut generated_tables = sqlite_portable_table_names(&generated).await;
    for table in RETIRED_TABLES {
        assert!(
            generated_tables.remove(*table),
            "missing historical table {table}"
        );
        assert!(
            !migrated_tables.contains(*table),
            "retired table {table} remains"
        );
    }
    assert_eq!(migrated_tables, generated_tables);

    for table in generated_tables {
        let migrated_columns = sqlite_table_column_names(&migrated, &table).await;
        let mut generated_columns = sqlite_table_column_names(&generated, &table).await;
        let retired_columns: &[&str] = match table.as_str() {
            "providers" => &[
                "priority",
                "provider_priority",
                "keep_priority_on_conversion",
            ],
            "provider_api_keys" => &["global_priority_by_format"],
            _ => &[],
        };
        for column in retired_columns {
            assert!(generated_columns.remove(*column));
        }
        assert_eq!(
            migrated_columns, generated_columns,
            "SQLite migration columns drifted for table {table}"
        );
    }

    let migrated_indexes = sqlite_named_index_names(&migrated).await;
    let generated_indexes = sqlite_named_index_names(&generated).await;
    let missing_indexes = generated_indexes
        .difference(&migrated_indexes)
        .cloned()
        .collect::<BTreeSet<_>>();
    assert!(
        missing_indexes.is_empty(),
        "SQLite migrations are missing generated logical indexes: {missing_indexes:?}"
    );
}

async fn sqlite_portable_table_names(pool: &SqlitePool) -> BTreeSet<String> {
    query_scalar::<_, String>(
        r#"
SELECT name
FROM sqlite_master
WHERE type = 'table'
  AND name NOT LIKE 'sqlite_%'
  AND name NOT IN ('_sqlx_migrations', 'schema_backfills')
ORDER BY name
"#,
    )
    .fetch_all(pool)
    .await
    .expect("sqlite table names should load")
    .into_iter()
    .collect()
}

async fn sqlite_table_column_names(pool: &SqlitePool, table: &str) -> BTreeSet<String> {
    query_scalar::<_, String>("SELECT name FROM pragma_table_info(?) ORDER BY cid")
        .bind(table)
        .fetch_all(pool)
        .await
        .expect("sqlite table columns should load")
        .into_iter()
        .collect()
}

async fn sqlite_named_index_names(pool: &SqlitePool) -> BTreeSet<String> {
    sqlx::query_as::<_, (String, String)>(
        r#"
SELECT name, tbl_name
FROM sqlite_master
WHERE type = 'index'
  AND sql IS NOT NULL
ORDER BY name
"#,
    )
    .fetch_all(pool)
    .await
    .expect("sqlite named indexes should load")
    .into_iter()
    .filter(|(_, table)| !RETIRED_TABLES.contains(&table.as_str()))
    .map(|(name, _)| name)
    .collect()
}

#[test]
fn split_baseline_sources_match_executable_migrations() {
    fn schema_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema")
    }

    fn compose_manifest(relative_manifest: &str) -> String {
        let root = schema_root();
        let manifest_path = root.join(relative_manifest);
        let manifest = fs::read_to_string(&manifest_path)
            .unwrap_or_else(|err| panic!("failed to read {manifest_path:?}: {err}"));
        let manifest_dir = manifest_path
            .parent()
            .expect("schema manifest should have a parent directory");

        let mut output = String::new();
        for line in manifest.lines() {
            let part = line.trim();
            if part.is_empty() || part.starts_with('#') {
                continue;
            }
            let part_path = manifest_dir.join(part);
            output.push_str(
                &fs::read_to_string(&part_path)
                    .unwrap_or_else(|err| panic!("failed to read {part_path:?}: {err}")),
            );
        }
        output
    }

    assert_eq!(
        include_str!("../../../../adapters/sqlite/migrations/20260403000000_baseline.sql"),
        compose_manifest("drivers/sqlite/baseline/manifest.txt")
    );
}

#[test]
fn sqlite_migrations_do_not_use_postgres_jsonb() {
    let sqlite_sources = super::sqlite::MIGRATOR
        .iter()
        .filter(|migration| migration.migration_type.is_up_migration())
        .map(|migration| migration.sql.as_ref());

    for source in sqlite_sources {
        assert!(
            !source.to_ascii_lowercase().contains("jsonb"),
            "Postgres jsonb must stay out of SQLite migrations"
        );
    }
}

#[test]
fn worker_boot_cleanup_migration_is_enabled_for_sqlite() {
    const VERSION: i64 = 20260731000000;

    for (driver, migrator) in [("sqlite", &super::sqlite::MIGRATOR)] {
        let migration = migrator
            .iter()
            .find(|migration| migration.version == VERSION)
            .unwrap_or_else(|| panic!("{driver} worker boot cleanup migration should be embedded"));
        let sql = migration.sql.as_ref();

        for required in [
            "DELETE FROM background_task_events",
            "DELETE FROM background_task_runs",
            "id LIKE 'boot:%'",
            "owner_instance IS NOT NULL",
            "created_by = 'system'",
            "progress_message = 'worker booted'",
        ] {
            assert!(
                sql.contains(required),
                "{driver} worker boot cleanup migration is missing {required}"
            );
        }

        assert!(
            sql.find("DELETE FROM background_task_events")
                < sql.find("DELETE FROM background_task_runs"),
            "{driver} must delete child events before worker boot runs"
        );
    }
}

#[test]
fn sqlite_migrations_include_enabled_incrementals() {
    let sqlite_versions = super::sqlite::MIGRATOR
        .iter()
        .filter(|migration| migration.migration_type.is_up_migration())
        .map(|migration| migration.version)
        .collect::<Vec<_>>();

    assert_eq!(
        sqlite_versions,
        vec![
            20260403000000,
            20260507120000,
            20260508000000,
            20260509000000,
            20260509120000,
            20260510120000,
            20260511120000,
            20260511130000,
            20260512000000,
            20260512090000,
            20260512110000,
            20260516000000,
            20260518000000,
            20260519000000,
            20260519120000,
            20260519130000,
            20260520000000,
            20260520010000,
            20260524000000,
            20260527000000,
            20260528000000,
            20260528020000,
            20260725000000,
            20260725010000,
            20260725020000,
            20260725030000,
            20260725040000,
            20260727000000,
            20260731000000,
            20260821000000,
            20260901000000,
            20260902000000,
            20260902010000,
            20260903000000,
        ]
    );
}

async fn sqlite_pool_before(version: i64) -> SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("historical sqlite pool should connect");
    for migration in super::sqlite::MIGRATOR
        .iter()
        .filter(|migration| migration.version < version)
    {
        sqlx::raw_sql(&migration.sql)
            .execute(&pool)
            .await
            .expect("historical sqlite migration should apply");
    }
    pool
}

#[tokio::test]
async fn sqlite_imported_timestamp_migration_normalizes_text_storage() {
    let pool = sqlite_pool_before(20260725000000).await;

    query(
        r#"
INSERT INTO global_models (id, name, created_at, updated_at)
VALUES
  ('timestamp-rfc3339', 'timestamp-rfc3339', '1970-01-01T00:00:01Z', '1970-01-01T08:00:02+08:00'),
  ('timestamp-sqlalchemy', 'timestamp-sqlalchemy', '1970-01-01 00:00:03.123456', '1970-01-01 00:00:04.987654'),
  ('timestamp-integer', 'timestamp-integer', 5, 6);
"#,
    )
    .execute(&pool)
    .await
    .expect("timestamp fixtures should insert");
    query(
        r#"
INSERT INTO "usage" (request_id, created_at_unix_ms, updated_at_unix_secs)
VALUES ('timestamp-usage', '1970-01-01T00:00:01.234900Z', '1970-01-01T00:00:02Z');
"#,
    )
    .execute(&pool)
    .await
    .expect("usage timestamp fixture should insert");

    let migration = super::sqlite::MIGRATOR
        .iter()
        .find(|migration| migration.version == 20260725000000)
        .expect("timestamp normalization migration should be embedded");
    sqlx::raw_sql(migration.sql.as_ref())
        .execute(&pool)
        .await
        .expect("timestamp normalization migration should apply");

    let rows = sqlx::query_as::<_, (String, i64, i64, String, String)>(
        r#"
SELECT id, created_at, updated_at, typeof(created_at), typeof(updated_at)
FROM global_models
WHERE id LIKE 'timestamp-%'
ORDER BY id
"#,
    )
    .fetch_all(&pool)
    .await
    .expect("normalized timestamps should decode as integers");

    assert_eq!(
        rows,
        vec![
            (
                "timestamp-integer".to_string(),
                5,
                6,
                "integer".to_string(),
                "integer".to_string(),
            ),
            (
                "timestamp-rfc3339".to_string(),
                1,
                2,
                "integer".to_string(),
                "integer".to_string(),
            ),
            (
                "timestamp-sqlalchemy".to_string(),
                3,
                4,
                "integer".to_string(),
                "integer".to_string(),
            ),
        ]
    );

    let usage_timestamps = sqlx::query_as::<_, (i64, i64, String, String)>(
        r#"
SELECT created_at_unix_ms, updated_at_unix_secs,
       typeof(created_at_unix_ms), typeof(updated_at_unix_secs)
FROM "usage"
WHERE request_id = 'timestamp-usage'
"#,
    )
    .fetch_one(&pool)
    .await
    .expect("normalized usage timestamps should decode as integers");
    assert_eq!(
        usage_timestamps,
        (1, 2, "integer".to_string(), "integer".to_string())
    );
}

#[tokio::test]
async fn sqlite_imported_timestamp_migration_rejects_non_integer_storage() {
    let pool = sqlite_pool_before(20260725000000).await;

    query(
        r#"
INSERT INTO global_models (id, name, created_at, updated_at)
VALUES ('timestamp-invalid', 'timestamp-invalid', 1.5, 1);
"#,
    )
    .execute(&pool)
    .await
    .expect("non-integer timestamp fixture should insert");

    let migration = super::sqlite::MIGRATOR
        .iter()
        .find(|migration| migration.version == 20260725000000)
        .expect("timestamp normalization migration should be embedded");
    let err = sqlx::raw_sql(migration.sql.as_ref())
        .execute(&pool)
        .await
        .expect_err("non-integer timestamp should fail the migration");
    assert!(err
        .to_string()
        .contains("imported_timestamp_storage_must_be_integer"));
}

#[tokio::test]
async fn sqlite_remaining_timestamp_migration_repairs_other_repository_domains() {
    let pool = sqlite_pool_before(20260725040000).await;

    query(
        r#"
INSERT INTO users (id, email, username, auth_source, created_at, updated_at)
VALUES ('timestamp-user', 'timestamp@example.com', 'timestamp-user', 'local', 1, 1);

INSERT INTO audit_logs (id, event_type, description, created_at)
VALUES ('timestamp-audit', 'test', 'test', '1970-01-01T00:00:01Z');

INSERT INTO request_candidates (
  id, request_id, candidate_index, status, created_at, started_at, finished_at
) VALUES (
  'timestamp-candidate', 'timestamp-request', 0, 'success',
  '1970-01-01T00:00:02Z', '1970-01-01T00:00:03Z', '1970-01-01T00:00:04Z'
);

INSERT INTO stats_daily (id, date, created_at, updated_at)
VALUES (
  'timestamp-stats', '1970-01-02',
  '1970-01-01T00:00:05Z', '1970-01-01T00:00:06Z'
);

INSERT INTO user_sessions (
  id, user_id, client_device_id, refresh_token_hash,
  last_seen_at, expires_at, created_at, updated_at
) VALUES (
  'timestamp-session', 'timestamp-user', 'device', 'hash',
  '1970-01-01T00:00:07Z', '1970-01-01T00:00:08Z',
  '1970-01-01T00:00:09Z', '1970-01-01T00:00:10Z'
);
"#,
    )
    .execute(&pool)
    .await
    .expect("remaining timestamp fixtures should insert");

    let migration = super::sqlite::MIGRATOR
        .iter()
        .find(|migration| migration.version == 20260725040000)
        .expect("remaining timestamp migration should be embedded");
    sqlx::raw_sql(migration.sql.as_ref())
        .execute(&pool)
        .await
        .expect("remaining timestamp migration should apply");

    let audit = sqlx::query_as::<_, (i64, String)>(
        "SELECT created_at, typeof(created_at) FROM audit_logs WHERE id = 'timestamp-audit'",
    )
    .fetch_one(&pool)
    .await
    .expect("normalized audit timestamp should load");
    assert_eq!(audit, (1, "integer".to_string()));

    let candidate = sqlx::query_as::<_, (i64, i64, i64, String, String, String)>(
        r#"
SELECT created_at, started_at, finished_at,
       typeof(created_at), typeof(started_at), typeof(finished_at)
FROM request_candidates
WHERE id = 'timestamp-candidate'
"#,
    )
    .fetch_one(&pool)
    .await
    .expect("normalized candidate timestamps should load");
    assert_eq!(
        candidate,
        (
            2,
            3,
            4,
            "integer".to_string(),
            "integer".to_string(),
            "integer".to_string(),
        )
    );

    let stats = sqlx::query_as::<_, (i64, i64, i64, String, String, String)>(
        r#"
SELECT date, created_at, updated_at,
       typeof(date), typeof(created_at), typeof(updated_at)
FROM stats_daily
WHERE id = 'timestamp-stats'
"#,
    )
    .fetch_one(&pool)
    .await
    .expect("normalized stats timestamps should load");
    assert_eq!(
        stats,
        (
            86_400,
            5,
            6,
            "integer".to_string(),
            "integer".to_string(),
            "integer".to_string(),
        )
    );

    let session = sqlx::query_as::<_, (i64, i64, i64, i64, String, String, String, String)>(
        r#"
SELECT last_seen_at, expires_at, created_at, updated_at,
       typeof(last_seen_at), typeof(expires_at), typeof(created_at), typeof(updated_at)
FROM user_sessions
WHERE id = 'timestamp-session'
"#,
    )
    .fetch_one(&pool)
    .await
    .expect("normalized session timestamps should load");
    assert_eq!(
        session,
        (
            7,
            8,
            9,
            10,
            "integer".to_string(),
            "integer".to_string(),
            "integer".to_string(),
            "integer".to_string(),
        )
    );
}

#[tokio::test]
async fn sqlite_remaining_timestamp_migration_rejects_invalid_storage() {
    let pool = sqlite_pool_before(20260725040000).await;

    query(
        r#"
INSERT INTO audit_logs (id, event_type, description, created_at)
VALUES ('timestamp-invalid-audit', 'test', 'test', 1.5);
"#,
    )
    .execute(&pool)
    .await
    .expect("invalid timestamp fixture should insert");

    let migration = super::sqlite::MIGRATOR
        .iter()
        .find(|migration| migration.version == 20260725040000)
        .expect("remaining timestamp migration should be embedded");
    let err = sqlx::raw_sql(migration.sql.as_ref())
        .execute(&pool)
        .await
        .expect_err("invalid remaining timestamp should fail the migration");
    assert!(err.to_string().contains("invalid_count = 0"));
}

#[tokio::test]
async fn endpoint_api_root_migration_moves_v1_from_stored_default_paths() {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("sqlite pool should connect");
    query(
        r#"
CREATE TABLE providers (
  id TEXT PRIMARY KEY,
  provider_type TEXT NOT NULL
);
"#,
    )
    .execute(&pool)
    .await
    .expect("providers table should be created");
    query(
        r#"
CREATE TABLE provider_endpoints (
  id TEXT PRIMARY KEY,
  provider_id TEXT NOT NULL,
  api_format TEXT NOT NULL,
  base_url TEXT NOT NULL,
  custom_path TEXT
);
"#,
    )
    .execute(&pool)
    .await
    .expect("provider_endpoints table should be created");
    query(
        r#"
INSERT INTO providers (id, provider_type) VALUES
  ('provider-custom', 'custom'),
  ('provider-fixed-vertex', 'vertex_ai'),
  ('provider-fixed-grok', 'grok');
"#,
    )
    .execute(&pool)
    .await
    .expect("providers fixture should insert");
    query(
        r#"
INSERT INTO provider_endpoints (id, provider_id, api_format, base_url, custom_path) VALUES
  ('openai-root', 'provider-custom', 'openai:chat', 'https://api.openai.example', NULL),
  ('responses-root', 'provider-custom', 'openai:responses', 'https://responses.example.com', NULL),
  ('responses-compact-root', 'provider-custom', 'openai:responses:compact', 'https://compact.example.com', NULL),
  ('openai-path-root', 'provider-custom', 'openai:chat', 'https://proxy.example.com/api', NULL),
  ('openai-old-default-path', 'provider-custom', 'openai:chat', 'https://proxy.example.com/api?tenant=demo', '/v1/chat/completions'),
  ('openai-mismatched-custom-path', 'provider-custom', 'openai:chat', 'https://proxy.example.com/api', '/v1/responses'),
  ('openai-v1-slash-old-default', 'provider-custom', 'openai:chat', 'https://already-versioned.example.com/v1/', '/v1/chat/completions'),
  ('openai-v4-old-default-path', 'provider-custom', 'openai:chat', 'https://open.bigmodel.cn/api/coding/paas/v4', '/v1/chat/completions'),
  ('embedding-root', 'provider-custom', 'openai:embedding', 'https://embedding.example.com', NULL),
  ('embedding-v4-old-default-path', 'provider-custom', 'openai:embedding', 'https://embedding.example.com/api/v4', '/v1/embeddings'),
  ('jina-embedding-root', 'provider-custom', 'jina:embedding', 'https://api.jina.example', NULL),
  ('rerank-old-default-path', 'provider-custom', 'openai:rerank', 'https://rerank.example.com/api', '/v1/rerank'),
  ('jina-rerank-old-default-path', 'provider-custom', 'jina:rerank', 'https://api.jina.example?tenant=demo', '/v1/rerank'),
  ('image-root', 'provider-custom', 'openai:image', 'https://image.example.com', NULL),
  ('image-edit-custom-path', 'provider-custom', 'openai:image', 'https://image.example.com/api', '/v1/images/edits'),
  ('image-v4-edit-custom-path', 'provider-custom', 'openai:image', 'https://image.example.com/api/v4', '/v1/images/edits'),
  ('video-root', 'provider-custom', 'openai:video', 'https://video.example.com', NULL),
  ('video-v1beta-old-default-path', 'provider-custom', 'openai:video', 'https://video.example.com/api/v1beta', '/v1/videos'),
  ('video-versioned-root', 'provider-custom', 'openai:video', 'https://ark.example.com/api/v3', NULL),
  ('google-versioned-segment-root', 'provider-custom', 'openai:embedding', 'https://generativelanguage.googleapis.com/v1beta/openai', NULL),
  ('gemini-root', 'provider-custom', 'gemini:generate_content', 'https://generativelanguage.googleapis.com', NULL),
  ('gemini-old-default-path', 'provider-custom', 'gemini:generate_content', 'https://generativelanguage.googleapis.com?tenant=demo', '/v1beta/models/{model}:{action}'),
  ('gemini-custom-path', 'provider-custom', 'gemini:generate_content', 'https://proxy.example.com/google', '/v1beta/models/gemini-upstream:generateContent'),
  ('gemini-versioned-old-default', 'provider-custom', 'gemini:generate_content', 'https://generativelanguage.googleapis.com/v1beta', '/v1beta/models/{model}:{action}'),
  ('gemini-embedding-root', 'provider-custom', 'gemini:embedding', 'https://generativelanguage.googleapis.com', NULL),
  ('gemini-embedding-old-default', 'provider-custom', 'gemini:embedding', 'https://generativelanguage.googleapis.com', '/v1beta/models/{model}:embedContent'),
  ('gemini-video-root', 'provider-custom', 'gemini:video', 'https://generativelanguage.googleapis.com', NULL),
  ('gemini-video-versioned-old-default', 'provider-custom', 'gemini:video', 'https://generativelanguage.googleapis.com/v1beta', '/v1beta/models/{model}:predictLongRunning'),
  ('fixed-vertex-gemini-root', 'provider-fixed-vertex', 'gemini:embedding', 'https://aiplatform.googleapis.com', NULL),
  ('claude-path-root', 'provider-custom', 'claude:messages', 'https://proxy.example.com/anthropic', NULL),
  ('claude-old-default-path', 'provider-custom', 'claude:messages', 'https://proxy.example.com/anthropic', '/v1/messages'),
  ('fixed-grok-root', 'provider-fixed-grok', 'openai:chat', 'https://grok.com', NULL);
"#,
    )
    .execute(&pool)
    .await
    .expect("endpoint fixture should insert");

    let migration = super::sqlite::MIGRATOR
        .iter()
        .find(|migration| migration.version == 20260528000000)
        .expect("endpoint API root migration should be embedded");
    sqlx::raw_sql(migration.sql.as_ref())
        .execute(&pool)
        .await
        .expect("endpoint API root migration should apply");

    let rows: Vec<(String, String, Option<String>)> =
        sqlx::query_as("SELECT id, base_url, custom_path FROM provider_endpoints ORDER BY id")
            .fetch_all(&pool)
            .await
            .expect("endpoint rows should load");
    let rows = rows
        .into_iter()
        .map(|(id, base_url, custom_path)| (id, (base_url, custom_path)))
        .collect::<std::collections::BTreeMap<_, _>>();

    assert_eq!(
        rows.get("openai-root"),
        Some(&("https://api.openai.example/v1".to_string(), None))
    );
    assert_eq!(
        rows.get("responses-root"),
        Some(&("https://responses.example.com/v1".to_string(), None))
    );
    assert_eq!(
        rows.get("responses-compact-root"),
        Some(&("https://compact.example.com/v1".to_string(), None))
    );
    assert_eq!(
        rows.get("openai-path-root"),
        Some(&("https://proxy.example.com/api/v1".to_string(), None))
    );
    assert_eq!(
        rows.get("openai-old-default-path"),
        Some(&(
            "https://proxy.example.com/api/v1?tenant=demo".to_string(),
            None
        ))
    );
    assert_eq!(
        rows.get("openai-mismatched-custom-path"),
        Some(&(
            "https://proxy.example.com/api/v1".to_string(),
            Some("/responses".to_string())
        ))
    );
    assert_eq!(
        rows.get("openai-v1-slash-old-default"),
        Some(&(
            "https://already-versioned.example.com/v1/".to_string(),
            None
        ))
    );
    assert_eq!(
        rows.get("openai-v4-old-default-path"),
        Some(&(
            "https://open.bigmodel.cn/api/coding/paas/v4".to_string(),
            None
        ))
    );
    assert_eq!(
        rows.get("embedding-root"),
        Some(&("https://embedding.example.com/v1".to_string(), None))
    );
    assert_eq!(
        rows.get("embedding-v4-old-default-path"),
        Some(&("https://embedding.example.com/api/v4".to_string(), None))
    );
    assert_eq!(
        rows.get("jina-embedding-root"),
        Some(&("https://api.jina.example/v1".to_string(), None))
    );
    assert_eq!(
        rows.get("rerank-old-default-path"),
        Some(&("https://rerank.example.com/api/v1".to_string(), None))
    );
    assert_eq!(
        rows.get("jina-rerank-old-default-path"),
        Some(&("https://api.jina.example/v1?tenant=demo".to_string(), None))
    );
    assert_eq!(
        rows.get("image-root"),
        Some(&("https://image.example.com/v1".to_string(), None))
    );
    assert_eq!(
        rows.get("image-edit-custom-path"),
        Some(&(
            "https://image.example.com/api/v1".to_string(),
            Some("/images/edits".to_string())
        ))
    );
    assert_eq!(
        rows.get("image-v4-edit-custom-path"),
        Some(&(
            "https://image.example.com/api/v4".to_string(),
            Some("/images/edits".to_string())
        ))
    );
    assert_eq!(
        rows.get("video-root"),
        Some(&("https://video.example.com/v1".to_string(), None))
    );
    assert_eq!(
        rows.get("video-v1beta-old-default-path"),
        Some(&("https://video.example.com/api/v1beta".to_string(), None))
    );
    assert_eq!(
        rows.get("video-versioned-root"),
        Some(&("https://ark.example.com/api/v3".to_string(), None))
    );
    assert_eq!(
        rows.get("google-versioned-segment-root"),
        Some(&(
            "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
            None
        ))
    );
    assert_eq!(
        rows.get("gemini-root"),
        Some(&(
            "https://generativelanguage.googleapis.com/v1beta".to_string(),
            None
        ))
    );
    assert_eq!(
        rows.get("gemini-old-default-path"),
        Some(&(
            "https://generativelanguage.googleapis.com/v1beta?tenant=demo".to_string(),
            None
        ))
    );
    assert_eq!(
        rows.get("gemini-custom-path"),
        Some(&(
            "https://proxy.example.com/google/v1beta".to_string(),
            Some("/models/gemini-upstream:generateContent".to_string())
        ))
    );
    assert_eq!(
        rows.get("gemini-versioned-old-default"),
        Some(&(
            "https://generativelanguage.googleapis.com/v1beta".to_string(),
            None
        ))
    );
    assert_eq!(
        rows.get("gemini-embedding-root"),
        Some(&(
            "https://generativelanguage.googleapis.com/v1beta".to_string(),
            None
        ))
    );
    assert_eq!(
        rows.get("gemini-embedding-old-default"),
        Some(&(
            "https://generativelanguage.googleapis.com/v1beta".to_string(),
            None
        ))
    );
    assert_eq!(
        rows.get("gemini-video-root"),
        Some(&(
            "https://generativelanguage.googleapis.com/v1beta".to_string(),
            None
        ))
    );
    assert_eq!(
        rows.get("gemini-video-versioned-old-default"),
        Some(&(
            "https://generativelanguage.googleapis.com/v1beta".to_string(),
            None
        ))
    );
    assert_eq!(
        rows.get("fixed-vertex-gemini-root"),
        Some(&("https://aiplatform.googleapis.com".to_string(), None))
    );
    assert_eq!(
        rows.get("claude-path-root"),
        Some(&("https://proxy.example.com/anthropic/v1".to_string(), None))
    );
    assert_eq!(
        rows.get("claude-old-default-path"),
        Some(&("https://proxy.example.com/anthropic/v1".to_string(), None))
    );
    assert_eq!(
        rows.get("fixed-grok-root"),
        Some(&("https://grok.com".to_string(), None))
    );
}

#[test]
fn fresh_sqlite_usage_schema_projects_upstream_stream_mode() {
    let sqlite_baseline =
        include_str!("../../../../adapters/sqlite/migrations/20260403000000_baseline.sql");

    assert!(sqlite_baseline.contains("upstream_is_stream INTEGER"));
}

#[tokio::test]
async fn sqlite_migrations_create_core_config_tables() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should connect");

    let pending = super::prepare_sqlite_database_for_startup(&pool)
        .await
        .expect("sqlite startup preparation should inspect pending migrations");
    assert!(
        !pending.is_empty(),
        "fresh sqlite databases should report pending migrations before migration"
    );

    super::run_sqlite_migrations(&pool)
        .await
        .expect("sqlite migrations should run");

    let pending = super::prepare_sqlite_database_for_startup(&pool)
        .await
        .expect("sqlite startup preparation should inspect applied migrations");
    assert!(
        pending.is_empty(),
        "sqlite startup preparation should report no pending migrations after migration"
    );

    for table_name in [
        "users",
        "user_preferences",
        "user_sessions",
        "api_keys",
        "management_tokens",
        "billing_rules",
        "dimension_collectors",
        "providers",
        "provider_api_keys",
        "provider_endpoints",
        "models",
        "global_models",
        "system_configs",
        "auth_modules",
        "proxy_nodes",
        "wallets",
        "wallet_transactions",
        "wallet_daily_usage_ledgers",
        "payment_orders",
        "payment_callbacks",
        "refund_requests",
        "redeem_code_batches",
        "redeem_codes",
    ] {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
        )
        .bind(table_name)
        .fetch_one(&pool)
        .await
        .expect("sqlite_master query should succeed");
        assert_eq!(exists, 1, "missing sqlite table {table_name}");
    }

    let tables = sqlite_portable_table_names(&pool).await;
    for table in RETIRED_TABLES {
        assert!(
            !tables.contains(*table),
            "retired sqlite table {table} remains"
        );
    }
    assert!(sqlite_table_column_names(&pool, "provider_api_keys")
        .await
        .contains("internal_priority"));

    let total_adjusted_exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('wallets') WHERE name = ?")
            .bind("total_adjusted")
            .fetch_one(&pool)
            .await
            .expect("sqlite wallet column query should succeed");
    assert_eq!(
        total_adjusted_exists, 1,
        "missing sqlite wallets.total_adjusted"
    );

    let upstream_is_stream_exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('usage') WHERE name = ?")
            .bind("upstream_is_stream")
            .fetch_one(&pool)
            .await
            .expect("sqlite usage column query should succeed");
    assert_eq!(
        upstream_is_stream_exists, 1,
        "missing sqlite usage.upstream_is_stream"
    );
}
