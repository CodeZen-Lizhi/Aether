use async_trait::async_trait;

use aether_data_contracts::repository::auth_modules::{
    AuthModuleReadRepository, AuthModuleWriteRepository, StoredLdapModuleConfig,
    StoredOAuthProviderModuleConfig,
};
use aether_data_contracts::DataLayerError;

use crate::SqlitePool;

// 20260902 drop 迁移已删除 oauth_providers/ldap_configs（单用户化阶段4）。
// 数据库侧仅保留 auth_modules 表（/api/auth/settings 本地登录开关仍在用），
// 本仓库的 OAuth/LDAP 读取与写入收敛为中性结果，仅保留 trait 兼容。

#[derive(Debug, Clone)]
pub struct SqliteAuthModuleReadRepository {
    _pool: SqlitePool,
}

impl SqliteAuthModuleReadRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { _pool: pool }
    }
}

#[derive(Debug, Clone)]
pub struct SqliteAuthModuleRepository {
    _pool: SqlitePool,
}

impl SqliteAuthModuleRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { _pool: pool }
    }
}

#[async_trait]
impl AuthModuleReadRepository for SqliteAuthModuleReadRepository {
    async fn list_enabled_oauth_providers(
        &self,
    ) -> Result<Vec<StoredOAuthProviderModuleConfig>, DataLayerError> {
        Ok(Vec::new())
    }

    async fn get_ldap_config(&self) -> Result<Option<StoredLdapModuleConfig>, DataLayerError> {
        Ok(None)
    }
}

#[async_trait]
impl AuthModuleReadRepository for SqliteAuthModuleRepository {
    async fn list_enabled_oauth_providers(
        &self,
    ) -> Result<Vec<StoredOAuthProviderModuleConfig>, DataLayerError> {
        Ok(Vec::new())
    }

    async fn get_ldap_config(&self) -> Result<Option<StoredLdapModuleConfig>, DataLayerError> {
        Ok(None)
    }
}

#[async_trait]
impl AuthModuleWriteRepository for SqliteAuthModuleRepository {
    async fn upsert_ldap_config(
        &self,
        _config: &StoredLdapModuleConfig,
    ) -> Result<Option<StoredLdapModuleConfig>, DataLayerError> {
        Ok(None)
    }
}
