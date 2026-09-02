use super::architectures::normalize_architecture_id;
use http::StatusCode;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{json, Map, Value};

pub const ADMIN_PROVIDER_OPS_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.7339.249 Electron/38.7.0 Safari/537.36";

pub fn build_headers(
    architecture_id: &str,
    config: &Map<String, Value>,
    credentials: &Map<String, Value>,
) -> Result<HeaderMap, String> {
    admin_provider_ops_verify_headers(architecture_id, config, credentials)
}

pub fn parse_verify_payload(
    architecture_id: &str,
    status: StatusCode,
    response_json: &Value,
    updated_credentials: Option<Map<String, Value>>,
) -> Value {
    match normalize_architecture_id(architecture_id) {
        "sub2api" => {
            admin_provider_ops_sub2api_verify_payload(status, response_json, updated_credentials)
        }
        "usage_api" => admin_provider_ops_usage_api_verify_payload(status, response_json),
        _ => admin_provider_ops_generic_verify_payload(status, response_json),
    }
}

pub fn admin_provider_ops_verify_failure(message: impl Into<String>) -> Value {
    json!({
        "success": false,
        "message": message.into(),
    })
}

pub fn admin_provider_ops_verify_success(
    data: Value,
    updated_credentials: Option<Map<String, Value>>,
) -> Value {
    let mut payload = Map::from_iter([
        ("success".to_string(), Value::Bool(true)),
        ("message".to_string(), Value::Null),
        ("data".to_string(), data),
        (
            "updated_credentials".to_string(),
            updated_credentials
                .clone()
                .map(Value::Object)
                .unwrap_or(Value::Null),
        ),
    ]);
    if updated_credentials
        .as_ref()
        .is_some_and(|value| value.is_empty())
    {
        payload.insert("updated_credentials".to_string(), Value::Null);
    }
    Value::Object(payload)
}

pub fn admin_provider_ops_verify_user_payload(
    username: Option<String>,
    display_name: Option<String>,
    email: Option<String>,
    quota: Option<f64>,
    extra: Option<Map<String, Value>>,
) -> Value {
    let resolved_username = username.filter(|value| !value.trim().is_empty());
    let resolved_display_name = display_name
        .filter(|value| !value.trim().is_empty())
        .or_else(|| resolved_username.clone());
    let mut payload = Map::new();
    payload.insert(
        "username".to_string(),
        resolved_username.map(Value::String).unwrap_or(Value::Null),
    );
    payload.insert(
        "display_name".to_string(),
        resolved_display_name
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    payload.insert(
        "email".to_string(),
        email.map(Value::String).unwrap_or(Value::Null),
    );
    payload.insert(
        "quota".to_string(),
        quota
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or(Value::Null),
    );
    payload.insert(
        "extra".to_string(),
        Value::Object(extra.unwrap_or_default()),
    );
    Value::Object(payload)
}

pub fn admin_provider_ops_verify_user_payload_with_usage(
    username: Option<String>,
    display_name: Option<String>,
    email: Option<String>,
    quota: Option<f64>,
    used_quota: Option<f64>,
    request_count: Option<u64>,
    extra: Option<Map<String, Value>>,
) -> Value {
    let mut payload =
        admin_provider_ops_verify_user_payload(username, display_name, email, quota, extra)
            .as_object()
            .cloned()
            .unwrap_or_default();
    payload.insert(
        "used_quota".to_string(),
        used_quota
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or(Value::Null),
    );
    payload.insert(
        "request_count".to_string(),
        request_count
            .map(serde_json::Number::from)
            .map(Value::Number)
            .unwrap_or(Value::Null),
    );
    Value::Object(payload)
}

pub fn admin_provider_ops_value_as_f64(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(raw)) => raw.trim().parse::<f64>().ok(),
        _ => None,
    }
}

pub fn admin_provider_ops_value_as_u64(value: Option<&Value>) -> Option<u64> {
    match value {
        Some(Value::Number(number)) => number.as_u64().or_else(|| {
            number
                .as_i64()
                .filter(|value| *value >= 0)
                .map(|value| value as u64)
        }),
        Some(Value::String(raw)) => raw.trim().parse::<u64>().ok(),
        _ => None,
    }
}

pub fn admin_provider_ops_json_object(value: &Value) -> Option<&Map<String, Value>> {
    value.as_object()
}

pub fn admin_provider_ops_frontend_updated_credentials(
    credentials: Map<String, Value>,
) -> Option<Map<String, Value>> {
    let filtered = credentials
        .into_iter()
        .filter(|(key, value)| {
            !key.starts_with('_')
                && !matches!(value, Value::Null)
                && !value.as_str().is_some_and(|raw| raw.trim().is_empty())
        })
        .collect::<Map<String, Value>>();
    (!filtered.is_empty()).then_some(filtered)
}

