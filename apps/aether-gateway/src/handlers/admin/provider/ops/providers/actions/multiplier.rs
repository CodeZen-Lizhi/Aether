use super::super::config::persist_admin_provider_ops_runtime_credentials;
use super::super::verify::{
    admin_provider_ops_execute_json_request, admin_provider_ops_sub2api_exchange_token,
    admin_provider_ops_sub2api_request_url, AdminProviderOpsExecuteJsonError,
};
use super::responses::{admin_provider_ops_action_error, admin_provider_ops_action_response};
use crate::handlers::admin::request::AdminAppState;
use aether_admin::provider::ops::multiplier::resolve_sub2api_key_multiplier;
use aether_admin::provider::ops::multiplier::Sub2ApiKeyMultiplier;
use aether_contracts::ProxySnapshot;
use aether_data_contracts::repository::provider_catalog::{
    ProviderCatalogKeyMultiplierUpdate, StoredProviderCatalogProvider,
};
use futures_util::StreamExt;
use serde_json::{json, Map, Value};

/// 获取账号接口数据；保留 HTTP 错误，拒绝业务失败和不合法响应。
async fn fetch_data(
    state: &AdminAppState<'_>,
    base: &str,
    path: &str,
    headers: &reqwest::header::HeaderMap,
    proxy: Option<&ProxySnapshot>,
) -> Result<Value, String> {
    let result = admin_provider_ops_execute_json_request(
        state,
        "provider-ops:multiplier",
        reqwest::Method::GET,
        &admin_provider_ops_sub2api_request_url(base, path),
        headers,
        None,
        proxy,
    )
    .await;
    let (status, value) = result.map_err(|error| match error {
        AdminProviderOpsExecuteJsonError::InvalidJson(_) => "上游响应不是有效 JSON".to_string(),
        AdminProviderOpsExecuteJsonError::Transport(_) => "上游倍率接口请求失败".to_string(),
    })?;
    if !status.is_success() {
        return Err(super::multiplier_upstream::status_message(status));
    }
    if value.get("code").and_then(Value::as_i64) != Some(0) {
        return Err("上游倍率接口返回业务失败".to_string());
    }
    value
        .get("data")
        .cloned()
        .ok_or_else(|| "上游倍率接口缺少 data".to_string())
}

/// 同步本地跟随上游的密钥；模式修改与结果写入均使用条件更新。
pub(super) async fn sync_multiplier(
    state: &AdminAppState<'_>,
    provider: &StoredProviderCatalogProvider,
    base: &str,
    credentials: &Map<String, Value>,
    proxy: Option<&ProxySnapshot>,
    config: Option<&Map<String, Value>>,
) -> Value {
    let result = run_sync(state, provider, base, credentials, proxy, config).await;
    match result {
        Ok(data) => {
            admin_provider_ops_action_response("success", "sync_multiplier", data, None, None, 0)
        }
        Err(message) => {
            admin_provider_ops_action_error("unknown_error", "sync_multiplier", message, None)
        }
    }
}

