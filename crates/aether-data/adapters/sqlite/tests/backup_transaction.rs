use std::collections::BTreeMap;
use std::time::Duration;

use aether_data_contracts::repository::auth::{
    AuthApiKeyReadRepository, AuthApiKeyWriteRepository,
};
use aether_data_contracts::repository::global_models::CreateAdminGlobalModelRecord;
use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogProvider;
use aether_data_contracts::repository::proxy_nodes::{ProxyNodeWriteRepository, StoredProxyNode};
use aether_data_contracts::repository::routing_profiles::{
    CreateRoutingGroupRecord, CreateRoutingGroupVersionRecord, RoutingGroupWriteRepository,
};
use aether_data_contracts::repository::users::UserReadRepository;
use aether_data_sqlite::{
    run_migrations, DataLayerError, SqliteAuthApiKeyReadRepository, SqliteBackupTransaction,
    SqliteConnectionSource, SqliteGlobalModelReadRepository, SqlitePool,
    SqliteProviderCatalogReadRepository, SqliteProxyNodeReadRepository,
    SqliteRoutingGroupRepository, SqliteUsageReadRepository, SqliteUserReadRepository,
};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::Row;
use tokio::time::timeout;

const OPERATION_TIMEOUT: Duration = Duration::from_secs(5);

async fn migrated_database() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .acquire_timeout(OPERATION_TIMEOUT)
        .connect("sqlite::memory:")
        .await
        .expect("connect isolated database");
    seed_database(&pool).await;
    pool
}

async fn seed_database(pool: &SqlitePool) {
    run_migrations(pool).await.expect("run real migrations");
    sqlx::raw_sql(
        r#"
INSERT INTO users (
  id, username, password_hash, role, auth_source, is_active, is_deleted,
  created_at, updated_at
) VALUES ('admin', 'original-admin', 'original-password-hash', 'admin', 'local', 1, 0, 1, 1);
INSERT INTO user_preferences (id, user_id, theme, created_at, updated_at)
VALUES ('preferences', 'admin', 'light', 1, 1);
INSERT INTO user_sessions (
  id, user_id, client_device_id, refresh_token_hash, last_seen_at, expires_at,
  created_at, updated_at
) VALUES ('session', 'admin', 'device', 'synthetic-refresh-hash', 1, 2000000000, 1, 1);
INSERT INTO api_keys (id, user_id, key_hash, name, created_at, updated_at)
VALUES ('original-key', 'admin', 'original-key-hash', 'Original key', 1, 1);
INSERT INTO routing_groups (
  id, name, enabled, is_system_default, config_json, version, created_at, updated_at
) VALUES ('original-group', 'Original group', 1, 1, '{}', 1, 1, 1);
"#,
    )
    .execute(pool)
    .await
    .expect("seed synthetic administrator data");
}

async fn apply_backup_writes(source: SqliteConnectionSource) -> Result<(), DataLayerError> {
    let providers = SqliteProviderCatalogReadRepository::new(source.clone());
    let provider = StoredProviderCatalogProvider::new(
        "imported-provider".to_string(),
        "Imported provider".to_string(),
        None,
        "custom".to_string(),
    )?;
    let created = providers.create_provider(&provider, None).await?;
    assert_eq!(
        created.id, provider.id,
        "savepoint write must reload its row"
    );

    let models = SqliteGlobalModelReadRepository::new(source.clone());
    assert!(models
        .create_admin_global_model(&CreateAdminGlobalModelRecord {
            id: "imported-model".to_string(),
            name: "imported-model".to_string(),
            display_name: "Imported model".to_string(),
            is_active: true,
            default_price_per_request: None,
            default_tiered_pricing: None,
            supported_capabilities: None,
            config: None,
            usage_count: Some(3),
        })
        .await?
        .is_some());

    let proxies = SqliteProxyNodeReadRepository::new(source.clone());
    proxies
        .restore_proxy_node(&StoredProxyNode::new(
            "imported-proxy".to_string(),
            "Imported proxy".to_string(),
            "127.0.0.1".to_string(),
            1080,
            true,
            "offline".to_string(),
            30,
            0,
            0,
            0,
            0,
            0,
            false,
            false,
            1,
        )?)
        .await?;

    let routing = SqliteRoutingGroupRepository::new(source.clone());
    routing
        .create_routing_group(CreateRoutingGroupRecord {
            id: "imported-group".to_string(),
            name: "Imported group".to_string(),
            description: None,
            enabled: true,
            is_system_default: true,
            config_json: json!({"provider": provider.id}),
            version: 1,
            created_at: 2,
            updated_at: 2,
            published_at: None,
        })
        .await?;
    routing
        .create_routing_group_version(CreateRoutingGroupVersionRecord {
            id: "imported-group-version".to_string(),
            group_id: "imported-group".to_string(),
            version: 1,
            config_json: json!({"provider": provider.id}),
            created_at: 2,
            created_by: Some("admin".to_string()),
        })
        .await?;

    let keys = SqliteAuthApiKeyReadRepository::new(source.clone());
    let mut key = keys
        .list_export_api_keys_by_ids(&["original-key".to_string()])
        .await?
        .pop()
        .expect("seed key exists");
    key.name = Some("Imported key".to_string());
    key.total_requests = 9;
    assert!(keys.restore_exported_api_key(&key).await?);
    key.api_key_id = "imported-key".to_string();
    key.key_hash = "imported-key-hash".to_string();
    assert!(keys.restore_exported_api_key(&key).await?);

    let users = SqliteUserReadRepository::new(source);
    let mut profile = users
        .find_export_user_by_id("admin")
        .await?
        .expect("seed administrator exists");
    profile.username = "imported-admin".to_string();
    profile.password_hash = Some("imported-password-hash".to_string());
    assert!(users.restore_admin_profile(&profile).await?);
    assert_eq!(
        users
            .revoke_all_user_sessions("admin", chrono::Utc::now(), "backup import")
            .await?,
        1
    );

    let mut preferences = users
        .read_user_preferences("admin")
        .await?
        .expect("seed preferences exist");
    preferences.theme = "dark".to_string();
    preferences.default_provider_id = Some(provider.id);
    let restored = users
        .write_user_preferences(&preferences)
        .await?
        .expect("restored preferences must reload");
    assert_eq!(restored.theme, "dark");
    assert_eq!(
        restored.default_provider_name.as_deref(),
        Some("Imported provider")
    );
    Ok(())
}

