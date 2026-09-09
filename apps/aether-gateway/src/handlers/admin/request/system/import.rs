use super::backup_keys::{
    apply_imported_provider_key_state, build_imported_provider_key_record,
    matches_imported_key_credentials, validate_imported_provider_key,
};
use super::{AdminAppState, AdminBackupState};
use crate::api::ai::admin_endpoint_signature_parts;
use crate::handlers::admin::model::ADMIN_EXTERNAL_MODELS_PROXY_NODE_CONFIG_KEY;
use crate::handlers::admin::provider::endpoints_admin::payloads::AdminProviderEndpointUpdatePatch;
use crate::handlers::admin::provider::shared::payloads::{
    AdminProviderCreateRequest, AdminProviderUpdatePatch,
};
use crate::handlers::admin::provider::write::keys::build_provider_catalog_key_admin_cas_update;
use crate::handlers::admin::shared::{
    normalize_json_array, normalize_json_object, normalize_string_list,
};
use crate::handlers::public::normalize_admin_base_url;
use crate::GatewayError;
use aether_admin::provider::endpoints as admin_provider_endpoints_pure;
use aether_admin::provider::models_write as admin_provider_models_write_pure;
use aether_admin::system::{
    normalize_admin_system_config_key, parse_admin_system_config_array,
    parse_admin_system_config_import_request, parse_admin_system_config_nested_array,
    AdminImportMergeMode, AdminSystemConfigEndpoint as ImportedEndpoint,
    AdminSystemConfigEntry as ImportedSystemConfig,
    AdminSystemConfigGlobalModel as ImportedGlobalModel, AdminSystemConfigImportCounter,
    AdminSystemConfigImportStats, AdminSystemConfigProvider as ImportedProvider,
    AdminSystemConfigProviderKey as ImportedProviderKey,
    AdminSystemConfigProviderModel as ImportedProviderModel,
    AdminSystemConfigProxyNode as ImportedProxyNode,
    ADMIN_SYSTEM_PROVIDER_OPS_SENSITIVE_CREDENTIAL_FIELDS,
};
use aether_data::repository::system::{
    AdminSystemStatsUserDailyAggregate, AdminSystemUsageAggregateImportMode,
    AdminSystemUsageAggregateImportSummary, AdminSystemUsageAggregateSnapshot,
};
use aether_data_contracts::repository::global_models::{
    CreateAdminGlobalModelRecord, UpdateAdminGlobalModelRecord, UpsertAdminProviderModelRecord,
};
use aether_data_contracts::repository::routing_profiles::{
    CreateRoutingGroupRecord, CreateRoutingGroupVersionRecord, RoutingGroupLookupKey,
    UpdateRoutingGroupRecord,
};
use axum::{body::Bytes, http};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const PROXY_NODE_CONFIG_KEYS: &[&str] = &[
    "system_proxy_node_id",
    ADMIN_EXTERNAL_MODELS_PROXY_NODE_CONFIG_KEY,
];

fn remap_proxy_node_setting(
    key: &str,
    value: &Value,
    nodes: &BTreeMap<String, String>,
) -> Result<Value, String> {
    match value {
        Value::Null => Ok(Value::Null),
        Value::String(id) => nodes
            .get(id)
            .map(|id| json!(id))
            .ok_or_else(|| format!("系统配置 '{key}' 引用的代理节点不在备份中")),
        _ => Err(format!("{key} 必须是字符串或 null")),
    }
}

fn remap_routing_strategy_config(
    value: &mut Value,
    provider_id_map: &BTreeMap<String, String>,
    key_id_map: &BTreeMap<String, String>,
) -> Result<(), String> {
    if let Some(rules) = value.get_mut("rules").and_then(Value::as_array_mut) {
        for rule in rules {
            if let Some(actions) = rule.get_mut("actions").and_then(Value::as_array_mut) {
                for action in actions {
                    let Some(action) = action.as_object_mut() else {
                        continue;
                    };
                    remap_routing_id_array(action, "provider_ids", provider_id_map)?;
                    remap_routing_id_array(action, "key_ids", key_id_map)?;
                    for (field, mapping) in
                        [("provider_id", provider_id_map), ("key_id", key_id_map)]
                    {
                        if let Some(id) = action.get_mut(field) {
                            let source = id
                                .as_str()
                                .ok_or_else(|| format!("调度策略 {field} 必须是字符串"))?;
                            *id = json!(mapping.get(source).ok_or_else(|| format!(
                                "调度策略引用的 {field} '{source}' 不存在"
                            ))?);
                        }
                    }
                }
            }
        }
    }
    if let Some(policies) = value
        .get_mut("model_policies")
        .and_then(Value::as_array_mut)
    {
        for policy in policies {
            let Some(policy) = policy.as_object_mut() else {
                continue;
            };
            remap_routing_id_array(policy, "allowed_providers", provider_id_map)?;
            remap_routing_id_array(policy, "allowed_keys", key_id_map)?;
            remap_routing_id_map(policy, "provider_priority_overrides", provider_id_map)?;
            remap_routing_id_map(policy, "key_priority_overrides", key_id_map)?;
        }
    }
    Ok(())
}

fn remap_routing_id_array(
    object: &mut Map<String, Value>,
    field: &str,
    id_map: &BTreeMap<String, String>,
) -> Result<(), String> {
    if let Some(Value::Array(ids)) = object.get_mut(field) {
        for id in ids {
            let source = id
                .as_str()
                .ok_or_else(|| format!("调度策略 {field} 必须是字符串列表"))?;
            *id = json!(id_map
                .get(source)
                .ok_or_else(|| format!("调度策略引用的 {field} '{source}' 不存在"))?);
        }
    }
    Ok(())
}

fn remap_routing_id_map(
    object: &mut Map<String, Value>,
    field: &str,
    id_map: &BTreeMap<String, String>,
) -> Result<(), String> {
    if let Some(Value::Object(entries)) = object.get_mut(field) {
        let mut remapped = Map::new();
        for (source, value) in entries.iter() {
            let target = id_map
                .get(source)
                .ok_or_else(|| format!("调度策略引用的 {field} '{source}' 不存在"))?;
            remapped.insert(target.clone(), value.clone());
        }
        *entries = remapped;
    }
    Ok(())
}

pub(super) fn validate_imported_config_references(
    document: &aether_admin::system::AdminSystemConfigDocument,
) -> Result<(), String> {
    let mut nodes = BTreeMap::new();
    let mut providers = BTreeMap::new();
    let mut keys = BTreeMap::new();
    let mut provider_names = BTreeSet::new();
    let mut model_names = BTreeSet::new();
    for model in &document.global_models {
        let name = trim_required(&model.name, "global_models.name")?;
        if !model_names.insert(name) {
            return Err(format!("备份包含重复全局模型 '{}'", model.name));
        }
    }
    for node in &document.proxy_nodes {
        let id = node
            .id
            .as_ref()
            .filter(|id| !id.is_empty())
            .ok_or("代理节点 id 为必填字段")?;
        if nodes.insert(id.clone(), id.clone()).is_some() {
            return Err("备份包含重复代理节点标识".to_string());
        }
    }
    for provider in &document.providers {
        let id = provider
            .id
            .as_ref()
            .filter(|id| !id.is_empty())
            .ok_or("提供商 id 为必填字段")?;
        if providers.insert(id.clone(), id.clone()).is_some()
            || !provider_names.insert(trim_required(&provider.name, "providers.name")?)
        {
            return Err("备份包含重复提供商标识或名称".to_string());
        }
        if provider
            .monthly_used_usd
            .is_some_and(|value| !value.is_finite() || value < 0.0)
            || [
                provider.quota_last_reset_at_unix_secs,
                provider.quota_expires_at_unix_secs,
            ]
            .into_iter()
            .flatten()
            .any(|value| value > i64::MAX as u64)
        {
            return Err("提供商配额或有效期无效".to_string());
        }
        remap_import_proxy(provider.proxy.clone(), &nodes)?;
        let mut endpoint_formats = BTreeSet::new();
        for endpoint in &provider.endpoints {
            if !endpoint_formats.insert(normalize_import_endpoint_format(&endpoint.api_format)?) {
                return Err("备份在同一提供商中包含重复 Endpoint 格式".to_string());
            }
            remap_import_proxy(endpoint.proxy.clone(), &nodes)?;
        }
        for key in &provider.api_keys {
            let id = key
                .id
                .as_ref()
                .filter(|id| !id.is_empty())
                .ok_or("渠道 Key id 为必填字段")?;
            if keys.insert(id.clone(), id.clone()).is_some() {
                return Err("备份包含重复渠道 Key 标识".to_string());
            }
            if key
                .expires_at_unix_secs
                .is_some_and(|value| value > i64::MAX as u64)
            {
                return Err("渠道 Key 有效期无效".to_string());
            }
            remap_import_proxy(key.proxy.clone(), &nodes)?;
            validate_imported_provider_key(key)?;
        }
        let mut mapped_models = BTreeSet::new();
        for model in &provider.models {
            let name = model
                .global_model_name
                .as_ref()
                .ok_or("模型缺少 global_model_name")?;
            if !model_names.contains(name.trim()) {
                return Err(format!("模型引用的全局模型 '{name}' 不在备份中"));
            }
            if !mapped_models.insert(name.trim()) {
                return Err("备份在同一提供商中包含重复模型映射".to_string());
            }
        }
    }
    let mut config_keys = BTreeSet::new();
    for config in &document.system_configs {
        let key = normalize_imported_system_config_key(&config.key);
        if key.is_empty() || !config_keys.insert(key.clone()) {
            return Err("备份包含空白或重复系统配置项".to_string());
        }
        if PROXY_NODE_CONFIG_KEYS.contains(&key.as_str()) {
            remap_proxy_node_setting(&key, &config.value, &nodes)?;
        }
    }
    if let Some(strategy) = &document.routing_strategy {
        validate_imported_routing_strategy_config(&strategy.config_json)?;
        remap_routing_strategy_config(&mut strategy.config_json.clone(), &providers, &keys)?;
    }
    Ok(())
}

