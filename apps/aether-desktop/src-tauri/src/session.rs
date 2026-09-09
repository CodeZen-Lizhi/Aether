use crate::secrets;
use uuid::Uuid;

pub struct DashboardConnection {
    pub generation: Uuid,
    pub port: u16,
    pub initialization_script: String,
}

/// Scoped to one managed gateway process. This is never a persistent account
/// password, a signing key, or a credential for the proxy API.
pub struct DesktopSession {
    secret: String,
    generation: Uuid,
}

impl DesktopSession {
    pub fn generate() -> Self {
        Self {
            secret: secrets::random_key(),
            generation: Uuid::new_v4(),
        }
    }

    pub fn secret(&self) -> &str {
        &self.secret
    }

    pub fn connection(&self, port: u16) -> Result<DashboardConnection, String> {
        let origin = serde_json::to_string(&format!("http://127.0.0.1:{port}"))
            .map_err(|_| "无法准备本机连接地址".to_string())?;
        let secret =
            serde_json::to_string(&self.secret).map_err(|_| "无法准备本机会话".to_string())?;
        let initialization_script = include_str!("session.js")
            .replace("__AETHER_SESSION_ORIGIN__", &origin)
            .replace("__AETHER_SESSION_SECRET__", &secret);
        Ok(DashboardConnection {
            generation: self.generation,
            port,
            initialization_script,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_child_gets_an_independent_in_memory_session() {
        let first = DesktopSession::generate();
        let second = DesktopSession::generate();
        assert_ne!(first.secret(), second.secret());
        assert_eq!(first.secret().len(), 64);
        assert!(first.secret().bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
}
