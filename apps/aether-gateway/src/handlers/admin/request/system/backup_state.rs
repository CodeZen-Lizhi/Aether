//! Backup-only access: every database operation uses the same transaction.
//! Keep application caching and other runtime effects out of this scope.

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use aether_data::repository::auth::{AuthApiKeyWriteRepository, StoredAuthApiKeyExportRecord};
use aether_data::repository::proxy_nodes::{
    ProxyNodeReadRepository, ProxyNodeWriteRepository, StoredProxyNode,
};
use aether_data::repository::system::{
    AdminSystemUsageAggregateImportMode, AdminSystemUsageAggregateImportSummary,
    AdminSystemUsageAggregateSnapshot, StoredSystemConfigEntry,
};
use aether_data::repository::users::{
    StoredUserExportRow, StoredUserPreferenceRecord, UserReadRepository,
};
use aether_data::{AdminBackupSession, DataLayerError};
use aether_data_contracts::repository::global_models::GlobalModelReadRepository;
use aether_data_contracts::repository::routing_profiles::{
    CreateRoutingGroupRecord, CreateRoutingGroupVersionRecord, RoutingGroupLookupKey,
    RoutingGroupReadRepository, RoutingGroupWriteRepository, StoredRoutingGroup,
    StoredRoutingGroupVersion, UpdateRoutingGroupRecord,
};
use aether_runtime_state::RuntimeLockLease;
use axum::{body::Bytes, http::StatusCode};
use serde_json::{json, Value};

use super::AdminAppState;
use crate::handlers::admin::model::{
    acquire_admin_external_models_config_mutation_lock, clear_admin_external_models_cache_entries,
    release_admin_external_models_config_mutation_lock,
    ADMIN_EXTERNAL_MODELS_PROXY_NODE_CONFIG_KEY,
};
use crate::{AppState, GatewayError};

pub(crate) struct AdminBackupState<'a> {
    admin: AdminAppState<'a>,
    data: AdminBackupSession,
}

fn backup_data_error(error: DataLayerError) -> GatewayError {
    match error {
        DataLayerError::InvalidInput(message) => GatewayError::Client {
            status: StatusCode::BAD_REQUEST,
            message,
        },
        DataLayerError::InvalidConfiguration(message) => GatewayError::Client {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message,
        },
        other => GatewayError::Internal(other.to_string()),
    }
}