fn insert_header(headers: &mut HeaderMap, name: &str, value: &str) -> Result<(), String> {
    let header_name =
        HeaderName::from_bytes(name.as_bytes()).map_err(|_| format!("无效的请求头: {name}"))?;
    let header_value =
        HeaderValue::from_str(value).map_err(|_| format!("无效的请求头值: {name}"))?;
    headers.insert(header_name, header_value);
    Ok(())
}

pub fn admin_provider_ops_verify_headers(
    architecture_id: &str,
    config: &Map<String, Value>,
    credentials: &Map<String, Value>,
) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    match normalize_architecture_id(architecture_id) {
        "generic_api" | "usage_api" => {
            let api_key = credentials
                .get("api_key")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            if !api_key.is_empty() {
                let auth_method = config
                    .get("auth_method")
                    .and_then(Value::as_str)
                    .unwrap_or("bearer");
                if auth_method == "header" {
                    let header_name = config
                        .get("header_name")
                        .and_then(Value::as_str)
                        .unwrap_or("X-API-Key");
                    insert_header(&mut headers, header_name, api_key)?;
                } else {
                    insert_header(&mut headers, "Authorization", &format!("Bearer {api_key}"))?;
                }
            }
        }
        "new_api" => {
            for (name, value) in [
                (
                    "User-Agent",
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.7339.249 Electron/38.7.0 Safari/537.36",
                ),
                ("Accept", "application/json"),
                ("Accept-Language", "zh-CN"),
                ("sec-ch-ua", "\"Not=A?Brand\";v=\"24\", \"Chromium\";v=\"140\""),
                ("sec-ch-ua-mobile", "?0"),
                ("sec-ch-ua-platform", "\"macOS\""),
                ("Sec-Fetch-Site", "cross-site"),
                ("Sec-Fetch-Mode", "cors"),
                ("Sec-Fetch-Dest", "empty"),
            ] {
                insert_header(&mut headers, name, value)?;
            }
            if let Some(api_key) = credentials.get("api_key").and_then(Value::as_str) {
                if !api_key.trim().is_empty() {
                    insert_header(
                        &mut headers,
                        "Authorization",
                        &format!("Bearer {}", api_key.trim()),
                    )?;
                }
            }
            if let Some(user_id) = credentials.get("user_id").and_then(Value::as_str) {
                if !user_id.trim().is_empty() {
                    insert_header(&mut headers, "New-Api-User", user_id.trim())?;
                }
            }
            if let Some(cookie) = credentials.get("cookie").and_then(Value::as_str) {
                if !cookie.trim().is_empty() {
                    insert_header(&mut headers, "Cookie", cookie.trim())?;
                }
            }
        }
        _ => {}
    }
    Ok(headers)
}

pub fn admin_provider_ops_generic_verify_payload(
    status: StatusCode,
    response_json: &Value,
) -> Value {
    verify_payload_with_auth_messages(
        status,
        response_json,
        "认证失败：无效的凭据",
        "认证失败：权限不足",
    )
}

