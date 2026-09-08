#[cfg(feature = "sqlite")]
mod sqlite;

#[cfg(feature = "sqlite")]
use super::summarize_pool;
use super::DataBackends;
use crate::maintenance::{
    DatabaseMaintenanceSummary, DatabasePoolSummary, DatabasePostgresActivityGroup,
    DatabasePostgresObservabilitySnapshot, StatsDailyAggregationInput,
    StatsDailyAggregationSummary, StatsHourlyAggregationInput, StatsHourlyAggregationSummary,
    WalletDailyUsageAggregationInput, WalletDailyUsageAggregationResult,
};
use crate::repository::system::{
    AdminSystemPurgeSummary, AdminSystemPurgeTarget, AdminSystemStats,
    AdminSystemUsageAggregateImportMode, AdminSystemUsageAggregateImportSummary,
    AdminSystemUsageAggregateSnapshot, StoredSystemConfigEntry,
};
use crate::DataLayerError;
use sqlx::migrate::MigrateError;

#[cfg(feature = "sqlite")]
async fn warm_pool<DB>(pool: &sqlx::Pool<DB>, min_connections: u32) -> Result<(), DataLayerError>
where
    DB: sqlx::Database,
{
    let mut connections = Vec::with_capacity(min_connections as usize);
    for _ in 0..min_connections {
        connections.push(pool.acquire().await.map_err(DataLayerError::sql)?);
    }
    Ok(())
}

#[cfg(feature = "sqlite")]
pub(super) fn maintenance_identifier(value: &str) -> Result<&str, DataLayerError> {
    let valid = !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    if valid {
        Ok(value)
    } else {
        Err(DataLayerError::InvalidInput(format!(
            "invalid maintenance table name: {value}"
        )))
    }
}

