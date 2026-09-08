use std::collections::{BTreeMap, BTreeSet};

use aether_admin::system::{
    AdminImportMergeMode, AdminSystemConfigImportCounter, ADMIN_SYSTEM_USERS_EXPORT_VERSION,
};
use aether_data::repository::auth::StoredAuthApiKeyExportRecord;
use aether_data::repository::system::{
    AdminSystemUsageAggregateImportSummary, AdminSystemUsageAggregateSnapshot,
};
use aether_data::repository::users::{StoredUserExportRow, StoredUserPreferenceRecord};
use axum::{body::Bytes, http::StatusCode};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use super::import::{build_imported_user_usage_total_aggregates, invalid_request};
use super::AdminAppState;
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
    version: String,
    exported_at: String,
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
    request_count: u64,
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

pub(super) fn parse_users_backup(value: Value) -> Result<UsersBackup, (StatusCode, Value)> {
    if value.get("version").and_then(Value::as_str) != Some(ADMIN_SYSTEM_USERS_EXPORT_VERSION) {
        return Err(invalid_request(format!(
            "仅支持当前版本导出的用户数据（版本 {ADMIN_SYSTEM_USERS_EXPORT_VERSION}）"
        )));
    }
    let backup: UsersBackup = serde_json::from_value(value)
        .map_err(|err| invalid_request(format!("用户备份格式无效: {err}")))?;
    chrono::DateTime::parse_from_rfc3339(&backup.exported_at)
        .map_err(|_| invalid_request("exported_at 必须是 RFC3339 时间"))?;
    let [user] = backup.users.as_slice() else {
        return Err(invalid_request("个人版备份必须包含一个管理员"));
    };
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
    normalize_admin_user_api_formats(profile.allowed_api_formats.clone())
        .map_err(invalid_request)?;
    normalize_feature_settings(profile.feature_settings.clone()).map_err(invalid_request)?;
    let check_provider = |id: &String| {
        if backup.provider_names.contains_key(id) {
            Ok(())
        } else {
            Err(invalid_request(format!("提供商引用 '{id}' 不在用户备份中")))
        }
    };
    for id in profile.allowed_providers.iter().flatten() {
        check_provider(id)?;
    }
    if let Some(id) = user
        .preferences
        .as_ref()
        .and_then(|preferences| preferences.default_provider_id.as_ref())
    {
        check_provider(id)?;
    }
    if profile.rate_limit.is_some_and(|value| value < 0) {
        return Err(invalid_request("rate_limit 必须是非负整数或 null"));
    }
    let mut ids = BTreeSet::new();
    let mut hashes = BTreeSet::new();
    for (key, standalone) in user
        .api_keys
        .iter()
        .map(|key| (key, false))
        .chain(backup.standalone_keys.iter().map(|key| (key, true)))
    {
        let record = &key.record;
        if record.api_key_id.is_empty()
            || record.key_hash.len() != 64
            || !record
                .key_hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || record.user_id != profile.id
            || record.is_standalone != standalone
            || !ids.insert(&record.api_key_id)
            || !hashes.insert(&record.key_hash)
        {
            return Err(invalid_request("API Key 的标识、归属或类型无效"));
        }
        for id in record.allowed_providers.iter().flatten() {
            check_provider(id)?;
        }
        normalize_admin_user_api_formats(record.allowed_api_formats.clone())
            .map_err(invalid_request)?;
        normalize_admin_user_ip_rules(record.ip_rules.clone()).map_err(invalid_request)?;
        normalize_feature_settings(record.feature_settings.clone()).map_err(invalid_request)?;
        if record.force_capabilities.as_ref().is_some_and(|value| {
            value
                .as_object()
                .is_none_or(|fields| fields.values().any(|value| !value.is_boolean()))
        }) || (record.auto_delete_on_expiry && record.expires_at_unix_secs.is_none())
        {
            return Err(invalid_request("API Key 能力配置或过期策略无效"));
        }
        if record.key_encrypted.is_some() {
            return Err(invalid_request("新版备份不接受源系统密文，请重新导出"));
        }
        if let Some(plaintext) = key.key.as_deref() {
            if plaintext.is_empty() || hash_admin_user_api_key(plaintext) != record.key_hash {
                return Err(invalid_request("API Key 内容与校验值不一致"));
            }
        }
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
    Ok(backup)
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

impl<'a> AdminAppState<'a> {
    pub(crate) async fn build_admin_system_users_export_payload(
        &self,
    ) -> Result<Value, GatewayError> {
        let users = self
            .list_export_users()
            .await?
            .into_iter()
            .filter(|user| user.role == "admin" && user.is_active)
            .collect::<Vec<_>>();
        let [profile] = users.as_slice() else {
            return Err(GatewayError::Internal(
                "个人版备份需要一个有效管理员".to_string(),
            ));
        };
        let totals = self
            .app()
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
                .map(|ciphertext| decrypt_admin_system_export_secret(self, &ciphertext))
                .transpose()?;
            let standalone = record.is_standalone;
            let key = ApiKeyBackup { key, record };
            if standalone {
                standalone_keys.push(key);
            } else {
                api_keys.push(key);
            }
        }
        let preferences = self
            .app()
            .read_user_preferences(&profile.id)
            .await?
            .map(Into::into);
        let provider_names = self
            .list_provider_catalog_providers(false)
            .await?
            .into_iter()
            .map(|provider| (provider.id, provider.name))
            .collect();
        let backup = UsersBackup {
            version: ADMIN_SYSTEM_USERS_EXPORT_VERSION.to_string(),
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
        if !self.has_auth_user_write_capability() || !self.has_auth_api_key_writer() {
            return Ok(Err((
                StatusCode::SERVICE_UNAVAILABLE,
                json!({"detail": "Admin system data unavailable"}),
            )));
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
        if merge_mode == AdminImportMergeMode::Error {
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
        let user = &mut backup.users[0];
        let user_id_map = BTreeMap::from([(user.profile.id.clone(), operator.id.clone())]);
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
        let existing_keys = self
            .list_auth_api_key_export_records_by_user_ids(std::slice::from_ref(&operator.id))
            .await?
            .into_iter()
            .map(|key| (key.key_hash.clone(), key))
            .collect::<BTreeMap<_, _>>();
        let mut key_id_map = BTreeMap::new();
        let mut planned_keys = Vec::new();
        let mut stats = UsersImportStats::default();
        for key in user
            .api_keys
            .iter_mut()
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
            if !self.app().restore_exported_api_key(&record).await? {
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
        let mut reauthentication_required = false;
        if merge_mode == AdminImportMergeMode::Overwrite {
            // Revoke before replacing the password so a failed revocation can be retried safely.
            if operator.password_hash != user.profile.password_hash {
                self.app()
                    .revoke_all_user_sessions(&operator.id, chrono::Utc::now(), "backup_restored")
                    .await?;
                reauthentication_required = true;
            }
            if !self.app().restore_admin_profile(&user.profile).await? {
                return Ok(Err(invalid_request("管理员资料未能写入，导入已中止")));
            }
            let preferences = user.preferences.clone().unwrap_or_else(|| {
                GatewayUserPreferenceView::default_for_user(&operator.id).into()
            });
            if self
                .app()
                .write_user_preferences(preferences)
                .await?
                .is_none()
            {
                return Ok(Err(invalid_request("用户偏好未能写入，导入已中止")));
            }
            stats.users.updated = 1;
        } else {
            stats.users.skipped = 1;
        }
        Ok(Ok(json!({
            "message": "用户数据导入成功", "stats": stats,
            "reauthentication_required": reauthentication_required,
        })))
    }
}
