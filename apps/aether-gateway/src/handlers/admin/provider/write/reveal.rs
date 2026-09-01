use crate::handlers::admin::request::AdminAppState;
use crate::provider_key_auth::provider_key_auth_semantics;
use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey;
use serde_json::json;

fn normalize_reveal_auth_type(value: &str) -> &str {
    match value.trim().to_ascii_lowercase().as_str() {
        "service_account" | "vertex_ai" => "service_account",
        "bearer" => "bearer",
        _ => "api_key",
    }
}

pub(crate) fn build_admin_reveal_key_payload(
    state: &AdminAppState<'_>,
    key: &StoredProviderCatalogKey,
) -> Result<serde_json::Value, String> {
    let auth_type = normalize_reveal_auth_type(&key.auth_type);
    let _ = provider_key_auth_semantics(key);
    if matches!(auth_type, "service_account") {
        let parsed_auth_config = state.parse_catalog_auth_config_json(key);
        if let Some(auth_config) = parsed_auth_config {
            return Ok(json!({
                "auth_type": auth_type,
                "auth_config": auth_config,
            }));
        }
        let decrypted = key
            .encrypted_api_key
            .as_deref()
            .and_then(|ciphertext| state.decrypt_catalog_secret_with_fallbacks(ciphertext))
            .ok_or_else(|| {
                "无法解密认证配置，可能是加密密钥已更改。请重新添加该密钥。".to_string()
            })?;
        if decrypted == "__placeholder__" {
            return Err("认证配置丢失，请重新添加该密钥。".to_string());
        }
        return Ok(json!({
            "auth_type": auth_type,
            "auth_config": decrypted,
        }));
    }

    let decrypted = match key.encrypted_api_key.as_deref().map(str::trim) {
        Some(ciphertext) if !ciphertext.is_empty() => state
            .decrypt_catalog_secret_with_fallbacks(ciphertext)
            .ok_or_else(|| {
                "无法解密 API Key，可能是加密密钥已更改。请重新添加该密钥。".to_string()
            })?,
        _ => String::new(),
    };
    Ok(json!({
        "auth_type": auth_type,
        "api_key": decrypted,
    }))
}
