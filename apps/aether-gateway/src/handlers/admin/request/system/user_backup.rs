use std::collections::{BTreeMap, BTreeSet};

use aether_admin::system::{
    validate_admin_backup_fields, AdminImportMergeMode, AdminSystemConfigImportCounter,
};
use aether_data::repository::auth::StoredAuthApiKeyExportRecord;
use aether_data::repository::system::{
    AdminSystemUsageAggregateImportSummary, AdminSystemUsageAggregateSnapshot,
};
use aether_data::repository::users::{StoredUserExportRow, StoredUserPreferenceRecord};
use axum::{body::Bytes, http::StatusCode};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use uuid::Uuid;

use super::backup_state::AdminBackupState;
use super::import::{build_imported_user_usage_total_aggregates, invalid_request};
use crate::handlers::admin::auth::hash_admin_user_api_key;
use crate::handlers::admin::shared::{
    normalize_admin_list_policy_mode, normalize_admin_rate_limit_policy_mode,
    normalize_admin_user_api_formats, normalize_admin_user_ip_rules,
};
use crate::handlers::admin::system::shared::export::decrypt_admin_system_export_secret;
use crate::handlers::shared::normalize_feature_settings;
use crate::{GatewayError, GatewayUserPreferenceView};

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct UsersBackup {
    exported_at: String,
    #[serde(default)]
    pub(super) provider_names: BTreeMap<String, String>,
    users: Vec<UserBackup>,
    standalone_keys: Vec<ApiKeyBackup>,
    usage_aggregates: AdminSystemUsageAggregateSnapshot,
}

#[derive(Debug, Serialize, Deserialize)]
struct UserBackup {
    #[serde(flatten)]
    profile: StoredUserExportRow,
    preferences: Option<StoredUserPreferenceRecord>,
    #[serde(default)]
    request_count: u64,
    #[serde(default)]
    total_tokens: u64,
    api_keys: Vec<ApiKeyBackup>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiKeyBackup {
    key: Option<String>,
    #[serde(flatten)]
    record: StoredAuthApiKeyExportRecord,
}

#[derive(Default, Serialize)]
struct UsersImportStats {
    users: AdminSystemConfigImportCounter,
    api_keys: AdminSystemConfigImportCounter,
    standalone_keys: AdminSystemConfigImportCounter,
    usage_aggregates: Option<AdminSystemUsageAggregateImportSummary>,
    errors: Vec<String>,
}

impl UsersBackup {
    pub(super) fn has_admin_profile(&self) -> bool {
        !self.users.is_empty()
    }

