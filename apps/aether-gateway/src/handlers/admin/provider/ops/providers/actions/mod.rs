mod checkin;
mod multiplier;
mod multiplier_upstream;
mod query_balance;
mod responses;
mod support;

use super::config::{
    admin_provider_ops_config_object, admin_provider_ops_connector_object,
    admin_provider_ops_decrypted_credentials, resolve_admin_provider_ops_base_url,
};
use super::support::ADMIN_PROVIDER_OPS_ACTION_RUST_ONLY_MESSAGE;
use super::verify::admin_provider_ops_resolve_proxy_snapshot;
use crate::handlers::admin::request::AdminAppState;
use aether_admin::provider::ops::{
    build_headers, get_architecture, normalize_architecture_id, resolve_action_config,
};
use aether_data_contracts::repository::provider_catalog::{
    StoredProviderCatalogEndpoint, StoredProviderCatalogProvider,
};

pub(super) fn admin_provider_ops_is_valid_action_type(action_type: &str) -> bool {
    matches!(
        action_type,
        "query_balance"
            | "sync_multiplier"
            | "checkin"
            | "claim_quota"
            | "refresh_token"
            | "get_usage"
            | "get_models"
            | "custom"
    )
}

/// 执行供应商操作；支持倍率查询的认证模板在余额刷新后附带独立同步结果。
async fn run_action(
    state: &AdminAppState<'_>,
    provider_id: &str,
    provider: Option<&StoredProviderCatalogProvider>,
    endpoints: &[StoredProviderCatalogEndpoint],
    action_type: &str,
    request_config: Option<&serde_json::Map<String, serde_json::Value>>,
) -> serde_json::Value {
    let Some(provider) = provider else {
        return responses::admin_provider_ops_action_not_configured(
            action_type,
            "请先配置供应商用户认证，再开启或同步跟随上游倍率",
        );
    };
    // 切回手动不依赖已删除的用户认证，并与倍率值一起原子保存。
    if action_type == "sync_multiplier"
        && request_config
            .and_then(|v| v.get("mode"))
            .and_then(serde_json::Value::as_str)
            == Some("manual")
    {
        return multiplier::sync_multiplier(
            state,
            provider,
            "",
            &serde_json::Map::new(),
            None,
            request_config,
        )
        .await;
    }
    let Some(provider_ops_config) = admin_provider_ops_config_object(provider) else {
        return responses::admin_provider_ops_action_not_configured(
            action_type,
            "请先配置供应商用户认证，再开启或同步跟随上游倍率",
        );
    };
    let architecture_id = provider_ops_config
        .get("architecture_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("generic_api");
    let architecture_id = normalize_architecture_id(architecture_id);
    let Some(architecture) = get_architecture(architecture_id) else {
        return responses::admin_provider_ops_action_not_supported(
            action_type,
            ADMIN_PROVIDER_OPS_ACTION_RUST_ONLY_MESSAGE,
        );
    };
    let Some(base_url) =
        resolve_admin_provider_ops_base_url(provider, endpoints, Some(provider_ops_config))
    else {
        return responses::admin_provider_ops_action_not_configured(
            action_type,
            "Provider 未配置 base_url",
        );
    };

    let connector_config = admin_provider_ops_connector_object(provider_ops_config)
        .and_then(|connector| connector.get("config"))
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    let proxy_snapshot =
        admin_provider_ops_resolve_proxy_snapshot(state, Some(&connector_config)).await;

    let credentials = admin_provider_ops_decrypted_credentials(
        state,
        admin_provider_ops_config_object(provider)
            .and_then(admin_provider_ops_connector_object)
            .and_then(|connector| connector.get("credentials")),
    );
    if action_type == "sync_multiplier" {
        return multiplier::sync_multiplier(
            state,
            provider,
            &base_url,
            &credentials,
            proxy_snapshot.as_ref(),
            request_config,
        )
        .await;
    }
    let headers = match build_headers(
        architecture.architecture_id,
        &connector_config,
        &credentials,
    ) {
        Ok(headers) => headers,
        Err(message) => {
            return responses::admin_provider_ops_action_not_configured(action_type, message);
        }
    };
    let Some(action_config) = resolve_action_config(
        architecture_id,
        provider_ops_config,
        action_type,
        request_config,
    ) else {
        return responses::admin_provider_ops_action_not_supported(
            action_type,
            ADMIN_PROVIDER_OPS_ACTION_RUST_ONLY_MESSAGE,
        );
    };

    match action_type {
        "query_balance" => {
            let payload = query_balance::admin_provider_ops_run_query_balance_action(
                state,
                provider_id,
                provider,
                &architecture,
                &base_url,
                &action_config,
                &headers,
                &credentials,
                proxy_snapshot.as_ref(),
            )
            .await;
            payload
        }
        "checkin" => {
            let has_cookie = ["cookie", "session_cookie"].into_iter().any(|key| {
                credentials
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty())
            });
            checkin::admin_provider_ops_run_checkin_action(
                state,
                &base_url,
                &architecture,
                &action_config,
                &headers,
                has_cookie,
                proxy_snapshot.as_ref(),
            )
            .await
        }
        _ => responses::admin_provider_ops_action_not_supported(
            action_type,
            ADMIN_PROVIDER_OPS_ACTION_RUST_ONLY_MESSAGE,
        ),
    }
}

