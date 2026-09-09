use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct Secrets {
    version: u32,
    pub jwt: String,
    pub encryption: String,
}

impl Secrets {
    fn generate() -> Self {
        Self {
            version: 1,
            jwt: random_key(),
            encryption: random_key(),
        }
    }

    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let secrets: Self = serde_json::from_slice(bytes)
            .map_err(|_| "钥匙串中的 Aether 密钥格式不正确，请恢复原密钥".to_string())?;
        if secrets.version != 1 || secrets.jwt.len() < 32 || secrets.encryption.len() < 32 {
            return Err("钥匙串中的 Aether 密钥不完整，请恢复原密钥".into());
        }
        Ok(secrets)
    }
}

pub(crate) fn random_key() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub fn service_name(data: &Path) -> String {
    let digest = Sha256::digest(data.to_string_lossy().as_bytes());
    format!(
        "com.aether.desktop.{}",
        digest[..12]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

/// Never replace missing keys for an existing database; it would make stored keys unreadable.
#[cfg(target_os = "macos")]
pub fn load_or_create(data: &Path, database_exists: bool) -> Result<Secrets, String> {
    use security_framework::passwords::{get_generic_password, set_generic_password};
    let service = service_name(data);
    match get_generic_password(&service, "gateway-secrets") {
        Ok(bytes) => Secrets::decode(&bytes),
        Err(error) if error.code() == -25300 => {
            if database_exists {
                return Err("检测到已有数据库，但钥匙串中缺少原加密密钥。请恢复钥匙串，或在新数据目录导入逻辑备份；不会替换原密钥。".into());
            }
            let secrets = Secrets::generate();
            let bytes = serde_json::to_vec(&secrets).map_err(|_| "密钥编码失败".to_string())?;
            set_generic_password(&service, "gateway-secrets", &bytes).map_err(keychain_error)?;
            Ok(secrets)
        }
        Err(error) => Err(keychain_error(error)),
    }
}

#[cfg(target_os = "macos")]
fn keychain_error(error: security_framework::base::Error) -> String {
    format!(
        "无法访问 Aether 钥匙串（{}）。请解锁登录钥匙串并允许本应用访问后重试。",
        error.code()
    )
}

#[cfg(not(target_os = "macos"))]
pub fn load_or_create(_data: &Path, _database_exists: bool) -> Result<Secrets, String> {
    Err("此版本的凭据存储仅支持 macOS".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_directories_have_separate_keychain_services() {
        assert_ne!(
            service_name(Path::new("/qa/one")),
            service_name(Path::new("/qa/two"))
        );
        assert_eq!(
            service_name(Path::new("/qa/one")),
            service_name(Path::new("/qa/one"))
        );
    }

    #[test]
    fn rejects_invalid_keys_without_replacing_them() {
        assert!(Secrets::decode(b"{}").is_err());
        assert!(Secrets::decode(br#"{"version":1,"jwt":"short","encryption":"short"}"#).is_err());
        let generated = Secrets::generate();
        let restored = Secrets::decode(&serde_json::to_vec(&generated).unwrap()).unwrap();
        assert_eq!(restored.encryption, generated.encryption);
        assert_ne!(generated.jwt, generated.encryption);
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "Uses and removes a disposable macOS login-Keychain item"]
    fn keychain_roundtrip_preserves_keys_and_refuses_missing_or_corrupt_records() {
        use security_framework::{
            os::macos::keychain::SecKeychain,
            passwords::{delete_generic_password, get_generic_password, set_generic_password},
        };

        // Fail instead of opening an authentication dialog in unattended checks.
        let _interaction = SecKeychain::disable_user_interaction().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let data = directory.path().canonicalize().unwrap();
        let service = service_name(&data);
        assert_eq!(
            get_generic_password(&service, "gateway-secrets")
                .unwrap_err()
                .code(),
            -25300
        );

        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = delete_generic_password(&self.0, "gateway-secrets");
            }
        }
        let _cleanup = Cleanup(service.clone());
        let created = load_or_create(&data, false).unwrap();
        let restored = load_or_create(&data, true).unwrap();
        assert!(created.jwt == restored.jwt && created.encryption == restored.encryption);

        delete_generic_password(&service, "gateway-secrets").unwrap();
        assert!(load_or_create(&data, true)
            .err()
            .unwrap()
            .contains("缺少原加密密钥"));
        assert_eq!(
            get_generic_password(&service, "gateway-secrets")
                .unwrap_err()
                .code(),
            -25300
        );

        set_generic_password(&service, "gateway-secrets", b"invalid-record").unwrap();
        assert!(load_or_create(&data, true).is_err());
        assert_eq!(
            get_generic_password(&service, "gateway-secrets").unwrap(),
            b"invalid-record"
        );
        delete_generic_password(&service, "gateway-secrets").unwrap();
    }
}
