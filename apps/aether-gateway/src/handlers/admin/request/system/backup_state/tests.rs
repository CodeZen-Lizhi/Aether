use std::path::PathBuf;
use std::time::Duration;

use aether_data::{DatabaseDriver, SqlDatabaseConfig, SqlPoolConfig};
use serde_json::{json, Value};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use tokio::time::timeout;

use super::{
    acquire_admin_external_models_config_mutation_lock, finish_backup_import, AdminAppState,
    AppState, BackupExternalModelsLock, StatusCode,
};
use crate::data::GatewayDataConfig;

const FINISH_TIMEOUT: Duration = Duration::from_secs(5);
const BEFORE_IMPORT: &str = "before-backup";
const AFTER_IMPORT: &str = "after-backup";
const EXTERNAL_CACHE_KEYS: [&str; 2] = [
    "aether:external:models_dev:v2",
    "aether:external:models_dev",
];
const ORIGINAL_EXTERNAL_CACHE: &str = r#"{"fixture":"before-backup"}"#;

struct BackupFixture {
    app: AppState,
    pool: sqlx::SqlitePool,
    directory: PathBuf,
}

impl BackupFixture {
    async fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("aether-backup-finish-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).expect("create isolated database directory");
        let path = directory.join("aether.db");
        let database = SqlDatabaseConfig::new(
            DatabaseDriver::Sqlite,
            format!("sqlite://{}", path.display()),
            SqlPoolConfig {
                min_connections: 0,
                max_connections: 1,
                acquire_timeout_ms: FINISH_TIMEOUT.as_millis() as u64,
                ..Default::default()
            },
        )
        .expect("build isolated database config");
        let app = AppState::new()
            .expect("create gateway")
            .with_data_config(
                GatewayDataConfig::from_database_config(database)
                    .with_encryption_key("synthetic-backup-finish-key"),
            )
            .expect("configure isolated SQLite gateway");
        app.run_database_migrations()
            .await
            .expect("run real SQLite migrations");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .acquire_timeout(FINISH_TIMEOUT)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .foreign_keys(true)
                    .journal_mode(SqliteJournalMode::Wal),
            )
            .await
            .expect("open independent connection to the test database");
        app.upsert_system_config_entry("site_name", &json!(BEFORE_IMPORT), None)
            .await
            .expect("seed original config");
        assert_eq!(
            app.read_system_config_json_value("site_name")
                .await
                .unwrap(),
            Some(json!(BEFORE_IMPORT)),
            "prime the application config cache before starting the transaction"
        );
        for key in EXTERNAL_CACHE_KEYS {
            app.runtime_kv_setex(key, ORIGINAL_EXTERNAL_CACHE, 600)
                .await
                .expect("seed external model cache");
        }
        Self {
            app,
            pool,
            directory,
        }
    }

    async fn assert_config(&self, expected: &str) {
        let persisted: String =
            sqlx::query_scalar("SELECT value FROM system_configs WHERE key = 'site_name'")
                .fetch_one(&self.pool)
                .await
                .expect("read persisted config independently");
        assert_eq!(
            serde_json::from_str::<Value>(&persisted).unwrap(),
            json!(expected)
        );
        assert_eq!(
            self.app
                .read_system_config_json_value("site_name")
                .await
                .unwrap(),
            Some(json!(expected)),
            "the application cache must agree with the committed database"
        );
        assert_eq!(
            timeout(
                FINISH_TIMEOUT,
                self.app.read_system_config_json_value_strong("site_name"),
            )
            .await
            .expect("backup must release its SQLite connection")
            .unwrap(),
            Some(json!(expected))
        );
    }

    async fn assert_external_cache(&self, expected: Option<&str>) {
        for key in EXTERNAL_CACHE_KEYS {
            assert_eq!(
                self.app.runtime_kv_get(key).await.unwrap().as_deref(),
                expected,
                "external model cache key {key}"
            );
        }
    }
}

impl Drop for BackupFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

async fn acquire_external_lock(app: &AppState) -> BackupExternalModelsLock {
    BackupExternalModelsLock {
        app: app.clone(),
        lease: Some(
            acquire_admin_external_models_config_mutation_lock(&AdminAppState::new(app))
                .await
                .expect("acquire external model mutation lease"),
        ),
    }
}

async fn wait_for_released_external_lock(app: &AppState) -> BackupExternalModelsLock {
    timeout(FINISH_TIMEOUT, async {
        loop {
            match acquire_admin_external_models_config_mutation_lock(&AdminAppState::new(app)).await
            {
                Ok(lease) => {
                    return BackupExternalModelsLock {
                        app: app.clone(),
                        lease: Some(lease),
                    };
                }
                Err((StatusCode::CONFLICT, _)) => tokio::task::yield_now().await,
                Err(error) => panic!("external model lease backend failed: {error:?}"),
            }
        }
    })
    .await
    .expect("finish must release the lease without waiting for its 10-minute expiry")
}

