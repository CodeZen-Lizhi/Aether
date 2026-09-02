use crate::handlers::admin::request::AdminAppState;
use crate::handlers::admin::shared::decrypt_catalog_secret_with_fallbacks;
use crate::handlers::shared::{
    api_key_placeholder_display, generate_gateway_api_key_plaintext, masked_gateway_api_key_display,
};

pub(crate) fn format_optional_unix_secs_iso8601(value: Option<u64>) -> Option<String> {
    let secs = value?;
    let secs = i64::try_from(secs).ok()?;
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0).map(|value| value.to_rfc3339())
}

pub(crate) fn masked_user_api_key_display(
    state: &AdminAppState<'_>,
    ciphertext: Option<&str>,
) -> String {
    let Some(ciphertext) = ciphertext.map(str::trim).filter(|value| !value.is_empty()) else {
        return api_key_placeholder_display();
    };
    let Some(full_key) = decrypt_catalog_secret_with_fallbacks(state.encryption_key(), ciphertext)
    else {
        return api_key_placeholder_display();
    };
    masked_gateway_api_key_display(Some(full_key.as_str()))
}

pub(crate) fn normalize_admin_optional_api_key_name(
    value: Option<String>,
) -> Result<Option<String>, String> {
    match value {
        None => Ok(None),
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err("API密钥名称不能为空".to_string());
            }
            Ok(Some(trimmed.chars().take(100).collect()))
        }
    }
}

pub(crate) fn generate_admin_user_api_key_plaintext() -> String {
    generate_gateway_api_key_plaintext()
}

pub(crate) fn hash_admin_user_api_key(value: &str) -> String {
    use sha2::Digest;

    let mut hasher = sha2::Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(crate) fn default_admin_user_api_key_name() -> String {
    format!("API Key {}", chrono::Utc::now().format("%Y%m%d%H%M%S"))
}
