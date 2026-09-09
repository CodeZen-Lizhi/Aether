use std::{
    collections::VecDeque,
    net::{Ipv4Addr, TcpListener},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::{
    config::{validate_port, Config, Paths},
    process::{append_log, Logs, ManagedChild},
    secrets,
    session::DashboardConnection,
};

#[cfg(all(test, target_os = "macos"))]
mod integration_tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Setup,
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Clone, Serialize)]
pub struct Status {
    pub phase: Phase,
    pub configured: bool,
    pub port: u16,
    pub gateway_url: String,
    pub data_dir: String,
    pub log_dir: String,
    pub autostart: bool,
    pub pid: Option<u32>,
    pub error: Option<String>,
    pub version: String,
}

struct Inner {
    config: Config,
    phase: Phase,
    process: Option<ManagedChild>,
    error: Option<String>,
    config_error: Option<String>,
}

pub struct Gateway {
    pub paths: Paths,
    inner: Mutex<Inner>,
    operation: Mutex<()>,
    pub logs: Logs,
    stop_requested: AtomicBool,
    pub quitting: AtomicBool,
}

impl Gateway {
    pub fn new(paths: Paths) -> Arc<Self> {
        let (config, error) = match paths.load_config() {
            Ok(config) => (config, None),
            Err(error) => (Config::default(), Some(error)),
        };
        let phase = if error.is_some() {
            Phase::Failed
        } else if config.configured {
            Phase::Stopped
        } else {
            Phase::Setup
        };
        Arc::new(Self {
            paths,
            inner: Mutex::new(Inner {
                config,
                phase,
                process: None,
                error: error.clone(),
                config_error: error,
            }),
            operation: Mutex::new(()),
            logs: Arc::new(Mutex::new(VecDeque::new())),
            stop_requested: AtomicBool::new(false),
            quitting: AtomicBool::new(false),
        })
    }