fn validate_imported_routing_strategy_config(value: &Value) -> Result<(), String> {
    let config = serde_json::from_value::<aether_routing_core::RoutingGroupConfig>(value.clone())
        .map_err(|err| format!("调度策略配置格式无效: {err}"))?;
    aether_routing_core::validate_routing_group_config(&config)
        .map_err(|err| format!("调度策略配置无效: {err}"))
}

pub(super) fn invalid_request(detail: impl Into<String>) -> (http::StatusCode, Value) {
    (
        http::StatusCode::BAD_REQUEST,
        json!({ "detail": detail.into() }),
    )
}

fn normalize_imported_system_config_key(key: &str) -> String {
    let normalized = normalize_admin_system_config_key(key);
    PROXY_NODE_CONFIG_KEYS
        .iter()
        .find(|key| normalized.eq_ignore_ascii_case(key))
        .map_or(normalized.clone(), |key| (*key).to_string())
}

fn build_admin_system_data_import_part_body(
    root: &Map<String, Value>,
    field_name: &str,
    merge_mode: AdminImportMergeMode,
) -> Result<Bytes, (http::StatusCode, Value)> {
    let mut part = match root.get(field_name) {
        Some(Value::Object(map)) => map.clone(),
        Some(_) => return Err(invalid_request(format!("{field_name} 必须是对象"))),
        None => return Err(invalid_request(format!("{field_name} 为必填字段"))),
    };

    let merge_mode_value = serde_json::to_value(merge_mode)
        .map_err(|err| invalid_request(format!("merge_mode 序列化失败: {err}")))?;
    part.insert("merge_mode".to_string(), merge_mode_value);

    serde_json::to_vec(&Value::Object(part))
        .map(Bytes::from)
        .map_err(|err| invalid_request(format!("{field_name} 序列化失败: {err}")))
}

fn trim_required(value: &str, field_name: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field_name} 不能为空"));
    }
    Ok(trimmed.to_string())
}

fn normalize_optional_price(value: Option<f64>, field_name: &str) -> Result<Option<f64>, String> {
    admin_provider_models_write_pure::normalize_optional_price(value, field_name)
}

fn normalize_supported_capabilities(value: Option<Vec<String>>) -> Option<Value> {
    normalize_string_list(value).map(|items| json!(items))
}

fn encrypt_imported_provider_config(
    state: &AdminAppState<'_>,
    config: Option<Value>,
) -> Result<Option<Value>, String> {
    let Some(mut config) = normalize_json_object(config, "config")? else {
        return Ok(None);
    };
    let Some(credentials) = config
        .get_mut("provider_ops")
        .and_then(Value::as_object_mut)
        .and_then(|provider_ops| provider_ops.get_mut("connector"))
        .and_then(Value::as_object_mut)
        .and_then(|connector| connector.get_mut("credentials"))
        .and_then(Value::as_object_mut)
    else {
        return Ok(Some(config));
    };

    for field in ADMIN_SYSTEM_PROVIDER_OPS_SENSITIVE_CREDENTIAL_FIELDS {
        let Some(Value::String(raw)) = credentials.get_mut(*field) else {
            continue;
        };
        if raw.is_empty() {
            continue;
        }
        let encrypted = state
            .encrypt_catalog_secret_with_fallbacks(raw)
            .ok_or_else(|| "gateway 未配置 Provider Ops 加密密钥".to_string())?;
        *raw = encrypted;
    }

    Ok(Some(config))
}

fn remap_import_proxy(
    proxy: Option<Value>,
    node_id_map: &BTreeMap<String, String>,
) -> Result<Option<Value>, String> {
    let Some(mut proxy) = normalize_json_object(proxy, "proxy")? else {
        return Ok(None);
    };
    if let Some(node_id) = proxy.get_mut("node_id") {
        let source = node_id
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or("proxy.node_id 必须是非空字符串")?;
        *node_id = json!(node_id_map
            .get(source)
            .ok_or_else(|| format!("引用的代理节点 '{source}' 不在备份中"))?);
    }
    Ok(Some(proxy))
}

fn normalize_import_endpoint_format(value: &str) -> Result<String, String> {
    let normalized = value.trim();
    admin_endpoint_signature_parts(normalized)
        .map(|(signature, _, _)| signature.to_string())
        .ok_or_else(|| format!("无效的 api_format: {value}"))
}

fn build_import_provider_model_record(
    provider_id: &str,
    existing_id: Option<&str>,
    global_model_id: &str,
    item: &ImportedProviderModel,
) -> Result<UpsertAdminProviderModelRecord, String> {
    let provider_model_name = trim_required(&item.provider_model_name, "provider_model_name")?;
    let provider_model_mappings = normalize_json_array(
        item.provider_model_mappings.clone(),
        "provider_model_mappings",
    )?;
    let price_per_request = normalize_optional_price(item.price_per_request, "price_per_request")?;
    let tiered_pricing = normalize_json_object(item.tiered_pricing.clone(), "tiered_pricing")?;
    let config = normalize_json_object(item.config.clone(), "config")?;

    UpsertAdminProviderModelRecord::new(
        existing_id
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
        provider_id.to_string(),
        global_model_id.to_string(),
        provider_model_name,
        provider_model_mappings,
        price_per_request,
        tiered_pricing,
        item.supports_vision,
        item.supports_function_calling,
        item.supports_streaming,
        item.supports_extended_thinking,
        item.supports_image_generation,
        item.is_active,
        true,
        config,
    )
    .map_err(|err| err.to_string())
}

fn usage_aggregate_import_mode(
    merge_mode: AdminImportMergeMode,
) -> AdminSystemUsageAggregateImportMode {
    match merge_mode {
        AdminImportMergeMode::Skip => AdminSystemUsageAggregateImportMode::Skip,
        AdminImportMergeMode::Overwrite => AdminSystemUsageAggregateImportMode::Overwrite,
        AdminImportMergeMode::Error => AdminSystemUsageAggregateImportMode::Error,
    }
}

fn imported_object_field<'a>(
    value: &'a Value,
    field_name: &str,
) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{field_name} 必须是对象"))
}

fn imported_optional_string(value: Option<&Value>) -> Result<Option<String>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(raw)) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
        _ => Err("字段必须是字符串".to_string()),
    }
}

fn imported_optional_u64(value: Option<&Value>, field_name: &str) -> Result<Option<u64>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_u64()
            .ok_or_else(|| format!("{field_name} 必须是非负整数"))
            .map(Some),
        _ => Err(format!("{field_name} 必须是非负整数")),
    }
}

pub(super) fn build_imported_user_usage_total_aggregates(
    users: &[Value],
    exported_at: Option<&Value>,
) -> Result<Vec<AdminSystemStatsUserDailyAggregate>, String> {
    let date_unix_secs = imported_export_day_unix_secs(exported_at);
    let mut rows = Vec::new();
    for (index, raw_user) in users.iter().enumerate() {
        let user = imported_object_field(raw_user, &format!("users[{index}]"))?;
        let Some(user_id) = imported_optional_string(user.get("id"))? else {
            continue;
        };
        let request_count = imported_optional_u64(user.get("request_count"), "request_count")?;
        let total_tokens = imported_optional_u64(user.get("total_tokens"), "total_tokens")?;
        if request_count.is_none() && total_tokens.is_none() {
            continue;
        }
        let total_requests = request_count.unwrap_or(0);
        let input_tokens = total_tokens.unwrap_or(0);
        if total_requests == 0 && input_tokens == 0 {
            continue;
        }
        rows.push(AdminSystemStatsUserDailyAggregate {
            user_id,
            username: imported_optional_string(user.get("username"))?,
            date_unix_secs,
            total_requests,
            success_requests: total_requests,
            error_requests: 0,
            input_tokens,
            output_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            total_cost: 0.0,
        });
    }
    Ok(rows)
}

fn imported_export_day_unix_secs(exported_at: Option<&Value>) -> u64 {
    imported_optional_string(exported_at)
        .ok()
        .flatten()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(&value).ok())
        .map(|value| unix_day_start_secs(value.timestamp()))
        .unwrap_or_else(|| unix_day_start_secs(chrono::Utc::now().timestamp()))
}

fn unix_day_start_secs(timestamp: i64) -> u64 {
    let timestamp = timestamp.max(0) as u64;
    timestamp - (timestamp % 86_400)
}

