use super::{
    json_object, ProviderOpsActionSpec, ProviderOpsArchitectureSpec, ProviderOpsAuthSpec,
    ProviderOpsBalanceMode, ProviderOpsCheckinMode, ProviderOpsVerifyMode,
};
use serde_json::{json, Map, Value};

pub(super) fn spec() -> ProviderOpsArchitectureSpec {
    let access_token_schema = json!({
        "type": "object",
        "properties": {
            "base_url": {
                "type": "string",
                "title": "站点地址",
                "description": "API 基础地址"
            },
            "api_key": {
                "type": "string",
                "title": "访问令牌 (系统访问令牌)",
                "description": "New API 个人设置里生成的系统访问令牌，不是 sk- 开头的模型调用令牌",
                "x-sensitive": true,
                "x-input-type": "password"
            },
            "user_id": {
                "type": "string",
                "title": "用户 ID",
                "description": "账号的数字用户 ID，控制台个人设置页可见；上游强制校验 New-Api-User 请求头"
            }
        },
        "required": ["api_key", "user_id"],
        "x-auth-method": "bearer",
        "x-auth-type": "api_key",
        "x-currency": "USD",
        "x-field-groups": [
            { "fields": ["base_url"] },
            {
                "fields": ["api_key", "user_id"],
                "layout": "inline",
                "x-flex": {
                    "api_key": 3,
                    "user_id": 1
                }
            }
        ],
        "x-quota-divisor": 500000,
        "x-validation": [
            {
                "type": "required",
                "fields": ["api_key", "user_id"],
                "message": "请填写访问令牌和用户 ID"
            }
        ]
    });
    let cookie_schema = json!({
        "type": "object",
        "properties": {
            "base_url": {
                "type": "string",
                "title": "站点地址",
                "description": "API 基础地址"
            },
            "cookie": {
                "type": "string",
                "title": "Cookie",
                "description": "用于 Cookie 认证",
                "x-sensitive": true,
                "x-input-type": "password"
            },
            "user_id": {
                "type": "string",
                "title": "用户 ID",
                "description": "账号的数字用户 ID，控制台个人设置页可见；上游强制校验 New-Api-User 请求头"
            }
        },
        "required": ["cookie", "user_id"],
        "x-auth-method": "bearer",
        "x-auth-type": "cookie",
        "x-currency": "USD",
        "x-field-groups": [
            { "fields": ["base_url"] },
            {
                "fields": ["cookie"],
                "x-help": "从浏览器开发者工具复制完整 Cookie"
            },
            { "fields": ["user_id"] }
        ],
        "x-field-hooks": {
            "cookie": {
                "action": "parse_new_api_user_id",
                "target": "user_id"
            }
        },
        "x-quota-divisor": 500000,
        "x-validation": [
            {
                "type": "required",
                "fields": ["cookie", "user_id"],
                "message": "请填写 Cookie 和用户 ID"
            }
        ]
    });

    ProviderOpsArchitectureSpec {
        architecture_id: "new_api",
        display_name: "New API",
        description: "New API 风格中转站的预设配置",
        hidden: false,
        credentials_schema: access_token_schema.clone(),
        verify_endpoint: "/api/user/self",
        verify_mode: ProviderOpsVerifyMode::DirectGet,
        balance_mode: ProviderOpsBalanceMode::SingleRequest,
        checkin_mode: ProviderOpsCheckinMode::NewApiCompatible,
        query_balance_cookie_auth_errors: false,
        supported_auth_types: vec![
            ProviderOpsAuthSpec {
                auth_type: "api_key",
                display_name: "访问令牌",
                credentials_schema: access_token_schema,
            },
            ProviderOpsAuthSpec {
                auth_type: "cookie",
                display_name: "Cookie",
                credentials_schema: cookie_schema,
            },
        ],
        supported_actions: vec![ProviderOpsActionSpec {
            action_type: "query_balance",
            display_name: "查询余额",
            description: "查询 New API 账户余额信息",
            config_schema: json!({
                "type": "object",
                "properties": {
                    "endpoint": {
                        "type": "string",
                        "title": "API 路径",
                        "description": "余额查询 API 路径",
                        "default": "/api/user/self"
                    },
                    "method": {
                        "type": "string",
                        "title": "请求方法",
                        "enum": ["GET", "POST"],
                        "default": "GET"
                    },
                    "quota_divisor": {
                        "type": "number",
                        "title": "额度除数",
                        "description": "将原始额度值转换为美元的除数",
                        "default": 500000
                    },
                    "currency": {
                        "type": "string",
                        "title": "货币单位",
                        "default": "USD"
                    }
                },
                "required": []
            }),
        }],
        default_connector: Some("api_key"),
    }
}

pub(super) fn default_action_config(action_type: &str) -> Option<Map<String, Value>> {
    match action_type {
        "query_balance" => Some(json_object(json!({
            "endpoint": "/api/user/self",
            "method": "GET",
            "quota_divisor": 500000,
            "checkin_endpoint": "/api/user/checkin",
            "currency": "USD"
        }))),
        "checkin" => Some(json_object(json!({
            "endpoint": "/api/user/checkin",
            "method": "POST"
        }))),
        _ => None,
    }
}