    fn source_user_id(&self) -> Option<&str> {
        self.users
            .first()
            .map(|user| user.profile.id.as_str())
            .or_else(|| {
                self.usage_aggregates
                    .stats_user_daily
                    .first()
                    .map(|row| row.user_id.as_str())
            })
            .or_else(|| {
                self.standalone_keys
                    .first()
                    .map(|key| key.record.user_id.as_str())
            })
    }
}

pub(super) fn parse_users_backup(mut value: Value) -> Result<UsersBackup, (StatusCode, Value)> {
    normalize_users_backup_content(&mut value)?;
    let mut backup: UsersBackup = serde_json::from_value(value.clone())
        .map_err(|_| invalid_request("用户备份字段缺失或格式无效"))?;
    let restored =
        serde_json::to_value(&backup).map_err(|_| invalid_request("用户备份无法完整解析"))?;
    validate_admin_backup_fields(&value, &restored, "user_data").map_err(invalid_request)?;
    chrono::DateTime::parse_from_rfc3339(&backup.exported_at)
        .map_err(|_| invalid_request("exported_at 必须是 RFC3339 时间"))?;
    let mut provider_names = BTreeSet::new();
    for (id, name) in &backup.provider_names {
        if id.trim().is_empty() || name.trim().is_empty() || !provider_names.insert(name) {
            return Err(invalid_request("提供商映射的标识或名称无效"));
        }
    }
    let check_provider = |id: &String| {
        if backup.provider_names.contains_key(id) {
            Ok(())
        } else {
            Err(invalid_request(format!(
                "提供商引用 '{id}' 无法解析，请一并导入包含提供商映射的配置"
            )))
        }
    };
    for user in &mut backup.users {
        validate_backup_user(user)?;
        for id in user.profile.allowed_providers.iter().flatten() {
            check_provider(id)?;
        }
        if let Some(id) = user
            .preferences
            .as_ref()
            .and_then(|preferences| preferences.default_provider_id.as_ref())
        {
            check_provider(id)?;
        }
    }
    let source_user_id = backup.source_user_id().map(str::to_string);
    let mut ids = BTreeSet::new();
    let mut hashes = BTreeSet::new();
    for (key, standalone) in backup
        .users
        .iter_mut()
        .flat_map(|user| user.api_keys.iter_mut().map(|key| (key, false)))
        .chain(backup.standalone_keys.iter_mut().map(|key| (key, true)))
    {
        let record = &mut key.record;
        if record.api_key_id.trim().is_empty()
            || record.key_hash.len() != 64
            || !record
                .key_hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || Some(record.user_id.as_str()) != source_user_id.as_deref()
            || record.is_standalone != standalone
            || !ids.insert(record.api_key_id.clone())
            || !hashes.insert(record.key_hash.clone())
        {
            return Err(invalid_request("API Key 的标识、归属或类型无效"));
        }
        for id in record.allowed_providers.iter().flatten() {
            check_provider(id)?;
        }
        record.allowed_api_formats =
            normalize_admin_user_api_formats(record.allowed_api_formats.clone())
                .map_err(invalid_request)?;
        record.ip_rules =
            normalize_admin_user_ip_rules(record.ip_rules.clone()).map_err(invalid_request)?;
        record.feature_settings =
            normalize_feature_settings(record.feature_settings.clone()).map_err(invalid_request)?;
        if record.force_capabilities.as_ref().is_some_and(|value| {
            value
                .as_object()
                .is_none_or(|fields| fields.values().any(|value| !value.is_boolean()))
        }) || (record.auto_delete_on_expiry && record.expires_at_unix_secs.is_none())
        {
            return Err(invalid_request("API Key 能力配置或过期策略无效"));
        }
        if record.key_encrypted.is_some() && key.key.is_none() {
            return Err(invalid_request(
                "API Key 只有源系统密文，无法在目标系统完整恢复",
            ));
        }
        if let Some(plaintext) = key.key.as_deref() {
            if plaintext.is_empty() || hash_admin_user_api_key(plaintext) != record.key_hash {
                return Err(invalid_request("API Key 内容与校验值不一致"));
            }
        }
        record.key_encrypted = None;
        if record.rate_limit.is_some_and(|value| value < 0)
            || record.concurrent_limit.is_some_and(|value| value < 0)
            || !record.total_cost_usd.is_finite()
            || record.total_cost_usd < 0.0
            || record.total_requests > i64::MAX as u64
            || record.total_tokens > i64::MAX as u64
            || [
                record.expires_at_unix_secs,
                record.created_at_unix_secs,
                record.last_used_at_unix_secs,
                record.updated_at_unix_secs,
            ]
            .into_iter()
            .flatten()
            .any(|value| value > i64::MAX as u64)
        {
            return Err(invalid_request("API Key 的限流、用量或时间字段无效"));
        }
    }
    validate_backup_usage(&backup, &ids)?;
    Ok(backup)
}

fn validate_backup_user(user: &mut UserBackup) -> Result<(), (StatusCode, Value)> {
    let profile = &user.profile;
    if profile.role != "admin"
        || profile.auth_source != "local"
        || !profile.is_active
        || profile.id.trim().is_empty()
        || profile.username.trim().is_empty()
        || profile.password_hash.as_deref().is_none_or(str::is_empty)
    {
        return Err(invalid_request("备份必须包含有效的本地管理员资料"));
    }
    let password_hash = profile
        .password_hash
        .as_deref()
        .expect("password checked above");
    let hash_parts = password_hash
        .parse::<bcrypt::HashParts>()
        .map_err(|_| invalid_request("管理员密码哈希无效"))?;
    if password_hash.len() != 60
        || !(4..=31).contains(&hash_parts.get_cost())
        || bcrypt::BASE_64
            .decode(hash_parts.get_salt())
            .map_or(true, |salt| salt.len() != 16)
        || password_hash
            .rsplit('$')
            .next()
            .and_then(|value| value.get(22..))
            .and_then(|value| bcrypt::BASE_64.decode(value).ok())
            .is_none_or(|hash| hash.len() != 23)
    {
        return Err(invalid_request("管理员密码哈希无效"));
    }
    if user
        .preferences
        .as_ref()
        .is_some_and(|preferences| preferences.user_id != profile.id)
        || user.request_count > i64::MAX as u64
        || user.total_tokens > i64::MAX as u64
    {
        return Err(invalid_request("管理员偏好或用量字段无效"));
    }
    for mode in [
        &profile.allowed_providers_mode,
        &profile.allowed_api_formats_mode,
        &profile.allowed_models_mode,
    ] {
        if normalize_admin_list_policy_mode(mode).as_ref() != Ok(mode) {
            return Err(invalid_request("用户权限模式无效"));
        }
    }
    if normalize_admin_rate_limit_policy_mode(&profile.rate_limit_mode).as_ref()
        != Ok(&profile.rate_limit_mode)
    {
        return Err(invalid_request("用户限流模式无效"));
    }
    if profile.rate_limit.is_some_and(|value| value < 0) {
        return Err(invalid_request("rate_limit 必须是非负整数或 null"));
    }
    user.profile.allowed_api_formats =
        normalize_admin_user_api_formats(profile.allowed_api_formats.clone())
            .map_err(invalid_request)?;
    user.profile.feature_settings =
        normalize_feature_settings(user.profile.feature_settings.clone())
            .map_err(invalid_request)?;
    Ok(())
}

fn normalize_users_backup_content(value: &mut Value) -> Result<(), (StatusCode, Value)> {
    let root = value
        .as_object_mut()
        .ok_or_else(|| invalid_request("用户备份必须是对象"))?;
    root.remove("version");
    root.remove("merge_mode");
    let users = root
        .get("users")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_request("users 必须是数组"))?;
    if users.len() > 1 {
        return Err(invalid_request("备份包含多位用户，无法完整恢复到个人版"));
    }
    let mut source_ids = BTreeSet::new();
    if let Some(user) = users.first() {
        source_ids.insert(required_backup_user_id(user.get("id"))?.to_string());
    }
    if let Some(rows) = root
        .get("usage_aggregates")
        .and_then(|value| value.get("stats_user_daily"))
        .and_then(Value::as_array)
    {
        for row in rows {
            source_ids.insert(required_backup_user_id(row.get("user_id"))?.to_string());
        }
    }
    let standalone_keys = root
        .get("standalone_keys")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_request("standalone_keys 必须是数组"))?;
    for key in standalone_keys {
        if let Some(user_id) = key.get("user_id").filter(|value| !value.is_null()) {
            source_ids.insert(required_backup_user_id(Some(user_id))?.to_string());
        }
    }
    if source_ids.len() > 1 {
        return Err(invalid_request(
            "备份包含多位用户的数据，无法确定完整恢复的归属",
        ));
    }
    // Older personal backups omitted the administrator entirely. This ID is only an
    // import mapping key; no fabricated administrator or password is ever persisted.
    let source_id = source_ids
        .into_iter()
        .next()
        .unwrap_or_else(|| "import:backup-owner".to_string());
    for user in root["users"].as_array_mut().expect("users checked above") {
        let user = user
            .as_object_mut()
            .ok_or_else(|| invalid_request("users 项必须是对象"))?;
        for (mode_field, list_field) in [
            ("allowed_providers_mode", "allowed_providers"),
            ("allowed_api_formats_mode", "allowed_api_formats"),
            ("allowed_models_mode", "allowed_models"),
        ] {
            if user.get(mode_field).is_none_or(Value::is_null) {
                let mode = if user
                    .get(list_field)
                    .and_then(Value::as_array)
                    .is_some_and(|values| !values.is_empty())
                {
                    "specific"
                } else {
                    "unrestricted"
                };
                user.insert(mode_field.to_string(), json!(mode));
            }
        }
        if user.get("rate_limit_mode").is_none_or(Value::is_null) {
            let mode = if user.get("rate_limit").is_some_and(|value| !value.is_null()) {
                "custom"
            } else {
                "system"
            };
            user.insert("rate_limit_mode".to_string(), json!(mode));
        }
        if let Some(keys) = user.get_mut("api_keys").and_then(Value::as_array_mut) {
            for key in keys {
                normalize_backup_api_key(key, &source_id, false)?;
            }
        }
    }
    for key in root["standalone_keys"]
        .as_array_mut()
        .expect("standalone_keys checked above")
    {
        normalize_backup_api_key(key, &source_id, true)?;
    }
    Ok(())
}