impl<'a> AdminBackupState<'a> {
    pub(crate) async fn import_admin_system_data(
        &self,
        request_body: &Bytes,
        operator_id: Option<&str>,
    ) -> Result<Result<Value, (http::StatusCode, Value)>, GatewayError> {
        let root = match serde_json::from_slice::<Value>(request_body) {
            Ok(Value::Object(map)) => map,
            _ => return Ok(Err(invalid_request("请求数据验证失败"))),
        };

        let recognized = root
            .iter()
            .filter(|(key, _)| {
                matches!(
                    key.as_str(),
                    "version" | "exported_at" | "merge_mode" | "config_data" | "user_data"
                )
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Map<_, _>>();
        if let Err(detail) = aether_admin::system::validate_admin_backup_fields(
            &Value::Object(root.clone()),
            &Value::Object(recognized),
            "backup",
        ) {
            return Ok(Err(invalid_request(detail)));
        }

        let merge_mode = match serde_json::from_value::<AdminImportMergeMode>(
            root.get("merge_mode").cloned().unwrap_or(Value::Null),
        ) {
            Ok(value) => value,
            Err(_) => {
                return Ok(Err(invalid_request(
                    "merge_mode 仅支持 skip / overwrite / error",
                )))
            }
        };

        let config_body =
            match build_admin_system_data_import_part_body(&root, "config_data", merge_mode) {
                Ok(value) => value,
                Err(err) => return Ok(Err(err)),
            };
        let users_body =
            match build_admin_system_data_import_part_body(&root, "user_data", merge_mode) {
                Ok(value) => value,
                Err(err) => return Ok(Err(err)),
            };

        // Validate both documents before the first write; invalid user data must not partially restore config.
        let config = match parse_admin_system_config_import_request(&config_body) {
            Ok(parsed) => parsed.request.document,
            Err(err) => return Ok(Err(err)),
        };
        if let Err(detail) = validate_imported_config_references(&config) {
            return Ok(Err(invalid_request(detail)));
        }
        let provider_names = config
            .providers
            .iter()
            .filter_map(|provider| {
                provider
                    .id
                    .as_ref()
                    .map(|id| (id.clone(), provider.name.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        let mut user_value = match serde_json::from_slice::<Value>(&users_body) {
            Ok(Value::Object(value)) => value,
            _ => return Ok(Err(invalid_request("用户备份格式无效"))),
        };
        user_value
            .entry("provider_names".to_string())
            .or_insert_with(|| json!(provider_names));
        let users = match super::user_backup::parse_users_backup(Value::Object(user_value.clone()))
        {
            Ok(backup) => backup,
            Err(err) => return Ok(Err(err)),
        };
        if users.provider_names != provider_names {
            return Ok(Err(invalid_request(
                "完整备份中的配置与用户提供商映射不一致",
            )));
        }
        if merge_mode == AdminImportMergeMode::Error && users.has_admin_profile() {
            return Ok(Err(invalid_request(
                "当前管理员已存在，请选择跳过或覆盖模式",
            )));
        }
        let users_body = Bytes::from(
            serde_json::to_vec(&user_value)
                .map_err(|err| GatewayError::Internal(err.to_string()))?,
        );
        let config_result = match self.import_admin_system_config(&config_body).await? {
            Ok(payload) => payload,
            Err(err) => return Ok(Err(err)),
        };
        let users_result = match self
            .import_admin_system_users(&users_body, operator_id)
            .await?
        {
            Ok(payload) => payload,
            Err(err) => return Ok(Err(err)),
        };

        Ok(Ok(json!({
            "message": "聚合数据导入成功",
            "config": config_result,
            "users": users_result,
        })))
    }

    pub(crate) async fn import_admin_system_config(
        &self,
        request_body: &Bytes,
    ) -> Result<Result<Value, (http::StatusCode, Value)>, GatewayError> {
        macro_rules! invalid {
            ($expr:expr) => {
                match $expr {
                    Ok(value) => value,
                    Err(detail) => return Ok(Err(invalid_request(detail))),
                }
            };
        }
        macro_rules! routed {
            ($expr:expr) => {
                match $expr {
                    Ok(value) => value,
                    Err(err) => return Ok(Err(err)),
                }
            };
        }

        let parsed = routed!(parse_admin_system_config_import_request(request_body));
        invalid!(validate_imported_config_references(
            &parsed.request.document
        ));
        let root = parsed.root;
        let merge_mode = parsed.request.merge_mode;

        let imported_global_models = routed!(
            parse_admin_system_config_array::<ImportedGlobalModel>(&root, "global_models")
        );
        let imported_providers = routed!(parse_admin_system_config_array::<ImportedProvider>(
            &root,
            "providers"
        ));
        let imported_proxy_nodes = routed!(parse_admin_system_config_array::<ImportedProxyNode>(
            &root,
            "proxy_nodes"
        ));
        let imported_system_configs = routed!(parse_admin_system_config_array::<
            ImportedSystemConfig,
        >(&root, "system_configs",));
        let imported_routing_strategy = parsed.request.document.routing_strategy;
        if let Some(strategy) = imported_routing_strategy.as_ref() {
            if let Err(detail) = validate_imported_routing_strategy_config(&strategy.config_json) {
                return Ok(Err(invalid_request(detail)));
            }
        }

        let mut stats = AdminSystemConfigImportStats::default();

        let (imported_proxy_configs, imported_system_configs): (Vec<_>, Vec<_>) =
            imported_system_configs.into_iter().partition(|item| {
                PROXY_NODE_CONFIG_KEYS
                    .contains(&normalize_imported_system_config_key(&item.value.key).as_str())
            });
        let mut existing_system_config_keys = self
            .list_system_config_entries()
            .await?
            .into_iter()
            .map(|entry| normalize_imported_system_config_key(&entry.key))
            .collect::<BTreeSet<_>>();
        let node_id_map = routed!(
            self.import_admin_system_proxy_nodes(
                imported_proxy_nodes
                    .into_iter()
                    .map(|item| item.value)
                    .collect(),
                merge_mode,
                &mut stats.proxy_nodes,
            )
            .await?
        );
        for imported_config_item in imported_proxy_configs {
            let config = imported_config_item.value;
            let key = normalize_imported_system_config_key(&config.key);
            let exists = existing_system_config_keys.contains(&key);
            match (exists, merge_mode) {
                (true, AdminImportMergeMode::Skip) => {
                    stats.system_configs.skipped += 1;
                    continue;
                }
                (true, AdminImportMergeMode::Error) => {
                    return Ok(Err(invalid_request(format!("系统配置 '{key}' 已存在"))))
                }
                _ => {}
            }
            let proxy_node_id =
                invalid!(remap_proxy_node_setting(&key, &config.value, &node_id_map));
            self.upsert_system_config_entry(&key, &proxy_node_id, config.description.as_deref())
                .await?;
            if exists {
                stats.system_configs.updated += 1;
            } else {
                stats.system_configs.created += 1;
            }
            existing_system_config_keys.insert(key);
        }

        let mut global_models_by_name = self
            .list_admin_system_backup_global_models()
            .await?
            .into_iter()
            .map(|model| (model.name.clone(), model))
            .collect::<BTreeMap<_, _>>();

        for imported_model in imported_global_models {
            let (_, model) = imported_model.into_parts();
            let name = invalid!(trim_required(&model.name, "name"));
            let display_name = invalid!(trim_required(&model.display_name, "display_name"));
            let default_price_per_request = invalid!(normalize_optional_price(
                model.default_price_per_request,
                "default_price_per_request",
            ));
            let default_tiered_pricing = invalid!(normalize_json_object(
                model.default_tiered_pricing,
                "default_tiered_pricing",
            ));
            let supported_capabilities =
                normalize_supported_capabilities(model.supported_capabilities);
            let config = invalid!(normalize_json_object(model.config, "config"));

            if let Some(existing) = global_models_by_name.get(&name).cloned() {
                match merge_mode {
                    AdminImportMergeMode::Skip => {
                        stats.global_models.skipped += 1;
                    }
                    AdminImportMergeMode::Error => {
                        return Ok(Err(invalid_request(format!("GlobalModel '{name}' 已存在"))));
                    }
                    AdminImportMergeMode::Overwrite => {
                        let mut record = invalid!(UpdateAdminGlobalModelRecord::new(
                            existing.id.clone(),
                            display_name,
                            model.is_active,
                            default_price_per_request,
                            default_tiered_pricing,
                            supported_capabilities,
                            config,
                        )
                        .map_err(|err| err.to_string()));
                        record.usage_count = model.usage_count;
                        let Some(updated) = self.update_admin_global_model(&record).await? else {
                            return Ok(Err(invalid_request(format!(
                                "更新 GlobalModel '{name}' 失败"
                            ))));
                        };
                        global_models_by_name.insert(name, updated);
                        stats.global_models.updated += 1;
                    }
                }
                continue;
            }

            let mut record = invalid!(CreateAdminGlobalModelRecord::new(
                Uuid::new_v4().to_string(),
                name.clone(),
                display_name,
                model.is_active,
                default_price_per_request,
                default_tiered_pricing,
                supported_capabilities,
                config,
            )
            .map_err(|err| err.to_string()));
            record.usage_count = model.usage_count;
            let Some(created) = self.create_admin_global_model(&record).await? else {
                return Ok(Err(invalid_request(format!(
                    "创建 GlobalModel '{name}' 失败"
                ))));
            };
            global_models_by_name.insert(name, created);
            stats.global_models.created += 1;
        }

        let mut providers_by_name = self
            .list_provider_catalog_providers(false)
            .await?
            .into_iter()
            .map(|provider| (provider.name.clone(), provider))
            .collect::<BTreeMap<_, _>>();
        let mut imported_provider_id_map = BTreeMap::<String, String>::new();
        let mut imported_key_id_map = BTreeMap::<String, String>::new();

        for imported_provider_item in imported_providers {
            let (raw_provider, imported_provider) = imported_provider_item.into_parts();
            let provider_name = invalid!(trim_required(&imported_provider.name, "name"));
            invalid!(
                crate::provider_transport::validate_anthropic_compatibility_profile_config(
                    imported_provider.config.as_ref(),
                )
                .map_err(|_| "无效的 Anthropic compatibility profile".to_string())
            );
            let existing_provider = providers_by_name.get(&provider_name).cloned();

            let provider = if let Some(existing) = existing_provider {
                match merge_mode {
                    AdminImportMergeMode::Skip => {
                        stats.providers.skipped += 1;
                        existing
                    }
                    AdminImportMergeMode::Error => {
                        return Ok(Err(invalid_request(format!(
                            "Provider '{provider_name}' 已存在"
                        ))));
                    }
                    AdminImportMergeMode::Overwrite => {
                        let mut fields = raw_provider.clone();
                        if imported_provider.billing_type.is_none() {
                            fields.remove("billing_type");
                        }
                        let patch = match AdminProviderUpdatePatch::from_object(fields) {
                            Ok(patch) => patch,
                            Err(_) => {
                                return Ok(Err(invalid_request(format!(
                                    "Provider '{provider_name}' 配置格式无效"
                                ))));
                            }
                        };
                        let mut updated = invalid!(
                            self.build_admin_update_provider_record(&existing, patch)
                                .await
                        );
                        updated.billing_type = imported_provider.billing_type.clone();
                        updated.monthly_used_usd = imported_provider.monthly_used_usd;
                        updated.quota_last_reset_at_unix_secs =
                            imported_provider.quota_last_reset_at_unix_secs;
                        updated.quota_expires_at_unix_secs =
                            imported_provider.quota_expires_at_unix_secs;
                        updated.proxy = invalid!(remap_import_proxy(
                            imported_provider.proxy.clone(),
                            &node_id_map
                        ));
                        updated.config = invalid!(encrypt_imported_provider_config(
                            self.admin(),
                            imported_provider.config.clone(),
                        ));
                        let Some(persisted) =
                            self.update_provider_catalog_provider(&updated).await?
                        else {
                            return Ok(Err(invalid_request(format!(
                                "更新 Provider '{provider_name}' 失败"
                            ))));
                        };
                        providers_by_name.insert(provider_name.clone(), persisted.clone());
                        stats.providers.updated += 1;
                        persisted
                    }
                }
            } else {
                let payload = match serde_json::from_value::<AdminProviderCreateRequest>(
                    Value::Object(raw_provider.clone()),
                ) {
                    Ok(payload) => payload,
                    Err(_) => {
                        return Ok(Err(invalid_request(format!(
                            "Provider '{provider_name}' 配置格式无效"
                        ))));
                    }
                };
                let (mut record, shift_existing_priorities_from) =
                    invalid!(self.build_admin_create_provider_record(payload).await);
                record.billing_type = imported_provider.billing_type.clone();
                record.monthly_used_usd = imported_provider.monthly_used_usd;
                record.quota_last_reset_at_unix_secs =
                    imported_provider.quota_last_reset_at_unix_secs;
                record.quota_expires_at_unix_secs = imported_provider.quota_expires_at_unix_secs;
                record.quota_reset_day = imported_provider.quota_reset_day;
                record.max_retries = imported_provider.max_retries;
                if let Some(enable_format_conversion) = imported_provider.enable_format_conversion {
                    record.enable_format_conversion = enable_format_conversion;
                }
                record.proxy = invalid!(remap_import_proxy(
                    imported_provider.proxy.clone(),
                    &node_id_map
                ));
                record.config = invalid!(encrypt_imported_provider_config(
                    self.admin(),
                    imported_provider.config.clone(),
                ));
                let Some(created) = self
                    .create_provider_catalog_provider(&record, shift_existing_priorities_from)
                    .await?
                else {
                    return Ok(Err(invalid_request(format!(
                        "创建 Provider '{provider_name}' 失败"
                    ))));
                };
                providers_by_name.insert(provider_name.clone(), created.clone());
                stats.providers.created += 1;
                created
            };
            if let Some(source_id) = imported_provider.id.as_deref() {
                imported_provider_id_map.insert(source_id.to_string(), provider.id.clone());
            }

            let imported_endpoints = routed!(parse_admin_system_config_nested_array::<
                ImportedEndpoint,
            >(&raw_provider, "endpoints"));
            let mut existing_endpoints_by_format = self
                .list_provider_catalog_endpoints_by_provider_ids(std::slice::from_ref(&provider.id))
                .await?
                .into_iter()
                .map(|endpoint| (endpoint.api_format.clone(), endpoint))
                .collect::<BTreeMap<_, _>>();

            for imported_endpoint_item in imported_endpoints {
                let (raw_endpoint, imported_endpoint) = imported_endpoint_item.into_parts();
                let normalized_api_format = invalid!(normalize_import_endpoint_format(
                    &imported_endpoint.api_format
                ));
                invalid!(
                    crate::provider_transport::validate_anthropic_compatibility_profile_config(
                        imported_endpoint.config.as_ref(),
                    )
                    .map_err(|_| "无效的 Anthropic compatibility profile".to_string())
                );
                let existing_endpoint = existing_endpoints_by_format
                    .get(&normalized_api_format)
                    .cloned();

                if let Some(existing_endpoint) = existing_endpoint {
                    match merge_mode {
                        AdminImportMergeMode::Skip => {
                            stats.endpoints.skipped += 1;
                        }
                        AdminImportMergeMode::Error => {
                            return Ok(Err(invalid_request(format!(
                                "Endpoint '{normalized_api_format}' 已存在于 Provider '{provider_name}'"
                            ))));
                        }
                        AdminImportMergeMode::Overwrite => {
                            let Some((normalized_signature, api_family, endpoint_kind)) =
                                admin_endpoint_signature_parts(&normalized_api_format)
                            else {
                                return Ok(Err(invalid_request(format!(
                                    "无效的 api_format: {}",
                                    imported_endpoint.api_format
                                ))));
                            };
                            let patch = match AdminProviderEndpointUpdatePatch::from_object(
                                raw_endpoint.clone(),
                            ) {
                                Ok(patch) => patch,
                                Err(_) => {
                                    return Ok(Err(invalid_request(
                                        "Provider Endpoint 配置格式无效",
                                    )));
                                }
                            };
                            let (fields, payload) = patch.into_parts();
                            let normalized_base_url = match payload.base_url.as_deref() {
                                Some(base_url) => {
                                    Some(invalid!(normalize_admin_base_url(base_url)))
                                }
                                None => None,
                            };
                            let update_fields =
                                admin_provider_endpoints_pure::AdminProviderEndpointUpdateFields {
                                    base_url: normalized_base_url,
                                    custom_path: payload.custom_path,
                                    header_rules: payload.header_rules,
                                    body_rules: payload.body_rules,
                                    max_retries: payload.max_retries,
                                    is_active: payload.is_active,
                                    config: payload.config,
                                    proxy: payload.proxy,
                                    format_acceptance_config: payload.format_acceptance_config,
                                };
                            let mut updated = invalid!(
                                admin_provider_endpoints_pure::apply_admin_provider_endpoint_update_fields(
                                    &existing_endpoint,
                                    |field| fields.contains(field)
                                        && (field != "max_retries" || imported_endpoint.max_retries.is_some()),
                                    |field| fields.is_null(field),
                                    &update_fields,
                                )
                            );
                            // Backups preserve the stored inheritance value; the
                            // interactive endpoint editor requires an explicit count.
                            updated.max_retries = imported_endpoint.max_retries;
                            if fields.contains("proxy") {
                                updated.proxy = invalid!(remap_import_proxy(
                                    imported_endpoint.proxy.clone(),
                                    &node_id_map,
                                ));
                            }
                            updated.api_format = normalized_signature.to_string();
                            updated.api_family = Some(api_family.to_string());
                            updated.endpoint_kind = Some(endpoint_kind.to_string());
                            updated.updated_at_unix_secs = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .ok()
                                .map(|duration| duration.as_secs());
                            let Some(persisted) =
                                self.update_provider_catalog_endpoint(&updated).await?
                            else {
                                return Ok(Err(invalid_request(format!(
                                    "更新 Endpoint '{normalized_api_format}' 失败"
                                ))));
                            };
                            existing_endpoints_by_format
                                .insert(normalized_api_format.clone(), persisted);
                            stats.endpoints.updated += 1;
                        }
                    }
                    continue;
                }

                let Some((normalized_signature, api_family, endpoint_kind)) =
                    admin_endpoint_signature_parts(&normalized_api_format)
                else {
                    return Ok(Err(invalid_request(format!(
                        "无效的 api_format: {}",
                        imported_endpoint.api_format
                    ))));
                };
                let now_unix_secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .ok()
                    .map(|duration| duration.as_secs())
                    .unwrap_or(0);
                let mut record = invalid!(
                    admin_provider_endpoints_pure::build_admin_provider_endpoint_record(
                        Uuid::new_v4().to_string(),
                        provider.id.clone(),
                        normalized_signature.to_string(),
                        api_family.to_string(),
                        endpoint_kind.to_string(),
                        invalid!(normalize_admin_base_url(&imported_endpoint.base_url)),
                        imported_endpoint.custom_path.clone(),
                        imported_endpoint.header_rules.clone(),
                        imported_endpoint.body_rules.clone(),
                        imported_endpoint.max_retries.unwrap_or(2),
                        imported_endpoint.config.clone(),
                        invalid!(remap_import_proxy(
                            imported_endpoint.proxy.clone(),
                            &node_id_map
                        )),
                        imported_endpoint.format_acceptance_config.clone(),
                        now_unix_secs,
                    )
                );
                record = record.with_health_score(1.0);
                record.max_retries = imported_endpoint.max_retries;
                record.is_active = imported_endpoint.is_active;
                let Some(created) = self.create_provider_catalog_endpoint(&record).await? else {
                    return Ok(Err(invalid_request(format!(
                        "创建 Endpoint '{normalized_api_format}' 失败"
                    ))));
                };
                existing_endpoints_by_format.insert(normalized_api_format, created);
                stats.endpoints.created += 1;
            }

            let now_unix_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            let imported_keys = routed!(parse_admin_system_config_nested_array::<
                ImportedProviderKey,
            >(&raw_provider, "api_keys"));
            let mut existing_keys = self
                .list_provider_catalog_keys_by_provider_ids(std::slice::from_ref(&provider.id))
                .await?;
            let initial_key_ids = existing_keys
                .iter()
                .map(|key| key.id.clone())
                .collect::<BTreeSet<_>>();
            let mut matched_key_ids = BTreeSet::new();
            for imported_key_item in imported_keys {
                let (raw_key, imported_key) = imported_key_item.into_parts();
                let source_id = imported_key.id.as_deref().expect("validated source key id");
                let existing_key_index = existing_keys
                    .iter()
                    .position(|key| key.id == source_id)
                    .or_else(|| {
                        existing_keys.iter().position(|key| {
                            initial_key_ids.contains(&key.id)
                                && !matched_key_ids.contains(&key.id)
                                && matches_imported_key_credentials(
                                    self.admin(),
                                    &imported_key,
                                    key,
                                )
                        })
                    });
                let proxy = invalid!(remap_import_proxy(imported_key.proxy.clone(), &node_id_map));
                if let Some(index) = existing_key_index {
                    let existing_key = existing_keys[index].clone();
                    if !matched_key_ids.insert(existing_key.id.clone()) {
                        return Ok(Err(invalid_request("多个渠道 Key 匹配同一个目标 Key")));
                    }
                    imported_key_id_map.insert(source_id.to_string(), existing_key.id.clone());
                    match merge_mode {
                        AdminImportMergeMode::Skip => {
                            stats.keys.skipped += 1;
                        }
                        AdminImportMergeMode::Error => {
                            return Ok(Err(invalid_request(format!(
                                "Provider '{provider_name}' 中存在重复 Key"
                            ))));
                        }
                        AdminImportMergeMode::Overwrite => {
                            let mut updated = invalid!(build_imported_provider_key_record(
                                self.admin(),
                                &provider.id,
                                &imported_key,
                                Some(&existing_key),
                                proxy,
                                now_unix_secs
                            ));
                            apply_imported_provider_key_state(
                                &mut updated,
                                &imported_key,
                                &raw_key,
                            );
                            let update = build_provider_catalog_key_admin_cas_update(
                                &existing_key,
                                updated,
                                &provider.provider_type,
                            );
                            if !self
                                .compare_and_update_provider_catalog_key_admin_state(&update)
                                .await?
                            {
                                return Ok(Err((
                                    http::StatusCode::CONFLICT,
                                    json!({
                                        "detail": format!("Provider '{provider_name}' 的 Key 已被其他请求更新，请重试")
                                    }),
                                )));
                            }
                            self.restore_provider_catalog_key_state(&update.key).await?;
                            let Some(persisted) = self
                                .read_provider_catalog_keys_by_ids(std::slice::from_ref(
                                    &existing_key.id,
                                ))
                                .await?
                                .into_iter()
                                .next()
                            else {
                                return Ok(Err(invalid_request("渠道 Key 更新后无法读取")));
                            };
                            existing_keys[index] = persisted;
                            stats.keys.updated += 1;
                        }
                    }
                    continue;
                }
                let mut record = invalid!(build_imported_provider_key_record(
                    self.admin(),
                    &provider.id,
                    &imported_key,
                    None,
                    proxy,
                    now_unix_secs
                ));
                apply_imported_provider_key_state(&mut record, &imported_key, &raw_key);
                let Some(created) = self.create_provider_catalog_key(&record).await? else {
                    return Ok(Err(invalid_request(format!(
                        "创建 Provider '{provider_name}' 的 Key 失败"
                    ))));
                };
                imported_key_id_map.insert(source_id.to_string(), created.id.clone());
                existing_keys.push(created);
                stats.keys.created += 1;
            }

            let imported_models = routed!(parse_admin_system_config_nested_array::<
                ImportedProviderModel,
            >(&raw_provider, "models"));
            let mut existing_models_by_name = self
                .list_admin_system_backup_provider_models(&provider.id)
                .await?
                .into_iter()
                .map(|model| (model.provider_model_name.clone(), model))
                .collect::<BTreeMap<_, _>>();

            for imported_model_item in imported_models {
                let (_, imported_model) = imported_model_item.into_parts();
                let Some(global_model_name) = imported_model
                    .global_model_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    return Ok(Err(invalid_request(format!(
                        "模型缺少 global_model_name (Provider: {provider_name})"
                    ))));
                };
                let Some(global_model_id) = global_models_by_name
                    .get(global_model_name)
                    .map(|model| model.id.clone())
                else {
                    return Ok(Err(invalid_request(format!(
                        "GlobalModel '{global_model_name}' 不存在"
                    ))));
                };

                let provider_model_name = invalid!(trim_required(
                    &imported_model.provider_model_name,
                    "provider_model_name"
                ));
                let existing_model = existing_models_by_name.get(&provider_model_name).cloned();

                if let Some(existing_model) = existing_model {
                    match merge_mode {
                        AdminImportMergeMode::Skip => {
                            stats.models.skipped += 1;
                        }
                        AdminImportMergeMode::Error => {
                            return Ok(Err(invalid_request(format!(
                                "Model '{provider_model_name}' 已存在于 Provider '{provider_name}'"
                            ))));
                        }
                        AdminImportMergeMode::Overwrite => {
                            let record = invalid!(build_import_provider_model_record(
                                &provider.id,
                                Some(&existing_model.id),
                                &global_model_id,
                                &imported_model,
                            ));
                            let Some(updated) = self.update_admin_provider_model(&record).await?
                            else {
                                return Ok(Err(invalid_request(format!(
                                    "更新 Provider '{provider_name}' 的模型 '{provider_model_name}' 失败"
                                ))));
                            };
                            existing_models_by_name.insert(provider_model_name, updated);
                            stats.models.updated += 1;
                        }
                    }
                    continue;
                }

                let record = invalid!(build_import_provider_model_record(
                    &provider.id,
                    None,
                    &global_model_id,
                    &imported_model,
                ));
                let Some(created) = self.create_admin_provider_model(&record).await? else {
                    return Ok(Err(invalid_request(format!(
                        "创建 Provider '{provider_name}' 的模型 '{provider_model_name}' 失败"
                    ))));
                };
                existing_models_by_name.insert(provider_model_name, created);
                stats.models.created += 1;
            }
        }

        if let Some(imported_strategy) = imported_routing_strategy {
            let mut config_json = imported_strategy.config_json;
            invalid!(remap_routing_strategy_config(
                &mut config_json,
                &imported_provider_id_map,
                &imported_key_id_map,
            ));
            let existing = self
                .find_routing_group(RoutingGroupLookupKey::SystemDefault)
                .await?;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|duration| duration.as_secs() as i64)
                .unwrap_or(0);
            let (persisted, should_record_version, was_existing) = if let Some(existing) = existing
            {
                match merge_mode {
                    AdminImportMergeMode::Skip => (existing, false, true),
                    AdminImportMergeMode::Error => {
                        return Ok(Err(invalid_request("系统默认调度策略已存在")));
                    }
                    AdminImportMergeMode::Overwrite => {
                        let latest_version = self
                            .list_routing_group_versions(&existing.id)
                            .await?
                            .into_iter()
                            .map(|version| version.version)
                            .max()
                            .unwrap_or(0);
                        let next_version = existing.version.max(latest_version).saturating_add(1);
                        let Some(updated) = self
                            .update_routing_group(
                                &existing.id,
                                UpdateRoutingGroupRecord {
                                    name: Some(imported_strategy.name),
                                    description: Some(imported_strategy.description),
                                    enabled: Some(imported_strategy.enabled),
                                    is_system_default: Some(true),
                                    config_json: Some(config_json),
                                    version: Some(next_version),
                                    updated_at: now,
                                    published_at: Some(Some(now)),
                                },
                            )
                            .await?
                        else {
                            return Ok(Err(invalid_request("更新系统默认调度策略失败")));
                        };
                        (updated, true, true)
                    }
                }
            } else {
                let Some(created) = self
                    .create_routing_group(CreateRoutingGroupRecord {
                        id: Uuid::new_v4().to_string(),
                        name: imported_strategy.name,
                        description: imported_strategy.description,
                        enabled: imported_strategy.enabled,
                        is_system_default: true,
                        config_json,
                        version: imported_strategy.version.max(1),
                        created_at: now,
                        updated_at: now,
                        published_at: Some(now),
                    })
                    .await?
                else {
                    return Ok(Err(invalid_request("创建系统默认调度策略失败")));
                };
                (created, true, false)
            };
            if was_existing {
                if should_record_version {
                    stats.routing_strategy.updated += 1;
                } else {
                    stats.routing_strategy.skipped += 1;
                }
            } else {
                stats.routing_strategy.created += 1;
            }
            if should_record_version {
                let recorded = self
                    .create_routing_group_version(CreateRoutingGroupVersionRecord {
                        id: Uuid::new_v4().to_string(),
                        group_id: persisted.id.clone(),
                        version: persisted.version,
                        config_json: persisted.config_json.clone(),
                        created_at: now,
                        created_by: None,
                    })
                    .await?;
                if recorded.is_none() {
                    return Ok(Err(invalid_request("调度策略历史记录未能写入，导入已中止")));
                }
            }
        }

        for imported_config_item in imported_system_configs {
            let (_, system_config) = imported_config_item.into_parts();
            let ImportedSystemConfig {
                key,
                value,
                description,
            } = system_config;
            let normalized_key = normalize_imported_system_config_key(&key);
            let exists = existing_system_config_keys.contains(&normalized_key);
            match (exists, merge_mode) {
                (true, AdminImportMergeMode::Skip) => {
                    stats.system_configs.skipped += 1;
                    continue;
                }
                (true, AdminImportMergeMode::Error) => {
                    return Ok(Err(invalid_request(format!(
                        "SystemConfig '{normalized_key}' 已存在"
                    ))));
                }
                _ => {}
            }

            let request_bytes = Bytes::from(
                serde_json::to_vec(&json!({
                    "value": value,
                    "description": description,
                }))
                .map_err(|err| GatewayError::Internal(err.to_string()))?,
            );
            let update_result = self
                .apply_backup_system_config(&key, &request_bytes)
                .await?;
            match update_result {
                Ok(_) => {
                    if exists {
                        stats.system_configs.updated += 1;
                    } else {
                        stats.system_configs.created += 1;
                        existing_system_config_keys.insert(normalized_key);
                    }
                }
                Err((status, payload)) => return Ok(Err((status, payload))),
            }
        }

        Ok(Ok(json!({
            "message": "配置导入成功",
            "stats": stats,
        })))
    }

    pub(super) async fn import_admin_system_user_usage_aggregates(
        &self,
        value: Option<&Value>,
        supplemental_user_daily: &[AdminSystemStatsUserDailyAggregate],
        user_id_map: &BTreeMap<String, String>,
        api_key_id_map: &BTreeMap<String, String>,
        merge_mode: AdminImportMergeMode,
    ) -> Result<Option<AdminSystemUsageAggregateImportSummary>, GatewayError> {
        let mut snapshot = match value {
            Some(value) if !value.is_null() => serde_json::from_value::<
                AdminSystemUsageAggregateSnapshot,
            >(value.clone())
            .map_err(|err| GatewayError::Client {
                status: http::StatusCode::BAD_REQUEST,
                message: format!("usage_aggregates 格式无效: {err}"),
            })?,
            _ => AdminSystemUsageAggregateSnapshot::default(),
        };
        let mut existing_user_totals = BTreeMap::<String, (u64, u64)>::new();
        for row in &snapshot.stats_user_daily {
            let total_tokens = row
                .input_tokens
                .saturating_add(row.output_tokens)
                .saturating_add(row.cache_creation_tokens)
                .saturating_add(row.cache_read_tokens);
            let entry = existing_user_totals
                .entry(row.user_id.clone())
                .or_insert((0, 0));
            entry.0 = entry.0.saturating_add(row.total_requests);
            entry.1 = entry.1.saturating_add(total_tokens);
        }
        for row in supplemental_user_daily {
            let existing = existing_user_totals
                .get(&row.user_id)
                .copied()
                .unwrap_or_default();
            let request_delta = row.total_requests.saturating_sub(existing.0);
            let token_delta = row.input_tokens.saturating_sub(existing.1);
            if request_delta == 0 && token_delta == 0 {
                continue;
            }
            if let Some(existing_row) = snapshot
                .stats_user_daily
                .iter_mut()
                .rev()
                .find(|existing_row| existing_row.user_id == row.user_id)
            {
                existing_row.total_requests =
                    existing_row.total_requests.saturating_add(request_delta);
                existing_row.success_requests =
                    existing_row.success_requests.saturating_add(request_delta);
                existing_row.input_tokens = existing_row.input_tokens.saturating_add(token_delta);
            } else {
                let mut row = row.clone();
                row.total_requests = request_delta;
                row.success_requests = request_delta;
                row.input_tokens = token_delta;
                snapshot.stats_user_daily.push(row);
            }
        }
        if snapshot.stats_daily.is_empty()
            && snapshot.stats_user_daily.is_empty()
            && snapshot.stats_daily_api_key.is_empty()
        {
            return Ok(None);
        }
        self.import_admin_system_usage_aggregates(
            &snapshot,
            user_id_map,
            api_key_id_map,
            usage_aggregate_import_mode(merge_mode),
        )
        .await
        .map(Some)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
    use aether_data_contracts::repository::provider_catalog::{
        StoredProviderCatalogKey, StoredProviderCatalogProvider,
    };
    use serde_json::json;

    use super::{
        build_imported_provider_key_record, build_imported_user_usage_total_aggregates,
        matches_imported_key_credentials, normalize_import_endpoint_format,
        remap_routing_strategy_config, validate_imported_provider_key, ImportedProviderKey,
    };
    use crate::admin_api::AdminAppState;
    use crate::data::GatewayDataState;
    use crate::AppState;

    #[test]
    fn backup_rejects_source_duplicates_after_import_normalization() {
        let base = json!({
            "exported_at": "2026-09-09T00:00:00Z",
            "global_models": [{"name": "model", "display_name": "Model"}],
            "providers": [{
                "id": "provider", "name": "channel", "api_keys": [],
                "endpoints": [{"api_format": "openai:chat", "base_url": "https://api.example.com"}],
                "models": [{"global_model_name": "model", "provider_model_name": "upstream"}],
            }],
            "proxy_nodes": [],
            "system_configs": [{"key": "site_name", "value": "test"}],
        });
        let parse = |value| {
            serde_json::from_value::<aether_admin::system::AdminSystemConfigDocument>(value)
                .unwrap()
        };
        assert!(super::validate_imported_config_references(&parse(base.clone())).is_ok());
        for field in [
            "global_models",
            "providers",
            "endpoints",
            "models",
            "system_configs",
        ] {
            let mut value = base.clone();
            match field {
                "global_models" => value[field]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"name": " model ", "display_name": "Other"})),
                "providers" => {
                    let mut duplicate = value[field][0].clone();
                    duplicate["id"] = json!("another-id");
                    duplicate["name"] = json!(" channel ");
                    value[field].as_array_mut().unwrap().push(duplicate);
                }
                "endpoints" => value["providers"][0][field].as_array_mut().unwrap().push(
                    json!({"api_format": " openai:chat ", "base_url": "https://other.example.com"}),
                ),
                "models" => value["providers"][0][field]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"global_model_name": " model ", "provider_model_name": "other"})),
                _ => value[field]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"key": " site_name ", "value": "other"})),
            }
            let error = super::validate_imported_config_references(&parse(value)).unwrap_err();
            assert!(error.contains("重复"), "{field}: {error}");
        }
    }

    #[test]
    fn users_import_validates_content_without_a_version_gate() {
        for version in ["1.3", "1.4", "1.5", "2.2"] {
            let error = super::super::user_backup::parse_users_backup(json!({"version": version}))
                .expect_err("the empty document has no restorable content");
            assert_eq!(error.0, axum::http::StatusCode::BAD_REQUEST);
            assert!(!error.1["detail"].as_str().unwrap().contains("版本"));
        }
    }

    #[test]
    fn import_remaps_routing_strategy_provider_and_key_ids() {
        let mut config = json!({
            "allowed_models": [],
            "default_policy": {
                "priority_mode": "provider",
                "scheduling_mode": "fixed_order"
            },
            "model_policies": [{
                "model": "gpt-test",
                "allowed_providers": ["provider-old"],
                "allowed_keys": ["key-old"],
                "provider_priority_overrides": {
                    "provider-old": 1
                },
                "key_priority_overrides": {"key-old": 3}
            }],
            "rules": [{
                "id": "ui_provider_priority",
                "priority": 1,
                "enabled": true,
                "phase": "client_request",
                "conditions": {},
                "actions": [
                    {"type": "restrict_providers", "provider_ids": ["provider-old"]},
                    {"type": "restrict_keys", "key_ids": ["key-old"]},
                    {"type": "set_provider_priority", "provider_id": "provider-old", "priority": 1},
                    {"type": "set_key_priority", "key_id": "key-old", "priority": 3}
                ]
            }]
        });

        remap_routing_strategy_config(
            &mut config,
            &std::collections::BTreeMap::from([(
                "provider-old".to_string(),
                "provider-new".to_string(),
            )]),
            &std::collections::BTreeMap::from([("key-old".to_string(), "key-new".to_string())]),
        )
        .expect("all routing references must map");

        assert_eq!(
            config["rules"][0]["actions"],
            json!([
                {"type": "restrict_providers", "provider_ids": ["provider-new"]},
                {"type": "restrict_keys", "key_ids": ["key-new"]},
                {"type": "set_provider_priority", "provider_id": "provider-new", "priority": 1},
                {"type": "set_key_priority", "key_id": "key-new", "priority": 3}
            ])
        );
        assert_eq!(
            config["model_policies"][0]["allowed_providers"],
            json!(["provider-new"])
        );
        assert_eq!(
            config["model_policies"][0]["provider_priority_overrides"],
            json!({"provider-new": 1})
        );
        assert_eq!(
            config["model_policies"][0]["key_priority_overrides"],
            json!({"key-new": 3})
        );
    }

    #[tokio::test]
    async fn users_backup_roundtrips_current_admin_keys_and_usage_with_sqlite() {
        let users = vec![
            json!({
                "id": "source-user-1",
                "username": "alice",
                "request_count": 12,
                "total_tokens": 3456
            }),
            json!({
                "id": "source-user-zero",
                "username": "zero",
                "request_count": 0,
                "total_tokens": 0
            }),
            json!({
                "username": "no-source-id",
                "request_count": 5,
                "total_tokens": 6
            }),
        ];

        let rows = build_imported_user_usage_total_aggregates(
            &users,
            Some(&json!("2026-05-25T12:34:56Z")),
        )
        .expect("supplemental usage aggregates should build");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].user_id, "source-user-1");
        assert_eq!(rows[0].username.as_deref(), Some("alice"));
        assert_eq!(rows[0].total_requests, 12);
        assert_eq!(rows[0].success_requests, 12);
        assert_eq!(rows[0].input_tokens, 3456);
        assert_eq!(rows[0].date_unix_secs % 86_400, 0);
        use crate::data::GatewayDataConfig;
        use aether_data::repository::auth::StoredAuthApiKeyExportRecord;
        use aether_data::repository::system::{
            AdminSystemUsageAggregateImportMode, AdminSystemUsageAggregateSnapshot,
        };
        use aether_data::repository::users::{StoredUserPreferenceRecord, StoredUserSessionRecord};
        use aether_data::{DatabaseDriver, SqlDatabaseConfig, SqlPoolConfig};
        use std::collections::BTreeMap;

        async fn stored_backup_keys(
            state: &AppState,
            user_id: &str,
        ) -> Vec<StoredAuthApiKeyExportRecord> {
            let session = state.data.begin_admin_backup(false).await.unwrap();
            let keys = session
                .api_keys()
                .list_backup_api_keys_by_user_ids(&[user_id.to_string()])
                .await
                .unwrap();
            session.commit().await.unwrap();
            keys
        }

        let mut states = Vec::new();
        let mut admins = Vec::new();
        for name in ["source", "target"] {
            let database = SqlDatabaseConfig::new(
                DatabaseDriver::Sqlite,
                "sqlite::memory:",
                SqlPoolConfig {
                    min_connections: 0,
                    max_connections: 1,
                    ..Default::default()
                },
            )
            .unwrap();
            let mut state = AppState::new()
                .unwrap()
                .with_data_config(
                    GatewayDataConfig::from_database_config(database)
                        .with_encryption_key(format!("{name}-backup-encryption")),
                )
                .unwrap();
            // This backup test exercises the persisted administrator and sessions,
            // rather than the unit-test-only authentication stores.
            state.auth_user_store = None;
            state.auth_session_store = None;
            state.run_database_migrations().await.unwrap();
            let provider = StoredProviderCatalogProvider::new(
                format!("{name}-provider"),
                "shared-provider".to_string(),
                None,
                "custom".to_string(),
            )
            .unwrap();
            state
                .create_provider_catalog_provider(&provider, None)
                .await
                .unwrap()
                .unwrap();
            let admin = state
                .create_local_auth_user_with_settings(
                    Some(format!("{name}@example.test")),
                    true,
                    format!("{name}-admin"),
                    bcrypt::hash(format!("{name}-password"), 4).unwrap(),
                    "admin".to_string(),
                    Some(vec![provider.id]),
                    None,
                    Some(vec!["gpt-5".to_string()]),
                    None,
                )
                .await
                .unwrap()
                .unwrap();
            admins.push(admin);
            states.push(state);
        }
        let source = &states[0];
        let target = &states[1];
        let source_user = &admins[0];
        let target_user = &admins[1];
        let source_admin = AdminAppState::new(source);
        let target_admin = AdminAppState::new(target);
        let now = chrono::Utc::now();
        target
            .create_user_session(StoredUserSessionRecord {
                id: "target-session".to_string(),
                user_id: target_user.id.clone(),
                client_device_id: "backup-test".to_string(),
                device_label: None,
                refresh_token_hash: "backup-test-refresh-hash".to_string(),
                prev_refresh_token_hash: None,
                rotated_at: None,
                last_seen_at: Some(now),
                expires_at: Some(now + chrono::Duration::days(1)),
                revoked_at: None,
                revoke_reason: None,
                ip_address: None,
                user_agent: None,
                created_at: Some(now),
                updated_at: Some(now),
            })
            .await
            .unwrap()
            .unwrap();
        source
            .write_user_preferences(StoredUserPreferenceRecord {
                user_id: source_user.id.clone(),
                avatar_url: Some("https://example.test/avatar.png".to_string()),
                bio: Some("backup profile".to_string()),
                default_provider_id: Some("source-provider".to_string()),
                default_provider_name: Some("shared-provider".to_string()),
                theme: "dark".to_string(),
                language: "en-US".to_string(),
                timezone: "Asia/Shanghai".to_string(),
                email_notifications: false,
                usage_alerts: true,
                announcement_notifications: false,
            })
            .await
            .unwrap()
            .unwrap();
        for (id, standalone, key) in [
            ("normal-key", false, "sk-backup-normal"),
            ("standalone-key", true, "sk-backup-standalone"),
        ] {
            let record = StoredAuthApiKeyExportRecord {
                user_id: source_user.id.clone(),
                api_key_id: id.to_string(),
                key_hash: crate::handlers::admin::auth::hash_admin_user_api_key(key),
                key_encrypted: (!standalone).then(|| {
                    aether_crypto::encrypt_python_fernet_plaintext("source-backup-encryption", key)
                        .unwrap()
                }),
                name: Some(id.to_string()),
                allowed_providers: Some(vec!["source-provider".to_string()]),
                allowed_api_formats: Some(vec!["openai:chat".to_string()]),
                allowed_models: Some(vec!["gpt-5".to_string()]),
                ip_rules: Some(vec!["192.0.2.0/24".to_string()]),
                rate_limit: standalone.then_some(0),
                concurrent_limit: Some(3),
                force_capabilities: Some(json!({"vision": true})),
                feature_settings: None,
                is_active: !standalone,
                is_locked: false,
                expires_at_unix_secs: Some(4_102_444_800),
                auto_delete_on_expiry: standalone,
                total_requests: 12,
                total_tokens: 150,
                total_cost_usd: 1.25,
                last_used_at_unix_secs: Some(1_700_000_000),
                created_at_unix_secs: Some(1_699_000_000),
                updated_at_unix_secs: Some(1_700_000_000),
                is_standalone: standalone,
            };
            assert!(source.restore_exported_api_key(&record).await.unwrap());
        }
        let snapshot: AdminSystemUsageAggregateSnapshot = serde_json::from_value(json!({
            "stats_daily": [],
            "stats_user_daily": [{"user_id": source_user.id, "username": source_user.username,
                "date_unix_secs": 1_699_920_000, "total_requests": 12, "success_requests": 11, "error_requests": 1,
                "input_tokens": 100, "output_tokens": 50, "cache_creation_tokens": 0, "cache_read_tokens": 0, "total_cost": 1.25}],
            "stats_daily_api_key": [{"api_key_id": "normal-key", "api_key_name": "normal-key",
                "date_unix_secs": 1_699_920_000, "total_requests": 12, "success_requests": 11, "error_requests": 1,
                "input_tokens": 100, "output_tokens": 50, "cache_creation_tokens": 0, "cache_read_tokens": 0, "total_cost": 1.25}],
        })).unwrap();
        source
            .import_admin_system_usage_aggregates(
                &snapshot,
                &BTreeMap::from([(source_user.id.clone(), source_user.id.clone())]),
                &BTreeMap::from([("normal-key".to_string(), "normal-key".to_string())]),
                AdminSystemUsageAggregateImportMode::Overwrite,
            )
            .await
            .unwrap();

        let backup = source_admin
            .build_admin_system_data_export_payload()
            .await
            .unwrap();
        assert_eq!(
            backup["user_data"]["users"][0]["api_keys"][0]["key"],
            "sk-backup-normal"
        );
        assert!(backup["user_data"]["users"][0]["api_keys"][0]["rate_limit"].is_null());
        assert!(backup["user_data"]["users"][0]["api_keys"][0]["key_encrypted"].is_null());
        let mut invalid = backup.clone();
        invalid["user_data"]["users"][0]["password_hash"] = json!("not-a-password-hash");
        invalid["merge_mode"] = json!("overwrite");
        assert!(target_admin
            .import_admin_system_data(
                &axum::body::Bytes::from(invalid.to_string()),
                Some(&target_user.id)
            )
            .await
            .unwrap()
            .is_err());
        assert!(stored_backup_keys(target, &target_user.id).await.is_empty());

        let mut request = backup.clone();
        request["merge_mode"] = json!("skip");
        let first = target_admin
            .import_admin_system_data(
                &axum::body::Bytes::from(request.to_string()),
                Some(&target_user.id),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first["users"]["stats"]["api_keys"]["created"], 1);
        assert_eq!(first["users"]["stats"]["standalone_keys"]["created"], 1);
        assert_eq!(first["users"]["reauthentication_required"], false);
        assert_eq!(
            target
                .find_user_auth_by_id(&target_user.id)
                .await
                .unwrap()
                .unwrap()
                .username,
            target_user.username
        );
        let keys_before = stored_backup_keys(target, &target_user.id).await;
        let normal_id = keys_before
            .iter()
            .find(|key| !key.is_standalone)
            .unwrap()
            .api_key_id
            .clone();
        let mut changed = keys_before
            .iter()
            .find(|key| !key.is_standalone)
            .unwrap()
            .clone();
        changed.rate_limit = Some(99);
        changed.concurrent_limit = Some(8);
        changed.expires_at_unix_secs = None;
        changed.force_capabilities = None;
        assert!(target.restore_exported_api_key(&changed).await.unwrap());
        request["merge_mode"] = json!("overwrite");
        let overwritten = target_admin
            .import_admin_system_data(
                &axum::body::Bytes::from(request.to_string()),
                Some(&target_user.id),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(overwritten["users"]["stats"]["api_keys"]["updated"], 1);
        assert_eq!(overwritten["users"]["reauthentication_required"], true);
        assert_eq!(target.count_active_admin_users().await.unwrap(), 1);
        let restored_user = target
            .find_user_auth_by_id(&target_user.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(restored_user.username, source_user.username);
        assert!(bcrypt::verify(
            "source-password",
            restored_user.password_hash.as_deref().unwrap()
        )
        .unwrap());
        assert!(target
            .list_user_sessions(&target_user.id)
            .await
            .unwrap()
            .is_empty());
        assert!(target
            .find_user_session(&target_user.id, "target-session")
            .await
            .unwrap()
            .unwrap()
            .revoked_at
            .is_some());
        let restored = target_admin
            .build_admin_system_data_export_payload()
            .await
            .unwrap();
        let user = &restored["user_data"]["users"][0];
        assert_eq!(user["id"], target_user.id);
        assert_eq!(user["allowed_providers"], json!(["target-provider"]));
        assert_eq!(
            user["preferences"]["default_provider_id"],
            "target-provider"
        );
        assert_eq!(user["preferences"]["theme"], "dark");
        assert_eq!(user["request_count"], 12);
        assert_eq!(user["total_tokens"], 150);
        let key = &user["api_keys"][0];
        assert_eq!(key["api_key_id"], normal_id);
        for field in [
            "key",
            "key_hash",
            "name",
            "allowed_models",
            "allowed_api_formats",
            "ip_rules",
            "rate_limit",
            "concurrent_limit",
            "force_capabilities",
            "is_active",
            "expires_at_unix_secs",
            "auto_delete_on_expiry",
            "total_requests",
            "total_tokens",
            "total_cost_usd",
            "last_used_at_unix_secs",
            "created_at_unix_secs",
        ] {
            assert_eq!(
                key[field], backup["user_data"]["users"][0]["api_keys"][0][field],
                "field={field}"
            );
        }
        assert_eq!(
            restored["user_data"]["usage_aggregates"]["stats_daily_api_key"][0]["api_key_id"],
            normal_id
        );
        assert_eq!(
            restored["user_data"]["usage_aggregates"]["stats_daily_api_key"][0]["total_requests"],
            12
        );
        let persisted = stored_backup_keys(target, &target_user.id).await;
        let normal = persisted.iter().find(|key| !key.is_standalone).unwrap();
        assert_eq!(
            aether_crypto::decrypt_python_fernet_ciphertext(
                "target-backup-encryption",
                normal.key_encrypted.as_deref().unwrap()
            )
            .unwrap(),
            "sk-backup-normal"
        );
        assert!(aether_crypto::decrypt_python_fernet_ciphertext(
            "source-backup-encryption",
            normal.key_encrypted.as_deref().unwrap()
        )
        .is_err());
        let standalone = persisted.iter().find(|key| key.is_standalone).unwrap();
        assert!(standalone.key_encrypted.is_none());
        assert!(!standalone.is_active);
        assert_eq!(standalone.rate_limit, Some(0));

        request["merge_mode"] = json!("skip");
        let repeated = target_admin
            .import_admin_system_data(
                &axum::body::Bytes::from(request.to_string()),
                Some(&target_user.id),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(repeated["users"]["stats"]["api_keys"]["skipped"], 1);
        assert_eq!(repeated["users"]["stats"]["standalone_keys"]["skipped"], 1);
        assert_eq!(stored_backup_keys(target, &target_user.id).await.len(), 2);
    }

    #[test]
    fn config_import_accepts_current_api_format_signatures() {
        for (raw, expected) in [
            ("openai:chat", "openai:chat"),
            ("openai:responses", "openai:responses"),
            ("openai:responses:compact", "openai:responses:compact"),
            ("openai:image", "openai:image"),
            ("claude:messages", "claude:messages"),
            ("gemini:generate_content", "gemini:generate_content"),
        ] {
            assert_eq!(normalize_import_endpoint_format(raw).unwrap(), expected);
        }
    }

    #[test]
    fn config_import_preserves_current_key_formats() {
        let item: ImportedProviderKey = serde_json::from_value(json!({
            "id": "key-1", "name": "current-key", "auth_type": "api_key",
            "api_formats": ["claude:messages", "openai:responses:compact"]
        }))
        .unwrap();
        validate_imported_provider_key(&item).unwrap();
        assert_eq!(
            item.api_formats.unwrap(),
            ["claude:messages", "openai:responses:compact"]
        );
    }

    #[test]
    fn config_import_rejects_unselected_key_format_scopes() {
        let item = serde_json::from_value(json!({
            "id": "key-1", "name": "current-key", "auth_type": "api_key",
            "api_formats": ["openai:responses"],
            "auth_type_by_format": {"openai:video": "bearer"}
        }))
        .unwrap();
        assert!(validate_imported_provider_key(&item).is_err());
    }

    #[test]
    fn config_import_preserves_explicit_empty_mismatch_scope() {
        let item = serde_json::from_value(json!({
            "id": "key-1", "name": "current-key", "auth_type": "api_key",
            "api_formats": ["openai:responses"],
            "allow_auth_channel_mismatch_formats": []
        }))
        .unwrap();
        let state = AppState::new().unwrap();
        let record = build_imported_provider_key_record(
            &AdminAppState::new(&state),
            "provider",
            &item,
            None,
            None,
            1,
        )
        .unwrap();
        assert_eq!(record.allow_auth_channel_mismatch_formats, Some(json!([])));
    }

    #[test]
    fn oauth_import_does_not_merge_accounts_with_the_same_name() {
        let state = AppState::new().unwrap().with_data_state_for_tests(
            GatewayDataState::disabled().with_encryption_key_for_tests("backup-test-key"),
        );
        let admin = AdminAppState::new(&state);
        let item: ImportedProviderKey = serde_json::from_value(json!({
            "id": "oauth-1", "name": "same-name", "auth_type": "oauth",
            "api_formats": ["openai:responses"],
            "auth_config": {"refresh_token": "refresh-1"}
        }))
        .unwrap();
        let existing =
            build_imported_provider_key_record(&admin, "provider", &item, None, None, 1).unwrap();
        let mut other = item.clone();
        other.id = Some("oauth-2".to_string());
        other.auth_config = Some(json!({"refresh_token": "refresh-2"}));
        assert!(!matches_imported_key_credentials(&admin, &other, &existing));
        assert!(matches_imported_key_credentials(&admin, &item, &existing));
    }

    #[test]
    fn oauth_import_preserves_access_token_without_auth_config() {
        let state = AppState::new().unwrap().with_data_state_for_tests(
            GatewayDataState::disabled().with_encryption_key_for_tests("backup-test-key"),
        );
        let admin = AdminAppState::new(&state);
        let item = serde_json::from_value(json!({
            "id": "oauth-1", "name": "oauth-key", "auth_type": "oauth",
            "api_formats": ["openai:responses"], "api_key": "access-token",
            "expires_at_unix_secs": 4_102_444_800u64
        }))
        .unwrap();
        let record =
            build_imported_provider_key_record(&admin, "provider", &item, None, None, 1).unwrap();
        assert_eq!(record.auth_type, "oauth");
        assert!(record.encrypted_auth_config.is_none());
        assert_eq!(record.expires_at_unix_secs, Some(4_102_444_800));
        assert_eq!(
            admin
                .decrypt_catalog_secret_with_fallbacks(record.encrypted_api_key.as_deref().unwrap())
                .as_deref(),
            Some("access-token")
        );
    }
}
