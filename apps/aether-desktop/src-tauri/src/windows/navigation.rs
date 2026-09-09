use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use tauri::{webview::DownloadEvent, AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;
use url::Url;
use uuid::Uuid;

use crate::{gateway::Gateway, process::append_log};

pub fn is_launcher_url(url: &Url) -> bool {
    url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/desktop.html"
        && match url.scheme() {
            "tauri" => url.host_str() == Some("localhost"),
            "http" | "https" => url.host_str() == Some("tauri.localhost"),
            _ => false,
        }
}

fn is_gateway_url(url: &Url, port: u16) -> bool {
    url.scheme() == "http"
        && url.host_str() == Some("127.0.0.1")
        && url.port_or_known_default() == Some(port)
        && url.username().is_empty()
        && url.password().is_none()
}

fn is_download_url(url: &Url, port: u16) -> bool {
    is_gateway_url(url, port)
        || (url.scheme() == "blob"
            && Url::parse(url.path()).is_ok_and(|inner| is_gateway_url(&inner, port)))
}

pub fn open_external(app: &AppHandle, url: &Url) {
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return;
    }
    if app.opener().open_url(url.as_str(), None::<&str>).is_err() {
        app.state::<Arc<Gateway>>()
            .record_error("无法打开默认浏览器，请检查系统设置后重试".into());
        let _ = super::show_launcher(app);
    }
}

pub fn allow_dashboard_navigation(app: &AppHandle, url: &Url, port: u16) -> bool {
    if is_download_url(url, port) {
        return true;
    }
    open_external(app, url);
    false
}

pub struct Downloads {
    app: AppHandle,
    port: u16,
    pending: Mutex<HashMap<String, PathBuf>>,
}

impl Downloads {
    pub fn new(app: AppHandle, port: u16) -> Self {
        Self {
            app,
            port,
            pending: Mutex::new(HashMap::new()),
        }
    }

    pub fn handle(&self, event: DownloadEvent<'_>) -> bool {
        let gateway = self.app.state::<Arc<Gateway>>();
        match event {
            DownloadEvent::Requested { url, destination } => {
                if !is_download_url(&url, self.port) {
                    return false;
                }
                let path = self
                    .app
                    .path()
                    .download_dir()
                    .map_err(|error| error.to_string())
                    .and_then(|directory| {
                        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
                        Ok(download_path(&directory, destination))
                    });
                match path {
                    Ok(path) => {
                        let Ok(mut pending) = self.pending.lock() else {
                            return false;
                        };
                        pending.insert(url.to_string(), path.clone());
                        *destination = path;
                        true
                    }
                    Err(_) => {
                        gateway.record_error("无法写入下载目录，请检查目录权限后重试导出".into());
                        false
                    }
                }
            }
            DownloadEvent::Finished { url, success, .. } => {
                let path = self
                    .pending
                    .lock()
                    .ok()
                    .and_then(|mut pending| pending.remove(url.as_str()));
                if success {
                    append_log(&gateway.logs, "[desktop] 文件已保存到下载目录");
                    if let Some(path) = path {
                        let _ = self.app.opener().reveal_item_in_dir(path);
                    }
                } else {
                    gateway.record_error("文件下载未完成，请重试导出".into());
                    let _ = super::show_launcher(&self.app);
                }
                true
            }
            _ => false,
        }
    }
}

fn download_path(directory: &Path, suggested: &Path) -> PathBuf {
    // Keep only a leaf name and add a random suffix, including for repeated exports.
    // WKDownload requires a destination that does not already exist.
    let name = suggested
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("aether-export");
    let mut safe = String::new();
    for character in name
        .chars()
        .filter(|c| !c.is_control() && !"/\\:.".contains(*c))
    {
        if safe.len() + character.len_utf8() > 140 {
            break;
        }
        safe.push(character);
    }
    if safe.trim().is_empty() {
        safe = "aether-export".into();
    }
    let extension = suggested
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| {
            value.len() <= 16 && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
        });
    let suffix = Uuid::new_v4().simple().to_string();
    let name = format!(
        "{}-{}{}",
        safe.trim(),
        &suffix[..12],
        extension.map_or(String::new(), |extension| format!(".{extension}"))
    );
    directory.join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_bundled_launcher_origin_is_trusted() {
        assert!(is_launcher_url(
            &Url::parse("tauri://localhost/desktop.html").unwrap()
        ));
        for url in [
            "http://127.0.0.1:8084/desktop.html",
            "https://tauri.localhost.evil/desktop.html",
            "tauri://localhost/index.html",
            "tauri://user@localhost/desktop.html",
        ] {
            assert!(!is_launcher_url(&Url::parse(url).unwrap()));
        }
    }

    #[test]
    fn downloads_are_bound_to_the_managed_gateway_origin() {
        assert!(is_download_url(
            &Url::parse("blob:http://127.0.0.1:8084/id").unwrap(),
            8084
        ));
        for url in [
            "http://127.0.0.1:8085/",
            "http://127.0.0.1.evil:8084/",
            "blob:https://example.com/id",
            "file:///etc/passwd",
            "javascript:alert(1)",
        ] {
            assert!(!is_download_url(&Url::parse(url).unwrap(), 8084));
        }
    }

    #[test]
    fn export_name_cannot_escape_downloads_or_overwrite_an_existing_export() {
        let directory = Path::new("/Downloads");
        let first = download_path(directory, Path::new("../../aether.json"));
        assert_eq!(first.parent(), Some(directory));
        assert_eq!(first.extension().unwrap(), "json");
        assert_ne!(
            first,
            download_path(directory, Path::new("../../aether.json"))
        );
        assert!(
            download_path(directory, Path::new(&"界".repeat(300)))
                .file_name()
                .unwrap()
                .len()
                < 255
        );
    }
}