/// 同轮并发启动余额与倍率，分别超时和返回结果，不让一个查询失败取消另一个。
pub(crate) async fn admin_provider_ops_local_action_response(
    state: &AdminAppState<'_>,
    provider_id: &str,
    provider: Option<&StoredProviderCatalogProvider>,
    endpoints: &[StoredProviderCatalogEndpoint],
    action_type: &str,
    request_config: Option<&serde_json::Map<String, serde_json::Value>>,
) -> serde_json::Value {
    if action_type != "query_balance" {
        return run_action(
            state,
            provider_id,
            provider,
            endpoints,
            action_type,
            request_config,
        )
        .await;
    }
    // 同轮查询共用已续期的凭据，避免两个并发请求轮换同一 Refresh Token。
    let mut prepared = provider.cloned();
    let mut preparation_error = None;
    if let Some(current) = prepared.as_mut() {
        if let Some(ops) = admin_provider_ops_config_object(current) {
            if ops
                .get("architecture_id")
                .and_then(serde_json::Value::as_str)
                == Some("sub2api")
            {
                let credentials = admin_provider_ops_decrypted_credentials(
                    state,
                    admin_provider_ops_connector_object(ops).and_then(|v| v.get("credentials")),
                );
                if let Some(base) =
                    resolve_admin_provider_ops_base_url(current, endpoints, Some(ops))
                {
                    let connector = admin_provider_ops_connector_object(ops)
                        .and_then(|v| v.get("config"))
                        .and_then(serde_json::Value::as_object)
                        .cloned()
                        .unwrap_or_default();
                    let proxy =
                        admin_provider_ops_resolve_proxy_snapshot(state, Some(&connector)).await;
                    let exchange = tokio::time::timeout(
                        std::time::Duration::from_secs(20),
                        super::verify::admin_provider_ops_sub2api_exchange_token(
                            state,
                            &base,
                            &credentials,
                            proxy.as_ref(),
                        ),
                    )
                    .await;
                    match exchange {
                        Ok(Ok((_, updated, _))) if !updated.is_empty() => {
                            match super::config::persist_admin_provider_ops_runtime_credentials(
                                state, current, &updated,
                            )
                            .await
                            {
                                Ok(Some(saved)) => *current = saved,
                                Ok(None) => {
                                    preparation_error =
                                        Some(("unknown_error", "用户认证配置已变化，请重新查询"))
                                }
                                Err(_) => {
                                    preparation_error =
                                        Some(("unknown_error", "账号续期凭据保存失败，请稍后重试"))
                                }
                            }
                        }
                        Ok(Ok(_)) => {}
                        Ok(Err(_)) => {
                            preparation_error = Some((
                                "auth_failed",
                                "用户认证失效或暂时无法验证，请检查用户认证配置",
                            ))
                        }
                        Err(_) => {
                            preparation_error = Some(("timeout", "用户认证验证超时，请稍后重试"))
                        }
                    }
                }
            }
        }
    }
    // 共享认证准备失败时结束本轮，避免两个分支再次轮换同一个旧令牌。
    if let Some((status, message)) = preparation_error {
        let mut balance =
            responses::admin_provider_ops_action_error(status, "query_balance", message, None);
        super::balance_cache::store_admin_provider_ops_balance_cache(state, provider_id, &balance)
            .await;
        if let Some(object) = balance.as_object_mut() {
            object.insert(
                "multiplier_sync".to_string(),
                responses::admin_provider_ops_action_error(
                    status,
                    "sync_multiplier",
                    message,
                    None,
                ),
            );
        }
        return balance;
    }
    let provider = prepared.as_ref();
    let balance = async {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            run_action(
                state,
                provider_id,
                provider,
                endpoints,
                "query_balance",
                request_config,
            ),
        )
        .await
        .unwrap_or_else(|_| {
            responses::admin_provider_ops_action_error(
                "timeout",
                "query_balance",
                "余额查询超时",
                None,
            )
        });
        // 先发布余额结果，慢倍率查询不能延迟余额缓存更新。
        super::balance_cache::store_admin_provider_ops_balance_cache(state, provider_id, &result)
            .await;
        result
    };
    // 倍率适配器分别限制各请求的时间预算，不用整批超时取消其他密钥。
    let multiplier = run_action(
        state,
        provider_id,
        provider,
        endpoints,
        "sync_multiplier",
        None,
    );
    let (mut payload, sync) = tokio::join!(balance, multiplier);
    if let Some(object) = payload.as_object_mut() {
        object.insert("multiplier_sync".to_string(), sync);
    }
    payload
}
