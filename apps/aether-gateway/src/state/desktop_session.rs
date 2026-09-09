//! The desktop host may exchange its process-local capability for a normal user session.

use std::fmt;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use aether_data::repository::users::{StoredUserAuthRecord, UserExportListQuery};
use aether_data_contracts::repository::global_models::AdminGlobalModelListQuery;
use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::{AppState, GatewayError};

const DESKTOP_SESSION_SECRET_ENV: &str = "AETHER_DESKTOP_SESSION_SECRET";
const DESKTOP_SESSION_HEADER: &str = "x-aether-desktop-session";
const DESKTOP_LOCAL_USERNAME: &str = "aether-local";
const DESKTOP_CAPABILITY_CONTEXT: &[u8] = b"aether-desktop-session-v1";

/// Validated explicitly by the gateway binary only when `--desktop-mode` is set.
#[derive(Clone)]
pub struct DesktopSessionConfig {
    authority: String,
    origin: String,
    capability_tag: [u8; 32],
}

impl fmt::Debug for DesktopSessionConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DesktopSessionConfig")
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}

impl DesktopSessionConfig {
    pub fn from_env(bind_addr: SocketAddr, exit_on_stdin_close: bool) -> io::Result<Self> {
        let secret = std::env::var(DESKTOP_SESSION_SECRET_ENV).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "桌面模式缺少 AETHER_DESKTOP_SESSION_SECRET",
            )
        })?;
        Self::new(&secret, bind_addr, exit_on_stdin_close)
    }

    pub fn new(secret: &str, bind_addr: SocketAddr, exit_on_stdin_close: bool) -> io::Result<Self> {
        if bind_addr.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST) || bind_addr.port() == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "桌面模式必须绑定 127.0.0.1 和明确的非零端口",
            ));
        }
        if !exit_on_stdin_close {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "桌面模式必须启用 --exit-on-stdin-close",
            ));
        }
        if !valid_capability_format(secret.as_bytes()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "AETHER_DESKTOP_SESSION_SECRET 必须为 64 位十六进制临时密钥",
            ));
        }

        let authority = bind_addr.to_string();
        Ok(Self {
            origin: format!("http://{authority}"),
            authority,
            // Retain only a verifier in application state, never the raw capability.
            capability_tag: capability_mac(secret.as_bytes())
                .finalize()
                .into_bytes()
                .into(),
        })
    }
}

#[derive(Debug)]
pub(crate) struct DesktopSession {
    config: DesktopSessionConfig,
    pub(crate) user_id: String,
}

impl DesktopSession {
    pub(crate) fn matches_origin(&self, headers: &HeaderMap) -> bool {
        single_header(headers, "host") == Some(self.config.authority.as_bytes())
            && single_header(headers, "origin") == Some(self.config.origin.as_bytes())
    }

    pub(crate) fn authorizes_capability(&self, headers: &HeaderMap) -> bool {
        let Some(capability) = single_header(headers, DESKTOP_SESSION_HEADER) else {
            return false;
        };
        valid_capability_format(capability)
            // RustCrypto's verify_slice compares the fixed-size MAC in constant time.
            && capability_mac(capability)
                .verify_slice(&self.config.capability_tag)
                .is_ok()
    }
}

fn valid_capability_format(capability: &[u8]) -> bool {
    capability.len() == 64 && capability.iter().all(u8::is_ascii_hexdigit)
}

fn capability_mac(capability: &[u8]) -> Hmac<Sha256> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(capability).expect("HMAC accepts a key of any length");
    mac.update(DESKTOP_CAPABILITY_CONTEXT);
    mac
}

fn single_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a [u8]> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?;
    if values.next().is_some() {
        return None;
    }
    Some(value.as_bytes())
}

pub(crate) fn is_usable_desktop_admin(user: &StoredUserAuthRecord) -> bool {
    user.role.eq_ignore_ascii_case("admin")
        && user.auth_source.eq_ignore_ascii_case("local")
        && user.is_active
        && !user.is_deleted
}