pub fn admin_provider_ops_usage_api_verify_payload(
    status: StatusCode,
    response_json: &Value,
) -> Value {
    if status == StatusCode::UNAUTHORIZED {
        return admin_provider_ops_verify_failure("认证失败：API Key 无效");
    }
    if status == StatusCode::FORBIDDEN {
        return admin_provider_ops_verify_failure("认证失败：API Key 无权查询用量");
    }
    if status != StatusCode::OK {
        return admin_provider_ops_verify_failure(format!("验证失败：HTTP {}", status.as_u16()));
    }

    let Some(data) = response_json.as_object() else {
        return admin_provider_ops_verify_failure("响应格式无效");
    };
    let is_valid = data
        .get("is_active")
        .and_then(Value::as_bool)
        .or_else(|| data.get("isValid").and_then(Value::as_bool));
    if is_valid == Some(false) {
        return admin_provider_ops_verify_failure("API Key 无效或已停用");
    }

    let quota = data.get("quota").and_then(Value::as_object);
    let remaining = admin_provider_ops_value_as_f64(data.get("remaining"))
        .or_else(|| quota.and_then(|quota| admin_provider_ops_value_as_f64(quota.get("remaining"))))
        .or_else(|| admin_provider_ops_value_as_f64(data.get("balance")));
    let Some(remaining) = remaining else {
        return admin_provider_ops_verify_failure("响应缺少余额字段");
    };
    let unit = data
        .get("unit")
        .and_then(Value::as_str)
        .or_else(|| quota.and_then(|quota| quota.get("unit").and_then(Value::as_str)))
        .unwrap_or("USD")
        .to_string();
    let plan_name = data
        .get("planName")
        .and_then(Value::as_str)
        .or_else(|| data.get("plan_name").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("API Key")
        .to_string();

    let mut extra = Map::new();
    extra.insert("unit".to_string(), Value::String(unit));
    if let Some(value) = data.get("balance") {
        extra.insert("balance".to_string(), value.clone());
    }
    if let Some(value) = data.get("mode") {
        extra.insert("mode".to_string(), value.clone());
    }
    extra.insert(
        "is_valid".to_string(),
        Value::Bool(is_valid.unwrap_or(true)),
    );

    admin_provider_ops_verify_success(
        admin_provider_ops_verify_user_payload(
            Some(plan_name.clone()),
            Some(plan_name),
            None,
            Some(remaining),
            Some(extra),
        ),
        None,
    )
}

fn verify_payload_with_auth_messages(
    status: StatusCode,
    response_json: &Value,
    unauthorized_message: &str,
    forbidden_message: &str,
) -> Value {
    if status == StatusCode::UNAUTHORIZED {
        return admin_provider_ops_verify_failure(unauthorized_message);
    }
    if status == StatusCode::FORBIDDEN {
        return admin_provider_ops_verify_failure(forbidden_message);
    }
    if status != StatusCode::OK {
        return admin_provider_ops_verify_failure(format!("验证失败：HTTP {}", status.as_u16()));
    }

    let user_data = if response_json.get("success").and_then(Value::as_bool) == Some(true)
        && response_json.get("data").is_some_and(Value::is_object)
    {
        response_json.get("data")
    } else if response_json.get("success").and_then(Value::as_bool) == Some(false) {
        return admin_provider_ops_verify_failure(
            response_json
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("验证失败"),
        );
    } else {
        Some(response_json)
    };

    let Some(user_data) = user_data.and_then(admin_provider_ops_json_object) else {
        return admin_provider_ops_verify_failure("响应格式无效");
    };

    let mut extra = Map::new();
    for (key, value) in user_data {
        if matches!(
            key.as_str(),
            "username" | "display_name" | "email" | "quota" | "used_quota" | "request_count"
        ) {
            continue;
        }
        extra.insert(key.clone(), value.clone());
    }

    admin_provider_ops_verify_success(
        admin_provider_ops_verify_user_payload_with_usage(
            user_data
                .get("username")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            user_data
                .get("display_name")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            user_data
                .get("email")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            admin_provider_ops_value_as_f64(user_data.get("quota")),
            admin_provider_ops_value_as_f64(user_data.get("used_quota")),
            admin_provider_ops_value_as_u64(user_data.get("request_count")),
            Some(extra),
        ),
        None,
    )
}

pub fn admin_provider_ops_sub2api_verify_payload(
    status: StatusCode,
    response_json: &Value,
    updated_credentials: Option<Map<String, Value>>,
) -> Value {
    if status == StatusCode::UNAUTHORIZED {
        return admin_provider_ops_verify_failure("认证失败：无效的凭据");
    }
    if status == StatusCode::FORBIDDEN {
        return admin_provider_ops_verify_failure("认证失败：权限不足");
    }
    if status != StatusCode::OK {
        return admin_provider_ops_verify_failure(format!("验证失败：HTTP {}", status.as_u16()));
    }

    let Some(payload) = admin_provider_ops_json_object(response_json) else {
        return admin_provider_ops_verify_failure("响应格式无效");
    };
    if payload.get("code").and_then(Value::as_i64).unwrap_or(-1) != 0 {
        return admin_provider_ops_verify_failure(
            payload
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("验证失败"),
        );
    }

    let Some(user_data) = payload.get("data").and_then(Value::as_object) else {
        return admin_provider_ops_verify_failure("响应格式无效");
    };
    let balance = admin_provider_ops_value_as_f64(user_data.get("balance")).unwrap_or(0.0);
    let points = admin_provider_ops_value_as_f64(user_data.get("points")).unwrap_or(0.0);
    let mut extra = Map::new();
    for key in ["balance", "points", "status", "concurrency"] {
        if let Some(value) = user_data.get(key) {
            extra.insert(key.to_string(), value.clone());
        }
    }

    let username_or_email = admin_provider_ops_sub2api_non_empty_string(user_data, "username")
        .or_else(|| admin_provider_ops_sub2api_non_empty_string(user_data, "email"));
    admin_provider_ops_verify_success(
        admin_provider_ops_verify_user_payload(
            username_or_email.clone(),
            username_or_email,
            admin_provider_ops_sub2api_non_empty_string(user_data, "email"),
            Some(balance + points),
            Some(extra),
        ),
        updated_credentials,
    )
}

fn admin_provider_ops_sub2api_non_empty_string(
    map: &Map<String, Value>,
    key: &str,
) -> Option<String> {
    map.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{
        admin_provider_ops_frontend_updated_credentials, admin_provider_ops_sub2api_verify_payload,
        admin_provider_ops_usage_api_verify_payload, admin_provider_ops_verify_headers,
    };
    use http::StatusCode;
    use serde_json::{json, Map, Value};

    #[test]
    fn frontend_updated_credentials_omits_internal_runtime_fields() {
        let filtered = admin_provider_ops_frontend_updated_credentials(Map::from_iter([
            ("refresh_token".to_string(), json!("refresh-token")),
            ("_cached_access_token".to_string(), json!("access-token")),
            ("_cached_token_expires_at".to_string(), json!(123456.0)),
            ("password".to_string(), Value::Null),
        ]));

        assert_eq!(
            filtered,
            Some(Map::from_iter([(
                "refresh_token".to_string(),
                json!("refresh-token")
            )]))
        );
    }

    #[test]
    fn sub2api_verify_payload_sums_balance_and_points() {
        let payload = admin_provider_ops_sub2api_verify_payload(
            StatusCode::OK,
            &json!({
                "code": 0,
                "data": {
                    "username": "sub2api-user",
                    "email": "sub2api@example.com",
                    "balance": 8.5,
                    "points": 1.5,
                    "status": "active",
                    "concurrency": 4
                }
            }),
            Some(Map::from_iter([(
                "refresh_token".to_string(),
                json!("refresh-token-new"),
            )])),
        );

        assert_eq!(payload["success"], json!(true));
        assert_eq!(payload["data"]["username"], json!("sub2api-user"));
        assert_eq!(payload["data"]["quota"], json!(10.0));
        assert_eq!(payload["data"]["extra"]["balance"], json!(8.5));
        assert_eq!(payload["data"]["extra"]["points"], json!(1.5));
        assert_eq!(payload["data"]["extra"]["status"], json!("active"));
        assert_eq!(payload["data"]["extra"]["concurrency"], json!(4));
        assert_eq!(
            payload["updated_credentials"],
            json!({ "refresh_token": "refresh-token-new" })
        );
    }

    #[test]
    fn sub2api_verify_payload_falls_back_to_email_when_username_is_null() {
        let payload = admin_provider_ops_sub2api_verify_payload(
            StatusCode::OK,
            &json!({
                "code": 0,
                "data": {
                    "username": null,
                    "email": "user@example.com",
                    "balance": 2.0,
                    "points": 0.0
                }
            }),
            None,
        );

        assert_eq!(payload["success"], json!(true));
        assert_eq!(payload["data"]["username"], json!("user@example.com"));
        assert_eq!(payload["data"]["display_name"], json!("user@example.com"));
        assert_eq!(payload["data"]["email"], json!("user@example.com"));
    }

    #[test]
    fn usage_api_headers_use_bearer_api_key() {
        let headers = admin_provider_ops_verify_headers(
            "usage_api",
            &Map::new(),
            &Map::from_iter([("api_key".to_string(), json!("example-api-key"))]),
        )
        .expect("headers should build");

        assert_eq!(
            headers
                .get(reqwest::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer example-api-key")
        );
    }

    #[test]
    fn usage_api_verify_payload_reads_remaining_and_plan() {
        let payload = admin_provider_ops_usage_api_verify_payload(
            StatusCode::OK,
            &json!({
                "remaining": 42.5,
                "balance": 42.5,
                "unit": "USD",
                "isValid": true,
                "planName": "Example Plan"
            }),
        );

        assert_eq!(payload["success"], json!(true));
        assert_eq!(payload["data"]["username"], json!("Example Plan"));
        assert_eq!(payload["data"]["quota"], json!(42.5));
        assert_eq!(payload["data"]["extra"]["unit"], json!("USD"));
        assert_eq!(payload["data"]["extra"]["is_valid"], json!(true));
    }

    #[test]
    fn usage_api_verify_payload_rejects_inactive_key() {
        let payload = admin_provider_ops_usage_api_verify_payload(
            StatusCode::OK,
            &json!({
                "remaining": 0,
                "isValid": false
            }),
        );

        assert_eq!(payload["success"], json!(false));
        assert_eq!(payload["message"], json!("API Key 无效或已停用"));
    }
}
