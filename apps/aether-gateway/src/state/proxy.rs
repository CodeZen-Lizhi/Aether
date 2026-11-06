use aether_contracts::ProxySnapshot;
use aether_data::repository::proxy_nodes::proxy_node_accepts_new_tunnels;
use base64::Engine;
use hyper_util::client::proxy::matcher::Matcher;
use serde_json::{json, Map, Value};

use super::AppState;
use crate::provider_transport::{GatewayProviderTransportSnapshot, TransportTunnelAffinityLookup};

const TUNNEL_BASE_URL_EXTRA_KEY: &str = "tunnel_base_url";
const TUNNEL_OWNER_INSTANCE_ID_EXTRA_KEY: &str = "tunnel_owner_instance_id";
const TUNNEL_OWNER_OBSERVED_AT_EXTRA_KEY: &str = "tunnel_owner_observed_at_unix_secs";

impl AppState {
    /// 按最终上游地址固定本次代理；继承环境代理遵循 NO_PROXY 规则，与请求记录共用快照。
    pub(crate) async fn resolve_request_proxy_snapshot(
        &self,
        transport: &GatewayProviderTransportSnapshot,
        upstream_url: &str,
    ) -> Option<ProxySnapshot> {
        let configured = self
            .resolve_transport_proxy_snapshot_with_tunnel_affinity(transport)
            .await;
        resolve_inherited_request_proxy(configured, upstream_url, &Matcher::from_env())
    }

    pub(crate) async fn read_system_proxy_node_id(&self) -> Option<String> {
        self.read_system_config_json_value("system_proxy_node_id")
            .await
            .ok()
            .flatten()
            .and_then(|value| value.as_str().map(str::trim).map(ToOwned::to_owned))
            .filter(|value| !value.is_empty())
    }

