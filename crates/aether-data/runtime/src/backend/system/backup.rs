//! A single database snapshot/transaction for an entire administrator backup.
//!
//! Repositories returned here share this transaction and never use application
//! caches. The caller publishes cache invalidations only after `commit` succeeds.

use std::collections::BTreeMap;

use crate::driver::sqlite::{
    SqliteAuthApiKeyReadRepository, SqliteBackupTransaction, SqliteGlobalModelReadRepository,
    SqliteProviderCatalogReadRepository, SqliteProxyNodeReadRepository,
    SqliteRoutingGroupRepository, SqliteUsageReadRepository, SqliteUserReadRepository,
};
use crate::error::SqlResultExt;
use crate::repository::system::{
    AdminSystemUsageAggregateImportMode, AdminSystemUsageAggregateImportSummary,
    AdminSystemUsageAggregateSnapshot, StoredSystemConfigEntry,
};
use crate::repository::usage::StoredUsageUserTotals;
use crate::{DataLayerError, SqliteBackend};

use super::sqlite::{
    export_sqlite_admin_system_usage_aggregates_on, import_sqlite_admin_system_usage_aggregates_on,
    list_system_config_entries_on, upsert_system_config_entry_on,
};

pub struct AdminBackupSession {
    transaction: SqliteBackupTransaction,
}

impl SqliteBackend {
    pub async fn begin_admin_backup(
        &self,
        write: bool,
    ) -> Result<AdminBackupSession, DataLayerError> {
        Ok(AdminBackupSession {
            transaction: SqliteBackupTransaction::begin(self.pool(), write)
                .await
                .map_sql_err()?,
        })
    }
}

impl AdminBackupSession {
    pub fn global_models(&self) -> SqliteGlobalModelReadRepository {
        SqliteGlobalModelReadRepository::new(self.transaction.source())
    }

    pub fn providers(&self) -> SqliteProviderCatalogReadRepository {
        SqliteProviderCatalogReadRepository::new(self.transaction.source())
    }

    pub fn proxy_nodes(&self) -> SqliteProxyNodeReadRepository {
        SqliteProxyNodeReadRepository::new(self.transaction.source())
    }

    pub fn routing_groups(&self) -> SqliteRoutingGroupRepository {
        SqliteRoutingGroupRepository::new(self.transaction.source())
    }

    pub fn api_keys(&self) -> SqliteAuthApiKeyReadRepository {
        SqliteAuthApiKeyReadRepository::new(self.transaction.source())
    }

    pub fn users(&self) -> SqliteUserReadRepository {
        SqliteUserReadRepository::new(self.transaction.source())
    }

    pub async fn list_system_config_entries(
        &self,
    ) -> Result<Vec<StoredSystemConfigEntry>, DataLayerError> {
        let source = self.transaction.source();
        let mut connection = source.acquire().await.map_sql_err()?;
        list_system_config_entries_on(&mut connection).await
    }

    pub async fn upsert_system_config_entry(
        &self,
        key: &str,
        value: &serde_json::Value,
        description: Option<&str>,
    ) -> Result<StoredSystemConfigEntry, DataLayerError> {
        let source = self.transaction.source();
        let mut connection = source.acquire().await.map_sql_err()?;
        upsert_system_config_entry_on(&mut connection, key, value, description).await
    }

    pub async fn export_usage_aggregates(
        &self,
    ) -> Result<AdminSystemUsageAggregateSnapshot, DataLayerError> {
        let source = self.transaction.source();
        let mut connection = source.acquire().await.map_sql_err()?;
        export_sqlite_admin_system_usage_aggregates_on(&mut connection).await
    }

    pub async fn import_usage_aggregates(
        &self,
        snapshot: &AdminSystemUsageAggregateSnapshot,
        user_id_map: &BTreeMap<String, String>,
        api_key_id_map: &BTreeMap<String, String>,
        mode: AdminSystemUsageAggregateImportMode,
    ) -> Result<AdminSystemUsageAggregateImportSummary, DataLayerError> {
        let source = self.transaction.source();
        let mut connection = source.acquire().await.map_sql_err()?;
        import_sqlite_admin_system_usage_aggregates_on(
            &mut connection,
            snapshot,
            user_id_map,
            api_key_id_map,
            mode,
        )
        .await
    }

    pub async fn summarize_usage_totals_by_user_ids(
        &self,
        user_ids: &[String],
    ) -> Result<Vec<StoredUsageUserTotals>, DataLayerError> {
        let source = self.transaction.source();
        let mut connection = source.acquire().await.map_sql_err()?;
        SqliteUsageReadRepository::summarize_usage_totals_by_user_ids_on(&mut connection, user_ids)
            .await
    }

    pub async fn commit(self) -> Result<(), DataLayerError> {
        self.transaction.commit().await.map_sql_err()
    }

    pub async fn rollback(self) -> Result<(), DataLayerError> {
        self.transaction.rollback().await.map_sql_err()
    }
}