async fn database_snapshot(pool: &SqlitePool) -> BTreeMap<String, Vec<String>> {
    let mut snapshot = BTreeMap::new();
    for table in [
        "providers",
        "provider_endpoints",
        "provider_api_keys",
        "global_models",
        "models",
        "proxy_nodes",
        "routing_groups",
        "routing_group_versions",
        "users",
        "api_keys",
        "user_preferences",
        "user_sessions",
        "referral_rewards",
    ] {
        let columns = sqlx::query(&format!("PRAGMA table_info(\"{table}\")"))
            .fetch_all(pool)
            .await
            .expect("read migrated schema")
            .iter()
            .map(|row| {
                let name: String = row.get("name");
                format!("\"{}\"", name.replace('"', "\"\""))
            })
            .collect::<Vec<_>>()
            .join(", ");
        let rows = sqlx::query_scalar(&format!(
            "SELECT json_array({columns}) FROM \"{table}\" ORDER BY id"
        ))
        .fetch_all(pool)
        .await
        .expect("snapshot every persisted column");
        snapshot.insert(table.to_string(), rows);
    }
    snapshot
}

#[tokio::test]
async fn backup_transaction_commits_all_repositories_without_relocking() {
    let pool = migrated_database().await;
    let transaction = SqliteBackupTransaction::begin(&pool, true)
        .await
        .expect("begin backup");
    let source = transaction.source();
    timeout(OPERATION_TIMEOUT, apply_backup_writes(source.clone()))
        .await
        .expect("single-connection writes and readbacks must not deadlock")
        .expect("all writes succeed");
    transaction.commit().await.expect("commit entire backup");

    let providers = SqliteProviderCatalogReadRepository::new(pool.clone());
    assert_eq!(providers.list_providers(false).await.unwrap().len(), 1);
    let user = SqliteUserReadRepository::new(pool.clone())
        .find_export_user_by_id("admin")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(user.username, "imported-admin");
    let keys = SqliteAuthApiKeyReadRepository::new(pool.clone())
        .list_export_api_keys_by_user_ids(&["admin".to_string()])
        .await
        .unwrap();
    assert_eq!(keys.len(), 2);
    assert!(keys.iter().all(|key| key.total_requests == 9));
    assert!(source.acquire().await.is_err());
    assert!(SqliteProviderCatalogReadRepository::new(source)
        .delete_provider("imported-provider")
        .await
        .is_err());
    assert_eq!(providers.list_providers(false).await.unwrap().len(), 1);
}

#[tokio::test]
async fn backup_transaction_rolls_back_all_rows_after_late_repository_failure() {
    let pool = migrated_database().await;
    sqlx::raw_sql(
        r#"
CREATE TRIGGER fail_backup_preferences BEFORE UPDATE ON user_preferences
WHEN NEW.theme = 'dark'
BEGIN
  SELECT RAISE(ABORT, 'injected final preferences failure');
END;
"#,
    )
    .execute(&pool)
    .await
    .expect("install late write failure");
    let before = database_snapshot(&pool).await;
    let transaction = SqliteBackupTransaction::begin(&pool, true).await.unwrap();
    let source = transaction.source();
    let error = timeout(OPERATION_TIMEOUT, apply_backup_writes(source.clone()))
        .await
        .expect("failure must return without deadlocking")
        .expect_err("last repository write must fail");
    assert!(error
        .to_string()
        .contains("injected final preferences failure"));
    transaction.rollback().await.expect("rollback all writes");
    assert!(source.acquire().await.is_err());
    assert_eq!(database_snapshot(&pool).await, before);
}

