use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use sqlx::{sqlite::SqliteRow, QueryBuilder, Row, Sqlite};

use aether_data_contracts::repository::users::{
    LdapAuthUserProvisioningOutcome, StoredUserAuthRecord,
    StoredUserExportRow, StoredUserGroup, StoredUserGroupMember, StoredUserGroupMembership,
    StoredUserOAuthLinkSummary, StoredUserPreferenceRecord, StoredUserSessionRecord,
    StoredUserSummary, UpsertUserGroupRecord, UserExportListQuery, UserExportSortBy,
    UserExportSummary, UserReadRepository,
};
use aether_data_contracts::DataLayerError;

use crate::error::SqlResultExt;
use crate::SqlitePool;

const USER_SUMMARY_COLUMNS: &str = r#"
SELECT
  id,
  username,
  email,
  role,
  is_active,
  is_deleted
FROM users
"#;

const USER_EXPORT_COLUMNS: &str = r#"
SELECT
  id,
  email,
  email_verified,
  username,
  password_hash,
  role,
  auth_source,
  allowed_providers,
  allowed_providers_mode,
  allowed_api_formats,
  allowed_api_formats_mode,
  allowed_models,
  allowed_models_mode,
  rate_limit,
  rate_limit_mode,
  model_capability_settings,
  feature_settings,
  is_active
FROM users
"#;

const USER_AUTH_COLUMNS: &str = r#"
SELECT
  id,
  email,
  email_verified,
  username,
  password_hash,
  role,
  auth_source,
  allowed_providers,
  allowed_providers_mode,
  allowed_api_formats,
  allowed_api_formats_mode,
  allowed_models,
  allowed_models_mode,
  is_active,
  is_deleted,
  created_at,
  last_login_at
FROM users
"#;

const USER_PREFERENCES_COLUMNS: &str = r#"
SELECT
  up.user_id,
  up.avatar_url,
  up.bio,
  up.default_provider_id,
  p.name AS default_provider_name,
  up.theme,
  up.language,
  up.timezone,
  up.email_notifications,
  up.usage_alerts,
  up.announcement_notifications
FROM user_preferences up
LEFT JOIN providers p
  ON p.id = up.default_provider_id
"#;

const USER_SESSION_COLUMNS: &str = r#"
SELECT
  id,
  user_id,
  client_device_id,
  device_label,
  refresh_token_hash,
  prev_refresh_token_hash,
  rotated_at,
  last_seen_at,
  expires_at,
  revoked_at,
  revoke_reason,
  ip_address,
  user_agent,
  created_at,
  updated_at
FROM user_sessions
"#;

#[derive(Debug, Clone)]
pub struct SqliteUserReadRepository {
    pool: SqlitePool,
}

impl SqliteUserReadRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    async fn fetch_summary_rows(
        &self,
        mut builder: QueryBuilder<'_, Sqlite>,
    ) -> Result<Vec<StoredUserSummary>, DataLayerError> {
        let rows = builder.build().fetch_all(&self.pool).await.map_sql_err()?;
        rows.iter().map(map_user_row).collect()
    }

    async fn fetch_export_rows(
        &self,
        mut builder: QueryBuilder<'_, Sqlite>,
    ) -> Result<Vec<StoredUserExportRow>, DataLayerError> {
        let rows = builder.build().fetch_all(&self.pool).await.map_sql_err()?;
        rows.iter().map(map_user_export_row).collect()
    }

    async fn fetch_auth_rows(
        &self,
        mut builder: QueryBuilder<'_, Sqlite>,
    ) -> Result<Vec<StoredUserAuthRecord>, DataLayerError> {
        let rows = builder.build().fetch_all(&self.pool).await.map_sql_err()?;
        rows.iter().map(map_user_auth_row).collect()
    }

}

