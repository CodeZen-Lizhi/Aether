use std::{
    collections::VecDeque,
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use crate::{
    config::{Config, Paths},
    secrets::Secrets,
    session::DesktopSession,
};

pub type Logs = Arc<Mutex<VecDeque<String>>>;
pub const LOG_LINES: usize = 100;
const MAX_LINE_BYTES: usize = 8192;
pub const SHUTDOWN_SECONDS: u64 = 20;

#[cfg(unix)]
fn prepare_gateway_file_limit() -> std::io::Result<()> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // GUI launches can inherit 256 descriptors, exhausting the gateway's reserve.
    unsafe {
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let target = limit.rlim_max.min(10_240).max(limit.rlim_cur);
        if target <= 256 {
            return Err(std::io::Error::from_raw_os_error(libc::EMFILE));
        }
        if target > limit.rlim_cur {
            limit.rlim_cur = target;
            if libc::setrlimit(libc::RLIMIT_NOFILE, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
    }
    Ok(())
}

pub struct ManagedChild {
    pub child: Child,
    pub ready: Arc<AtomicBool>,
    pub session: DesktopSession,
}

pub fn append_log(logs: &Logs, line: impl Into<String>) {
    if let Ok(mut lines) = logs.lock() {
        if lines.len() == LOG_LINES {
            lines.pop_front();
        }
        lines.push_back(line.into());
    }
}

impl ManagedChild {
    pub fn spawn(
        paths: &Paths,
        config: &Config,
        secrets: &Secrets,
        logs: &Logs,
    ) -> Result<Self, String> {
        if !paths.gateway.is_file() {
            return Err("应用缺少网关程序，请重新安装完整的 Aether 应用".into());
        }
        if !paths.web.join("index.html").is_file() {
            return Err("应用缺少管理页面资源，请重新安装完整的 Aether 应用".into());
        }
        let mut command = Command::new(&paths.gateway);
        let session = DesktopSession::generate();
        let database_url = url::Url::from_file_path(&paths.database)
            .map_err(|_| "数据库路径无法转换为本机文件地址".to_string())?;
        command.env_clear();
        for name in [
            "HOME",
            "PATH",
            "TMPDIR",
            "LANG",
            "LC_ALL",
            "TZ",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "NO_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
            "no_proxy",
            "SSL_CERT_FILE",
            "SSL_CERT_DIR",
        ] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command
            .current_dir(&paths.data)
            .args([
                "--app-host",
                "127.0.0.1",
                "--app-port",
                &config.port.to_string(),
                "--listener-shards",
                "1",
                "--shutdown-timeout-seconds",
                &SHUTDOWN_SECONDS.to_string(),
                "--exit-on-stdin-close",
                "--desktop-mode",
                "--static-dir",
            ])
            .arg(&paths.web)
            .env("ENVIRONMENT", "production")
            .env("AETHER_DATABASE_DRIVER", "sqlite")
            .env(
                "AETHER_DATABASE_URL",
                format!("sqlite://{}", database_url.path()),
            )
            .env("AETHER_RUNTIME_BACKEND", "memory")
            .env("AETHER_GATEWAY_AUTO_PREPARE_DATABASE", "true")
            .env("JWT_SECRET_KEY", &secrets.jwt)
            .env("ENCRYPTION_KEY", &secrets.encryption)
            .env("AETHER_DESKTOP_SESSION_SECRET", session.secret())
            .env("AUTH_REFRESH_COOKIE_SECURE", "false")
            .env("AUTH_REFRESH_COOKIE_SAMESITE", "lax")
            .env("CORS_ORIGINS", format!("http://127.0.0.1:{}", config.port))
            .env("AETHER_LOG_FORMAT", "json")
            .env("AETHER_LOG_DESTINATION", "both")
            .env("AETHER_LOG_DIR", &paths.logs)
            .env("RUST_LOG", "aether_gateway=info,aether_data=info")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let redactions = vec![
            secrets.jwt.clone(),
            secrets.encryption.clone(),
            session.secret().to_string(),
        ];
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
            // Only async-signal-safe libc calls belong in the post-fork hook.
            unsafe {
                command.pre_exec(|| {
                    libc::umask(0o077);
                    prepare_gateway_file_limit()
                });
            }
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("启动网关失败：{error}"))?;
        let ready = Arc::new(AtomicBool::new(false));
        let redactions = Arc::new(redactions);
        if let Some(stdout) = child.stdout.take() {
            consume_output(stdout, logs.clone(), ready.clone(), redactions.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            consume_output(stderr, logs.clone(), ready.clone(), redactions);
        }
        append_log(
            logs,
            format!("[desktop] 网关已启动，PID {}，等待就绪", child.id()),
        );
        Ok(Self {
            child,
            ready,
            session,
        })
    }

    pub fn stop(&mut self, logs: &Logs) -> Result<(), String> {
        if self
            .child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            return Ok(());
        }
        // EOF is also delivered automatically if the desktop process crashes.
        self.child.stdin.take();
        let deadline = Instant::now() + Duration::from_secs(SHUTDOWN_SECONDS + 3);
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    append_log(logs, format!("[desktop] 网关已退出：{status}"));
                    return Ok(());
                }
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
                Ok(None) => {
                    append_log(logs, "[desktop] 网关退出超时，终止受管进程");
                    self.child
                        .kill()
                        .map_err(|error| format!("终止网关失败：{error}"))?;
                    self.child
                        .wait()
                        .map_err(|error| format!("等待网关退出失败：{error}"))?;
                    return Err("网关未在等待时间内退出，已终止受管进程；请检查日志".into());
                }
                Err(error) => return Err(format!("读取网关退出状态失败：{error}")),
            }
        }
    }
}

