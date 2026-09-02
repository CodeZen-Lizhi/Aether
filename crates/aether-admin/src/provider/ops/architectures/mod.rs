mod generic_api;
mod new_api;
mod sub2api;
mod usage_api;

use serde_json::{json, Map, Value};
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderOpsVerifyMode {
    DirectGet,
    Sub2ApiExchange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderOpsBalanceMode {
    SingleRequest,
    Sub2ApiDualRequest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderOpsCheckinMode {
    None,
    NewApiCompatible,
}

#[derive(Clone, Debug)]
pub struct ProviderOpsAuthSpec {
    pub auth_type: &'static str,
    pub display_name: &'static str,
    pub credentials_schema: Value,
}

#[derive(Clone, Debug)]
pub struct ProviderOpsActionSpec {
    pub action_type: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub config_schema: Value,
}

#[derive(Clone, Debug)]
pub struct ProviderOpsArchitectureSpec {
    pub architecture_id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub hidden: bool,
    pub credentials_schema: Value,
    pub verify_endpoint: &'static str,
    pub verify_mode: ProviderOpsVerifyMode,
    pub balance_mode: ProviderOpsBalanceMode,
    pub checkin_mode: ProviderOpsCheckinMode,
    pub query_balance_cookie_auth_errors: bool,
    pub supported_auth_types: Vec<ProviderOpsAuthSpec>,
    pub supported_actions: Vec<ProviderOpsActionSpec>,
    pub default_connector: Option<&'static str>,
}

impl ProviderOpsArchitectureSpec {
    pub fn api_payload(&self) -> Value {
        json!({
            "architecture_id": self.architecture_id,
            "display_name": self.display_name,
            "description": self.description,
            "credentials_schema": self.credentials_schema,
            "supported_auth_types": self.supported_auth_types.iter().map(|item| {
                json!({
                    "type": item.auth_type,
                    "display_name": item.display_name,
                    "credentials_schema": item.credentials_schema,
                })
            }).collect::<Vec<_>>(),
            "supported_actions": self.supported_actions.iter().map(|item| {
                json!({
                    "type": item.action_type,
                    "display_name": item.display_name,
                    "description": item.description,
                    "config_schema": item.config_schema,
                })
            }).collect::<Vec<_>>(),
            "default_connector": self.default_connector,
        })
    }
}

static PROVIDER_OPS_ARCHITECTURES: LazyLock<Vec<ProviderOpsArchitectureSpec>> =
    LazyLock::new(|| {
        vec![
            generic_api::spec(),
            new_api::spec(),
            sub2api::spec(),
            usage_api::spec(),
        ]
    });

pub fn list_architectures(include_hidden: bool) -> Vec<ProviderOpsArchitectureSpec> {
    PROVIDER_OPS_ARCHITECTURES
        .iter()
        .filter(|spec| include_hidden || !spec.hidden)
        .cloned()
        .collect()
}

pub fn get_architecture(architecture_id: &str) -> Option<ProviderOpsArchitectureSpec> {
    let normalized = normalize_architecture_id(architecture_id);
    PROVIDER_OPS_ARCHITECTURES
        .iter()
        .find(|spec| spec.architecture_id == normalized)
        .cloned()
}

pub fn normalize_architecture_id(architecture_id: &str) -> &'static str {
    match architecture_id.trim() {
        "" => "generic_api",
        "generic_api" => "generic_api",
        "new_api" => "new_api",
        "sub2api" => "sub2api",
        "usage_api" => "usage_api",
        _ => "generic_api",
    }
}

pub fn admin_provider_ops_is_supported_auth_type(auth_type: &str) -> bool {
    matches!(
        auth_type,
        "api_key" | "session_login" | "oauth" | "cookie" | "none"
    )
}

pub fn resolve_action_config(
    architecture_id: &str,
    provider_ops_config: &Map<String, Value>,
    action_type: &str,
    request_override: Option<&Map<String, Value>>,
) -> Option<Map<String, Value>> {
    let mut resolved =
        default_action_config(normalize_architecture_id(architecture_id), action_type)?;

    if let Some(saved) = provider_action_config_object(provider_ops_config, action_type) {
        for (key, value) in saved {
            resolved.insert(key.clone(), value.clone());
        }
    }

    if let Some(request_override) = request_override {
        for (key, value) in request_override {
            resolved.insert(key.clone(), value.clone());
        }
    }

    Some(resolved)
}

fn provider_action_config_object<'a>(
    provider_ops_config: &'a Map<String, Value>,
    action_type: &str,
) -> Option<&'a Map<String, Value>> {
    provider_ops_config
        .get("actions")
        .and_then(Value::as_object)
        .and_then(|actions| actions.get(action_type))
        .and_then(Value::as_object)
        .and_then(|action| action.get("config"))
        .and_then(Value::as_object)
}

fn default_action_config(architecture_id: &str, action_type: &str) -> Option<Map<String, Value>> {
    match architecture_id {
        "generic_api" => generic_api::default_action_config(action_type),
        "new_api" => new_api::default_action_config(action_type),
        "sub2api" => sub2api::default_action_config(action_type),
        "usage_api" => usage_api::default_action_config(action_type),
        _ => None,
    }
}