    pub(crate) async fn resolve_proxy_node_snapshot(
        &self,
        node_id: Option<&str>,
    ) -> Option<ProxySnapshot> {
        let node_id = node_id.map(str::trim).filter(|value| !value.is_empty())?;
        let node = self.find_proxy_node(node_id).await.ok().flatten()?;
        if node.status.trim() != "online" {
            return None;
        }
        if !proxy_node_accepts_new_tunnels(&node) {
            return None;
        }
        if node.tunnel_mode && node.tunnel_connected {
            let mut extra = Map::new();
            let owner = self
                .lookup_tunnel_attachment_owner(node_id)
                .await
                .ok()
                .flatten();
            if let Some(owner) = owner {
                extra.insert(
                    TUNNEL_BASE_URL_EXTRA_KEY.to_string(),
                    Value::String(owner.relay_base_url),
                );
                extra.insert(
                    TUNNEL_OWNER_INSTANCE_ID_EXTRA_KEY.to_string(),
                    Value::String(owner.gateway_instance_id),
                );
                extra.insert(
                    TUNNEL_OWNER_OBSERVED_AT_EXTRA_KEY.to_string(),
                    json!(owner.observed_at_unix_secs),
                );
            } else if !self.tunnel.has_local_proxy(node_id) {
                return None;
            }
            return Some(ProxySnapshot {
                enabled: Some(true),
                mode: Some("tunnel".to_string()),
                node_id: Some(node.id),
                label: Some(node.name),
                url: None,
                extra: if extra.is_empty() {
                    None
                } else {
                    Some(Value::Object(extra))
                },
            });
        }
        if !node.is_manual {
            return None;
        }
        let proxy_url = node
            .proxy_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())?;
        Some(ProxySnapshot {
            enabled: Some(true),
            mode: proxy_mode_from_url(Some(proxy_url)),
            node_id: Some(node.id),
            label: Some(node.name),
            url: proxy_url_with_node_auth(
                proxy_url,
                node.proxy_username.as_deref(),
                node.proxy_password.as_deref(),
            )
            .or_else(|| Some(proxy_url.to_string())),
            extra: None,
        })
    }

    pub(crate) async fn resolve_system_proxy_snapshot(&self) -> Option<ProxySnapshot> {
        let node_id = self.read_system_proxy_node_id().await;
        self.resolve_proxy_node_snapshot(node_id.as_deref()).await
    }

    pub(crate) async fn resolve_transport_proxy_snapshot_with_tunnel_affinity(
        &self,
        transport: &GatewayProviderTransportSnapshot,
    ) -> Option<ProxySnapshot> {
        self.resolve_transport_proxy_with_source_with_tunnel_affinity(transport)
            .await
            .map(|(snapshot, _)| snapshot)
    }

    pub(crate) async fn resolve_transport_proxy_source_with_tunnel_affinity(
        &self,
        transport: &GatewayProviderTransportSnapshot,
    ) -> Option<&'static str> {
        self.resolve_transport_proxy_with_source_with_tunnel_affinity(transport)
            .await
            .map(|(_, source)| source)
    }

    pub(crate) async fn resolve_configured_proxy_snapshot_with_tunnel_affinity(
        &self,
        raw: Option<&Value>,
    ) -> Option<ProxySnapshot> {
        let object = raw?.as_object()?;
        if !proxy_enabled(object) {
            return None;
        }

        let node_id = json_string_field(object, "node_id");
        if let Some(snapshot) = self.resolve_proxy_node_snapshot(node_id.as_deref()).await {
            return Some(snapshot);
        }
        if let Some(node_id) = node_id.as_deref() {
            if !proxy_object_has_inline_url(object)
                && self.find_proxy_node(node_id).await.ok().flatten().is_some()
            {
                return None;
            }
        }

        proxy_snapshot_from_object(object)
    }

    async fn resolve_transport_proxy_with_source_with_tunnel_affinity(
        &self,
        transport: &GatewayProviderTransportSnapshot,
    ) -> Option<(ProxySnapshot, &'static str)> {
        if let Some(snapshot) = self
            .resolve_configured_proxy_snapshot_with_tunnel_affinity(transport.key.proxy.as_ref())
            .await
        {
            return Some((snapshot, "key"));
        }
        if let Some(snapshot) = self
            .resolve_configured_proxy_snapshot_with_tunnel_affinity(
                transport.endpoint.proxy.as_ref(),
            )
            .await
        {
            return Some((snapshot, "endpoint"));
        }
        if let Some(snapshot) = self
            .resolve_configured_proxy_snapshot_with_tunnel_affinity(
                transport.provider.proxy.as_ref(),
            )
            .await
        {
            return Some((snapshot, "provider"));
        }
        self.resolve_system_proxy_snapshot()
            .await
            .map(|snapshot| (snapshot, "system"))
    }
}

/// 显式代理优先；无匹配时固定为直连，避免客户端稍后再次读取环境改变实际路径。
fn resolve_inherited_request_proxy(
    configured: Option<ProxySnapshot>,
    upstream_url: &str,
    matcher: &Matcher,
) -> Option<ProxySnapshot> {
    if configured.is_some() {
        return configured;
    }
    let mut target = url::Url::parse(upstream_url).ok()?;
    match target.scheme() {
        "ws" => {
            target.set_scheme("http").ok()?;
        }
        "wss" => {
            target.set_scheme("https").ok()?;
        }
        _ => {}
    }
    let uri = target.as_str().parse::<http::Uri>().ok()?;
    let Some(intercept) = matcher.intercept(&uri) else {
        return Some(ProxySnapshot {
            enabled: Some(false),
            ..ProxySnapshot::default()
        });
    };
    let mut proxy_url = url::Url::parse(&intercept.uri().to_string()).ok()?;
    // Matcher 将认证从地址分离；还原给既有客户端，记录层仍负责去掉凭据。
    let decoded_basic = intercept
        .basic_auth()
        .and_then(|header| header.to_str().ok())
        .and_then(|value| value.strip_prefix("Basic "))
        .and_then(|value| base64::engine::general_purpose::STANDARD.decode(value).ok())
        .and_then(|value| String::from_utf8(value).ok());
    let auth = intercept.raw_auth().or_else(|| {
        decoded_basic
            .as_deref()
            .and_then(|value| value.split_once(':'))
    });
    if let Some((username, password)) = auth {
        proxy_url.set_username(&username.replace('%', "%25")).ok()?;
        proxy_url
            .set_password(Some(&password.replace('%', "%25")))
            .ok()?;
    }
    Some(ProxySnapshot {
        enabled: Some(true),
        mode: proxy_mode_from_url(Some(proxy_url.as_str())),
        label: Some("继承环境代理".to_string()),
        url: Some(proxy_url.to_string()),
        extra: Some(json!({"source": "inherited"})),
        ..ProxySnapshot::default()
    })
}

