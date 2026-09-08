use super::ADMIN_SYSTEM_DATA_EXPORT_VERSION;
use crate::constants::DEFAULT_USER_GROUP_CONFIG_KEY;
use crate::handlers::admin::request::AdminAppState;
use crate::handlers::admin::system::shared::configs::is_sensitive_admin_system_config_key;
use crate::handlers::admin::system::shared::export::{
    build_admin_system_export_providers_payload, decrypt_admin_system_export_secret,
    ADMIN_SYSTEM_EXPORT_PAGE_LIMIT,
};
use crate::GatewayError;
use aether_admin::system::{
    AdminSystemConfigDocument, AdminSystemConfigEntry, AdminSystemConfigGlobalModel,
    AdminSystemConfigProxyNode, AdminSystemConfigRoutingStrategy,
    ADMIN_SYSTEM_CONFIG_EXPORT_VERSION,
};
use aether_data_contracts::repository::global_models::{
    AdminGlobalModelListQuery, AdminProviderModelListQuery, StoredAdminGlobalModel,
    StoredAdminProviderModel,
};
use aether_data_contracts::repository::routing_profiles::RoutingGroupLookupKey;
use chrono::Utc;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

impl<'a> AdminAppState<'a> {
    pub(crate) async fn list_admin_system_backup_global_models(
        &self,
    ) -> Result<Vec<StoredAdminGlobalModel>, GatewayError> {
        let mut models = Vec::new();
        loop {
            let page = self
                .list_admin_global_models(&AdminGlobalModelListQuery {
                    offset: models.len(),
                    limit: ADMIN_SYSTEM_EXPORT_PAGE_LIMIT,
                    is_active: None,
                    search: None,
                })
                .await?;
            let page_len = page.items.len();
            models.extend(page.items);
            if page_len == 0 || models.len() >= page.total {
                break;
            }
        }
        Ok(models)
    }

    pub(crate) async fn list_admin_system_backup_provider_models(
        &self,
        provider_id: &str,
    ) -> Result<Vec<StoredAdminProviderModel>, GatewayError> {
        let mut models = Vec::new();
        loop {
            let page = self
                .list_admin_provider_models(&AdminProviderModelListQuery {
                    provider_id: provider_id.to_string(),
                    offset: models.len(),
                    limit: ADMIN_SYSTEM_EXPORT_PAGE_LIMIT,
                    is_active: None,
                })
                .await?;
            let page_len = page.len();
            models.extend(page);
            if page_len < ADMIN_SYSTEM_EXPORT_PAGE_LIMIT {
                break;
            }
        }
        Ok(models)
    }

    pub(crate) async fn build_admin_system_config_export_payload(
        &self,
    ) -> Result<serde_json::Value, GatewayError> {
        let global_models = self.list_admin_system_backup_global_models().await?;
        let global_model_name_by_id = global_models
            .iter()
            .map(|model| (model.id.clone(), model.name.clone()))
            .collect::<BTreeMap<_, _>>();
        let global_models_data = global_models
            .iter()
            .map(|model| AdminSystemConfigGlobalModel {
                name: model.name.clone(),
                display_name: model.display_name.clone(),
                usage_count: Some(model.usage_count),
                default_price_per_request: model.default_price_per_request,
                default_tiered_pricing: model.default_tiered_pricing.clone(),
                supported_capabilities: model.supported_capabilities.as_ref().and_then(|value| {
                    value.as_array().map(|items| {
                        items
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .map(ToOwned::to_owned)
                            .collect::<Vec<_>>()
                    })
                }),
                config: model.config.clone(),
                is_active: model.is_active,
            })
            .collect::<Vec<_>>();
        let providers_data =
            build_admin_system_export_providers_payload(self, &global_model_name_by_id).await?;

        let existing_user_group_ids = if self.has_user_data_reader() {
            self.list_user_groups()
                .await?
                .into_iter()
                .map(|group| group.id)
                .collect::<BTreeSet<_>>()
        } else {
            BTreeSet::new()
        };
        let system_configs = self.list_system_config_entries().await?;
        let system_configs_data = system_configs
            .iter()
            .map(|entry| -> Result<_, GatewayError> {
                let mut value = if is_sensitive_admin_system_config_key(&entry.key) {
                    entry
                        .value
                        .as_str()
                        .filter(|ciphertext| !ciphertext.is_empty())
                        .map(|ciphertext| decrypt_admin_system_export_secret(self, ciphertext))
                        .transpose()?
                        .map(serde_json::Value::String)
                        .unwrap_or_else(|| entry.value.clone())
                } else {
                    entry.value.clone()
                };
                if entry.key == DEFAULT_USER_GROUP_CONFIG_KEY
                    && value
                        .as_str()
                        .is_some_and(|group_id| !existing_user_group_ids.contains(group_id))
                {
                    value = serde_json::Value::Null;
                }
                Ok(AdminSystemConfigEntry {
                    key: entry.key.clone(),
                    value,
                    description: entry.description.clone(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let proxy_nodes = self.list_proxy_nodes().await?;
        let proxy_nodes_data = proxy_nodes
            .iter()
            .map(|node| AdminSystemConfigProxyNode {
                id: Some(node.id.clone()),
                name: Some(node.name.clone()),
                ip: Some(node.ip.clone()),
                port: Some(node.port),
                region: node.region.clone(),
                is_manual: Some(node.is_manual),
                proxy_url: node.proxy_url.clone(),
                proxy_username: node.proxy_username.clone(),
                proxy_password: node.proxy_password.clone(),
                tunnel_mode: Some(node.tunnel_mode),
                heartbeat_interval: Some(node.heartbeat_interval),
                remote_config: node.remote_config.clone(),
                config_version: Some(node.config_version),
            })
            .collect::<Vec<_>>();

        let routing_strategy = self
            .find_routing_group(RoutingGroupLookupKey::SystemDefault)
            .await?
            .map(|group| AdminSystemConfigRoutingStrategy {
                id: Some(group.id),
                name: group.name,
                description: group.description,
                enabled: group.enabled,
                is_system_default: group.is_system_default,
                config_json: group.config_json,
                version: group.version,
                published_at: group.published_at,
            });

        let document = AdminSystemConfigDocument {
            version: ADMIN_SYSTEM_CONFIG_EXPORT_VERSION.to_string(),
            exported_at: Utc::now().to_rfc3339(),
            global_models: global_models_data,
            providers: providers_data,
            proxy_nodes: proxy_nodes_data,
            system_configs: system_configs_data,
            routing_strategy,
        };

        super::import::validate_imported_config_references(&document)
            .map_err(GatewayError::Internal)?;
        serde_json::to_value(document).map_err(|err| GatewayError::Internal(err.to_string()))
    }

    pub(crate) async fn build_admin_system_data_export_payload(
        &self,
    ) -> Result<serde_json::Value, GatewayError> {
        let config_data = self.build_admin_system_config_export_payload().await?;
        let user_data = self.build_admin_system_users_export_payload().await?;
        let provider_names = config_data["providers"]
            .as_array()
            .expect("providers export is an array")
            .iter()
            .map(|provider| {
                (
                    provider["id"]
                        .as_str()
                        .expect("provider export has an id")
                        .to_string(),
                    provider["name"].clone(),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        if user_data["provider_names"] != json!(provider_names) {
            return Err(GatewayError::Internal(
                "导出期间提供商配置发生变化，请重试".to_string(),
            ));
        }

        Ok(json!({
            "version": ADMIN_SYSTEM_DATA_EXPORT_VERSION,
            "exported_at": Utc::now().to_rfc3339(),
            "config_data": config_data,
            "user_data": user_data,
        }))
    }
}
