use super::super::verify::admin_provider_ops_execute_json_request;
use crate::handlers::admin::request::AdminAppState;
use aether_admin::provider::ops::{build_headers, multiplier::Sub2ApiKeyMultiplier};
use aether_contracts::ProxySnapshot;
use serde_json::{Map, Value};

/// 执行 New API 只读查询；完整密钥读取虽使用 POST，但不创建或修改令牌。
async fn query(
    state: &AdminAppState<'_>,
    base: &str,
    path: &str,
    method: reqwest::Method,
    headers: &reqwest::header::HeaderMap,
    proxy: Option<&ProxySnapshot>,
) -> Result<Value, String> {
    let url = format!("{}{}", base.trim_end_matches('/'), path);
    let (status, value) = admin_provider_ops_execute_json_request(
        state,
        "provider-ops:multiplier:new-api",
        method,
        &url,
        headers,
        None,
        proxy,
    )
    .await
    .map_err(|_| "New API 查询请求失败".to_string())?;
    if !status.is_success() {
        return Err(status_message(status));
    }
    if value.get("success").and_then(Value::as_bool) != Some(true) {
        return Err("当前认证信息无法查询 New API 密钥或分组".to_string());
    }
    value
        .get("data")
        .cloned()
        .ok_or_else(|| "New API 缺少 data".to_string())
}

/// 用账号凭据精确匹配本地密钥，拒绝 auto 动态分组和非法倍率。
pub(super) async fn resolve_new_api(
    state: &AdminAppState<'_>,
    base: &str,
    credentials: &Map<String, Value>,
    proxy: Option<&ProxySnapshot>,
    secret: &str,
) -> Result<Sub2ApiKeyMultiplier, String> {
    let headers = build_headers("new_api", &Map::new(), credentials)?;
    let mut matched = None;
    let mut complete = false;
    for page in 1..=100 {
        let data = query(
            state,
            base,
            &format!("/api/token/?p={page}&size=100"),
            reqwest::Method::GET,
            &headers,
            proxy,
        )
        .await?;
        let items = data
            .as_array()
            .or_else(|| data.get("items").and_then(Value::as_array))
            .ok_or_else(|| "New API 密钥列表格式无效".to_string())?;
        for item in items {
            let listed = item.get("key").and_then(Value::as_str).unwrap_or("");
            let full = if listed.contains('*') {
                let id = item
                    .get("id")
                    .and_then(Value::as_i64)
                    .filter(|id| *id > 0)
                    .ok_or_else(|| "New API 密钥 ID 无效".to_string())?;
                query(
                    state,
                    base,
                    &format!("/api/token/{id}/key"),
                    reqwest::Method::POST,
                    &headers,
                    proxy,
                )
                .await?
                .get("key")
                .and_then(Value::as_str)
                .ok_or_else(|| "New API 未返回完整密钥".to_string())?
                .to_string()
            } else {
                listed.to_string()
            };
            if !full.is_empty()
                && full.strip_prefix("sk-").unwrap_or(&full)
                    == secret.strip_prefix("sk-").unwrap_or(secret)
            {
                if matched.is_some() {
                    return Err("New API 返回重复匹配密钥".to_string());
                }
                matched = Some(item.clone());
            }
        }
        let total = data.get("total").and_then(Value::as_u64);
        if total.is_some_and(|total| page * 100 >= total) || (total.is_none() && items.len() < 100)
        {
            complete = true;
            break;
        }
        if items.is_empty() {
            return Err("New API 密钥分页不完整".to_string());
        }
    }
    if !complete {
        return Err("New API 密钥数量超过同步上限".to_string());
    }
    let key = matched.ok_or_else(|| "未找到匹配的 New API 密钥".to_string())?;
    let group = key
        .get("group")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if group.is_empty() {
        // 空分组可能由上游自动选择，不能假定用户分组就是实际计费分组。
        return Err("该密钥未指定固定分组，无法同步固定倍率".to_string());
    }
    if group == "auto" {
        return Err("auto 自动分组不支持跟随固定倍率".to_string());
    }
    let groups = match query(
        state,
        base,
        "/api/user/self/groups",
        reqwest::Method::GET,
        &headers,
        proxy,
    )
    .await
    {
        Ok(value) => value,
        Err(_) => {
            query(
                state,
                base,
                "/api/user/groups",
                reqwest::Method::GET,
                &headers,
                proxy,
            )
            .await?
        }
    };
    let multiplier = groups
        .get(&group)
        .and_then(|value| value.get("ratio"))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value >= 0.0)
        .ok_or_else(|| "New API 分组没有有效倍率".to_string())?;
    Ok(Sub2ApiKeyMultiplier {
        group_id: 0,
        group_name: Some(group),
        multiplier,
    })
}

/// 将上游状态码转换为无响应正文、无凭据的可操作提示。
pub(super) fn status_message(status: http::StatusCode) -> String {
    match status.as_u16() {
        401 => "用户认证已失效，请重新配置后再试",
        403 => "当前认证无权查询该密钥倍率，请检查账号权限或分组",
        404 | 405 | 501 => "该供应商未提供兼容的上游倍率查询接口",
        429 => "上游请求过于频繁，请稍后重试",
        500..=599 => "上游服务暂时不可用，请稍后重试",
        _ => "上游倍率查询失败，请检查供应商认证配置",
    }
    .to_string()
}