fn required_backup_user_id(value: Option<&Value>) -> Result<&str, (StatusCode, Value)> {
    value
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| invalid_request("用户资料或统计中的用户标识无效"))
}

fn normalize_backup_api_key(
    key: &mut Value,
    source_user_id: &str,
    standalone: bool,
) -> Result<(), (StatusCode, Value)> {
    let key = key
        .as_object_mut()
        .ok_or_else(|| invalid_request("API Key 项必须是对象"))?;
    if key.get("user_id").is_none_or(Value::is_null) {
        key.insert("user_id".to_string(), json!(source_user_id));
    }
    if key.get("is_standalone").is_none_or(Value::is_null) {
        key.insert("is_standalone".to_string(), json!(standalone));
    }
    for field in ["expires_at", "last_used_at", "created_at", "updated_at"] {
        normalize_backup_timestamp_alias(key, field)?;
    }
    Ok(())
}

fn normalize_backup_timestamp_alias(
    object: &mut Map<String, Value>,
    field: &str,
) -> Result<(), (StatusCode, Value)> {
    let Some(legacy) = object.remove(field) else {
        return Ok(());
    };
    let normalized = match legacy {
        Value::Null => Value::Null,
        Value::Number(value) if value.as_u64().is_some() => Value::Number(value),
        Value::String(value) => {
            let timestamp = chrono::DateTime::parse_from_rfc3339(&value)
                .ok()
                .and_then(|time| u64::try_from(time.timestamp()).ok())
                .ok_or_else(|| invalid_request(format!("{field} 必须是有效时间或 null")))?;
            json!(timestamp)
        }
        _ => return Err(invalid_request(format!("{field} 必须是有效时间或 null"))),
    };
    let canonical = format!("{field}_unix_secs");
    match object.get(&canonical) {
        Some(current) if *current != normalized => {
            Err(invalid_request(format!("{field} 时间字段互相冲突")))
        }
        Some(_) => Ok(()),
        None => {
            object.insert(canonical, normalized);
            Ok(())
        }
    }
}

