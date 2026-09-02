use async_trait::async_trait;

use aether_data_contracts::repository::oauth_providers::{
    OAuthProviderReadRepository, OAuthProviderWriteRepository, StoredOAuthProviderConfig,
    UpsertOAuthProviderConfigRecord,
};
use aether_data_contracts::DataLayerError;

use crate::SqlitePool;

// 20260902 drop 迁移已删除 oauth_providers/user_oauth_links（单用户化阶段4）。
// 读取方法收敛为中性结果，写入方法显式报错（数据已无处可写），仅保留 trait 兼容。

#[derive(Debug, Clone)]
pub struct SqliteOAuthProviderRepository {
    _pool: SqlitePool,
}

impl SqliteOAuthProviderRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { _pool: pool }
    }
}

#[async_trait]
impl OAuthProviderReadRepository for SqliteOAuthProviderRepository {
    async fn list_oauth_provider_configs(
        &self,
    ) -> Result<Vec<StoredOAuthProviderConfig>, DataLayerError> {
        Ok(Vec::new())
    }

    async fn get_oauth_provider_config(
        &self,
        _provider_type: &str,
    ) -> Result<Option<StoredOAuthProviderConfig>, DataLayerError> {
        Ok(None)
    }

    async fn count_locked_users_if_provider_disabled(
        &self,
        _provider_type: &str,
        _ldap_exclusive: bool,
    ) -> Result<usize, DataLayerError> {
        Ok(0)
    }
}

#[async_trait]
impl OAuthProviderWriteRepository for SqliteOAuthProviderRepository {
    async fn upsert_oauth_provider_config(
        &self,
        _record: &UpsertOAuthProviderConfigRecord,
    ) -> Result<StoredOAuthProviderConfig, DataLayerError> {
        Err(DataLayerError::InvalidInput(
            "oauth_providers 已随单用户化阶段4 drop 迁移移除，无法再写入 OAuth 供应商配置"
                .to_string(),
        ))
    }

    async fn delete_oauth_provider_config(
        &self,
        _provider_type: &str,
    ) -> Result<bool, DataLayerError> {
        Ok(false)
    }
}
