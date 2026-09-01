use std::collections::BTreeMap;

use aether_contracts::{ExecutionPlan, ExecutionResult, ProxySnapshot, RequestBody};
use aether_provider_transport::auth::{
    ensure_upstream_auth_header, resolve_local_gemini_auth, resolve_local_openai_bearer_auth,
    resolve_local_standard_auth,
};
use aether_provider_transport::{
    apply_local_header_rules, resolve_transport_execution_timeouts, resolve_transport_profile,
    GatewayProviderTransportSnapshot,
};
use async_trait::async_trait;
use serde_json::json;

use crate::{build_models_fetch_url, deepseek_anthropic_models_fetch_uses_openai_auth};

const CLAUDE_VERSION_HEADER: &str = "2023-06-01";

const BROWSER_FINGERPRINT_HEADERS: &[(&str, &str)] = &[
    (
        "user-agent",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.7339.249 Electron/38.7.0 Safari/537.36",
    ),
    ("accept", "application/json"),
    ("accept-encoding", "gzip, deflate, br"),
    ("accept-language", "zh-CN"),
    ("sec-ch-ua", "\"Not=A?Brand\";v=\"24\", \"Chromium\";v=\"140\""),
    ("sec-ch-ua-mobile", "?0"),
    ("sec-ch-ua-platform", "\"macOS\""),
    ("sec-fetch-site", "cross-site"),
    ("sec-fetch-mode", "cors"),
    ("sec-fetch-dest", "empty"),
];

#[async_trait]
pub trait ModelFetchTransportRuntime: Send + Sync {
    async fn resolve_model_fetch_proxy(
        &self,
        transport: &GatewayProviderTransportSnapshot,
    ) -> Option<ProxySnapshot>;

    async fn execute_model_fetch_execution_plan(
        &self,
        plan: &ExecutionPlan,
    ) -> Result<ExecutionResult, String>;
}

pub async fn build_models_fetch_execution_plan(
    runtime: &(impl ModelFetchTransportRuntime + ?Sized),
    transport: &GatewayProviderTransportSnapshot,
) -> Result<ExecutionPlan, String> {
    build_standard_models_fetch_execution_plan(runtime, transport, None).await
}

struct ModelFetchExecutionPlanRequest {
    method: String,
    url: String,
    headers: BTreeMap<String, String>,
    content_type: Option<String>,
    body: RequestBody,
    client_api_format: String,
    provider_api_format: String,
    model_name: Option<String>,
}

pub async fn build_standard_models_fetch_execution_plan(
    runtime: &(impl ModelFetchTransportRuntime + ?Sized),
    transport: &GatewayProviderTransportSnapshot,
    after_id: Option<&str>,
) -> Result<ExecutionPlan, String> {
    let api_format = transport.endpoint.api_format.trim().to_ascii_lowercase();
    let provider_api_format = api_format.clone();
    let is_deepseek_anthropic_models_fetch = api_format.starts_with("claude:")
        && deepseek_anthropic_models_fetch_uses_openai_auth(&transport.endpoint.base_url);
    let mut headers = standard_models_fetch_headers(&api_format);
    if is_deepseek_anthropic_models_fetch {
        headers.remove("anthropic-version");
        headers.insert("accept".to_string(), "application/json".to_string());
    }
    let mut protected_headers = Vec::<String>::new();

    if api_format.starts_with("openai:") || api_format.starts_with("claude:") {
        let resolved_auth = if is_deepseek_anthropic_models_fetch {
            resolve_local_openai_bearer_auth(transport)
        } else {
            resolve_standard_header_auth(transport)
        };
        let (auth_header_name, auth_header_value) = resolved_auth.ok_or_else(|| {
            "Rust models fetch auth resolution is not supported for this key".to_string()
        })?;
        insert_non_empty_auth_header(
            &mut headers,
            &mut protected_headers,
            &auth_header_name,
            &auth_header_value,
        );
        headers = apply_fetch_header_rules(transport, headers, &protected_headers)?;
        ensure_upstream_auth_header(&mut headers, &auth_header_name, &auth_header_value);
    } else {
        headers = apply_fetch_header_rules(transport, headers, &protected_headers)?;
    }

    let upstream_url = build_standard_models_fetch_url(transport, after_id)?;
    build_execution_plan(
        runtime,
        transport,
        ModelFetchExecutionPlanRequest {
            method: "GET".to_string(),
            url: upstream_url,
            headers,
            content_type: None,
            body: RequestBody {
                json_body: None,
                body_bytes_b64: None,
                body_ref: None,
            },
            client_api_format: provider_api_format.clone(),
            provider_api_format,
            model_name: Some("models".to_string()),
        },
    )
    .await
}