// Without the SQLite feature, empty configurations keep the same optional-backend API.
#[cfg_attr(not(feature = "sqlite"), allow(unused_variables))]
impl DataBackends {
    pub fn has_database_maintenance_backend(&self) -> bool {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.is_some()
        }
        #[cfg(not(feature = "sqlite"))]
        {
            false
        }
    }

    pub fn has_database_pool_summary(&self) -> bool {
        self.has_database_maintenance_backend()
    }

    /// Establishes the configured minimum number of SQL connections before the service reports
    /// ready. Driver pools are built lazily, so relying on request traffic to grow them can make
    /// the first concurrency ramp consume nearly every connection in the small cold pool.
    pub async fn warm_database_pool(&self) -> Result<(), DataLayerError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return warm_pool(sqlite.pool(), sqlite.config().pool.min_connections).await;
        }
        Ok(())
    }

    pub fn has_system_config_backend(&self) -> bool {
        self.has_database_maintenance_backend()
    }

    pub fn has_wallet_daily_usage_aggregation_backend(&self) -> bool {
        self.has_database_maintenance_backend()
    }

    pub fn has_stats_hourly_aggregation_backend(&self) -> bool {
        self.has_database_maintenance_backend()
    }

    pub fn has_stats_daily_aggregation_backend(&self) -> bool {
        self.has_database_maintenance_backend()
    }

    pub async fn run_database_maintenance(
        &self,
        table_names: &[&str],
    ) -> Result<DatabaseMaintenanceSummary, DataLayerError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return sqlite.run_table_maintenance(table_names).await;
        }
        Ok(DatabaseMaintenanceSummary::default())
    }

    pub async fn run_database_migrations(&self) -> Result<bool, MigrateError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            crate::lifecycle::migrate::run_sqlite_migrations(sqlite.pool()).await?;
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn run_database_backfills(&self) -> Result<bool, MigrateError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            crate::lifecycle::backfill::run_sqlite_backfills(sqlite.pool()).await?;
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn pending_database_migrations(
        &self,
    ) -> Result<Option<Vec<crate::lifecycle::migrate::PendingMigrationInfo>>, MigrateError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return crate::lifecycle::migrate::pending_sqlite_migrations(sqlite.pool())
                .await
                .map(Some);
        }
        Ok(None)
    }

    pub async fn prepare_database_for_startup(
        &self,
    ) -> Result<Option<Vec<crate::lifecycle::migrate::PendingMigrationInfo>>, MigrateError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return crate::lifecycle::migrate::prepare_sqlite_database_for_startup(sqlite.pool())
                .await
                .map(Some);
        }
        Ok(None)
    }

    pub async fn pending_database_backfills(
        &self,
    ) -> Result<Option<Vec<crate::lifecycle::backfill::PendingBackfillInfo>>, MigrateError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return crate::lifecycle::backfill::pending_sqlite_backfills(sqlite.pool())
                .await
                .map(Some);
        }
        Ok(None)
    }

    pub fn database_pool_summary(&self) -> Option<DatabasePoolSummary> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return Some(summarize_pool(
                crate::DatabaseDriver::Sqlite,
                usize::try_from(sqlite.pool().size()).unwrap_or(usize::MAX),
                sqlite.pool().num_idle(),
                sqlite.config().pool.max_connections,
            ));
        }
        None
    }

    pub async fn postgres_observability_snapshot(
        &self,
    ) -> Result<Option<DatabasePostgresObservabilitySnapshot>, DataLayerError> {
        Ok(None)
    }

    pub async fn postgres_activity_groups(
        &self,
        limit: i64,
    ) -> Result<Vec<DatabasePostgresActivityGroup>, DataLayerError> {
        let _ = limit;
        Ok(Vec::new())
    }

    pub async fn aggregate_wallet_daily_usage(
        &self,
        input: &WalletDailyUsageAggregationInput,
    ) -> Result<WalletDailyUsageAggregationResult, DataLayerError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return sqlite.aggregate_wallet_daily_usage(input).await;
        }
        Ok(WalletDailyUsageAggregationResult::default())
    }

    pub async fn aggregate_stats_hourly(
        &self,
        input: &StatsHourlyAggregationInput,
    ) -> Result<Option<StatsHourlyAggregationSummary>, DataLayerError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return sqlite.aggregate_stats_hourly(input).await;
        }
        Ok(None)
    }

    pub async fn aggregate_stats_daily(
        &self,
        input: &StatsDailyAggregationInput,
    ) -> Result<Option<StatsDailyAggregationSummary>, DataLayerError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return sqlite.aggregate_stats_daily(input).await;
        }
        Ok(None)
    }

    pub async fn find_system_config_value(
        &self,
        key: &str,
    ) -> Result<Option<serde_json::Value>, DataLayerError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return sqlite.find_system_config_value(key).await;
        }
        Ok(None)
    }

    pub async fn list_system_config_entries(
        &self,
    ) -> Result<Vec<StoredSystemConfigEntry>, DataLayerError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return sqlite.list_system_config_entries().await;
        }
        Ok(Vec::new())
    }

    pub async fn upsert_system_config_entry(
        &self,
        key: &str,
        value: &serde_json::Value,
        description: Option<&str>,
    ) -> Result<Option<StoredSystemConfigEntry>, DataLayerError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return sqlite
                .upsert_system_config_entry(key, value, description)
                .await
                .map(Some);
        }
        Ok(None)
    }

    pub async fn delete_system_config_value(&self, key: &str) -> Result<bool, DataLayerError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return sqlite.delete_system_config_value(key).await;
        }
        Ok(false)
    }

    pub async fn read_admin_system_stats(&self) -> Result<AdminSystemStats, DataLayerError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return sqlite.read_admin_system_stats().await;
        }
        Ok(AdminSystemStats::default())
    }

    pub async fn purge_admin_system_data(
        &self,
        target: AdminSystemPurgeTarget,
    ) -> Result<AdminSystemPurgeSummary, DataLayerError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return sqlite.purge_admin_system_data(target).await;
        }
        Ok(AdminSystemPurgeSummary::default())
    }

    pub async fn export_admin_system_usage_aggregates(
        &self,
    ) -> Result<AdminSystemUsageAggregateSnapshot, DataLayerError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return sqlite.export_admin_system_usage_aggregates().await;
        }
        Ok(AdminSystemUsageAggregateSnapshot::default())
    }

    pub async fn import_admin_system_usage_aggregates(
        &self,
        snapshot: &AdminSystemUsageAggregateSnapshot,
        user_id_map: &std::collections::BTreeMap<String, String>,
        api_key_id_map: &std::collections::BTreeMap<String, String>,
        mode: AdminSystemUsageAggregateImportMode,
    ) -> Result<AdminSystemUsageAggregateImportSummary, DataLayerError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return sqlite
                .import_admin_system_usage_aggregates(snapshot, user_id_map, api_key_id_map, mode)
                .await;
        }
        Ok(AdminSystemUsageAggregateImportSummary::default())
    }

    pub async fn purge_admin_request_bodies_batch(
        &self,
        batch_size: usize,
    ) -> Result<AdminSystemPurgeSummary, DataLayerError> {
        #[cfg(feature = "sqlite")]
        if let Some(sqlite) = self.sqlite.as_ref() {
            return sqlite.purge_admin_request_bodies_batch(batch_size).await;
        }
        Ok(AdminSystemPurgeSummary::default())
    }
}