impl AppState {
    /// Bootstrap only an empty identity store, or reuse its unambiguous local administrator.
    /// This runs before the listener accepts requests and does not change existing credentials.
    pub async fn with_desktop_session(mut self, config: DesktopSessionConfig) -> io::Result<Self> {
        let user = self
            .prepare_desktop_identity()
            .await
            .map_err(|error| io::Error::other(format!("桌面本机身份准备失败: {error:?}")))?;
        self.desktop_session = Some(Arc::new(DesktopSession {
            config,
            user_id: user.id,
        }));
        Ok(self)
    }

    async fn prepare_desktop_identity(&self) -> Result<StoredUserAuthRecord, GatewayError> {
        if !self.has_auth_user_write_capability() || !self.has_auth_wallet_write_capability() {
            return Err(GatewayError::Internal(
                "桌面模式需要可持久化的本机用户与钱包数据库".to_string(),
            ));
        }

        let admin_ids = self.desktop_admin_ids().await?;
        if admin_ids.len() > 1 {
            return Err(GatewayError::Internal(
                "本机数据中存在多个管理员，无法确定桌面身份；请先明确保留的本机管理员".to_string(),
            ));
        }
        if let Some(user_id) = admin_ids.first() {
            let user = self.find_user_auth_by_id(user_id).await?.ok_or_else(|| {
                GatewayError::Internal("本机管理员记录已变更，请重新检查本机数据".to_string())
            })?;
            if !is_usable_desktop_admin(&user) {
                return Err(GatewayError::Internal(
                    "本机管理员已禁用或不是本地账号，无法建立桌面会话".to_string(),
                ));
            }
            return Ok(user);
        }

        // Export queries omit deleted users. Consult the existing SQL-backed total,
        // which includes them, so a database with deleted identities is not treated as new.
        if self.desktop_database_has_identity_data().await? {
            return Err(GatewayError::Internal(
                "已有本机数据缺少唯一有效的本地管理员，不能自动创建或提升账号权限".to_string(),
            ));
        }

        // This random password is never exposed or retained. Normal Web deployments
        // still require their usual password login and never use this bootstrap path.
        let password = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|error| GatewayError::Internal(format!("本机管理员凭据生成失败: {error}")))?;
        self.create_local_auth_user_with_settings(
            None,
            true,
            DESKTOP_LOCAL_USERNAME.to_string(),
            password_hash,
            "admin".to_string(),
            None,
            None,
            None,
            None,
        )
        .await?
        .ok_or_else(|| GatewayError::Internal("本机管理员存储不可用".to_string()))
    }

    async fn desktop_admin_ids(&self) -> Result<Vec<String>, GatewayError> {
        #[cfg(test)]
        if let Some(store) = self.auth_user_store.as_ref() {
            return Ok(store
                .lock()
                .expect("auth user store should lock")
                .values()
                .filter(|user| user.role.eq_ignore_ascii_case("admin") && !user.is_deleted)
                .take(2)
                .map(|user| user.id.clone())
                .collect());
        }

        Ok(self
            .list_export_users_page(&UserExportListQuery {
                role: Some("admin".to_string()),
                limit: 2,
                ..UserExportListQuery::default()
            })
            .await?
            .into_iter()
            .map(|user| user.id)
            .collect())
    }

    async fn desktop_database_has_identity_data(&self) -> Result<bool, GatewayError> {
        #[cfg(test)]
        if let Some(store) = self.auth_user_store.as_ref() {
            if !store
                .lock()
                .expect("auth user store should lock")
                .is_empty()
            {
                return Ok(true);
            }
        }

        let stats = self.read_admin_system_stats().await?;
        if stats.total_users > 0 || stats.total_api_keys > 0 || stats.total_requests > 0 {
            return Ok(true);
        }
        if !self
            .list_provider_catalog_providers(false)
            .await?
            .is_empty()
        {
            return Ok(true);
        }
        let models = self
            .list_admin_global_models(&AdminGlobalModelListQuery {
                limit: 1,
                ..AdminGlobalModelListQuery::default()
            })
            .await?;
        // Migration-created defaults in system_configs/routing_groups are expected
        // in a fresh database; providers and global models are user-owned data.
        Ok(models.total > 0 || !models.items.is_empty())
    }
}

#[cfg(test)]
#[path = "desktop_session/tests.rs"]
mod tests;