async fn build_execution_plan(
    runtime: &(impl ModelFetchTransportRuntime + ?Sized),
    transport: &GatewayProviderTransportSnapshot,
    request: ModelFetchExecutionPlanRequest,
) -> Result<ExecutionPlan, String> {
    let ModelFetchExecutionPlanRequest {
        method,
        url,
        headers,
        content_type,
        body,
        client_api_format,
        provider_api_format,
        model_name,
    } = request;

    let transport_profile = resolve_transport_profile(transport);

    Ok(ExecutionPlan {
        request_id: format!(
            "req-model-fetch-{}-{}",
            transport.key.id,
            provider_api_format.replace(':', "-")
        ),
        candidate_id: None,
        provider_name: Some(transport.provider.name.clone()),
        provider_id: transport.provider.id.clone(),
        endpoint_id: transport.endpoint.id.clone(),
        key_id: transport.key.id.clone(),
        method,
        url,
        headers,
        content_type,
        content_encoding: None,
        body,
        stream: false,
        client_api_format,
        provider_api_format,
        model_name,
        proxy: runtime.resolve_model_fetch_proxy(transport).await,
        transport_profile,
        timeouts: resolve_transport_execution_timeouts(transport),
    })
}

fn resolve_standard_header_auth(
    transport: &GatewayProviderTransportSnapshot,
) -> Option<(String, String)> {
    let api_format = transport.endpoint.api_format.trim().to_ascii_lowercase();
    if api_format.starts_with("openai:") {
        return resolve_local_openai_bearer_auth(transport);
    }
    if api_format.starts_with("claude:") {
        return resolve_local_standard_auth(transport);
    }
    None
}

fn apply_fetch_header_rules(
    transport: &GatewayProviderTransportSnapshot,
    mut headers: BTreeMap<String, String>,
    protected_headers: &[String],
) -> Result<BTreeMap<String, String>, String> {
    let protected = protected_headers
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if !apply_local_header_rules(
        &mut headers,
        transport.endpoint.header_rules.as_ref(),
        &protected,
        &json!({}),
        None,
    ) {
        return Err("Endpoint header_rules application failed".to_string());
    }
    Ok(headers)
}

fn standard_models_fetch_headers(api_format: &str) -> BTreeMap<String, String> {
    let api_format = aether_ai_formats::normalize_api_format_alias(api_format);
    match api_format.as_str() {
        "claude:messages" => BTreeMap::from([(
            "anthropic-version".to_string(),
            CLAUDE_VERSION_HEADER.to_string(),
        )]),
        "gemini:generate_content" => BROWSER_FINGERPRINT_HEADERS
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
        _ => BTreeMap::new(),
    }
}