#[tokio::test(flavor = "current_thread")]
async fn detached_backup_finish_commits_refreshes_caches_and_releases_lease() {
    let fixture = BackupFixture::new().await;
    let admin = AdminAppState::new(&fixture.app);
    let lock = acquire_external_lock(&fixture.app).await;
    let scope = admin.begin_backup(true).await.unwrap();
    scope
        .upsert_system_config_entry("site_name", &json!(AFTER_IMPORT), None)
        .await
        .expect("write config inside the backup transaction");
    assert_eq!(
        fixture
            .app
            .read_system_config_json_value("site_name")
            .await
            .unwrap(),
        Some(json!(BEFORE_IMPORT)),
        "transaction writes must not update the application cache before commit"
    );
    assert_eq!(
        acquire_admin_external_models_config_mutation_lock(&admin)
            .await
            .unwrap_err()
            .0,
        StatusCode::CONFLICT
    );

    let finish = finish_backup_import(scope.data, fixture.app.clone(), Some(lock));
    // This current-thread test does not yield between spawning and detaching.
    // The request is gone before the finish task can execute its first poll.
    drop(finish);

    let mut next_lock = wait_for_released_external_lock(&fixture.app).await;
    fixture.assert_config(AFTER_IMPORT).await;
    fixture.assert_external_cache(None).await;
    next_lock.release().await;
}

#[tokio::test(flavor = "current_thread")]
async fn cancelling_uncommitted_backup_rolls_back_and_releases_lease() {
    let fixture = BackupFixture::new().await;
    let app = fixture.app.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let request = tokio::spawn(async move {
        let admin = AdminAppState::new(&app);
        let lock = acquire_external_lock(&app).await;
        let scope = admin.begin_backup(true).await.unwrap();
        scope
            .upsert_system_config_entry("site_name", &json!(AFTER_IMPORT), None)
            .await
            .expect("stage config before the request is cancelled");
        started.send(()).unwrap();
        std::future::pending::<()>().await;
        drop(scope);
        drop(lock);
    });
    timeout(FINISH_TIMEOUT, ready)
        .await
        .expect("request must reach the uncommitted state")
        .unwrap();
    assert_eq!(
        acquire_admin_external_models_config_mutation_lock(&AdminAppState::new(&fixture.app))
            .await
            .unwrap_err()
            .0,
        StatusCode::CONFLICT
    );
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());

    let mut next_lock = wait_for_released_external_lock(&fixture.app).await;
    fixture.assert_config(BEFORE_IMPORT).await;
    fixture
        .assert_external_cache(Some(ORIGINAL_EXTERNAL_CACHE))
        .await;
    next_lock.release().await;
}

#[tokio::test(flavor = "current_thread")]
async fn failed_backup_commit_preserves_cached_values_and_releases_lease() {
    let fixture = BackupFixture::new().await;
    // Use a migrated, enforced foreign key to fail COMMIT after config writes
    // succeed. No application hook or replacement repository participates.
    sqlx::raw_sql(
        r#"
CREATE TRIGGER backup_commit_foreign_key_failure AFTER UPDATE ON system_configs
WHEN NEW.key = 'site_name'
BEGIN
  INSERT INTO referral_rewards (
    id, referral_id, inviter_user_id, invitee_user_id, reward_type, trigger_point,
    idempotency_key, amount_usd, created_at, updated_at
  ) VALUES (
    'backup-orphan-reward', 'retired-referral', 'missing-inviter', 'missing-invitee',
    'credit', 'register', 'backup-orphan-reward-key', 1.0, 1, 1
  );
END;
"#,
    )
    .execute(&fixture.pool)
    .await
    .expect("install deferred foreign key failure");
    let admin = AdminAppState::new(&fixture.app);
    let lock = acquire_external_lock(&fixture.app).await;
    let scope = admin.begin_backup(true).await.unwrap();
    scope
        .upsert_system_config_entry("site_name", &json!(AFTER_IMPORT), None)
        .await
        .expect("foreign key failure must be deferred until commit");
    let error = timeout(
        FINISH_TIMEOUT,
        finish_backup_import(scope.data, fixture.app.clone(), Some(lock)),
    )
    .await
    .expect("finish task must complete")
    .expect("finish task must not panic")
    .expect_err("the foreign key must reject commit");
    assert!(error
        .into_message()
        .to_ascii_lowercase()
        .contains("foreign key"));

    let mut next_lock = acquire_external_lock(&fixture.app).await;
    fixture.assert_config(BEFORE_IMPORT).await;
    fixture
        .assert_external_cache(Some(ORIGINAL_EXTERNAL_CACHE))
        .await;
    let orphan_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM referral_rewards")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(
        orphan_rows, 0,
        "commit failure must roll back the trigger write too"
    );
    next_lock.release().await;
}
