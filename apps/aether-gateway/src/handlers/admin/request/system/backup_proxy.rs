use std::collections::{BTreeMap, BTreeSet};

use aether_admin::system::{
    AdminImportMergeMode, AdminSystemConfigImportCounter, AdminSystemConfigProxyNode,
};
use aether_data::repository::proxy_nodes::StoredProxyNode;
use axum::http::StatusCode;
use serde_json::Value;

use super::import::invalid_request;
use super::AdminAppState;
use crate::GatewayError;

fn decode_node(item: AdminSystemConfigProxyNode) -> Result<StoredProxyNode, (StatusCode, Value)> {
    let required = |value: Option<String>, field: &str| {
        value
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| invalid_request(format!("代理节点 {field} 为必填字段")))
    };
    let id = required(item.id, "id")?;
    let name = required(item.name, "name")?;
    let ip = required(item.ip, "ip")?;
    let port = item
        .port
        .ok_or_else(|| invalid_request("代理节点 port 为必填字段"))?;
    let is_manual = item
        .is_manual
        .ok_or_else(|| invalid_request("代理节点 is_manual 为必填字段"))?;
    let tunnel_mode = item.tunnel_mode.unwrap_or(false);
    if !(0..=65_535).contains(&port) || (port == 0 && (is_manual || !tunnel_mode)) {
        return Err(invalid_request("代理节点端口无效"));
    }
    if is_manual {
        let url = item
            .proxy_url
            .as_deref()
            .ok_or_else(|| invalid_request("手动代理节点缺少 proxy_url"))?;
        let parsed = url::Url::parse(url).map_err(|_| invalid_request("代理节点 URL 无效"))?;
        if !matches!(parsed.scheme(), "http" | "https" | "socks5" | "socks5h")
            || parsed.host_str().is_none()
        {
            return Err(invalid_request("代理节点 URL 无效"));
        }
    }
    let heartbeat_interval = item.heartbeat_interval.unwrap_or(0);
    let config_version = item.config_version.unwrap_or(0);
    if heartbeat_interval < 0
        || config_version < 0
        || item
            .remote_config
            .as_ref()
            .is_some_and(|value| !value.is_object())
    {
        return Err(invalid_request("代理节点远程配置无效"));
    }
    let mut node = StoredProxyNode::new(
        id,
        name,
        ip,
        port,
        is_manual,
        if is_manual { "online" } else { "offline" }.to_string(),
        heartbeat_interval,
        0,
        0,
        0,
        0,
        0,
        tunnel_mode,
        false,
        config_version,
    )
    .map_err(|err| invalid_request(err.to_string()))?;
    node.region = item.region;
    node.proxy_url = item.proxy_url;
    node.proxy_username = item.proxy_username;
    node.proxy_password = item.proxy_password;
    node.remote_config = item.remote_config;
    Ok(node)
}

impl<'a> AdminAppState<'a> {
    pub(super) async fn import_admin_system_proxy_nodes(
        &self,
        imported: Vec<AdminSystemConfigProxyNode>,
        mode: AdminImportMergeMode,
        counter: &mut AdminSystemConfigImportCounter,
    ) -> Result<Result<BTreeMap<String, String>, (StatusCode, Value)>, GatewayError> {
        if !imported.is_empty() && !self.has_proxy_node_writer() {
            return Ok(Err(invalid_request("当前环境无法写入代理节点")));
        }
        let nodes = match imported
            .into_iter()
            .map(decode_node)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(nodes) => nodes,
            Err(err) => return Ok(Err(err)),
        };
        let mut existing = self.list_proxy_nodes().await?;
        let mut mapping = BTreeMap::new();
        let mut addresses = BTreeSet::new();
        let mut planned = Vec::new();
        for mut node in nodes {
            let source_id = node.id.clone();
            if mapping.contains_key(&source_id) {
                return Ok(Err(invalid_request("备份包含重复代理节点标识")));
            }
            if !addresses.insert((node.ip.clone(), node.port)) {
                return Ok(Err(invalid_request("备份包含重复代理节点地址")));
            }
            let matched = existing
                .iter()
                .find(|current| current.id == node.id)
                .or_else(|| {
                    existing
                        .iter()
                        .find(|current| current.ip == node.ip && current.port == node.port)
                });
            if let Some(current) = matched {
                if current.is_manual != node.is_manual {
                    return Ok(Err(invalid_request("代理节点地址对应不同类型的节点")));
                }
                mapping.insert(source_id, current.id.clone());
                match mode {
                    AdminImportMergeMode::Skip => {
                        counter.skipped += 1;
                        continue;
                    }
                    AdminImportMergeMode::Error => {
                        return Ok(Err(invalid_request(format!(
                            "代理节点 '{}' 已存在",
                            current.name
                        ))))
                    }
                    AdminImportMergeMode::Overwrite => {}
                }
                if existing.iter().any(|other| {
                    other.id != current.id && other.ip == node.ip && other.port == node.port
                }) {
                    return Ok(Err(invalid_request("代理节点地址已被其他节点占用")));
                }
                let mut restored = current.clone();
                restored.name = node.name;
                restored.ip = node.ip;
                restored.port = node.port;
                restored.region = node.region;
                restored.proxy_url = node.proxy_url;
                restored.proxy_username = node.proxy_username;
                restored.proxy_password = node.proxy_password;
                restored.heartbeat_interval = node.heartbeat_interval;
                restored.tunnel_mode = node.tunnel_mode;
                if restored.remote_config != node.remote_config {
                    restored.config_version = restored
                        .config_version
                        .max(node.config_version)
                        .saturating_add(1);
                }
                restored.remote_config = node.remote_config;
                node = restored;
                counter.updated += 1;
            } else {
                mapping.insert(source_id, node.id.clone());
                counter.created += 1;
            }
            existing.retain(|current| current.id != node.id);
            existing.push(node.clone());
            planned.push(node);
        }
        for node in planned {
            if !self.app().restore_proxy_node(&node).await? {
                return Ok(Err(invalid_request("代理节点未能写入，导入已中止")));
            }
        }
        Ok(Ok(mapping))
    }
}