#[tokio::test]
async fn backup_transaction_drop_rolls_back_even_with_source_clones_and_a_guard() {
    let pool = migrated_database().await;
    let before = database_snapshot(&pool).await;
    let transaction = SqliteBackupTransaction::begin(&pool, true).await.unwrap();
    let source = transaction.source();
    apply_backup_writes(source.clone()).await.unwrap();
    let connection = source.acquire().await.unwrap();
    drop(transaction);
    assert!(source.clone().acquire().await.is_err());
    drop(connection);
    assert_eq!(database_snapshot(&pool).await, before);
}

#[tokio::test]
async fn cancelling_backup_rolls_back_and_closes_remaining_sources() {
    let pool = migrated_database().await;
    let before = database_snapshot(&pool).await;
    let transaction = SqliteBackupTransaction::begin(&pool, true).await.unwrap();
    let source = transaction.source();
    let task_source = source.clone();
    let (ready, started) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        apply_backup_writes(task_source.clone()).await.unwrap();
        let connection = task_source.acquire().await.unwrap();
        ready.send(()).unwrap();
        std::future::pending::<()>().await;
        drop(connection);
        drop(transaction);
    });
    timeout(OPERATION_TIMEOUT, started)
        .await
        .expect("backup must reach cancellation point")
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(source.acquire().await.is_err());
    assert_eq!(database_snapshot(&pool).await, before);
}

#[tokio::test]
async fn failed_backup_commit_rolls_back_deferred_foreign_keys_and_all_prior_writes() {
    let pool = migrated_database().await;
    let before = database_snapshot(&pool).await;
    let transaction = SqliteBackupTransaction::begin(&pool, true).await.unwrap();
    let source = transaction.source();
    apply_backup_writes(source.clone()).await.unwrap();
    sqlx::query(
        r#"
INSERT INTO referral_rewards (
  id, referral_id, inviter_user_id, invitee_user_id, reward_type, trigger_point,
  idempotency_key, amount_usd, created_at, updated_at
) VALUES ('orphan-reward', 'retired-referral', 'missing-user', 'admin', 'credit',
  'register', 'orphan-reward-key', 1.0, 1, 1)
"#,
    )
    .execute(&mut *source.acquire().await.unwrap())
    .await
    .expect("foreign keys stay enabled but deferred until commit");
    assert!(transaction.commit().await.is_err());
    assert!(source.acquire().await.is_err());
    assert_eq!(database_snapshot(&pool).await, before);
    let deferred: i64 = sqlx::query_scalar("PRAGMA defer_foreign_keys")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(deferred, 0);
    let enabled: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(enabled, 1);
}

#[tokio::test]
async fn read_backup_uses_one_snapshot_across_repositories_and_usage_totals() {
    let path = std::env::temp_dir().join(format!("aether-backup-{}.db", uuid::Uuid::new_v4()));
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await
        .unwrap();
    seed_database(&pool).await;
    sqlx::query(
        "INSERT INTO stats_user_daily (id, user_id, date, total_requests, input_tokens, created_at, updated_at) VALUES ('daily', 'admin', 86400, 2, 7, 1, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let writer = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    let transaction = SqliteBackupTransaction::begin(&pool, false).await.unwrap();
    let source = transaction.source();
    let users = SqliteUserReadRepository::new(source.clone());
    let original = users.find_export_user_by_id("admin").await.unwrap();
    timeout(
        OPERATION_TIMEOUT,
        sqlx::raw_sql(
            "UPDATE users SET username = 'concurrent-admin' WHERE id = 'admin'; UPDATE stats_user_daily SET total_requests = 5 WHERE id = 'daily';",
        )
        .execute(&writer),
    )
    .await
    .expect("read backup must allow WAL writers")
    .unwrap();
    assert_eq!(
        users.find_export_user_by_id("admin").await.unwrap(),
        original
    );
    let totals = SqliteUsageReadRepository::summarize_usage_totals_by_user_ids_on(
        &mut *source.acquire().await.unwrap(),
        &["admin".to_string()],
    )
    .await
    .unwrap();
    assert_eq!(totals[0].request_count, 2);
    assert_eq!(totals[0].total_tokens, 7);
    transaction.commit().await.unwrap();
    assert!(source.acquire().await.is_err());
    let totals = SqliteUsageReadRepository::new(pool.clone())
        .summarize_usage_totals_by_user_ids(&["admin".to_string()])
        .await
        .unwrap();
    assert_eq!(totals[0].request_count, 5);
    writer.close().await;
    pool.close().await;
    std::fs::remove_file(path).expect("remove isolated database");
}