/// 读取上游后逐项条件写入；匹配失败保留倍率并持久化失败状态。
async fn run_sync(
    state: &AdminAppState<'_>,
    provider: &StoredProviderCatalogProvider,
    base: &str,
    credentials: &Map<String, Value>,
    proxy: Option<&ProxySnapshot>,
    config: Option<&Map<String, Value>>,
) -> Result<Value, String> {
    let architecture = provider
        .config
        .as_ref()
        .and_then(|value| value.pointer("/provider_ops/architecture_id"))
        .and_then(Value::as_str)
        .unwrap_or("generic_api");
    if config.is_some_and(|config| {
        ["key_id", "mode"].iter().any(|field| {
            config.get(*field).is_some_and(|value| {
                !value.is_null()
                    && (!value.is_string()
                        || value.as_str().is_some_and(|value| value.trim().is_empty()))
            })
        })
    }) {
        return Err("倍率模式或密钥参数无效".to_string());
    }
    let selected_id = config
        .and_then(|config| config.get("key_id"))
        .and_then(Value::as_str);
    let mode = config
        .and_then(|config| config.get("mode"))
        .and_then(Value::as_str);
    if mode.is_some_and(|mode| !matches!(mode, "manual" | "upstream"))
        || (mode.is_some() && selected_id.is_none())
    {
        return Err("倍率模式或密钥参数无效".to_string());
    }
    let mut keys = state
        .list_provider_catalog_keys_by_provider_ids(std::slice::from_ref(&provider.id))
        .await
        .map_err(|_| "读取本地密钥失败".to_string())?;
    keys.retain(|key| selected_id.is_none_or(|id| key.id == id));
    if selected_id.is_some() && keys.is_empty() {
        return Err("密钥不属于当前供应商".to_string());
    }
    let manual_multiplier = config.and_then(|value| value.get("multiplier"));
    if manual_multiplier.is_some_and(|value| {
        mode != Some("manual")
            || value
                .as_f64()
                .is_none_or(|value| !value.is_finite() || value < 0.0)
    }) {
        return Err("手动倍率必须是大于或等于 0 的有效数值".to_string());
    }
    if mode == Some("manual") {
        let key = &keys[0];
        let metadata = json!({"source":"manual","generation":uuid::Uuid::new_v4().to_string(),"status":"manual"});
        let written = state
            .as_ref()
            .compare_and_update_provider_catalog_key_multiplier(
                &ProviderCatalogKeyMultiplierUpdate {
                    expected_key: key.clone(),
                    expected_provider_ops: None,
                    metadata,
                    multiplier: manual_multiplier.and_then(Value::as_f64),
                },
            )
            .await
            .map_err(|_| "保存倍率设置失败".to_string())?;
        if !written {
            return Err("密钥配置已变化，请刷新后重试".to_string());
        }
        return Ok(json!({"updated":1,"unchanged":0,"failed":0,"skipped":0,"results":[]}));
    }
    if mode == Some("upstream") && !matches!(keys[0].auth_type.as_str(), "api_key" | "bearer") {
        return Err("该密钥的认证方式不支持上游倍率查询".to_string());
    }
    // 仅首次开启时也选中手动密钥；探测成功后才原子保存来源和倍率。
    let skipped = keys
        .iter()
        .filter(|key| {
            key.upstream_metadata
                .as_ref()
                .and_then(|value| value.pointer("/multiplier/source"))
                .and_then(Value::as_str)
                != Some("upstream")
        })
        .count();
    keys.retain(|key| {
        mode == Some("upstream")
            || key
                .upstream_metadata
                .as_ref()
                .and_then(|value| value.pointer("/multiplier/source"))
                .and_then(Value::as_str)
                == Some("upstream")
    });
    if keys.is_empty() {
        return Ok(json!({"updated":0,"unchanged":0,"failed":0,"skipped":skipped,"results":[]}));
    }
    if credentials.is_empty() {
        return Err("请先配置供应商用户认证，再开启或同步跟随上游倍率".to_string());
    }
    let mut expected_provider_ops = provider
        .config
        .as_ref()
        .and_then(|value| value.get("provider_ops"))
        .cloned();
    let upstream_request = async {
        if architecture != "sub2api" {
            return Ok((Vec::new(), Vec::new(), Map::new()));
        }
        let (token, updated, _) =
            admin_provider_ops_sub2api_exchange_token(state, base, credentials, proxy)
                .await
                .map_err(|_| "用户认证失效或暂时无法验证，请检查用户认证配置".to_string())?;
        if !updated.is_empty() {
            let saved = persist_admin_provider_ops_runtime_credentials(state, provider, &updated)
                .await
                .map_err(|_| "保存账号续期凭据失败".to_string())?
                .ok_or_else(|| "用户认证配置已变化，本次未更新倍率".to_string())?;
            expected_provider_ops = saved
                .config
                .as_ref()
                .and_then(|value| value.get("provider_ops"))
                .cloned();
        }
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|_| "访问令牌无效".to_string())?,
        );
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static(
                aether_admin::provider::ops::ADMIN_PROVIDER_OPS_USER_AGENT,
            ),
        );
        let rates = fetch_data(state, base, "/api/v1/groups/rates", &headers, proxy).await?;
        let rates = if rates.is_null() {
            Map::new()
        } else {
            rates
                .as_object()
                .cloned()
                .ok_or_else(|| "专属倍率格式无效".to_string())?
        };
        let groups = fetch_data(state, base, "/api/v1/groups/available", &headers, proxy).await?;
        let groups = groups
            .as_array()
            .cloned()
            .ok_or_else(|| "分组列表格式无效".to_string())?;
        let mut upstream_keys = Vec::new();
        for page in 1..=100 {
            let data = fetch_data(
                state,
                base,
                &format!("/api/v1/keys?page={page}&page_size=100"),
                &headers,
                proxy,
            )
            .await?;
            let items = data
                .get("items")
                .and_then(Value::as_array)
                .ok_or_else(|| "密钥列表格式无效".to_string())?;
            let total = data
                .get("total")
                .and_then(Value::as_u64)
                .ok_or_else(|| "密钥分页总数无效".to_string())?;
            upstream_keys.extend(items.iter().cloned());
            if upstream_keys.len() as u64 >= total {
                return Ok::<_, String>((upstream_keys, groups, rates));
            }
            if items.is_empty() {
                return Err("上游密钥分页不完整".to_string());
            }
        }
        Err("上游密钥数量超过同步上限".to_string())
    };
    // 整轮网络请求有总预算，避免大量分页或慢站点长期占用余额刷新任务。
    let upstream = tokio::time::timeout(std::time::Duration::from_secs(60), upstream_request)
        .await
        .unwrap_or_else(|_| Err("上游倍率同步超时".to_string()));
    let mut results = Vec::new();
    let mut updated = 0;
    let mut unchanged = 0;
    let mut failed = 0;
    // 每个密钥独立查询并限制并发，慢请求不会阻止其他密钥发起查询。
    let resolved_keys = futures_util::stream::iter(keys.into_iter().map(|key| {
        let upstream = &upstream;
        async move {
            let resolved = if architecture != "sub2api" {
                if let Some(secret) = key
                    .encrypted_api_key
                    .as_deref()
                    .and_then(|value| state.decrypt_catalog_secret_with_fallbacks(value))
                {
                    resolve_other_multiplier(state, architecture, base, credentials, proxy, &secret)
                        .await
                } else {
                    Err("本地密钥无法解密".to_string())
                }
            } else {
                match &upstream {
                    Ok((upstream_keys, groups, rates)) => key
                        .encrypted_api_key
                        .as_deref()
                        .and_then(|ciphertext| {
                            state.decrypt_catalog_secret_with_fallbacks(ciphertext)
                        })
                        .ok_or_else(|| "本地密钥无法解密".to_string())
                        .and_then(|secret| {
                            resolve_sub2api_key_multiplier(&secret, upstream_keys, groups, rates)
                        }),
                    Err(error) => Err(error.clone()),
                }
            };
            (key, resolved)
        }
    }))
    .buffer_unordered(4);
    futures_util::pin_mut!(resolved_keys);
    while let Some((key, resolved)) = resolved_keys.next().await {
        if mode == Some("upstream") {
            if let Err(error) = &resolved {
                return Err(error.clone());
            }
        }
        let previous = key
            .upstream_metadata
            .as_ref()
            .and_then(|value| value.get("multiplier"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let mut metadata = previous
            .as_object()
            .cloned()
            .ok_or_else(|| "倍率元数据格式无效".to_string())?;
        let now = chrono::Utc::now().to_rfc3339();
        metadata.insert("last_attempt_at".to_string(), json!(now));
        // 每次结果写入都改变版本，重叠同步的旧基线不能再次写入。
        metadata.insert(
            "generation".to_string(),
            json!(uuid::Uuid::new_v4().to_string()),
        );
        let multiplier = match resolved {
            Ok(value) => {
                metadata.insert("source".to_string(), json!("upstream"));
                metadata.insert("status".to_string(), json!("success"));
                metadata.insert("error".to_string(), Value::Null);
                metadata.insert("last_success_at".to_string(), json!(now));
                metadata.insert("group_id".to_string(), json!(value.group_id));
                metadata.insert("group_name".to_string(), json!(value.group_name));
                Some(value.multiplier)
            }
            Err(error) => {
                metadata.insert("status".to_string(), json!("failed"));
                metadata.insert("error".to_string(), json!(error));
                None
            }
        };
        // 在同一数据库条件更新中核验认证快照，清除或更换认证后丢弃在途结果。
        let written = state
            .as_ref()
            .compare_and_update_provider_catalog_key_multiplier(
                &ProviderCatalogKeyMultiplierUpdate {
                    expected_key: key.clone(),
                    expected_provider_ops: expected_provider_ops.clone(),
                    metadata: Value::Object(metadata.clone()),
                    multiplier,
                },
            )
            .await
            .map_err(|_| "写入倍率同步结果失败".to_string())?;
        if !written {
            failed += 1;
            results.push(json!({"key_id":key.id,"status":"conflict","error":"密钥配置或同步结果已变化，请重试"}));
        } else if let Some(value) = multiplier {
            if value == key.default_rate_multiplier {
                unchanged += 1;
            } else {
                updated += 1;
            }
            results.push(json!({"key_id":key.id,"status":"success","multiplier":value}));
        } else {
            failed += 1;
            results.push(json!({"key_id":key.id,"status":"failed","error":metadata.get("error")}));
        }
    }
    Ok(
        json!({"updated":updated,"unchanged":unchanged,"failed":failed,"skipped":skipped,"results":results}),
    )
}

/// 按认证模板查询 New API 分组或兼容的密钥计费接口；失败保留本地倍率。
async fn resolve_other_multiplier(
    state: &AdminAppState<'_>,
    architecture: &str,
    base: &str,
    credentials: &Map<String, Value>,
    proxy: Option<&ProxySnapshot>,
    secret: &str,
) -> Result<Sub2ApiKeyMultiplier, String> {
    let query = async {
        if architecture == "new_api" {
            return super::multiplier_upstream::resolve_new_api(
                state,
                base,
                credentials,
                proxy,
                secret,
            )
            .await;
        }
        if architecture != "usage_api" {
            return Err("该供应商暂不支持上游倍率查询".to_string());
        }
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!("Bearer {secret}"))
                .map_err(|_| "本地密钥格式无效".to_string())?,
        );
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static(
                aether_admin::provider::ops::ADMIN_PROVIDER_OPS_USER_AGENT,
            ),
        );
        let (status, value) = admin_provider_ops_execute_json_request(
            state,
            "provider-ops:multiplier:billing",
            reqwest::Method::GET,
            &admin_provider_ops_sub2api_request_url(base, "/v1/sub2api/billing"),
            &headers,
            None,
            proxy,
        )
        .await
        .map_err(|_| "上游倍率查询请求失败".to_string())?;
        if !status.is_success() {
            return Err(super::multiplier_upstream::status_message(status));
        }
        let multiplier = value
            .get("effective_rate_multiplier")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && *value >= 0.0)
            .ok_or_else(|| "上游未提供有效的密钥倍率".to_string())?;
        Ok(Sub2ApiKeyMultiplier {
            group_id: 0,
            group_name: None,
            multiplier,
        })
    };
    tokio::time::timeout(std::time::Duration::from_secs(60), query)
        .await
        .unwrap_or_else(|_| Err("上游倍率查询超时".to_string()))
}