fn build_standard_models_fetch_url(
    transport: &GatewayProviderTransportSnapshot,
    after_id: Option<&str>,
) -> Result<String, String> {
    let api_format = transport.endpoint.api_format.trim().to_ascii_lowercase();
    if api_format.starts_with("gemini:") {
        let secret = resolve_local_gemini_auth(transport)
            .and_then(|(name, value)| {
                name.eq_ignore_ascii_case("x-goog-api-key").then_some(value)
            })
            .or_else(|| {
                let secret = transport.key.decrypted_api_key.trim();
                (!secret.is_empty()).then_some(secret.to_string())
            })
            .ok_or_else(|| "Gemini models fetch requires an API key".to_string())?;

        let (url, _) = build_models_fetch_url(
            &transport.endpoint.api_format,
            &transport.endpoint.base_url,
        )
        .ok_or_else(|| "Rust models fetch does not support this provider format yet".to_string())?;
        return Ok(append_query_param(url, "key", &secret));
    }

    let (mut url, _) =
        build_models_fetch_url(&transport.endpoint.api_format, &transport.endpoint.base_url)
            .ok_or_else(|| "Rust models fetch does not support this provider format yet".to_string())?;

    if api_format.starts_with("claude:")
        && !deepseek_anthropic_models_fetch_uses_openai_auth(&transport.endpoint.base_url)
    {
        url = append_query_param(url, "limit", "100");
        if let Some(after_id) = after_id.map(str::trim).filter(|value| !value.is_empty()) {
            url = append_query_param(url, "after_id", after_id);
        }
    }

    Ok(url)
}

fn append_query_param(mut url: String, key: &str, value: &str) -> String {
    if key.trim().is_empty() || value.trim().is_empty() {
        return url;
    }
    let separator = if url.contains('?') { '&' } else { '?' };
    url.push(separator);
    url.push_str(key.trim());
    url.push('=');
    url.push_str(value.trim());
    url
}

fn insert_non_empty_auth_header(
    headers: &mut BTreeMap<String, String>,
    protected_headers: &mut Vec<String>,
    name: &str,
    value: &str,
) {
    let name = name.trim();
    let value = value.trim();
    if name.is_empty() || value.is_empty() {
        return;
    }

    protected_headers.push(name.to_string());
    headers.insert(name.to_string(), value.to_string());
}

#[cfg(test)]
mod tests {
    use aether_contracts::{ExecutionPlan, ExecutionResult, ProxySnapshot};
    use aether_provider_transport::snapshot::{
        GatewayProviderTransportEndpoint, GatewayProviderTransportKey,
        GatewayProviderTransportProvider, GatewayProviderTransportSnapshot,
    };
    use async_trait::async_trait;

    use super::{
        build_models_fetch_execution_plan, build_standard_models_fetch_execution_plan,
        ModelFetchTransportRuntime,
    };

    struct TestRuntime {
        proxy: Option<ProxySnapshot>,
    }

    #[async_trait]
    impl ModelFetchTransportRuntime for TestRuntime {
        async fn resolve_model_fetch_proxy(
            &self,
            _transport: &GatewayProviderTransportSnapshot,
        ) -> Option<ProxySnapshot> {
            self.proxy.clone()
        }

        async fn execute_model_fetch_execution_plan(
            &self,
            _plan: &ExecutionPlan,
        ) -> Result<ExecutionResult, String> {
            unreachable!("tests only validate plan construction")
        }
    }

    fn sample_transport(api_format: &str, auth_type: &str) -> GatewayProviderTransportSnapshot {
        GatewayProviderTransportSnapshot {
            provider: GatewayProviderTransportProvider {
                id: "provider-1".to_string(),
                name: "Provider One".to_string(),
                provider_type: "custom".to_string(),
                website: None,
                is_active: true,
                enable_format_conversion: false,
                concurrent_limit: None,
                max_retries: None,
                proxy: None,
                request_timeout_secs: Some(30.0),
                stream_first_byte_timeout_secs: Some(5.0),
                config: None,
            },
            endpoint: GatewayProviderTransportEndpoint {
                id: "endpoint-1".to_string(),
                provider_id: "provider-1".to_string(),
                api_format: api_format.to_string(),
                api_family: None,
                endpoint_kind: None,
                is_active: true,
                base_url: "https://example.com".to_string(),
                header_rules: None,
                body_rules: None,
                max_retries: None,
                custom_path: None,
                config: None,
                format_acceptance_config: None,
                proxy: None,
            },
            key: GatewayProviderTransportKey {
                id: "key-1".to_string(),
                provider_id: "provider-1".to_string(),
                name: "key".to_string(),
                auth_type: auth_type.to_string(),
                is_active: true,
                api_formats: Some(vec![api_format.to_string()]),
                auth_type_by_format: None,
                allow_auth_channel_mismatch_formats: None,
                allowed_models: None,
                capabilities: None,
                rate_multipliers: None,
                expires_at_unix_secs: None,
                proxy: None,
                fingerprint: None,
                upstream_metadata: None,
                decrypted_api_key: "secret".to_string(),
                decrypted_auth_config: None,
            },
        }
    }