#[async_trait]
impl UserReadRepository for SqliteUserReadRepository {
    async fn list_users_by_ids(
        &self,
        user_ids: &[String],
    ) -> Result<Vec<StoredUserSummary>, DataLayerError> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(USER_SUMMARY_COLUMNS);
        builder.push(" WHERE id IN (");
        {
            let mut separated = builder.separated(", ");
            for user_id in user_ids {
                separated.push_bind(user_id);
            }
        }
        builder.push(") ORDER BY id ASC");
        self.fetch_summary_rows(builder).await
    }

    async fn list_users_by_username_search(
        &self,
        username_search: &str,
    ) -> Result<Vec<StoredUserSummary>, DataLayerError> {
        let username_search = username_search.trim();
        if username_search.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(USER_SUMMARY_COLUMNS);
        builder
            .push(" WHERE is_deleted = 0 AND LOWER(username) LIKE ")
            .push_bind(format!("%{}%", username_search.to_ascii_lowercase()))
            .push(" ORDER BY id ASC");
        self.fetch_summary_rows(builder).await
    }

    async fn list_export_users(&self) -> Result<Vec<StoredUserExportRow>, DataLayerError> {
        let mut builder = QueryBuilder::<Sqlite>::new(USER_EXPORT_COLUMNS);
        builder.push(" WHERE is_deleted = 0 ORDER BY id ASC");
        self.fetch_export_rows(builder).await
    }

    async fn list_export_users_page(
        &self,
        query: &UserExportListQuery,
    ) -> Result<Vec<StoredUserExportRow>, DataLayerError> {
        let mut builder = QueryBuilder::<Sqlite>::new(USER_EXPORT_COLUMNS);
        builder.push(" WHERE is_deleted = 0");
        if let Some(role) = query.role.as_deref() {
            builder
                .push(" AND LOWER(role) = ")
                .push_bind(role.trim().to_ascii_lowercase());
        }
        if let Some(is_active) = query.is_active {
            builder.push(" AND is_active = ").push_bind(is_active);
        }
        // 20260902 drop 迁移已删除 user_group_members，按群组过滤用户不再可用，
        // 此处收敛为忽略 group_id 条件。
        if query
            .group_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
        {
            builder.push(" AND 0 = 1");
        }
        if let Some(search) = query
            .search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let pattern = format!("%{}%", search.to_ascii_lowercase());
            builder
                .push(" AND (LOWER(id) LIKE ")
                .push_bind(pattern.clone())
                .push(" OR LOWER(username) LIKE ")
                .push_bind(pattern.clone())
                .push(" OR LOWER(COALESCE(email, '')) LIKE ")
                .push_bind(pattern)
                .push(")");
        }
        match query.sort_by {
            UserExportSortBy::CreatedAt => {
                builder
                    .push(" ORDER BY created_at ")
                    .push(if query.sort_order.is_desc() {
                        "DESC"
                    } else {
                        "ASC"
                    })
                    .push(", id ASC");
            }
            UserExportSortBy::Id => {
                builder.push(" ORDER BY id ASC");
            }
        }

        builder
            .push(" LIMIT ")
            .push_bind(i64::try_from(query.limit).map_err(|_| {
                DataLayerError::InvalidInput(format!("invalid user export limit: {}", query.limit))
            })?)
            .push(" OFFSET ")
            .push_bind(i64::try_from(query.skip).map_err(|_| {
                DataLayerError::InvalidInput(format!("invalid user export skip: {}", query.skip))
            })?);
        self.fetch_export_rows(builder).await
    }

    async fn count_export_users(&self, query: &UserExportListQuery) -> Result<u64, DataLayerError> {
        let mut builder = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) AS total FROM users");
        builder.push(" WHERE is_deleted = 0");
        if let Some(role) = query.role.as_deref() {
            builder
                .push(" AND LOWER(role) = ")
                .push_bind(role.trim().to_ascii_lowercase());
        }
        if let Some(is_active) = query.is_active {
            builder.push(" AND is_active = ").push_bind(is_active);
        }
        // 20260902 drop 迁移已删除 user_group_members，按群组过滤用户不再可用，
        // 此处收敛为忽略 group_id 条件。
        if query
            .group_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
        {
            builder.push(" AND 0 = 1");
        }
        if let Some(search) = query
            .search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let pattern = format!("%{}%", search.to_ascii_lowercase());
            builder
                .push(" AND (LOWER(id) LIKE ")
                .push_bind(pattern.clone())
                .push(" OR LOWER(username) LIKE ")
                .push_bind(pattern.clone())
                .push(" OR LOWER(COALESCE(email, '')) LIKE ")
                .push_bind(pattern)
                .push(")");
        }

        let row = builder.build().fetch_one(&self.pool).await.map_sql_err()?;
        Ok(row.try_get::<i64, _>("total").map_sql_err()?.max(0) as u64)
    }

    async fn summarize_export_users(&self) -> Result<UserExportSummary, DataLayerError> {
        let row = sqlx::query(
            r#"
SELECT
  COUNT(*) AS total,
  SUM(CASE WHEN is_active = 1 THEN 1 ELSE 0 END) AS active
FROM users
WHERE is_deleted = 0
"#,
        )
        .fetch_one(&self.pool)
        .await
        .map_sql_err()?;

        Ok(UserExportSummary {
            total: row.try_get::<i64, _>("total").map_sql_err()?.max(0) as u64,
            active: row
                .try_get::<Option<i64>, _>("active")
                .map_sql_err()?
                .unwrap_or(0)
                .max(0) as u64,
        })
    }

    async fn find_export_user_by_id(
        &self,
        user_id: &str,
    ) -> Result<Option<StoredUserExportRow>, DataLayerError> {
        let mut builder = QueryBuilder::<Sqlite>::new(USER_EXPORT_COLUMNS);
        builder
            .push(" WHERE is_deleted = 0 AND id = ")
            .push_bind(user_id)
            .push(" LIMIT 1");
        Ok(self.fetch_export_rows(builder).await?.into_iter().next())
    }

    async fn list_non_admin_export_users(
        &self,
    ) -> Result<Vec<StoredUserExportRow>, DataLayerError> {
        let mut builder = QueryBuilder::<Sqlite>::new(USER_EXPORT_COLUMNS);
        builder.push(" WHERE is_deleted = 0 AND LOWER(role) != 'admin' ORDER BY id ASC");
        self.fetch_export_rows(builder).await
    }

    // 20260902 drop 迁移已删除 user_groups/user_group_members（单用户化阶段4），
    // 以下群组方法收敛为中性结果，仅保留 trait 兼容，不再触碰数据库。

    async fn list_user_groups(&self) -> Result<Vec<StoredUserGroup>, DataLayerError> {
        Ok(Vec::new())
    }

    async fn find_user_group_by_id(
        &self,
        _group_id: &str,
    ) -> Result<Option<StoredUserGroup>, DataLayerError> {
        Ok(None)
    }

    async fn list_user_groups_by_ids(
        &self,
        _group_ids: &[String],
    ) -> Result<Vec<StoredUserGroup>, DataLayerError> {
        Ok(Vec::new())
    }

    async fn create_user_group(
        &self,
        _record: UpsertUserGroupRecord,
    ) -> Result<Option<StoredUserGroup>, DataLayerError> {
        Ok(None)
    }

    async fn update_user_group(
        &self,
        _group_id: &str,
        _record: UpsertUserGroupRecord,
    ) -> Result<Option<StoredUserGroup>, DataLayerError> {
        Ok(None)
    }

    async fn delete_user_group(&self, _group_id: &str) -> Result<bool, DataLayerError> {
        Ok(false)
    }

    async fn list_user_group_members(
        &self,
        _group_id: &str,
    ) -> Result<Vec<StoredUserGroupMember>, DataLayerError> {
        Ok(Vec::new())
    }

    async fn replace_user_group_members(
        &self,
        _group_id: &str,
        _user_ids: &[String],
    ) -> Result<Vec<StoredUserGroupMember>, DataLayerError> {
        Ok(Vec::new())
    }

    async fn list_user_groups_for_user(
        &self,
        _user_id: &str,
    ) -> Result<Vec<StoredUserGroup>, DataLayerError> {
        Ok(Vec::new())
    }

    async fn list_user_group_memberships_by_user_ids(
        &self,
        _user_ids: &[String],
    ) -> Result<Vec<StoredUserGroupMembership>, DataLayerError> {
        Ok(Vec::new())
    }

    async fn replace_user_groups_for_user(
        &self,
        _user_id: &str,
        _group_ids: &[String],
    ) -> Result<Vec<StoredUserGroup>, DataLayerError> {
        Ok(Vec::new())
    }

    async fn add_user_to_group(
        &self,
        _group_id: &str,
        _user_id: &str,
    ) -> Result<bool, DataLayerError> {
        Ok(false)
    }

    async fn find_user_auth_by_id(
        &self,
        user_id: &str,
    ) -> Result<Option<StoredUserAuthRecord>, DataLayerError> {
        let mut builder = QueryBuilder::<Sqlite>::new(USER_AUTH_COLUMNS);
        builder
            .push(" WHERE id = ")
            .push_bind(user_id)
            .push(" LIMIT 1");
        Ok(self.fetch_auth_rows(builder).await?.into_iter().next())
    }

    async fn list_user_auth_by_ids(
        &self,
        user_ids: &[String],
    ) -> Result<Vec<StoredUserAuthRecord>, DataLayerError> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(USER_AUTH_COLUMNS);
        builder.push(" WHERE id IN (");
        {
            let mut separated = builder.separated(", ");
            for user_id in user_ids {
                separated.push_bind(user_id);
            }
        }
        builder.push(") ORDER BY id ASC");
        self.fetch_auth_rows(builder).await
    }

    async fn find_user_auth_by_identifier(
        &self,
        identifier: &str,
    ) -> Result<Option<StoredUserAuthRecord>, DataLayerError> {
        let mut builder = QueryBuilder::<Sqlite>::new(USER_AUTH_COLUMNS);
        builder
            .push(" WHERE email = ")
            .push_bind(identifier)
            .push(" OR username = ")
            .push_bind(identifier)
            .push(" LIMIT 1");
        Ok(self.fetch_auth_rows(builder).await?.into_iter().next())
    }

    async fn find_user_auth_by_email(
        &self,
        email: &str,
    ) -> Result<Option<StoredUserAuthRecord>, DataLayerError> {
        let mut builder = QueryBuilder::<Sqlite>::new(USER_AUTH_COLUMNS);
        builder
            .push(" WHERE email = ")
            .push_bind(email)
            .push(" LIMIT 1");
        Ok(self.fetch_auth_rows(builder).await?.into_iter().next())
    }

    async fn find_active_user_auth_by_email_ci(
        &self,
        email: &str,
    ) -> Result<Option<StoredUserAuthRecord>, DataLayerError> {
        let mut builder = QueryBuilder::<Sqlite>::new(USER_AUTH_COLUMNS);
        builder
            .push(" WHERE LOWER(email) = LOWER(")
            .push_bind(email)
            .push(") AND is_deleted = 0 LIMIT 1");
        Ok(self.fetch_auth_rows(builder).await?.into_iter().next())
    }

    async fn find_user_auth_by_username(
        &self,
        username: &str,
    ) -> Result<Option<StoredUserAuthRecord>, DataLayerError> {
        let mut builder = QueryBuilder::<Sqlite>::new(USER_AUTH_COLUMNS);
        builder
            .push(" WHERE username = ")
            .push_bind(username)
            .push(" LIMIT 1");
        Ok(self.fetch_auth_rows(builder).await?.into_iter().next())
    }

    // 20260902 drop 迁移已删除 user_oauth_links/oauth_providers（单用户化阶段4），
    // 以下 OAuth 关联方法收敛为中性结果，仅保留 trait 兼容，不再触碰数据库。

    async fn list_user_oauth_links(
        &self,
        _user_id: &str,
    ) -> Result<Vec<StoredUserOAuthLinkSummary>, DataLayerError> {
        Ok(Vec::new())
    }

    async fn find_oauth_linked_user(
        &self,
        _provider_type: &str,
        _provider_user_id: &str,
    ) -> Result<Option<StoredUserAuthRecord>, DataLayerError> {
        Ok(None)
    }

    async fn touch_oauth_link(
        &self,
        _provider_type: &str,
        _provider_user_id: &str,
        _provider_username: Option<&str>,
        _provider_email: Option<&str>,
        _extra_data: Option<serde_json::Value>,
        _touched_at: DateTime<Utc>,
    ) -> Result<bool, DataLayerError> {
        Ok(false)
    }

    async fn create_oauth_auth_user(
        &self,
        _email: Option<String>,
        _username: String,
        _created_at: DateTime<Utc>,
    ) -> Result<Option<StoredUserAuthRecord>, DataLayerError> {
        Ok(None)
    }

    async fn find_oauth_link_owner(
        &self,
        _provider_type: &str,
        _provider_user_id: &str,
    ) -> Result<Option<String>, DataLayerError> {
        Ok(None)
    }

    async fn has_user_oauth_provider_link(
        &self,
        _user_id: &str,
        _provider_type: &str,
    ) -> Result<bool, DataLayerError> {
        Ok(false)
    }

    async fn count_user_oauth_links(&self, _user_id: &str) -> Result<u64, DataLayerError> {
        Ok(0)
    }

    async fn upsert_user_oauth_link(
        &self,
        _user_id: &str,
        _provider_type: &str,
        _provider_user_id: &str,
        _provider_username: Option<&str>,
        _provider_email: Option<&str>,
        _extra_data: Option<serde_json::Value>,
        _linked_at: DateTime<Utc>,
    ) -> Result<(), DataLayerError> {
        Ok(())
    }

    async fn delete_user_oauth_link(
        &self,
        _user_id: &str,
        _provider_type: &str,
    ) -> Result<bool, DataLayerError> {
        Ok(false)
    }

    async fn get_or_create_ldap_auth_user(
        &self,
        _email: String,
        _username: String,
        _ldap_dn: Option<String>,
        _ldap_username: Option<String>,
        _logged_in_at: DateTime<Utc>,
    ) -> Result<Option<LdapAuthUserProvisioningOutcome>, DataLayerError> {
        // 20260902 drop 迁移已删除 ldap_configs/oauth_providers（单用户化阶段4），
        // LDAP 登录供应收敛为不可用，仅保留 trait 兼容。
        Ok(None)
    }

    async fn touch_auth_user_last_login(
        &self,
        user_id: &str,
        logged_in_at: DateTime<Utc>,
    ) -> Result<bool, DataLayerError> {
        let result = sqlx::query("UPDATE users SET last_login_at = ?, updated_at = ? WHERE id = ?")
            .bind(logged_in_at.timestamp())
            .bind(logged_in_at.timestamp())
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_sql_err()?;
        Ok(result.rows_affected() > 0)
    }

    async fn update_local_auth_user_profile(
        &self,
        user_id: &str,
        email: Option<String>,
        username: Option<String>,
    ) -> Result<Option<StoredUserAuthRecord>, DataLayerError> {
        let now = chrono::Utc::now().timestamp();
        let result = sqlx::query(
            "UPDATE users SET email = COALESCE(?, email), username = COALESCE(?, username), updated_at = ? WHERE id = ?",
        )
        .bind(email)
        .bind(username)
        .bind(now)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_sql_err()?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.find_user_auth_by_id(user_id).await
    }

    async fn update_local_auth_user_password_hash(
        &self,
        user_id: &str,
        password_hash: String,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<StoredUserAuthRecord>, DataLayerError> {
        let result = sqlx::query("UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?")
            .bind(password_hash)
            .bind(updated_at.timestamp())
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_sql_err()?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.find_user_auth_by_id(user_id).await
    }

    async fn update_local_auth_user_admin_fields(
        &self,
        user_id: &str,
        role: Option<String>,
        allowed_providers_present: bool,
        allowed_providers: Option<Vec<String>>,
        allowed_api_formats_present: bool,
        allowed_api_formats: Option<Vec<String>>,
        allowed_models_present: bool,
        allowed_models: Option<Vec<String>>,
        rate_limit_present: bool,
        rate_limit: Option<i32>,
        is_active: Option<bool>,
    ) -> Result<Option<StoredUserAuthRecord>, DataLayerError> {
        let allowed_providers_mode = if allowed_providers
            .as_ref()
            .is_some_and(|values| !values.is_empty())
        {
            "specific"
        } else {
            "unrestricted"
        };
        let allowed_api_formats_mode = if allowed_api_formats
            .as_ref()
            .is_some_and(|values| !values.is_empty())
        {
            "specific"
        } else {
            "unrestricted"
        };
        let allowed_models_mode = if allowed_models
            .as_ref()
            .is_some_and(|values| !values.is_empty())
        {
            "specific"
        } else {
            "unrestricted"
        };
        let rate_limit_mode = if rate_limit.is_some() {
            "custom"
        } else {
            "system"
        };
        let result = sqlx::query(
            r#"
UPDATE users
SET role = CASE WHEN ? THEN COALESCE(?, role) ELSE role END,
    allowed_providers = CASE WHEN ? THEN ? ELSE allowed_providers END,
    allowed_providers_mode = CASE WHEN ? THEN ? ELSE allowed_providers_mode END,
    allowed_api_formats = CASE WHEN ? THEN ? ELSE allowed_api_formats END,
    allowed_api_formats_mode = CASE WHEN ? THEN ? ELSE allowed_api_formats_mode END,
    allowed_models = CASE WHEN ? THEN ? ELSE allowed_models END,
    allowed_models_mode = CASE WHEN ? THEN ? ELSE allowed_models_mode END,
    rate_limit = CASE WHEN ? THEN ? ELSE rate_limit END,
    rate_limit_mode = CASE WHEN ? THEN ? ELSE rate_limit_mode END,
    is_active = CASE WHEN ? THEN ? ELSE is_active END,
    updated_at = ?
WHERE id = ?
"#,
        )
        .bind(role.is_some())
        .bind(role)
        .bind(allowed_providers_present)
        .bind(optional_string_list_json(
            allowed_providers,
            "users.allowed_providers",
        )?)
        .bind(allowed_providers_present)
        .bind(allowed_providers_mode)
        .bind(allowed_api_formats_present)
        .bind(optional_string_list_json(
            allowed_api_formats,
            "users.allowed_api_formats",
        )?)
        .bind(allowed_api_formats_present)
        .bind(allowed_api_formats_mode)
        .bind(allowed_models_present)
        .bind(optional_string_list_json(
            allowed_models,
            "users.allowed_models",
        )?)
        .bind(allowed_models_present)
        .bind(allowed_models_mode)
        .bind(rate_limit_present)
        .bind(rate_limit)
        .bind(rate_limit_present)
        .bind(rate_limit_mode)
        .bind(is_active.is_some())
        .bind(is_active)
        .bind(chrono::Utc::now().timestamp())
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_sql_err()?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.find_user_auth_by_id(user_id).await
    }

    async fn update_local_auth_user_policy_modes(
        &self,
        user_id: &str,
        allowed_providers_mode: Option<String>,
        allowed_api_formats_mode: Option<String>,
        allowed_models_mode: Option<String>,
        rate_limit_mode: Option<String>,
    ) -> Result<Option<StoredUserAuthRecord>, DataLayerError> {
        let result = sqlx::query(
            r#"
UPDATE users
SET allowed_providers_mode = CASE WHEN ? THEN ? ELSE allowed_providers_mode END,
    allowed_api_formats_mode = CASE WHEN ? THEN ? ELSE allowed_api_formats_mode END,
    allowed_models_mode = CASE WHEN ? THEN ? ELSE allowed_models_mode END,
    rate_limit_mode = CASE WHEN ? THEN ? ELSE rate_limit_mode END,
    updated_at = ?
WHERE id = ?
"#,
        )
        .bind(allowed_providers_mode.is_some())
        .bind(allowed_providers_mode)
        .bind(allowed_api_formats_mode.is_some())
        .bind(allowed_api_formats_mode)
        .bind(allowed_models_mode.is_some())
        .bind(allowed_models_mode)
        .bind(rate_limit_mode.is_some())
        .bind(rate_limit_mode)
        .bind(chrono::Utc::now().timestamp())
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_sql_err()?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.find_user_auth_by_id(user_id).await
    }

    async fn update_user_model_capability_settings(
        &self,
        user_id: &str,
        settings: Option<serde_json::Value>,
    ) -> Result<Option<serde_json::Value>, DataLayerError> {
        let normalized = normalize_optional_json_value(settings);
        let result = sqlx::query(
            "UPDATE users SET model_capability_settings = ?, updated_at = ? WHERE id = ?",
        )
        .bind(optional_json_string(
            normalized.clone(),
            "users.model_capability_settings",
        )?)
        .bind(chrono::Utc::now().timestamp())
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_sql_err()?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        Ok(normalized)
    }

    async fn update_user_feature_settings(
        &self,
        user_id: &str,
        settings: Option<serde_json::Value>,
    ) -> Result<Option<serde_json::Value>, DataLayerError> {
        let normalized = normalize_optional_json_value(settings);
        let result =
            sqlx::query("UPDATE users SET feature_settings = ?, updated_at = ? WHERE id = ?")
                .bind(optional_json_string(
                    normalized.clone(),
                    "users.feature_settings",
                )?)
                .bind(chrono::Utc::now().timestamp())
                .bind(user_id)
                .execute(&self.pool)
                .await
                .map_sql_err()?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        Ok(normalized)
    }

    async fn create_local_auth_user(
        &self,
        email: Option<String>,
        email_verified: bool,
        username: String,
        password_hash: String,
    ) -> Result<Option<StoredUserAuthRecord>, DataLayerError> {
        let user_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
INSERT INTO users (
  id, email, email_verified, username, password_hash, role, auth_source,
  allowed_providers_mode, allowed_api_formats_mode, allowed_models_mode, rate_limit_mode,
  is_active, is_deleted, created_at, updated_at
)
VALUES (?, ?, ?, ?, ?, 'user', 'local', 'inherit', 'inherit', 'inherit', 'inherit', 1, 0, ?, ?)
"#,
        )
        .bind(&user_id)
        .bind(email)
        .bind(email_verified)
        .bind(username)
        .bind(password_hash)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_sql_err()?;
        self.find_user_auth_by_id(&user_id).await
    }

    async fn create_local_auth_user_with_settings(
        &self,
        email: Option<String>,
        email_verified: bool,
        username: String,
        password_hash: String,
        role: String,
        allowed_providers: Option<Vec<String>>,
        allowed_api_formats: Option<Vec<String>>,
        allowed_models: Option<Vec<String>>,
        rate_limit: Option<i32>,
    ) -> Result<Option<StoredUserAuthRecord>, DataLayerError> {
        let user_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        let allowed_providers_mode = if allowed_providers
            .as_ref()
            .is_some_and(|values| !values.is_empty())
        {
            "specific"
        } else {
            "unrestricted"
        };
        let allowed_api_formats_mode = if allowed_api_formats
            .as_ref()
            .is_some_and(|values| !values.is_empty())
        {
            "specific"
        } else {
            "unrestricted"
        };
        let allowed_models_mode = if allowed_models
            .as_ref()
            .is_some_and(|values| !values.is_empty())
        {
            "specific"
        } else {
            "unrestricted"
        };
        let rate_limit_mode = if rate_limit.is_some() {
            "custom"
        } else {
            "system"
        };
        sqlx::query(
            r#"
INSERT INTO users (
  id, email, email_verified, username, password_hash, role, auth_source,
  allowed_providers, allowed_providers_mode,
  allowed_api_formats, allowed_api_formats_mode,
  allowed_models, allowed_models_mode,
  rate_limit, rate_limit_mode,
  is_active, is_deleted, created_at, updated_at
)
VALUES (?, ?, ?, ?, ?, ?, 'local', ?, ?, ?, ?, ?, ?, ?, ?, 1, 0, ?, ?)
"#,
        )
        .bind(&user_id)
        .bind(email)
        .bind(email_verified)
        .bind(username)
        .bind(password_hash)
        .bind(role)
        .bind(optional_string_list_json(
            allowed_providers,
            "users.allowed_providers",
        )?)
        .bind(allowed_providers_mode)
        .bind(optional_string_list_json(
            allowed_api_formats,
            "users.allowed_api_formats",
        )?)
        .bind(allowed_api_formats_mode)
        .bind(optional_string_list_json(
            allowed_models,
            "users.allowed_models",
        )?)
        .bind(allowed_models_mode)
        .bind(rate_limit)
        .bind(rate_limit_mode)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_sql_err()?;
        self.find_user_auth_by_id(&user_id).await
    }

    async fn delete_local_auth_user(&self, user_id: &str) -> Result<bool, DataLayerError> {
        let result = sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_sql_err()?;
        Ok(result.rows_affected() > 0)
    }

    async fn count_active_admin_users(&self) -> Result<u64, DataLayerError> {
        let total: i64 = sqlx::query_scalar(
            r#"
SELECT COUNT(*)
FROM users
WHERE LOWER(role) = 'admin'
  AND is_deleted = 0
  AND is_active = 1
"#,
        )
        .fetch_one(&self.pool)
        .await
        .map_sql_err()?;
        Ok(total.max(0) as u64)
    }

    async fn read_user_preferences(
        &self,
        user_id: &str,
    ) -> Result<Option<StoredUserPreferenceRecord>, DataLayerError> {
        let mut builder = QueryBuilder::<Sqlite>::new(USER_PREFERENCES_COLUMNS);
        builder.push(" WHERE up.user_id = ").push_bind(user_id);
        let row = builder
            .build()
            .fetch_optional(&self.pool)
            .await
            .map_sql_err()?;
        row.as_ref().map(map_user_preference_row).transpose()
    }

    async fn write_user_preferences(
        &self,
        preferences: &StoredUserPreferenceRecord,
    ) -> Result<Option<StoredUserPreferenceRecord>, DataLayerError> {
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
INSERT INTO user_preferences (
  id, user_id, avatar_url, bio, default_provider_id, theme, language, timezone,
  email_notifications, usage_alerts, announcement_notifications, created_at, updated_at
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(user_id) DO UPDATE SET
  avatar_url = excluded.avatar_url,
  bio = excluded.bio,
  default_provider_id = excluded.default_provider_id,
  theme = excluded.theme,
  language = excluded.language,
  timezone = excluded.timezone,
  email_notifications = excluded.email_notifications,
  usage_alerts = excluded.usage_alerts,
  announcement_notifications = excluded.announcement_notifications,
  updated_at = excluded.updated_at
"#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&preferences.user_id)
        .bind(preferences.avatar_url.as_deref())
        .bind(preferences.bio.as_deref())
        .bind(preferences.default_provider_id.as_deref())
        .bind(&preferences.theme)
        .bind(&preferences.language)
        .bind(&preferences.timezone)
        .bind(preferences.email_notifications)
        .bind(preferences.usage_alerts)
        .bind(preferences.announcement_notifications)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_sql_err()?;
        self.read_user_preferences(&preferences.user_id).await
    }

    async fn find_user_session(
        &self,
        user_id: &str,
        session_id: &str,
    ) -> Result<Option<StoredUserSessionRecord>, DataLayerError> {
        let mut builder = QueryBuilder::<Sqlite>::new(USER_SESSION_COLUMNS);
        builder
            .push(" WHERE user_id = ")
            .push_bind(user_id)
            .push(" AND id = ")
            .push_bind(session_id)
            .push(" LIMIT 1");
        let row = builder
            .build()
            .fetch_optional(&self.pool)
            .await
            .map_sql_err()?;
        row.as_ref().map(map_user_session_row).transpose()
    }

    async fn list_user_sessions(
        &self,
        user_id: &str,
    ) -> Result<Vec<StoredUserSessionRecord>, DataLayerError> {
        let mut builder = QueryBuilder::<Sqlite>::new(USER_SESSION_COLUMNS);
        builder
            .push(" WHERE user_id = ")
            .push_bind(user_id)
            .push(" AND revoked_at IS NULL AND expires_at > ")
            .push_bind(Utc::now().timestamp())
            .push(" ORDER BY last_seen_at DESC, created_at DESC");
        let rows = builder.build().fetch_all(&self.pool).await.map_sql_err()?;
        rows.iter().map(map_user_session_row).collect()
    }

    async fn create_user_session(
        &self,
        session: &StoredUserSessionRecord,
    ) -> Result<Option<StoredUserSessionRecord>, DataLayerError> {
        let now = session
            .created_at
            .or(session.updated_at)
            .or(session.last_seen_at)
            .unwrap_or_else(Utc::now);
        sqlx::query(
            r#"
UPDATE user_sessions
SET revoked_at = ?, revoke_reason = 'replaced_by_new_login', updated_at = ?
WHERE user_id = ? AND client_device_id = ? AND revoked_at IS NULL AND expires_at > ?
"#,
        )
        .bind(now.timestamp())
        .bind(now.timestamp())
        .bind(&session.user_id)
        .bind(&session.client_device_id)
        .bind(now.timestamp())
        .execute(&self.pool)
        .await
        .map_sql_err()?;
        sqlx::query(
            r#"
INSERT INTO user_sessions (
  id, user_id, client_device_id, device_label, device_type, ip_address, user_agent,
  refresh_token_hash, last_seen_at, expires_at, created_at, updated_at
) VALUES (?, ?, ?, ?, 'unknown', ?, ?, ?, ?, ?, ?, ?)
"#,
        )
        .bind(&session.id)
        .bind(&session.user_id)
        .bind(&session.client_device_id)
        .bind(session.device_label.as_deref())
        .bind(session.ip_address.as_deref())
        .bind(session.user_agent.as_deref())
        .bind(&session.refresh_token_hash)
        .bind(session.last_seen_at.unwrap_or(now).timestamp())
        .bind(session.expires_at.unwrap_or(now).timestamp())
        .bind(session.created_at.unwrap_or(now).timestamp())
        .bind(session.updated_at.unwrap_or(now).timestamp())
        .execute(&self.pool)
        .await
        .map_sql_err()?;
        self.find_user_session(&session.user_id, &session.id).await
    }

    async fn touch_user_session(
        &self,
        user_id: &str,
        session_id: &str,
        touched_at: DateTime<Utc>,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<bool, DataLayerError> {
        let result = sqlx::query(
            r#"
UPDATE user_sessions
SET last_seen_at = ?, ip_address = COALESCE(?, ip_address),
    user_agent = COALESCE(?, user_agent), updated_at = ?
WHERE user_id = ? AND id = ?
"#,
        )
        .bind(touched_at.timestamp())
        .bind(ip_address)
        .bind(user_agent.map(|value| value.chars().take(1000).collect::<String>()))
        .bind(touched_at.timestamp())
        .bind(user_id)
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_sql_err()?;
        Ok(result.rows_affected() > 0)
    }

    async fn update_user_session_device_label(
        &self,
        user_id: &str,
        session_id: &str,
        device_label: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, DataLayerError> {
        let result = sqlx::query(
            r#"
UPDATE user_sessions
SET device_label = ?, updated_at = ?
WHERE user_id = ? AND id = ?
"#,
        )
        .bind(device_label.chars().take(120).collect::<String>())
        .bind(updated_at.timestamp())
        .bind(user_id)
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_sql_err()?;
        Ok(result.rows_affected() > 0)
    }

    async fn rotate_user_session_refresh_token(
        &self,
        user_id: &str,
        session_id: &str,
        previous_refresh_token_hash: &str,
        next_refresh_token_hash: &str,
        rotated_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<bool, DataLayerError> {
        let result = sqlx::query(
            r#"
UPDATE user_sessions
SET prev_refresh_token_hash = ?, rotated_at = ?, refresh_token_hash = ?,
    expires_at = ?, last_seen_at = ?, ip_address = COALESCE(?, ip_address),
    user_agent = COALESCE(?, user_agent), updated_at = ?
WHERE user_id = ? AND id = ?
"#,
        )
        .bind(previous_refresh_token_hash)
        .bind(rotated_at.timestamp())
        .bind(next_refresh_token_hash)
        .bind(expires_at.timestamp())
        .bind(rotated_at.timestamp())
        .bind(ip_address)
        .bind(user_agent.map(|value| value.chars().take(1000).collect::<String>()))
        .bind(rotated_at.timestamp())
        .bind(user_id)
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_sql_err()?;
        Ok(result.rows_affected() > 0)
    }

    async fn revoke_user_session(
        &self,
        user_id: &str,
        session_id: &str,
        revoked_at: DateTime<Utc>,
        reason: &str,
    ) -> Result<bool, DataLayerError> {
        let result = sqlx::query(
            "UPDATE user_sessions SET revoked_at = ?, revoke_reason = ?, updated_at = ? WHERE user_id = ? AND id = ?",
        )
        .bind(revoked_at.timestamp())
        .bind(reason.chars().take(100).collect::<String>())
        .bind(revoked_at.timestamp())
        .bind(user_id)
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_sql_err()?;
        Ok(result.rows_affected() > 0)
    }

    async fn revoke_all_user_sessions(
        &self,
        user_id: &str,
        revoked_at: DateTime<Utc>,
        reason: &str,
    ) -> Result<u64, DataLayerError> {
        let result = sqlx::query(
            "UPDATE user_sessions SET revoked_at = ?, revoke_reason = ?, updated_at = ? WHERE user_id = ? AND revoked_at IS NULL",
        )
        .bind(revoked_at.timestamp())
        .bind(reason.chars().take(100).collect::<String>())
        .bind(revoked_at.timestamp())
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_sql_err()?;
        Ok(result.rows_affected())
    }

    async fn count_active_local_admin_users_with_valid_password(
        &self,
    ) -> Result<u64, DataLayerError> {
        let total: i64 = sqlx::query_scalar(
            r#"
SELECT COUNT(*)
FROM users
WHERE LOWER(role) = 'admin'
  AND LOWER(auth_source) = 'local'
  AND is_deleted = 0
  AND is_active = 1
  AND LENGTH(password_hash) = 60
  AND (
    password_hash LIKE '$2a$%'
    OR password_hash LIKE '$2b$%'
    OR password_hash LIKE '$2y$%'
  )
"#,
        )
        .fetch_one(&self.pool)
        .await
        .map_sql_err()?;
        Ok(total.max(0) as u64)
    }
}

fn optional_json_from_string(
    value: Option<String>,
    field_name: &str,
) -> Result<Option<serde_json::Value>, DataLayerError> {
    value
        .map(|value| {
            serde_json::from_str(&value).map_err(|err| {
                DataLayerError::UnexpectedValue(format!(
                    "{field_name} contains invalid JSON: {err}"
                ))
            })
        })
        .transpose()
}

fn optional_string_list_json(
    value: Option<Vec<String>>,
    field_name: &str,
) -> Result<Option<String>, DataLayerError> {
    value
        .map(|value| {
            serde_json::to_string(&value).map_err(|err| {
                DataLayerError::UnexpectedValue(format!(
                    "{field_name} could not be serialized as JSON: {err}"
                ))
            })
        })
        .transpose()
}

fn optional_json_string(
    value: Option<serde_json::Value>,
    field_name: &str,
) -> Result<Option<String>, DataLayerError> {
    value
        .map(|value| {
            serde_json::to_string(&value).map_err(|err| {
                DataLayerError::UnexpectedValue(format!(
                    "{field_name} could not be serialized as JSON: {err}"
                ))
            })
        })
        .transpose()
}

fn normalize_optional_json_value(value: Option<serde_json::Value>) -> Option<serde_json::Value> {
    match value {
        Some(serde_json::Value::Null) | None => None,
        Some(value) => Some(value),
    }
}

fn optional_datetime_from_unix_secs(value: Option<i64>) -> Option<DateTime<Utc>> {
    value.and_then(|value| Utc.timestamp_opt(value, 0).single())
}

fn map_user_row(row: &SqliteRow) -> Result<StoredUserSummary, DataLayerError> {
    StoredUserSummary::new(
        row.try_get("id").map_sql_err()?,
        row.try_get("username").map_sql_err()?,
        row.try_get("email").map_sql_err()?,
        row.try_get("role").map_sql_err()?,
        row.try_get("is_active").map_sql_err()?,
        row.try_get("is_deleted").map_sql_err()?,
    )
}

fn map_user_export_row(row: &SqliteRow) -> Result<StoredUserExportRow, DataLayerError> {
    let feature_settings = optional_json_from_string(
        row.try_get("feature_settings").map_sql_err()?,
        "users.feature_settings",
    )?;
    StoredUserExportRow::new(
        row.try_get("id").map_sql_err()?,
        row.try_get("email").map_sql_err()?,
        row.try_get("email_verified").map_sql_err()?,
        row.try_get("username").map_sql_err()?,
        row.try_get("password_hash").map_sql_err()?,
        row.try_get("role").map_sql_err()?,
        row.try_get("auth_source").map_sql_err()?,
        optional_json_from_string(
            row.try_get("allowed_providers").map_sql_err()?,
            "users.allowed_providers",
        )?,
        optional_json_from_string(
            row.try_get("allowed_api_formats").map_sql_err()?,
            "users.allowed_api_formats",
        )?,
        optional_json_from_string(
            row.try_get("allowed_models").map_sql_err()?,
            "users.allowed_models",
        )?,
        row.try_get("rate_limit").map_sql_err()?,
        optional_json_from_string(
            row.try_get("model_capability_settings").map_sql_err()?,
            "users.model_capability_settings",
        )?,
        row.try_get("is_active").map_sql_err()?,
    )
    .map(|record| record.with_feature_settings(feature_settings))
    .and_then(|record| {
        record.with_policy_modes(
            row.try_get("allowed_providers_mode").map_sql_err()?,
            row.try_get("allowed_api_formats_mode").map_sql_err()?,
            row.try_get("allowed_models_mode").map_sql_err()?,
            row.try_get("rate_limit_mode").map_sql_err()?,
        )
    })
}

fn map_user_auth_row(row: &SqliteRow) -> Result<StoredUserAuthRecord, DataLayerError> {
    StoredUserAuthRecord::new(
        row.try_get("id").map_sql_err()?,
        row.try_get("email").map_sql_err()?,
        row.try_get("email_verified").map_sql_err()?,
        row.try_get("username").map_sql_err()?,
        row.try_get("password_hash").map_sql_err()?,
        row.try_get("role").map_sql_err()?,
        row.try_get("auth_source").map_sql_err()?,
        optional_json_from_string(
            row.try_get("allowed_providers").map_sql_err()?,
            "users.allowed_providers",
        )?,
        optional_json_from_string(
            row.try_get("allowed_api_formats").map_sql_err()?,
            "users.allowed_api_formats",
        )?,
        optional_json_from_string(
            row.try_get("allowed_models").map_sql_err()?,
            "users.allowed_models",
        )?,
        row.try_get("is_active").map_sql_err()?,
        row.try_get("is_deleted").map_sql_err()?,
        optional_datetime_from_unix_secs(row.try_get("created_at").map_sql_err()?),
        optional_datetime_from_unix_secs(row.try_get("last_login_at").map_sql_err()?),
    )
    .and_then(|record| {
        record.with_policy_modes(
            row.try_get("allowed_providers_mode").map_sql_err()?,
            row.try_get("allowed_api_formats_mode").map_sql_err()?,
            row.try_get("allowed_models_mode").map_sql_err()?,
        )
    })
}

fn map_user_preference_row(row: &SqliteRow) -> Result<StoredUserPreferenceRecord, DataLayerError> {
    let user_id: String = row.try_get("user_id").map_sql_err()?;
    if user_id.trim().is_empty() {
        return Err(DataLayerError::UnexpectedValue(
            "user_preferences.user_id is empty".to_string(),
        ));
    }

    Ok(StoredUserPreferenceRecord {
        user_id,
        avatar_url: row.try_get("avatar_url").map_sql_err()?,
        bio: row.try_get("bio").map_sql_err()?,
        default_provider_id: row.try_get("default_provider_id").map_sql_err()?,
        default_provider_name: row.try_get("default_provider_name").map_sql_err()?,
        theme: row.try_get("theme").map_sql_err()?,
        language: row.try_get("language").map_sql_err()?,
        timezone: row.try_get("timezone").map_sql_err()?,
        email_notifications: row.try_get("email_notifications").map_sql_err()?,
        usage_alerts: row.try_get("usage_alerts").map_sql_err()?,
        announcement_notifications: row.try_get("announcement_notifications").map_sql_err()?,
    })
}

fn map_user_session_row(row: &SqliteRow) -> Result<StoredUserSessionRecord, DataLayerError> {
    StoredUserSessionRecord::new(
        row.try_get("id").map_sql_err()?,
        row.try_get("user_id").map_sql_err()?,
        row.try_get("client_device_id").map_sql_err()?,
        row.try_get("device_label").map_sql_err()?,
        row.try_get("refresh_token_hash").map_sql_err()?,
        row.try_get("prev_refresh_token_hash").map_sql_err()?,
        optional_datetime_from_unix_secs(row.try_get("rotated_at").map_sql_err()?),
        optional_datetime_from_unix_secs(row.try_get("last_seen_at").map_sql_err()?),
        optional_datetime_from_unix_secs(row.try_get("expires_at").map_sql_err()?),
        optional_datetime_from_unix_secs(row.try_get("revoked_at").map_sql_err()?),
        row.try_get("revoke_reason").map_sql_err()?,
        row.try_get("ip_address").map_sql_err()?,
        row.try_get("user_agent").map_sql_err()?,
        optional_datetime_from_unix_secs(row.try_get("created_at").map_sql_err()?),
        optional_datetime_from_unix_secs(row.try_get("updated_at").map_sql_err()?),
    )
}

#[cfg(test)]
mod tests {
    use super::SqliteUserReadRepository;
    use crate::run_migrations;
    use aether_data_contracts::repository::users::{
        StoredUserPreferenceRecord, StoredUserSessionRecord, UserExportListQuery,
        UserReadRepository,
    };

    #[tokio::test]
    async fn sqlite_repository_reads_user_contract_views() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("sqlite pool should connect");
        run_migrations(&pool)
            .await
            .expect("sqlite migrations should run");
        sqlx::query(
            r#"
INSERT INTO users (
  id, email, email_verified, username, password_hash, role, auth_source,
  allowed_providers, allowed_api_formats, allowed_models, model_capability_settings,
  rate_limit, is_active, is_deleted, created_at, updated_at, last_login_at
) VALUES
  (
    'admin-1', 'admin@example.com', 1, 'admin', NULL, 'admin', 'local',
    NULL, NULL, NULL, NULL, 100, 1, 0, 1, 1, NULL
  ),
  (
    'user-1', 'user-1@example.com', 1, 'alice', 'hash', 'user', 'local',
    '["openai"]', '["openai:chat"]', '["gpt-4.1"]', '{"gpt-4.1":{"cache_1h":true}}',
    60, 1, 0, 2, 2, 3
  ),
  (
    'user-2', NULL, 0, 'deleted', NULL, 'user', 'local',
    NULL, NULL, NULL, NULL, NULL, 0, 1, 4, 4, NULL
  )
"#,
        )
        .execute(&pool)
        .await
        .expect("seed users should insert");
        let valid_hash = format!("$2b$12${}", "a".repeat(53));
        sqlx::query(
            r#"
INSERT INTO users (
  id, email, email_verified, username, password_hash, role, auth_source,
  allowed_providers, allowed_api_formats, allowed_models, model_capability_settings,
  rate_limit, is_active, is_deleted, created_at, updated_at, last_login_at
) VALUES (
  'admin-2', 'admin-2@example.com', 1, 'admin2', ?, 'admin', 'local',
  NULL, NULL, NULL, NULL, 100, 1, 0, 5, 5, NULL
)
"#,
        )
        .bind(valid_hash)
        .execute(&pool)
        .await
        .expect("valid local admin should insert");

        let repository = SqliteUserReadRepository::new(pool);
        let summaries = repository
            .list_users_by_ids(&["user-1".to_string(), "admin-1".to_string()])
            .await
            .expect("summaries should load");
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].id, "admin-1");

        let searched = repository
            .list_users_by_username_search("ali")
            .await
            .expect("username search should load");
        assert_eq!(searched.len(), 1);
        assert_eq!(searched[0].id, "user-1");

        let exports = repository
            .list_non_admin_export_users()
            .await
            .expect("non-admin exports should load");
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].allowed_models, Some(vec!["gpt-4.1".to_string()]));

        let page = repository
            .list_export_users_page(&UserExportListQuery {
                skip: 0,
                limit: 10,
                role: Some("user".to_string()),
                is_active: Some(true),
                search: None,
                group_id: None,
                ..Default::default()
            })
            .await
            .expect("export page should load");
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].id, "user-1");

        let summary = repository
            .summarize_export_users()
            .await
            .expect("export summary should load");
        assert_eq!(summary.total, 3);
        assert_eq!(summary.active, 3);

        let auth = repository
            .find_user_auth_by_identifier("user-1@example.com")
            .await
            .expect("auth lookup should load")
            .expect("auth user should exist");
        assert_eq!(auth.id, "user-1");
        assert_eq!(auth.last_login_at.expect("last login").timestamp(), 3);
        let logged_in_at = chrono::DateTime::from_timestamp(123, 0).expect("valid time");
        assert!(repository
            .touch_auth_user_last_login("user-1", logged_in_at)
            .await
            .expect("last login touch should update"));
        assert!(!repository
            .touch_auth_user_last_login("missing-user", logged_in_at)
            .await
            .expect("missing last login touch should be harmless"));
        let touched_auth = repository
            .find_user_auth_by_id("user-1")
            .await
            .expect("auth lookup should load")
            .expect("auth user should exist");
        assert_eq!(
            touched_auth.last_login_at.expect("last login").timestamp(),
            123
        );
        let profile_updated = repository
            .update_local_auth_user_profile(
                "user-1",
                Some("user-1b@example.com".to_string()),
                Some("alice-b".to_string()),
            )
            .await
            .expect("profile update should succeed")
            .expect("profile update should return user");
        assert_eq!(
            profile_updated.email.as_deref(),
            Some("user-1b@example.com")
        );
        assert_eq!(profile_updated.username, "alice-b");
        let password_updated = repository
            .update_local_auth_user_password_hash(
                "user-1",
                "new-password-hash".to_string(),
                logged_in_at,
            )
            .await
            .expect("password update should succeed")
            .expect("password update should return user");
        assert_eq!(
            password_updated.password_hash.as_deref(),
            Some("new-password-hash")
        );
        let created = repository
            .create_local_auth_user_with_settings(
                Some("created@example.com".to_string()),
                true,
                "created-user".to_string(),
                "created-hash".to_string(),
                "admin".to_string(),
                Some(vec!["openai".to_string()]),
                Some(vec!["chat".to_string()]),
                Some(vec!["gpt-4.1".to_string()]),
                Some(25),
            )
            .await
            .expect("local user create should succeed")
            .expect("local user create should return user");
        assert_eq!(created.email.as_deref(), Some("created@example.com"));
        assert_eq!(created.username, "created-user");
        assert_eq!(created.role, "admin");
        assert_eq!(created.allowed_providers, Some(vec!["openai".to_string()]));
        assert_eq!(created.allowed_api_formats, Some(vec!["chat".to_string()]));
        assert_eq!(created.allowed_models, Some(vec!["gpt-4.1".to_string()]));
        let admin_updated = repository
            .update_local_auth_user_admin_fields(
                &created.id,
                Some("user".to_string()),
                true,
                None,
                true,
                Some(vec!["responses".to_string()]),
                true,
                Some(vec!["gpt-4.1-mini".to_string()]),
                true,
                Some(5),
                Some(false),
            )
            .await
            .expect("admin fields update should succeed")
            .expect("admin fields update should return user");
        assert_eq!(admin_updated.role, "user");
        assert_eq!(admin_updated.allowed_providers, None);
        assert_eq!(
            admin_updated.allowed_api_formats,
            Some(vec!["responses".to_string()])
        );
        assert_eq!(
            admin_updated.allowed_models,
            Some(vec!["gpt-4.1-mini".to_string()])
        );
        assert!(!admin_updated.is_active);
        assert_eq!(
            repository
                .update_user_model_capability_settings(
                    &created.id,
                    Some(serde_json::json!({"gpt-4.1-mini": {"enabled": true}})),
                )
                .await
                .expect("model settings update should succeed"),
            Some(serde_json::json!({"gpt-4.1-mini": {"enabled": true}}))
        );
        assert_eq!(
            repository
                .update_user_model_capability_settings(&created.id, Some(serde_json::Value::Null))
                .await
                .expect("model settings clear should succeed"),
            None
        );

        let by_email = repository
            .find_user_auth_by_email("user-1b@example.com")
            .await
            .expect("email lookup should load")
            .expect("email lookup should find user");
        assert_eq!(by_email.id, "user-1");
        let by_username = repository
            .find_user_auth_by_username("alice-b")
            .await
            .expect("username lookup should load")
            .expect("username lookup should find user");
        assert_eq!(by_username.id, "user-1");
        assert!(repository
            .find_user_auth_by_email("alice")
            .await
            .expect("email lookup should load")
            .is_none());
        assert_eq!(
            repository
                .count_active_admin_users()
                .await
                .expect("active admin count should load"),
            2
        );
        assert_eq!(
            repository
                .count_active_local_admin_users_with_valid_password()
                .await
                .expect("valid local admin count should load"),
            1
        );
        let preferences = StoredUserPreferenceRecord {
            user_id: "user-1".to_string(),
            avatar_url: Some("https://example.test/avatar.png".to_string()),
            bio: Some("hello".to_string()),
            default_provider_id: None,
            default_provider_name: None,
            theme: "dark".to_string(),
            language: "en-US".to_string(),
            timezone: "UTC".to_string(),
            email_notifications: false,
            usage_alerts: true,
            announcement_notifications: false,
        };
        assert_eq!(
            repository
                .write_user_preferences(&preferences)
                .await
                .expect("preferences should write"),
            Some(preferences.clone())
        );
        assert_eq!(
            repository
                .read_user_preferences("user-1")
                .await
                .expect("preferences should read"),
            Some(preferences)
        );
        let now = chrono::Utc::now();
        let session = StoredUserSessionRecord::new(
            "session-1".to_string(),
            "user-1".to_string(),
            "device-1".to_string(),
            Some("Laptop".to_string()),
            StoredUserSessionRecord::hash_refresh_token("refresh-1"),
            None,
            None,
            Some(now),
            Some(now + chrono::Duration::hours(1)),
            None,
            None,
            Some("127.0.0.1".to_string()),
            Some("agent".to_string()),
            Some(now),
            Some(now),
        )
        .expect("session should build");
        assert_eq!(
            repository
                .create_user_session(&session)
                .await
                .expect("session should create")
                .map(|session| session.id),
            Some("session-1".to_string())
        );
        assert_eq!(
            repository
                .list_user_sessions("user-1")
                .await
                .expect("sessions should list")
                .len(),
            1
        );
        assert!(repository
            .revoke_user_session("user-1", "session-1", now, "logout")
            .await
            .expect("session should revoke"));
        assert!(repository
            .list_user_sessions("user-1")
            .await
            .expect("sessions should list")
            .is_empty());

        let by_ids = repository
            .list_user_auth_by_ids(&["user-1".to_string()])
            .await
            .expect("auth list should load");
        assert_eq!(by_ids.len(), 1);
        assert_eq!(by_ids[0].username, "alice-b");
        assert!(repository
            .delete_local_auth_user("user-1")
            .await
            .expect("delete should succeed"));
        assert!(!repository
            .delete_local_auth_user("user-1")
            .await
            .expect("second delete should succeed"));
        assert!(repository
            .find_user_auth_by_id("user-1")
            .await
            .expect("deleted auth lookup should load")
            .is_none());

        assert!(repository
            .find_export_user_by_id("user-2")
            .await
            .expect("deleted user lookup should run")
            .is_none());
    }
}