impl<'a> AdminBackupState<'a> {
    pub(crate) fn admin(&self) -> &AdminAppState<'a> {
        &self.admin
    }

    pub(crate) async fn list_admin_global_models(
        &self,
        query: &aether_data_contracts::repository::global_models::AdminGlobalModelListQuery,
    ) -> Result<
        aether_data_contracts::repository::global_models::StoredAdminGlobalModelPage,
        GatewayError,
    > {
        self.data
            .global_models()
            .list_admin_global_models(query)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn list_admin_provider_models(
        &self,
        query: &aether_data_contracts::repository::global_models::AdminProviderModelListQuery,
    ) -> Result<
        Vec<aether_data_contracts::repository::global_models::StoredAdminProviderModel>,
        GatewayError,
    > {
        self.data
            .global_models()
            .list_admin_provider_models(query)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn update_admin_global_model(
        &self,
        record: &aether_data_contracts::repository::global_models::UpdateAdminGlobalModelRecord,
    ) -> Result<
        Option<aether_data_contracts::repository::global_models::StoredAdminGlobalModel>,
        GatewayError,
    > {
        self.data
            .global_models()
            .update_admin_global_model(record)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn create_admin_global_model(
        &self,
        record: &aether_data_contracts::repository::global_models::CreateAdminGlobalModelRecord,
    ) -> Result<
        Option<aether_data_contracts::repository::global_models::StoredAdminGlobalModel>,
        GatewayError,
    > {
        self.data
            .global_models()
            .create_admin_global_model(record)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn update_admin_provider_model(
        &self,
        record: &aether_data_contracts::repository::global_models::UpsertAdminProviderModelRecord,
    ) -> Result<
        Option<aether_data_contracts::repository::global_models::StoredAdminProviderModel>,
        GatewayError,
    > {
        self.data
            .global_models()
            .update_admin_provider_model(record)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn create_admin_provider_model(
        &self,
        record: &aether_data_contracts::repository::global_models::UpsertAdminProviderModelRecord,
    ) -> Result<
        Option<aether_data_contracts::repository::global_models::StoredAdminProviderModel>,
        GatewayError,
    > {
        self.data
            .global_models()
            .create_admin_provider_model(record)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn list_provider_catalog_providers(
        &self,
        active_only: bool,
    ) -> Result<
        Vec<aether_data_contracts::repository::provider_catalog::StoredProviderCatalogProvider>,
        GatewayError,
    > {
        self.data
            .providers()
            .list_providers(active_only)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn list_provider_catalog_endpoints_by_provider_ids(
        &self,
        provider_ids: &[String],
    ) -> Result<
        Vec<aether_data_contracts::repository::provider_catalog::StoredProviderCatalogEndpoint>,
        GatewayError,
    > {
        self.data
            .providers()
            .list_endpoints_by_provider_ids(provider_ids)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn list_provider_catalog_keys_by_provider_ids(
        &self,
        provider_ids: &[String],
    ) -> Result<
        Vec<aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey>,
        GatewayError,
    > {
        self.data
            .providers()
            .list_keys_by_provider_ids(provider_ids)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn update_provider_catalog_provider(
        &self,
        provider: &aether_data_contracts::repository::provider_catalog::StoredProviderCatalogProvider,
    ) -> Result<
        Option<aether_data_contracts::repository::provider_catalog::StoredProviderCatalogProvider>,
        GatewayError,
    > {
        self.data
            .providers()
            .update_provider(provider)
            .await
            .map(Some)
            .map_err(backup_data_error)
    }

    pub(crate) async fn create_provider_catalog_provider(
        &self,
        provider: &aether_data_contracts::repository::provider_catalog::StoredProviderCatalogProvider,
        shift_existing_priorities_from: Option<i32>,
    ) -> Result<
        Option<aether_data_contracts::repository::provider_catalog::StoredProviderCatalogProvider>,
        GatewayError,
    > {
        self.data
            .providers()
            .create_provider(provider, shift_existing_priorities_from)
            .await
            .map(Some)
            .map_err(backup_data_error)
    }

    pub(crate) async fn update_provider_catalog_endpoint(
        &self,
        endpoint: &aether_data_contracts::repository::provider_catalog::StoredProviderCatalogEndpoint,
    ) -> Result<
        Option<aether_data_contracts::repository::provider_catalog::StoredProviderCatalogEndpoint>,
        GatewayError,
    > {
        self.data
            .providers()
            .update_endpoint(endpoint)
            .await
            .map(Some)
            .map_err(backup_data_error)
    }

    pub(crate) async fn create_provider_catalog_endpoint(
        &self,
        endpoint: &aether_data_contracts::repository::provider_catalog::StoredProviderCatalogEndpoint,
    ) -> Result<
        Option<aether_data_contracts::repository::provider_catalog::StoredProviderCatalogEndpoint>,
        GatewayError,
    > {
        self.data
            .providers()
            .create_endpoint(endpoint)
            .await
            .map(Some)
            .map_err(backup_data_error)
    }

    pub(crate) async fn create_provider_catalog_key(
        &self,
        key: &aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey,
    ) -> Result<
        Option<aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey>,
        GatewayError,
    > {
        self.data
            .providers()
            .create_key(key)
            .await
            .map(Some)
            .map_err(backup_data_error)
    }

    pub(crate) async fn compare_and_update_provider_catalog_key_admin_state(
        &self,
        update: &aether_data_contracts::repository::provider_catalog::ProviderCatalogKeyAdminCasUpdate,
    ) -> Result<bool, GatewayError> {
        self.data
            .providers()
            .compare_and_update_key_admin_state(update)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn restore_provider_catalog_key_state(
        &self,
        key: &aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey,
    ) -> Result<(), GatewayError> {
        self.data
            .providers()
            .restore_key_backup_state(key)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn read_provider_catalog_keys_by_ids(
        &self,
        key_ids: &[String],
    ) -> Result<
        Vec<aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey>,
        GatewayError,
    > {
        self.data
            .providers()
            .list_keys_by_ids(key_ids)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn find_routing_group(
        &self,
        lookup: RoutingGroupLookupKey<'_>,
    ) -> Result<Option<StoredRoutingGroup>, GatewayError> {
        self.data
            .routing_groups()
            .find_routing_group(lookup)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn list_routing_group_versions(
        &self,
        group_id: &str,
    ) -> Result<Vec<StoredRoutingGroupVersion>, GatewayError> {
        self.data
            .routing_groups()
            .list_routing_group_versions(group_id)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn update_routing_group(
        &self,
        id: &str,
        patch: UpdateRoutingGroupRecord,
    ) -> Result<Option<StoredRoutingGroup>, GatewayError> {
        self.data
            .routing_groups()
            .update_routing_group(id, patch)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn create_routing_group(
        &self,
        record: CreateRoutingGroupRecord,
    ) -> Result<Option<StoredRoutingGroup>, GatewayError> {
        self.data
            .routing_groups()
            .create_routing_group(record)
            .await
            .map(Some)
            .map_err(backup_data_error)
    }

    pub(crate) async fn create_routing_group_version(
        &self,
        record: CreateRoutingGroupVersionRecord,
    ) -> Result<Option<StoredRoutingGroupVersion>, GatewayError> {
        self.data
            .routing_groups()
            .create_routing_group_version(record)
            .await
            .map(Some)
            .map_err(backup_data_error)
    }

    pub(crate) async fn list_export_users(
        &self,
    ) -> Result<Vec<aether_data::repository::users::StoredUserExportRow>, GatewayError> {
        self.data
            .users()
            .list_export_users()
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn find_user_auth_by_id(
        &self,
        user_id: &str,
    ) -> Result<Option<aether_data::repository::users::StoredUserAuthRecord>, GatewayError> {
        self.data
            .users()
            .find_user_auth_by_id(user_id)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn is_other_user_auth_username_taken(
        &self,
        username: &str,
        user_id: &str,
    ) -> Result<bool, GatewayError> {
        Ok(self
            .data
            .users()
            .find_user_auth_by_username(username)
            .await
            .map_err(backup_data_error)?
            .is_some_and(|user| user.id != user_id))
    }

    pub(crate) async fn is_other_user_auth_email_taken(
        &self,
        email: &str,
        user_id: &str,
    ) -> Result<bool, GatewayError> {
        Ok(self
            .data
            .users()
            .find_user_auth_by_email(email)
            .await
            .map_err(backup_data_error)?
            .is_some_and(|user| user.id != user_id))
    }

    pub(crate) async fn list_auth_api_key_export_records_by_user_ids(
        &self,
        user_ids: &[String],
    ) -> Result<Vec<aether_data::repository::auth::StoredAuthApiKeyExportRecord>, GatewayError>
    {
        self.data
            .api_keys()
            .list_backup_api_keys_by_user_ids(user_ids)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn list_proxy_nodes(
        &self,
    ) -> Result<Vec<aether_data::repository::proxy_nodes::StoredProxyNode>, GatewayError> {
        self.data
            .proxy_nodes()
            .list_proxy_nodes()
            .await
            .map_err(backup_data_error)
    }

    pub(crate) fn encryption_key(&self) -> Option<&str> {
        self.admin.encryption_key()
    }

    pub(crate) fn encrypt_catalog_secret_with_fallbacks(&self, plaintext: &str) -> Option<String> {
        self.admin.encrypt_catalog_secret_with_fallbacks(plaintext)
    }

    pub(crate) async fn restore_proxy_node(
        &self,
        node: &StoredProxyNode,
    ) -> Result<bool, GatewayError> {
        self.data
            .proxy_nodes()
            .restore_proxy_node(node)
            .await
            .map(|()| true)
            .map_err(backup_data_error)
    }

    pub(crate) async fn restore_exported_api_key(
        &self,
        record: &StoredAuthApiKeyExportRecord,
    ) -> Result<bool, GatewayError> {
        self.data
            .api_keys()
            .restore_exported_api_key(record)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn restore_admin_profile(
        &self,
        profile: &StoredUserExportRow,
    ) -> Result<bool, GatewayError> {
        self.data
            .users()
            .restore_admin_profile(profile)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn read_user_preferences(
        &self,
        user_id: &str,
    ) -> Result<Option<StoredUserPreferenceRecord>, GatewayError> {
        self.data
            .users()
            .read_user_preferences(user_id)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn write_user_preferences(
        &self,
        preferences: StoredUserPreferenceRecord,
    ) -> Result<Option<StoredUserPreferenceRecord>, GatewayError> {
        self.data
            .users()
            .write_user_preferences(&preferences)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn revoke_all_user_sessions(
        &self,
        user_id: &str,
        revoked_at: chrono::DateTime<chrono::Utc>,
        reason: &str,
    ) -> Result<u64, GatewayError> {
        self.data
            .users()
            .revoke_all_user_sessions(user_id, revoked_at, reason)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn list_system_config_entries(
        &self,
    ) -> Result<Vec<StoredSystemConfigEntry>, GatewayError> {
        self.data
            .list_system_config_entries()
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn upsert_system_config_entry(
        &self,
        key: &str,
        value: &Value,
        description: Option<&str>,
    ) -> Result<StoredSystemConfigEntry, GatewayError> {
        self.data
            .upsert_system_config_entry(key, value, description)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn export_admin_system_usage_aggregates(
        &self,
    ) -> Result<AdminSystemUsageAggregateSnapshot, GatewayError> {
        self.data
            .export_usage_aggregates()
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn import_admin_system_usage_aggregates(
        &self,
        snapshot: &AdminSystemUsageAggregateSnapshot,
        user_id_map: &BTreeMap<String, String>,
        api_key_id_map: &BTreeMap<String, String>,
        mode: AdminSystemUsageAggregateImportMode,
    ) -> Result<AdminSystemUsageAggregateImportSummary, GatewayError> {
        self.data
            .import_usage_aggregates(snapshot, user_id_map, api_key_id_map, mode)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn summarize_usage_totals_by_user_ids(
        &self,
        user_ids: &[String],
    ) -> Result<Vec<aether_data_contracts::repository::usage::StoredUsageUserTotals>, GatewayError>
    {
        self.data
            .summarize_usage_totals_by_user_ids(user_ids)
            .await
            .map_err(backup_data_error)
    }

    pub(crate) async fn build_admin_create_provider_record(
        &self,
        payload: crate::handlers::admin::provider::shared::payloads::AdminProviderCreateRequest,
    ) -> Result<
        (
            aether_data_contracts::repository::provider_catalog::StoredProviderCatalogProvider,
            Option<i32>,
        ),
        String,
    > {
        let providers = self
            .list_provider_catalog_providers(false)
            .await
            .map_err(|err| err.into_message())?;
        crate::handlers::admin::provider::write::provider::build_admin_create_provider_record_from_existing(&providers, payload)
    }

    pub(crate) async fn build_admin_update_provider_record(
        &self,
        existing: &aether_data_contracts::repository::provider_catalog::StoredProviderCatalogProvider,
        patch: crate::handlers::admin::provider::shared::payloads::AdminProviderUpdatePatch,
    ) -> Result<
        aether_data_contracts::repository::provider_catalog::StoredProviderCatalogProvider,
        String,
    > {
        let providers = self
            .list_provider_catalog_providers(false)
            .await
            .map_err(|err| err.into_message())?;
        crate::handlers::admin::provider::write::provider::build_admin_update_provider_record_from_existing(&providers, existing, patch)
    }

    pub(crate) async fn apply_backup_system_config(
        &self,
        key: &str,
        request: &Bytes,
    ) -> Result<Result<(), (StatusCode, Value)>, GatewayError> {
        let update = match aether_admin::system::parse_admin_system_config_update(key, request) {
            Ok(update) => update,
            Err(error) => return Ok(Err(error)),
        };
        let mut value = update.value;
        if aether_admin::system::is_sensitive_admin_system_config_key(&update.normalized_key) {
            if let Some(plaintext) = value.as_str().filter(|text| !text.is_empty()) {
                let Some(key) = self.encryption_key().filter(|key| !key.trim().is_empty()) else {
                    return Ok(Err((
                        StatusCode::SERVICE_UNAVAILABLE,
                        json!({"detail": "系统配置写入需要可用的加密密钥"}),
                    )));
                };
                value = json!(
                    aether_crypto::encrypt_python_fernet_plaintext(key, plaintext)
                        .map_err(|err| GatewayError::Internal(err.to_string()))?
                );
            }
        }
        self.upsert_system_config_entry(
            &update.normalized_key,
            &value,
            update.description.as_deref(),
        )
        .await?;
        Ok(Ok(()))
    }
}

#[derive(Clone, Copy)]
enum BackupImportKind {
    Config,
    Users,
    All,
}

struct BackupExternalModelsLock {
    app: AppState,
    lease: Option<RuntimeLockLease>,
}

impl BackupExternalModelsLock {
    async fn release(&mut self) {
        if let Some(lease) = self.lease.as_ref() {
            release_admin_external_models_config_mutation_lock(
                &AdminAppState::new(&self.app),
                lease,
            )
            .await;
            self.lease = None;
        }
    }
}

impl Drop for BackupExternalModelsLock {
    fn drop(&mut self) {
        if let Some(lease) = self.lease.take() {
            let app = self.app.clone();
            tokio::spawn(async move {
                release_admin_external_models_config_mutation_lock(
                    &AdminAppState::new(&app),
                    &lease,
                )
                .await;
            });
        }
    }
}

fn finish_backup_import(
    data: AdminBackupSession,
    app: AppState,
    mut lock: Option<BackupExternalModelsLock>,
) -> tokio::task::JoinHandle<Result<(), GatewayError>> {
    // SQLite may execute COMMIT even if its awaiting request is cancelled.
    // Keep commit, cache invalidation and lease release alive together.
    tokio::spawn(async move {
        let result = data.commit().await.map_err(backup_data_error);
        if result.is_ok() {
            app.invalidate_admin_backup_caches();
            if lock.is_some()
                && clear_admin_external_models_cache_entries(&AdminAppState::new(&app))
                    .await
                    .is_err()
            {
                tracing::warn!("failed to clear external models cache after backup import");
            }
        }
        if let Some(lock) = lock.as_mut() {
            lock.release().await;
        }
        result
    })
}

impl<'a> AdminAppState<'a> {
    async fn begin_backup(&self, write: bool) -> Result<AdminBackupState<'a>, GatewayError> {
        let data = self
            .app()
            .data
            .begin_admin_backup(write)
            .await
            .map_err(backup_data_error)?;
        Ok(AdminBackupState { admin: *self, data })
    }

    async fn import_backup(
        &self,
        request: &Bytes,
        operator_id: Option<&str>,
        kind: BackupImportKind,
    ) -> Result<Result<Value, (StatusCode, Value)>, GatewayError> {
        let changes_external_proxy =
            serde_json::from_slice::<Value>(request)
                .ok()
                .is_some_and(|root| {
                    let config = if matches!(kind, BackupImportKind::All) {
                        &root["config_data"]
                    } else {
                        &root
                    };
                    !matches!(kind, BackupImportKind::Users)
                        && (config["proxy_nodes"]
                            .as_array()
                            .is_some_and(|nodes| !nodes.is_empty())
                            || config["system_configs"].as_array().is_some_and(|entries| {
                                entries.iter().any(|entry| {
                                    entry["key"].as_str().is_some_and(|key| {
                                        key.trim().eq_ignore_ascii_case(
                                            ADMIN_EXTERNAL_MODELS_PROXY_NODE_CONFIG_KEY,
                                        )
                                    })
                                })
                            }))
                });
        let mut lock = if changes_external_proxy {
            match acquire_admin_external_models_config_mutation_lock(self).await {
                Ok(lease) => Some(BackupExternalModelsLock {
                    app: self.cloned_app(),
                    lease: Some(lease),
                }),
                Err(error) => return Ok(Err(error)),
            }
        } else {
            None
        };
        let result = async {
            let scope = self.begin_backup(true).await?;
            let result = match kind {
                BackupImportKind::Config => scope.import_admin_system_config(request).await,
                BackupImportKind::Users => {
                    scope.import_admin_system_users(request, operator_id).await
                }
                BackupImportKind::All => scope.import_admin_system_data(request, operator_id).await,
            };
            match result {
                Ok(Ok(payload)) => {
                    finish_backup_import(scope.data, self.cloned_app(), lock.take())
                        .await
                        .map_err(|error| {
                            GatewayError::Internal(format!("备份提交任务失败: {error}"))
                        })??;
                    Ok(Ok(payload))
                }
                failure => {
                    scope.data.rollback().await.map_err(backup_data_error)?;
                    failure
                }
            }
        }
        .await;
        if let Some(lock) = lock.as_mut() {
            lock.release().await;
        }
        result
    }

    pub(crate) async fn import_admin_system_config(
        &self,
        request: &Bytes,
    ) -> Result<Result<Value, (StatusCode, Value)>, GatewayError> {
        self.import_backup(request, None, BackupImportKind::Config)
            .await
    }

    pub(crate) async fn import_admin_system_users(
        &self,
        request: &Bytes,
        operator_id: Option<&str>,
    ) -> Result<Result<Value, (StatusCode, Value)>, GatewayError> {
        self.import_backup(request, operator_id, BackupImportKind::Users)
            .await
    }

    pub(crate) async fn import_admin_system_data(
        &self,
        request: &Bytes,
        operator_id: Option<&str>,
    ) -> Result<Result<Value, (StatusCode, Value)>, GatewayError> {
        self.import_backup(request, operator_id, BackupImportKind::All)
            .await
    }

    pub(crate) async fn build_admin_system_config_export_payload(
        &self,
    ) -> Result<Value, GatewayError> {
        let scope = self.begin_backup(false).await?;
        let payload = scope.build_admin_system_config_export_payload().await?;
        scope.data.commit().await.map_err(backup_data_error)?;
        Ok(payload)
    }

    pub(crate) async fn build_admin_system_users_export_payload(
        &self,
    ) -> Result<Value, GatewayError> {
        let scope = self.begin_backup(false).await?;
        let payload = scope.build_admin_system_users_export_payload().await?;
        scope.data.commit().await.map_err(backup_data_error)?;
        Ok(payload)
    }

    pub(crate) async fn build_admin_system_data_export_payload(
        &self,
    ) -> Result<Value, GatewayError> {
        let scope = self.begin_backup(false).await?;
        let payload = scope.build_admin_system_data_export_payload().await?;
        scope.data.commit().await.map_err(backup_data_error)?;
        Ok(payload)
    }
}