    #[tokio::test]
    async fn builds_openai_models_fetch_plan_with_bearer_authorization() {
        let runtime = TestRuntime { proxy: None };
        let transport = sample_transport("openai:chat", "api_key");
        let plan = build_models_fetch_execution_plan(&runtime, &transport)
            .await
            .expect("plan");

        assert_eq!(plan.url, "https://example.com/models");
        assert_eq!(
            plan.headers.get("authorization").map(String::as_str),
            Some("Bearer secret")
        );
    }

    #[tokio::test]
    async fn builds_bigmodel_coding_models_fetch_plan() {
        let runtime = TestRuntime { proxy: None };
        let mut transport = sample_transport("openai:chat", "api_key");
        transport.endpoint.base_url = "https://open.bigmodel.cn/api/coding/paas/v4".to_string();
        let plan = build_models_fetch_execution_plan(&runtime, &transport)
            .await
            .expect("plan");

        assert_eq!(
            plan.url,
            "https://open.bigmodel.cn/api/coding/paas/v4/models"
        );
        assert_eq!(
            plan.headers.get("authorization").map(String::as_str),
            Some("Bearer secret")
        );
    }

    #[tokio::test]
    async fn builds_claude_models_fetch_plan_with_pagination() {
        let runtime = TestRuntime { proxy: None };
        let transport = sample_transport("claude:messages", "api_key");
        let plan =
            build_standard_models_fetch_execution_plan(&runtime, &transport, Some("cursor-1"))
                .await
                .expect("plan");

        assert_eq!(
            plan.url,
            "https://example.com/models?limit=100&after_id=cursor-1"
        );
        assert_eq!(
            plan.headers.get("anthropic-version").map(String::as_str),
            Some("2023-06-01")
        );
        assert_eq!(
            plan.headers.get("x-api-key").map(String::as_str),
            Some("secret")
        );
    }

    #[tokio::test]
    async fn builds_deepseek_anthropic_models_fetch_plan_with_openai_models_endpoint() {
        let runtime = TestRuntime { proxy: None };
        let mut transport = sample_transport("claude:messages", "api_key");
        transport.endpoint.base_url = "https://api.deepseek.com/anthropic".to_string();
        let plan = build_models_fetch_execution_plan(&runtime, &transport)
            .await
            .expect("plan");

        assert_eq!(plan.url, "https://api.deepseek.com/models");
        assert_eq!(
            plan.headers.get("authorization").map(String::as_str),
            Some("Bearer secret")
        );
        assert!(!plan.headers.contains_key("x-api-key"));
        assert!(!plan.headers.contains_key("anthropic-version"));
    }

    #[tokio::test]
    async fn builds_gemini_models_fetch_plan_with_browser_headers_and_query_auth() {
        let runtime = TestRuntime { proxy: None };
        let transport = sample_transport("gemini:generate_content", "api_key");
        let plan = build_models_fetch_execution_plan(&runtime, &transport)
            .await
            .expect("plan");

        assert_eq!(plan.url, "https://example.com/v1beta/models?key=secret");
        assert!(plan.headers.contains_key("sec-ch-ua"));
        assert_eq!(
            plan.headers.get("accept-language").map(String::as_str),
            Some("zh-CN")
        );
    }
}