fn proxy_enabled(object: &Map<String, Value>) -> bool {
    object
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn proxy_snapshot_from_object(object: &Map<String, Value>) -> Option<ProxySnapshot> {
    let mode = json_string_field(object, "mode");
    let node_id = json_string_field(object, "node_id");
    let label = json_string_field(object, "label");
    let url = json_string_field(object, "url").or_else(|| json_string_field(object, "proxy_url"));

    if node_id.is_none() && url.is_none() {
        return None;
    }

    let mut extra = Map::new();
    for (key, value) in object {
        if matches!(
            key.as_str(),
            "enabled" | "mode" | "node_id" | "label" | "url" | "proxy_url"
        ) {
            continue;
        }
        extra.insert(key.clone(), value.clone());
    }

    Some(ProxySnapshot {
        enabled: object.get("enabled").and_then(Value::as_bool),
        mode,
        node_id,
        label,
        url,
        extra: if extra.is_empty() {
            None
        } else {
            Some(Value::Object(extra))
        },
    })
}

fn proxy_object_has_inline_url(object: &Map<String, Value>) -> bool {
    json_string_field(object, "url").is_some() || json_string_field(object, "proxy_url").is_some()
}

fn json_string_field(object: &Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn proxy_mode_from_url(proxy_url: Option<&str>) -> Option<String> {
    let proxy_url = proxy_url?.trim();
    if proxy_url.is_empty() {
        return None;
    }
    let scheme = url::Url::parse(proxy_url)
        .ok()
        .map(|value| value.scheme().to_ascii_lowercase())
        .unwrap_or_default();
    if scheme.starts_with("socks") {
        Some("socks".to_string())
    } else {
        Some("http".to_string())
    }
}

fn proxy_url_with_node_auth(
    proxy_url: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Option<String> {
    let username = username.map(str::trim).filter(|value| !value.is_empty())?;
    let mut parsed = url::Url::parse(proxy_url).ok()?;
    if parsed.set_username(username).is_err() {
        return None;
    }
    let password = password.map(str::trim).filter(|value| !value.is_empty());
    if parsed.set_password(password).is_err() {
        return None;
    }
    Some(parsed.to_string())
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use std::sync::Arc;

    use aether_data::repository::proxy_nodes::{InMemoryProxyNodeRepository, StoredProxyNode};
    use serde_json::json;

    use super::{proxy_url_with_node_auth, resolve_inherited_request_proxy};
    use crate::{data::GatewayDataState, AppState};

    #[test]
    /// 继承代理按协议和 NO_PROXY 选择，显式代理保持优先且记录不泄露认证。
    fn inherited_request_proxy_preserves_selection_auth_and_trace() {
        let password = format!("p{}ss", '%');
        let matcher = super::Matcher::builder()
            .http(format!("http://alice:{password}@127.0.0.1:8080"))
            .https(format!("socks5h://bob:{password}@127.0.0.1:1080"))
            .no("localhost,192.168.0.0/16")
            .build();
        let proxy =
            resolve_inherited_request_proxy(None, "http://provider.invalid/v1", &matcher).unwrap();
        let trace =
            aether_provider_transport::build_request_trace_proxy_value(None, Some(&proxy)).unwrap();
        assert_eq!(trace["url"], "http://127.0.0.1:8080");
        assert_eq!(trace["source"], "inherited");
        assert_eq!(trace["node_name"], "继承环境代理");
        assert!(!trace.to_string().contains("alice"));
        assert!(!trace.to_string().contains("p%25ss"));
        assert!(proxy.url.as_deref().unwrap().contains("alice:p%25ss@"));
        let secure =
            resolve_inherited_request_proxy(None, "wss://provider.invalid/v1", &matcher).unwrap();
        assert!(secure.url.as_deref().unwrap().contains("bob:p%25ss@"));
        for target in ["http://localhost/v1", "https://192.168.1.2/v1"] {
            let direct = resolve_inherited_request_proxy(None, target, &matcher).unwrap();
            assert_eq!(direct.enabled, Some(false));
            assert!(aether_provider_transport::build_request_trace_proxy_value(
                None,
                Some(&direct)
            )
            .is_none());
        }
        assert_eq!(
            resolve_inherited_request_proxy(Some(proxy.clone()), "http://localhost/v1", &matcher),
            Some(proxy)
        );
    }

    #[tokio::test]
    /// 通过真实传输执行继承代理，核对收到的认证以及请求记录中的代理地址一致。
    async fn inherited_request_proxy_reaches_recorded_proxy_through_real_transport() {
        use aether_contracts::{ExecutionPlan, ExecutionTimeouts, RequestBody};
        use std::collections::BTreeMap;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let password = format!("p{}ss", '%');
        let matcher = super::Matcher::builder()
            .all(format!("http://alice:{password}@{addr}"))
            .build();
        let proxy = resolve_inherited_request_proxy(
            None,
            "http://provider.invalid/v1/chat/completions",
            &matcher,
        )
        .unwrap();
        let trace =
            aether_provider_transport::build_request_trace_proxy_value(None, Some(&proxy)).unwrap();
        assert_eq!(trace["url"], format!("http://{addr}"));
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"\r\n\r\n") {
                assert!(bytes.len() < 16 * 1024);
                bytes.push(socket.read_u8().await.unwrap());
            }
            let headers = String::from_utf8(bytes).unwrap();
            assert!(
                headers.starts_with("POST http://provider.invalid/v1/chat/completions HTTP/1.1")
            );
            let expected_auth = base64::engine::general_purpose::STANDARD.encode("alice:p%ss");
            assert!(headers.lines().any(|line| line
                .to_ascii_lowercase()
                .starts_with("proxy-authorization:")
                && line.ends_with(&expected_auth)));
            let body_len = headers
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                .unwrap();
            let mut request_body = vec![0; body_len];
            socket.read_exact(&mut request_body).await.unwrap();
            let body = br#"{"id":"proxy-test","choices":[{"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}]}"#;
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes()).await.unwrap();
            socket.write_all(body).await.unwrap();
        });
        let plan = ExecutionPlan {
            request_id: "inherited-proxy-test".into(),
            candidate_id: None,
            provider_name: None,
            provider_id: "provider".into(),
            endpoint_id: "endpoint".into(),
            key_id: "key".into(),
            method: "POST".into(),
            url: "http://provider.invalid/v1/chat/completions".into(),
            headers: BTreeMap::new(),
            content_type: Some("application/json".into()),
            content_encoding: None,
            body: RequestBody::from_json(json!({"model":"fixture","messages":[]})),
            stream: false,
            client_api_format: "openai:chat".into(),
            provider_api_format: "openai:chat".into(),
            model_name: None,
            proxy: Some(proxy),
            transport_profile: None,
            timeouts: Some(ExecutionTimeouts {
                connect_ms: Some(2000),
                total_ms: Some(3000),
                ..ExecutionTimeouts::default()
            }),
        };
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            crate::execution_runtime::transport::DirectSyncExecutionRuntime::new()
                .execute_sync(&plan),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result.status_code, 200);
        server.await.unwrap();
    }

    #[test]
    fn proxy_url_with_node_auth_omits_empty_password_separator() {
        assert_eq!(
            proxy_url_with_node_auth("socks5://proxy.example:1080", Some("alice"), None).as_deref(),
            Some("socks5://alice@proxy.example:1080")
        );
    }

    #[tokio::test]
    async fn resolve_proxy_node_snapshot_rejects_unroutable_tunnel_node() {
        let repository = Arc::new(InMemoryProxyNodeRepository::seed(vec![sample_tunnel_node(
            "proxy-node-stale",
        )]));
        let state = AppState::new()
            .expect("state should build")
            .with_data_state_for_tests(GatewayDataState::with_proxy_node_repository_for_tests(
                repository,
            ));

        let snapshot = state
            .resolve_proxy_node_snapshot(Some("proxy-node-stale"))
            .await;

        assert_eq!(snapshot, None);
    }

    #[tokio::test]
    async fn resolve_proxy_node_snapshot_keeps_tunnel_node_with_owner_hint() {
        let repository = Arc::new(InMemoryProxyNodeRepository::seed(vec![sample_tunnel_node(
            "proxy-node-owned",
        )]));
        let state = AppState::new()
            .expect("state should build")
            .with_data_state_for_tests(
                GatewayDataState::with_proxy_node_repository_for_tests(repository)
                    .with_system_config_values_for_tests(vec![(
                        "tunnel.attachments.proxy-node-owned".to_string(),
                        json!({
                            "gateway_instance_id": "gateway-owner",
                            "relay_base_url": "http://gateway-owner.internal",
                            "conn_count": 1,
                            "observed_at_unix_secs": 4_102_444_800u64,
                        }),
                    )]),
            );

        let snapshot = state
            .resolve_proxy_node_snapshot(Some("proxy-node-owned"))
            .await
            .expect("owned tunnel snapshot should resolve");

        assert_eq!(snapshot.mode.as_deref(), Some("tunnel"));
        assert_eq!(snapshot.node_id.as_deref(), Some("proxy-node-owned"));
        assert_eq!(
            snapshot
                .extra
                .as_ref()
                .and_then(|extra| extra.get("tunnel_base_url"))
                .and_then(serde_json::Value::as_str),
            Some("http://gateway-owner.internal")
        );
    }

    #[tokio::test]
    async fn resolve_configured_proxy_snapshot_rejects_unroutable_stored_tunnel_reference() {
        let repository = Arc::new(InMemoryProxyNodeRepository::seed(vec![sample_tunnel_node(
            "proxy-node-stale",
        )]));
        let state = AppState::new()
            .expect("state should build")
            .with_data_state_for_tests(GatewayDataState::with_proxy_node_repository_for_tests(
                repository,
            ));

        let snapshot = state
            .resolve_configured_proxy_snapshot_with_tunnel_affinity(Some(&json!({
                "node_id": "proxy-node-stale",
                "enabled": true,
            })))
            .await;

        assert_eq!(snapshot, None);
    }

    #[tokio::test]
    async fn resolve_configured_proxy_snapshot_keeps_inline_url_when_stored_node_is_unroutable() {
        let repository = Arc::new(InMemoryProxyNodeRepository::seed(vec![sample_tunnel_node(
            "proxy-node-stale",
        )]));
        let state = AppState::new()
            .expect("state should build")
            .with_data_state_for_tests(GatewayDataState::with_proxy_node_repository_for_tests(
                repository,
            ));

        let snapshot = state
            .resolve_configured_proxy_snapshot_with_tunnel_affinity(Some(&json!({
                "node_id": "proxy-node-stale",
                "url": "http://proxy.example:8080",
                "enabled": true,
            })))
            .await
            .expect("inline proxy URL should still resolve");

        assert_eq!(snapshot.node_id.as_deref(), Some("proxy-node-stale"));
        assert_eq!(snapshot.url.as_deref(), Some("http://proxy.example:8080"));
    }

    fn sample_tunnel_node(id: &str) -> StoredProxyNode {
        StoredProxyNode::new(
            id.to_string(),
            id.to_string(),
            "127.0.0.1".to_string(),
            0,
            false,
            "online".to_string(),
            15,
            1,
            0,
            0,
            0,
            0,
            true,
            true,
            1,
        )
        .expect("sample tunnel node should build")
    }
}
