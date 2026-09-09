use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_PORT: u16 = 8084;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub port: u16,
    pub configured: bool,
    #[serde(default)]
    pub prepared_version: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            port: DEFAULT_PORT,
            configured: false,
            prepared_version: None,
        }
    }
}

#[derive(Clone)]
pub struct Paths {
    pub data: PathBuf,
    pub logs: PathBuf,
    pub settings: PathBuf,
    pub database: PathBuf,
    pub gateway: PathBuf,
    pub web: PathBuf,
}

impl Paths {
    pub fn new(data: PathBuf, gateway: PathBuf, web: PathBuf) -> Result<Self, String> {
        if !data.is_absolute() {
            return Err("数据目录必须是绝对路径".into());
        }
        private_directory(&data)?;
        let data = data
            .canonicalize()
            .map_err(|error| format!("无法访问数据目录：{error}"))?;
        let logs = data.join("logs");
        private_directory(&logs)?;
        Ok(Self {
            settings: data.join("desktop.json"),
            database: data.join("aether.db"),
            data,
            logs,
            gateway,
            web,
        })
    }

    pub fn load_config(&self) -> Result<Config, String> {
        match fs::read(&self.settings) {
            Ok(bytes) => {
                let config: Config = serde_json::from_slice(&bytes).map_err(|_| {
                    "桌面设置文件损坏，请保留数据并恢复 desktop.json 备份".to_string()
                })?;
                if config.version != 1 {
                    return Err("桌面设置版本不兼容，请使用对应版本的 Aether".into());
                }
                validate_port(config.port)?;
                Ok(config)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(error) => Err(format!("读取桌面设置失败：{error}")),
        }
    }

    pub fn save_config(&self, config: &Config) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(config).map_err(|error| error.to_string())?;
        atomic_write(&self.settings, &bytes)
    }

    /// The managed gateway is stopped here: copying DB + WAL is a coherent snapshot.
    /// Keep snapshots outside the app bundle so replacing the application is reversible.
    pub fn backup_before_upgrade(&self, config: &Config) -> Result<(), String> {
        if !self.database.exists()
            || config.prepared_version.as_deref() == Some(env!("CARGO_PKG_VERSION"))
        {
            return Ok(());
        }
        let snapshots = self.data.join("backups");
        private_directory(&snapshots)?;
        let snapshot = snapshots.join(format!(
            "before-{}-{}",
            env!("CARGO_PKG_VERSION"),
            Uuid::new_v4().simple()
        ));
        private_directory(&snapshot)?;
        for suffix in ["", "-wal", "-shm"] {
            let name = format!("aether.db{suffix}");
            let source = self.data.join(&name);
            if source.exists() {
                fs::copy(&source, snapshot.join(&name))
                    .map_err(|error| format!("升级前备份失败，已停止启动：{error}"))?;
            }
        }
        if self.settings.exists() {
            fs::copy(&self.settings, snapshot.join("desktop.json"))
                .map_err(|error| format!("备份设置失败：{error}"))?;
        }
        Ok(())
    }
}

pub fn validate_port(port: u16) -> Result<(), String> {
    if port < 1024 {
        return Err("端口范围为 1024–65535".into());
    }
    Ok(())
}

fn private_directory(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| format!("创建数据目录失败：{error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("设置目录权限失败：{error}"))?;
    }
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension(format!("{}.tmp", Uuid::new_v4().simple()));
    let result = (|| -> std::io::Result<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error| format!("保存桌面设置失败：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(temp: &tempfile::TempDir) -> Paths {
        Paths::new(temp.path().to_path_buf(), "gateway".into(), "web".into()).unwrap()
    }

    #[test]
    fn rejects_privileged_ports() {
        assert!(validate_port(8084).is_ok());
        assert!(validate_port(65535).is_ok());
        assert!(validate_port(1023).is_err());
        assert!(validate_port(0).is_err());
    }

    #[test]
    fn persists_config_but_rejects_corrupt_or_future_versions() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(&temp);
        let config = Config {
            port: 18084,
            configured: true,
            ..Config::default()
        };
        paths.save_config(&config).unwrap();
        assert_eq!(paths.load_config().unwrap().port, 18084);
        fs::write(&paths.settings, b"{broken}").unwrap();
        assert!(paths.load_config().is_err());
        paths
            .save_config(&Config {
                version: 99,
                ..config
            })
            .unwrap();
        assert!(paths.load_config().is_err());
    }

    #[test]
    fn upgrade_snapshot_preserves_wal_and_does_not_modify_database() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(&temp);
        fs::write(&paths.database, b"database").unwrap();
        fs::write(paths.data.join("aether.db-wal"), b"pending transactions").unwrap();
        paths.backup_before_upgrade(&Config::default()).unwrap();
        let snapshot = fs::read_dir(paths.data.join("backups"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            fs::read(snapshot.join("aether.db-wal")).unwrap(),
            b"pending transactions"
        );
        assert_eq!(fs::read(&paths.database).unwrap(), b"database");
    }
}