fn consume_output(
    reader: impl std::io::Read + Send + 'static,
    logs: Logs,
    ready: Arc<AtomicBool>,
    redactions: Arc<Vec<String>>,
) {
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        loop {
            match bounded_line(&mut reader) {
                Ok(Some(line)) => {
                    if is_ready(&line) {
                        ready.store(true, Ordering::Release);
                    }
                    let line = redactions
                        .iter()
                        .fold(line, |line, secret| line.replace(secret, "[redacted]"));
                    append_log(&logs, line);
                }
                Ok(None) => break,
                Err(_) => {
                    append_log(&logs, "[desktop] 网关日志流读取失败");
                    break;
                }
            }
        }
    });
}

fn is_ready(line: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .is_some_and(|record| {
            record["fields"]["event_name"] == "gateway_ready"
                || record["event_name"] == "gateway_ready"
        })
}

fn bounded_line(reader: &mut impl BufRead) -> std::io::Result<Option<String>> {
    let mut bytes = Vec::new();
    let mut overflow = false;
    let mut consumed_any = false;
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            break;
        }
        let end = buffer.iter().position(|byte| *byte == b'\n');
        let count = end.map_or(buffer.len(), |index| index + 1);
        if !overflow && bytes.len() + count <= MAX_LINE_BYTES {
            bytes.extend_from_slice(&buffer[..count]);
        } else {
            overflow = true;
            bytes.clear();
        }
        reader.consume(count);
        consumed_any = true;
        if end.is_some() {
            break;
        }
    }
    if !consumed_any {
        return Ok(None);
    }
    if overflow {
        return Ok(Some("[desktop] 已省略过长的日志行".into()));
    }
    Ok(Some(String::from_utf8_lossy(&bytes).trim_end().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn gateway_file_limit_recovers_low_soft_limit_and_respects_hard_limit() {
        use std::os::unix::process::CommandExt;

        for (soft, hard, expected) in [(256, 10_240, 10_240), (256, 512, 512), (1024, 1024, 1024)] {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", "ulimit -Sn"]);
            unsafe {
                command.pre_exec(move || {
                    let limit = libc::rlimit {
                        rlim_cur: soft,
                        rlim_max: hard,
                    };
                    if libc::setrlimit(libc::RLIMIT_NOFILE, &limit) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    prepare_gateway_file_limit()
                });
            }
            let output = command.output().unwrap();
            assert!(output.status.success());
            assert_eq!(
                String::from_utf8(output.stdout).unwrap().trim(),
                expected.to_string()
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn gateway_file_limit_rejects_exhausted_hard_limit() {
        use std::os::unix::process::CommandExt;

        let mut command = Command::new("/bin/sh");
        command.args(["-c", "exit 0"]);
        unsafe {
            command.pre_exec(|| {
                let limit = libc::rlimit {
                    rlim_cur: 256,
                    rlim_max: 256,
                };
                if libc::setrlimit(libc::RLIMIT_NOFILE, &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                prepare_gateway_file_limit()
            });
        }
        assert_eq!(
            command.output().unwrap_err().raw_os_error(),
            Some(libc::EMFILE)
        );
    }

    #[test]
    fn readiness_requires_structured_gateway_event() {
        assert!(!is_ready("upstream said gateway_ready"));
        assert!(is_ready(r#"{"fields":{"event_name":"gateway_ready"}}"#));
        assert!(!is_ready(r#"{"fields":{"event_name":"gateway_starting"}}"#));
    }

    #[test]
    fn oversized_lines_are_discarded_without_leaking_partial_values() {
        let bytes = format!("{}\nnext\n", "secret".repeat(2000));
        let mut reader = BufReader::new(bytes.as_bytes());
        assert_eq!(
            bounded_line(&mut reader).unwrap().unwrap(),
            "[desktop] 已省略过长的日志行"
        );
        assert_eq!(bounded_line(&mut reader).unwrap().unwrap(), "next");
        assert!(bounded_line(&mut reader).unwrap().is_none());
    }

    #[test]
    fn ring_buffer_retains_only_recent_lines() {
        let logs = Arc::new(Mutex::new(VecDeque::new()));
        for index in 0..150 {
            append_log(&logs, index.to_string());
        }
        let lines = logs.lock().unwrap();
        assert_eq!(lines.len(), LOG_LINES);
        assert_eq!(lines.front().unwrap(), "50");
    }
}