    fn inner(&self) -> Result<MutexGuard<'_, Inner>, String> {
        self.inner
            .lock()
            .map_err(|_| "网关状态不可用，请重启应用".into())
    }

    pub fn status(&self, autostart: bool) -> Result<Status, String> {
        let mut inner = self.inner()?;
        if let Some(process) = inner.process.as_mut() {
            if let Some(exit) = process
                .child
                .try_wait()
                .map_err(|error| format!("读取网关状态失败：{error}"))?
            {
                inner.process = None;
                if inner.phase != Phase::Stopping {
                    let error = format!("网关意外退出（{exit}），请检查日志后重新启动");
                    append_log(&self.logs, &error);
                    inner.phase = Phase::Failed;
                    inner.error = Some(error);
                }
            }
        }
        Ok(Status {
            phase: inner.phase,
            configured: inner.config.configured,
            port: inner.config.port,
            gateway_url: format!("http://127.0.0.1:{}", inner.config.port),
            data_dir: self.paths.data.display().to_string(),
            log_dir: self.paths.logs.display().to_string(),
            autostart,
            pid: inner.process.as_ref().map(|process| process.child.id()),
            error: inner.error.clone(),
            version: env!("CARGO_PKG_VERSION").into(),
        })
    }

    /// Keep window creation and its credentials in the same lifecycle operation.
    /// Otherwise a concurrent restart can leave a window with the old capability.
    pub fn with_dashboard<T>(
        &self,
        operation: impl FnOnce(DashboardConnection) -> Result<T, String>,
    ) -> Result<T, String> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "网关操作不可用".to_string())?;
        if self.quitting.load(Ordering::Acquire) {
            return Err("应用正在退出".into());
        }
        self.status(false)?;
        let connection = {
            let inner = self.inner()?;
            if inner.phase != Phase::Running {
                return Err("网关尚未运行，请先在客户端设置中启动".into());
            }
            inner
                .process
                .as_ref()
                .ok_or("本机连接不可用，请重启网关")?
                .session
                .connection(inner.config.port)?
        };
        operation(connection)
    }

    /// A failed snapshot from the watcher can outlive a successful manual retry.
    pub fn with_failure(
        &self,
        operation: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "网关操作不可用".to_string())?;
        if self.status(false)?.phase == Phase::Failed {
            operation()
        } else {
            Ok(())
        }
    }

    pub fn start(&self) -> Result<(), String> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "网关操作不可用".to_string())?;
        self.start_locked()
    }

    fn start_locked(&self) -> Result<(), String> {
        if self.quitting.load(Ordering::Acquire) {
            return Err("应用正在退出".into());
        }
        self.status(false)?;
        let config = {
            let mut inner = self.inner()?;
            if let Some(error) = &inner.config_error {
                return Err(error.clone());
            }
            if inner.process.is_some() {
                return Ok(());
            }
            inner.phase = Phase::Starting;
            inner.error = None;
            inner.config.clone()
        };
        self.stop_requested.store(false, Ordering::Release);
        let result = self.launch(&config);
        if let Err(error) = result {
            let process = self.inner()?.process.take();
            if let Some(mut process) = process {
                let _ = process.stop(&self.logs);
            }
            let mut inner = self.inner()?;
            let cancelled = self.stop_requested.load(Ordering::Acquire);
            inner.phase = if cancelled {
                Phase::Stopped
            } else {
                Phase::Failed
            };
            inner.error = if cancelled { None } else { Some(error.clone()) };
            append_log(&self.logs, format!("[desktop] {error}"));
            return Err(error);
        }
        Ok(())
    }

    fn launch(&self, config: &Config) -> Result<(), String> {
        let probe = TcpListener::bind((Ipv4Addr::LOCALHOST, config.port))
            .map_err(|_| format!("端口 {} 已被占用，请停止占用程序或修改端口", config.port))?;
        let keys = secrets::load_or_create(&self.paths.data, self.paths.database.exists())?;
        self.paths.backup_before_upgrade(config)?;
        drop(probe);
        let process = ManagedChild::spawn(&self.paths, config, &keys, &self.logs)?;
        let ready = process.ready.clone();
        self.inner()?.process = Some(process);
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| error.to_string())?;
        let base = format!("http://127.0.0.1:{}", config.port);
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if self.stop_requested.load(Ordering::Acquire) || self.quitting.load(Ordering::Acquire)
            {
                return Err("网关启动已取消".into());
            }
            let status = self.status(false)?;
            if status.phase == Phase::Failed {
                return Err(status.error.unwrap_or_else(|| "网关启动失败".into()));
            }
            if ready.load(Ordering::Acquire)
                && client
                    .get(format!("{base}/_gateway/health"))
                    .send()
                    .is_ok_and(|response| response.status().is_success())
            {
                break;
            }
            if Instant::now() >= deadline {
                return Err("网关启动超时，请检查日志中的数据库或网络错误".into());
            }
            thread::sleep(Duration::from_millis(150));
        }
        let mut config = config.clone();
        config.configured = true;
        config.prepared_version = Some(env!("CARGO_PKG_VERSION").into());
        self.paths.save_config(&config)?;
        let mut inner = self.inner()?;
        inner.config = config;
        inner.phase = Phase::Running;
        inner.error = None;
        append_log(&self.logs, "[desktop] 网关已就绪");
        Ok(())
    }

    #[cfg(test)]
    pub fn stop(&self) -> Result<(), String> {
        self.stop_with(|| Ok(()))
    }

    pub fn stop_with(
        &self,
        before_stop: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        self.stop_requested.store(true, Ordering::Release);
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "网关操作不可用".to_string())?;
        let before_stop = before_stop();
        self.stop_locked().and(before_stop)
    }

    fn stop_locked(&self) -> Result<(), String> {
        let process = {
            let mut inner = self.inner()?;
            inner.phase = Phase::Stopping;
            inner.process.take()
        };
        let result = process.map_or(Ok(()), |mut process| process.stop(&self.logs));
        let mut inner = self.inner()?;
        inner.phase = if inner.config_error.is_some() || result.is_err() {
            Phase::Failed
        } else if inner.config.configured {
            Phase::Stopped
        } else {
            Phase::Setup
        };
        inner.error = inner
            .config_error
            .clone()
            .or_else(|| result.as_ref().err().cloned());
        result
    }

    #[cfg(test)]
    pub fn restart(&self) -> Result<(), String> {
        self.restart_with(|| Ok(()))
    }

    pub fn restart_with(
        &self,
        before_stop: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        self.stop_requested.store(true, Ordering::Release);
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "网关操作不可用".to_string())?;
        let before_stop = before_stop();
        self.stop_locked()?;
        before_stop?;
        self.start_locked()
    }

    pub fn set_port(&self, port: u16) -> Result<(), String> {
        validate_port(port)?;
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "网关操作不可用".to_string())?;
        let mut inner = self.inner()?;
        if let Some(error) = &inner.config_error {
            return Err(error.clone());
        }
        if inner.process.is_some() {
            return Err("请先停止网关，再修改端口".into());
        }
        let mut config = inner.config.clone();
        config.port = port;
        self.paths.save_config(&config)?;
        inner.config = config;
        inner.error = None;
        inner.phase = if inner.config.configured {
            Phase::Stopped
        } else {
            Phase::Setup
        };
        Ok(())
    }

    pub fn record_error(&self, error: String) {
        append_log(&self.logs, format!("[desktop] {error}"));
        if let Ok(mut inner) = self.inner() {
            inner.error = Some(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gateway(temp: &tempfile::TempDir) -> Arc<Gateway> {
        Gateway::new(
            Paths::new(
                temp.path().to_path_buf(),
                "missing-gateway".into(),
                "web".into(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn fresh_install_persists_port_without_credentials() {
        let temp = tempfile::tempdir().unwrap();
        let gateway = gateway(&temp);
        assert_eq!(gateway.status(false).unwrap().phase, Phase::Setup);
        assert!(gateway.with_dashboard(|_| Ok(())).is_err());
        gateway.set_port(18084).unwrap();
        let saved = std::fs::read_to_string(&gateway.paths.settings).unwrap();
        assert!(!saved.contains("password"));
        assert!(!saved.contains("secret"));
        assert_eq!(gateway.paths.load_config().unwrap().port, 18084);
    }

    #[test]
    fn corrupt_configuration_remains_visible_and_is_never_overwritten() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("desktop.json"), b"broken").unwrap();
        let gateway = gateway(&temp);
        assert_eq!(gateway.status(false).unwrap().phase, Phase::Failed);
        assert!(gateway.set_port(18084).is_err());
        assert!(gateway.start().is_err());
        gateway.stop().unwrap();
        assert_eq!(gateway.status(false).unwrap().phase, Phase::Failed);
        assert!(gateway.status(false).unwrap().error.is_some());
        assert_eq!(
            std::fs::read_to_string(&gateway.paths.settings).unwrap(),
            "broken"
        );
    }

    #[test]
    fn quitting_prevents_queued_start_operations() {
        let temp = tempfile::tempdir().unwrap();
        let gateway = gateway(&temp);
        gateway.quitting.store(true, Ordering::Release);
        assert_eq!(gateway.start().unwrap_err(), "应用正在退出");
    }
}