fn validate_backup_usage(
    backup: &UsersBackup,
    key_ids: &BTreeSet<String>,
) -> Result<(), (StatusCode, Value)> {
    let mut daily_ids = BTreeSet::new();
    let mut user_daily_ids = BTreeSet::new();
    let mut key_daily_ids = BTreeSet::new();
    for row in &backup.usage_aggregates.stats_daily {
        if !daily_ids.insert(row.date_unix_secs) {
            return Err(invalid_request("用量统计包含重复的日期"));
        }
    }
    for row in &backup.usage_aggregates.stats_user_daily {
        if Some(row.user_id.as_str()) != backup.source_user_id()
            || !user_daily_ids.insert((&row.user_id, row.date_unix_secs))
        {
            return Err(invalid_request("用户用量统计的归属或日期无效"));
        }
    }
    for row in &backup.usage_aggregates.stats_daily_api_key {
        if !key_ids.contains(&row.api_key_id)
            || !key_daily_ids.insert((&row.api_key_id, row.date_unix_secs))
        {
            return Err(invalid_request(
                "API Key 用量统计引用的密钥不存在或日期重复",
            ));
        }
    }
    let snapshot = serde_json::to_value(&backup.usage_aggregates)
        .map_err(|_| invalid_request("用量统计无法完整解析"))?;
    for (section, rows) in snapshot
        .as_object()
        .expect("snapshot serializes to an object")
    {
        for row in rows
            .as_array()
            .expect("snapshot sections serialize to arrays")
        {
            for (field, value) in row
                .as_object()
                .expect("aggregate rows serialize to objects")
            {
                if value.as_u64().is_some_and(|value| value > i64::MAX as u64)
                    || (matches!(field.as_str(), "total_cost" | "actual_total_cost")
                        && value
                            .as_f64()
                            .is_none_or(|value| !value.is_finite() || value < 0.0))
                {
                    return Err(invalid_request(format!(
                        "usage_aggregates.{section}.{field} 无效"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn remap_provider_ids(
    ids: &mut Option<Vec<String>>,
    mapping: &BTreeMap<String, String>,
) -> Result<(), (StatusCode, Value)> {
    for id in ids.iter_mut().flatten() {
        *id = mapping.get(id).cloned().ok_or_else(|| {
            invalid_request(format!("用户数据引用的提供商 '{id}' 不存在，请先导入配置"))
        })?;
    }
    Ok(())
}

impl<'a> AdminBackupState<'a> {
    pub(crate) async fn build_admin_system_users_export_payload(
        &self,
    ) -> Result<Value, GatewayError> {
        let users = self.list_export_users().await?;
        let [profile] = users.as_slice() else {
            return Err(GatewayError::Internal(
                "个人版备份需要一个有效管理员".to_string(),
            ));
        };
        let totals = self
            .summarize_usage_totals_by_user_ids(std::slice::from_ref(&profile.id))
            .await?;
        let records = self
            .list_auth_api_key_export_records_by_user_ids(std::slice::from_ref(&profile.id))
            .await?;
        let mut api_keys = Vec::new();
        let mut standalone_keys = Vec::new();
        for mut record in records {
            let key = record
                .key_encrypted
                .take()
                .map(|ciphertext| decrypt_admin_system_export_secret(self.admin(), &ciphertext))
                .transpose()?;
            let standalone = record.is_standalone;
            let key = ApiKeyBackup { key, record };
            if standalone {
                standalone_keys.push(key);
            } else {
                api_keys.push(key);
            }
        }
        let preferences = self.read_user_preferences(&profile.id).await?;
        let provider_names = self
            .list_provider_catalog_providers(false)
            .await?
            .into_iter()
            .map(|provider| (provider.id, provider.name))
            .collect();
        let backup = UsersBackup {
            exported_at: chrono::Utc::now().to_rfc3339(),
            provider_names,
            users: vec![UserBackup {
                profile: profile.clone(),
                preferences,
                api_keys,
                request_count: totals
                    .first()
                    .map(|totals| totals.request_count)
                    .unwrap_or(0),
                total_tokens: totals
                    .first()
                    .map(|totals| totals.total_tokens)
                    .unwrap_or(0),
            }],
            standalone_keys,
            usage_aggregates: self.export_admin_system_usage_aggregates().await?,
        };
        let payload =
            serde_json::to_value(backup).map_err(|err| GatewayError::Internal(err.to_string()))?;
        parse_users_backup(payload.clone()).map_err(|(_, error)| {
            GatewayError::Internal(
                error["detail"]
                    .as_str()
                    .unwrap_or("用户资料无法导出")
                    .to_string(),
            )
        })?;
        Ok(payload)
    }

    pub(crate) async fn import_admin_system_users(
        &self,
        request_body: &Bytes,
        operator_id: Option<&str>,
    ) -> Result<Result<Value, (StatusCode, Value)>, GatewayError> {
        macro_rules! checked {
            ($value:expr) => {
                match $value {
                    Ok(value) => value,
                    Err(err) => return Ok(Err(err)),
                }
            };
        }
        let root: Value =
            checked!(serde_json::from_slice(request_body)
                .map_err(|_| invalid_request("请求数据验证失败")));
        let merge_mode = checked!(serde_json::from_value::<AdminImportMergeMode>(
            root.get("merge_mode").cloned().unwrap_or(Value::Null)
        )
        .map_err(|_| invalid_request("merge_mode 仅支持 skip / overwrite / error")));
        let mut backup = checked!(parse_users_backup(root));
        let Some(operator_id) = operator_id else {
            return Ok(Err((
                StatusCode::FORBIDDEN,
                json!({"detail": "需要管理员身份"}),
            )));
        };
        let Some(operator) = self
            .find_user_auth_by_id(operator_id)
            .await?
            .filter(|user| {
                user.role == "admin"
                    && user.auth_source == "local"
                    && user.is_active
                    && !user.is_deleted
            })
        else {
            return Ok(Err((
                StatusCode::FORBIDDEN,
                json!({"detail": "需要有效管理员身份"}),
            )));
        };
        if merge_mode == AdminImportMergeMode::Error && backup.has_admin_profile() {
            return Ok(Err(invalid_request(
                "当前管理员已存在，请选择跳过或覆盖模式",
            )));
        }
        let providers_by_name = self
            .list_provider_catalog_providers(false)
            .await?
            .into_iter()
            .map(|provider| (provider.name, provider.id))
            .collect::<BTreeMap<_, _>>();
        let provider_id_map = backup
            .provider_names
            .iter()
            .filter_map(|(id, name)| {
                providers_by_name
                    .get(name)
                    .map(|target| (id.clone(), target.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        let user_values = serde_json::to_value(&backup.users)
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        let supplemental = checked!(build_imported_user_usage_total_aggregates(
            user_values.as_array().expect("users serialize to an array"),
            Some(&json!(backup.exported_at)),
        )
        .map_err(invalid_request));
        let user_id_map = backup
            .source_user_id()
            .map(|source_id| (source_id.to_string(), operator.id.clone()))
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        if let Some(user) = backup.users.first_mut() {
            user.profile.id = operator.id.clone();
            checked!(remap_provider_ids(
                &mut user.profile.allowed_providers,
                &provider_id_map
            ));
            if let Some(preferences) = user.preferences.as_mut() {
                preferences.user_id = operator.id.clone();
                if let Some(id) = preferences.default_provider_id.as_mut() {
                    *id = checked!(provider_id_map
                        .get(id)
                        .cloned()
                        .ok_or_else(|| invalid_request("默认提供商不存在，请先导入配置")));
                }
            }
            if merge_mode == AdminImportMergeMode::Overwrite {
                if self
                    .is_other_user_auth_username_taken(&user.profile.username, &operator.id)
                    .await?
                    || match user.profile.email.as_deref() {
                        Some(email) => {
                            self.is_other_user_auth_email_taken(email, &operator.id)
                                .await?
                        }
                        None => false,
                    }
                {
                    return Ok(Err(invalid_request("备份中的用户名或邮箱已被占用")));
                }
            }
        }
        let existing_keys = self
            .list_auth_api_key_export_records_by_user_ids(std::slice::from_ref(&operator.id))
            .await?
            .into_iter()
            .map(|key| (key.key_hash.clone(), key))
            .collect::<BTreeMap<_, _>>();
        let mut key_id_map = BTreeMap::new();
        let mut planned_keys = Vec::new();
        let mut stats = UsersImportStats::default();
        for key in backup
            .users
            .iter_mut()
            .flat_map(|user| user.api_keys.iter_mut())
            .chain(backup.standalone_keys.iter_mut())
        {
            let record = &mut key.record;
            record.user_id = operator.id.clone();
            checked!(remap_provider_ids(
                &mut record.allowed_providers,
                &provider_id_map
            ));
            record.key_encrypted = match key.key.as_deref() {
                Some(plaintext) => Some(checked!(self
                    .encrypt_catalog_secret_with_fallbacks(plaintext)
                    .ok_or_else(|| invalid_request("API Key 加密失败，请检查目标系统加密配置")))),
                None => None,
            };
            let counter = if record.is_standalone {
                &mut stats.standalone_keys
            } else {
                &mut stats.api_keys
            };
            let source_id = record.api_key_id.clone();
            if let Some(existing) = existing_keys.get(&record.key_hash) {
                if merge_mode == AdminImportMergeMode::Error {
                    return Ok(Err(invalid_request("API Key 已存在，请选择跳过或覆盖模式")));
                }
                if existing.is_standalone != record.is_standalone {
                    return Ok(Err(invalid_request("已存在同一密钥的不同 API Key 类型")));
                }
                key_id_map.insert(source_id, existing.api_key_id.clone());
                if merge_mode == AdminImportMergeMode::Skip {
                    counter.skipped += 1;
                    continue;
                }
                record.api_key_id = existing.api_key_id.clone();
                record.created_at_unix_secs = existing.created_at_unix_secs;
                counter.updated += 1;
            } else {
                record.api_key_id = Uuid::new_v4().to_string();
                key_id_map.insert(source_id, record.api_key_id.clone());
                counter.created += 1;
            }
            planned_keys.push(record.clone());
        }
        for record in planned_keys {
            if !self.restore_exported_api_key(&record).await? {
                return Ok(Err(invalid_request("API Key 未能写入，导入已中止")));
            }
        }
        let aggregates = serde_json::to_value(&backup.usage_aggregates)
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        stats.usage_aggregates = self
            .import_admin_system_user_usage_aggregates(
                Some(&aggregates),
                &supplemental,
                &user_id_map,
                &key_id_map,
                merge_mode,
            )
            .await?;
        if stats.usage_aggregates.as_ref().is_some_and(|summary| {
            summary.skipped_unmapped_user_daily != 0 || summary.skipped_unmapped_api_key_daily != 0
        }) {
            return Ok(Err(invalid_request("存在无法关联的用量统计，导入已中止")));
        }
        let mut reauthentication_required = false;
        if let Some(user) = backup.users.first() {
            if merge_mode == AdminImportMergeMode::Overwrite {
                if operator.password_hash != user.profile.password_hash {
                    self.revoke_all_user_sessions(
                        &operator.id,
                        chrono::Utc::now(),
                        "backup_restored",
                    )
                    .await?;
                    reauthentication_required = true;
                }
                if !self.restore_admin_profile(&user.profile).await? {
                    return Ok(Err(invalid_request("管理员资料未能写入，导入已中止")));
                }
                let preferences = user.preferences.clone().unwrap_or_else(|| {
                    GatewayUserPreferenceView::default_for_user(&operator.id).into()
                });
                if self.write_user_preferences(preferences).await?.is_none() {
                    return Ok(Err(invalid_request("用户偏好未能写入，导入已中止")));
                }
                stats.users.updated = 1;
            } else {
                stats.users.skipped = 1;
            }
        }
        Ok(Ok(json!({
            "message": "用户数据导入成功", "stats": stats,
            "reauthentication_required": reauthentication_required,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::{hash_admin_user_api_key, parse_users_backup};
    use serde_json::{json, Value};

    fn legacy_backup() -> Value {
        let mut user_daily = json!({
            "date_unix_secs": 1_788_912_000_u64,
            "total_requests": 1, "success_requests": 1, "error_requests": 0,
            "input_tokens": 2, "output_tokens": 1,
            "cache_creation_tokens": 0, "cache_read_tokens": 0, "total_cost": 0.01,
        });
        let mut key_daily = user_daily.clone();
        user_daily["user_id"] = json!("source-user");
        user_daily["username"] = json!("source-admin");
        key_daily["api_key_id"] = json!("source-key");
        key_daily["api_key_name"] = json!("fixture-key");
        json!({
            "exported_at": "2026-09-08T00:00:00Z",
            "user_groups": [], "users": [],
            "standalone_keys": [{
                "api_key_id": "source-key",
                "key_hash": hash_admin_user_api_key("sk-backup-fixture"),
                "key": "sk-backup-fixture", "name": "fixture-key",
                "allowed_providers": null, "allowed_api_formats": null,
                "allowed_models": null, "ip_rules": null,
                "rate_limit": 100, "concurrent_limit": 5,
                "force_capabilities": null, "feature_settings": {"custom_feature": true},
                "is_active": true, "expires_at": null, "auto_delete_on_expiry": false,
                "total_requests": 1, "total_tokens": 3, "total_cost_usd": 0.01,
                "wallet": null,
            }],
            "usage_aggregates": {
                "stats_daily": [],
                "stats_user_daily": [user_daily],
                "stats_daily_api_key": [key_daily],
            },
        })
    }

    fn backup_with_admin() -> Value {
        let mut backup = legacy_backup();
        backup["users"] = json!([{
            "id": "source-user", "username": "source-admin", "email_verified": true,
            "password_hash": bcrypt::hash("fixture-admin-password", 4).unwrap(),
            "role": "admin", "auth_source": "local", "is_active": true,
            "allowed_providers": null, "allowed_api_formats": ["openai:chat"],
            "allowed_models": [], "rate_limit": 7,
            "api_keys": [], "request_count": 1, "total_tokens": 3,
        }]);
        backup
    }

    #[test]
    fn legacy_and_unversioned_backups_keep_an_absent_admin_absent() {
        for version in [
            None,
            Some(json!("1.5")),
            Some(json!("future")),
            Some(json!(19)),
        ] {
            let mut value = legacy_backup();
            if let Some(version) = version {
                value["version"] = version;
            }
            let backup = parse_users_backup(value).expect("content should determine compatibility");
            assert!(!backup.has_admin_profile());
            assert_eq!(backup.source_user_id(), Some("source-user"));
            assert!(backup.provider_names.is_empty());
            let key = &backup.standalone_keys[0].record;
            assert_eq!(key.user_id, "source-user");
            assert!(key.is_standalone);
            assert_eq!(key.total_requests, 1);
            assert_eq!(key.total_tokens, 3);
            assert_eq!(key.feature_settings, Some(json!({"custom_feature": true})));
            let restored = serde_json::to_value(&backup).unwrap();
            assert!(restored.get("version").is_none());
            assert!(restored["users"].as_array().unwrap().is_empty());
            assert!(restored["standalone_keys"][0].get("expires_at").is_none());
        }
    }

    #[test]
    fn standalone_keys_without_user_statistics_receive_only_an_internal_owner_mapping() {
        let mut value = legacy_backup();
        value["usage_aggregates"]["stats_user_daily"] = json!([]);
        let backup = parse_users_backup(value).unwrap();
        assert!(!backup.has_admin_profile());
        assert_eq!(
            backup.source_user_id(),
            Some(backup.standalone_keys[0].record.user_id.as_str())
        );
    }

    #[test]
    fn user_backup_normalizes_known_legacy_permissions_and_timestamps() {
        let mut value = backup_with_admin();
        value["standalone_keys"][0]["expires_at"] = json!("2026-09-08T00:00:00Z");
        value["standalone_keys"][0]["auto_delete_on_expiry"] = json!(true);
        let backup = parse_users_backup(value).unwrap();
        let user = &backup.users[0];
        assert_eq!(user.profile.allowed_providers_mode, "unrestricted");
        assert_eq!(user.profile.allowed_api_formats_mode, "specific");
        assert_eq!(user.profile.allowed_models_mode, "unrestricted");
        assert_eq!(user.profile.rate_limit_mode, "custom");
        assert_eq!(
            backup.standalone_keys[0].record.expires_at_unix_secs,
            Some(
                chrono::DateTime::parse_from_rfc3339("2026-09-08T00:00:00Z")
                    .unwrap()
                    .timestamp() as u64
            )
        );
    }

    #[test]
    fn user_backup_rejects_ambiguous_users_and_missing_key_references() {
        let mut value = legacy_backup();
        let mut other = value["usage_aggregates"]["stats_user_daily"][0].clone();
        other["user_id"] = json!("another-user");
        value["usage_aggregates"]["stats_user_daily"]
            .as_array_mut()
            .unwrap()
            .push(other);
        let error = parse_users_backup(value).expect_err("multiple users cannot collapse silently");
        assert!(error.1["detail"].as_str().unwrap().contains("多位用户"));

        let mut value = legacy_backup();
        value["usage_aggregates"]["stats_daily_api_key"][0]["api_key_id"] = json!("missing-key");
        let error = parse_users_backup(value).expect_err("unmapped statistics cannot be dropped");
        assert!(error.1["detail"]
            .as_str()
            .unwrap()
            .contains("引用的密钥不存在"));
    }

    #[test]
    fn user_backup_rejects_nonempty_retired_and_unknown_data() {
        let mut retired_groups = legacy_backup();
        retired_groups["user_groups"] = json!([{"name": "legacy-group"}]);
        let mut wallet = legacy_backup();
        wallet["standalone_keys"][0]["wallet"] = json!({"balance": 10});
        let mut unknown = legacy_backup();
        unknown["standalone_keys"][0]["future_credential"] = json!("synthetic-secret");
        for value in [retired_groups, wallet, unknown] {
            let error = parse_users_backup(value).expect_err("unsupported data cannot disappear");
            let detail = error.1["detail"].as_str().unwrap();
            assert!(detail.contains("包含无法恢复的数据"));
            assert!(!detail.contains("synthetic-secret"));
        }
    }

    #[test]
    fn user_backup_requires_resolvable_provider_restrictions() {
        let mut value = legacy_backup();
        value["standalone_keys"][0]["allowed_providers"] = json!(["source-provider"]);
        let error = parse_users_backup(value.clone()).expect_err("restrictions require a mapping");
        assert!(error.1["detail"].as_str().unwrap().contains("提供商引用"));
        value["provider_names"] = json!({"source-provider": "fixture-provider"});
        assert!(parse_users_backup(value).is_ok());
    }

    #[test]
    fn user_backup_requires_valid_admin_passwords_and_key_credentials() {
        let mut admin = backup_with_admin();
        admin["users"][0]["password_hash"] = json!("invalid-hash");
        let mut wrong_plaintext = legacy_backup();
        wrong_plaintext["standalone_keys"][0]["key"] = json!("wrong-plaintext");
        let mut ciphertext_only = legacy_backup();
        ciphertext_only["standalone_keys"][0]["key"] = Value::Null;
        ciphertext_only["standalone_keys"][0]["key_encrypted"] = json!("source-ciphertext");
        for value in [admin, wrong_plaintext, ciphertext_only] {
            assert!(parse_users_backup(value).is_err());
        }
        let mut redundant_ciphertext = legacy_backup();
        redundant_ciphertext["standalone_keys"][0]["key_encrypted"] = json!("source-ciphertext");
        let backup = parse_users_backup(redundant_ciphertext).unwrap();
        assert!(backup.standalone_keys[0].record.key_encrypted.is_none());
    }

    #[test]
    fn user_backup_rejects_duplicate_or_out_of_range_statistics() {
        let mut duplicate = legacy_backup();
        let row = duplicate["usage_aggregates"]["stats_user_daily"][0].clone();
        duplicate["usage_aggregates"]["stats_user_daily"]
            .as_array_mut()
            .unwrap()
            .push(row);
        let mut overflow = legacy_backup();
        overflow["usage_aggregates"]["stats_user_daily"][0]["total_requests"] = json!(u64::MAX);
        let mut negative_cost = legacy_backup();
        negative_cost["usage_aggregates"]["stats_user_daily"][0]["total_cost"] = json!(-1.0);
        for value in [duplicate, overflow, negative_cost] {
            assert!(parse_users_backup(value).is_err());
        }
    }
}