pub(super) fn json_object(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        get_architecture, list_architectures, normalize_architecture_id, resolve_action_config,
    };
    use serde_json::{json, Value};

    #[test]
    fn list_architectures_keeps_only_supported_presets_visible() {
        let visible = list_architectures(false);
        assert_eq!(visible.len(), 3);
        assert_eq!(
            visible
                .iter()
                .map(|item| item.architecture_id)
                .collect::<Vec<_>>(),
            vec!["new_api", "sub2api", "usage_api"]
        );

        let all = list_architectures(true);
        assert_eq!(all.len(), 4);
        assert!(all.iter().any(|item| item.architecture_id == "generic_api"));
    }

    #[test]
    fn normalize_architecture_id_falls_back_to_generic_api_for_removed_presets() {
        assert_eq!(normalize_architecture_id(""), "generic_api");
        assert_eq!(normalize_architecture_id("new_api"), "new_api");
        assert_eq!(normalize_architecture_id("usage_api"), "usage_api");
        assert_eq!(normalize_architecture_id("anyrouter"), "generic_api");
        assert_eq!(normalize_architecture_id("yescode"), "generic_api");
        assert_eq!(normalize_architecture_id("unknown"), "generic_api");
    }

    #[test]
    fn get_architecture_returns_generic_api_for_unknown_id() {
        let architecture = get_architecture("unknown").expect("architecture should exist");
        assert_eq!(architecture.architecture_id, "generic_api");
        assert!(architecture.hidden);
    }

    #[test]
    fn new_api_splits_auth_types_into_access_token_and_cookie() {
        let architecture = get_architecture("new_api").expect("architecture should exist");
        assert_eq!(
            architecture
                .supported_auth_types
                .iter()
                .map(|item| item.auth_type)
                .collect::<Vec<_>>(),
            vec!["api_key", "cookie"]
        );
        assert_eq!(architecture.default_connector, Some("api_key"));
        assert_eq!(
            architecture.credentials_schema.get("x-auth-type"),
            Some(&json!("api_key")),
            "顶层 credentials_schema 应指向默认项（访问令牌）"
        );

        let access_token = &architecture.supported_auth_types[0].credentials_schema;
        assert_eq!(access_token.get("x-auth-type"), Some(&json!("api_key")));
        assert_eq!(
            access_token.get("required"),
            Some(&json!(["api_key", "user_id"])),
        );
        assert!(
            access_token.pointer("/properties/cookie").is_none(),
            "访问令牌 schema 不应包含 cookie 字段"
        );
        assert_eq!(
            access_token.pointer("/x-validation"),
            Some(&json!([{
                "type": "required",
                "fields": ["api_key", "user_id"],
                "message": "请填写访问令牌和用户 ID"
            }])),
        );

        let cookie = &architecture.supported_auth_types[1].credentials_schema;
        assert_eq!(cookie.get("x-auth-type"), Some(&json!("cookie")));
        assert_eq!(cookie.get("required"), Some(&json!(["cookie", "user_id"])),);
        assert!(
            cookie.pointer("/properties/api_key").is_none(),
            "Cookie schema 不应包含 api_key 字段"
        );
        assert_eq!(
            cookie.pointer("/x-field-hooks/cookie"),
            Some(&json!({
                "action": "parse_new_api_user_id",
                "target": "user_id"
            })),
            "粘贴 Cookie 自动解析用户 ID 的 field hook 应保留"
        );
        assert_eq!(
            cookie.pointer("/x-validation"),
            Some(&json!([{
                "type": "required",
                "fields": ["cookie", "user_id"],
                "message": "请填写 Cookie 和用户 ID"
            }])),
        );
    }

    #[test]
    fn new_api_schemas_only_use_explicit_required_validation() {
        let architecture = get_architecture("new_api").expect("architecture should exist");
        for auth_type in &architecture.supported_auth_types {
            let validations = auth_type
                .credentials_schema
                .get("x-validation")
                .and_then(Value::as_array)
                .expect("x-validation should be array");
            assert!(
                !validations.is_empty(),
                "auth_type {} 应保留校验规则",
                auth_type.auth_type
            );
            for rule in validations {
                assert_eq!(
                    rule.get("type").and_then(Value::as_str),
                    Some("required"),
                    "auth_type {} 应使用显式 required 校验，不再是 any_required/conditional_required",
                    auth_type.auth_type
                );
            }
        }
    }

    #[test]
    fn resolve_action_config_merges_default_saved_and_request_values() {
        let resolved = resolve_action_config(
            "new_api",
            &json!({
                "actions": {
                    "query_balance": {
                        "config": {
                            "endpoint": "/custom/path",
                            "currency": "CNY"
                        }
                    }
                }
            })
            .as_object()
            .cloned()
            .expect("config should be object"),
            "query_balance",
            Some(
                &json!({
                    "quota_divisor": 42
                })
                .as_object()
                .cloned()
                .expect("override should be object"),
            ),
        )
        .expect("action config should resolve");

        assert_eq!(resolved.get("endpoint"), Some(&json!("/custom/path")));
        assert_eq!(resolved.get("currency"), Some(&json!("CNY")));
        assert_eq!(resolved.get("quota_divisor"), Some(&json!(42)));
        assert_eq!(resolved.get("method"), Some(&json!("GET")));
    }
}
