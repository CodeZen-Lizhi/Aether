use std::sync::Arc;

use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;

use crate::{
    gateway::{Gateway, Status},
    windows,
};

/// Check both the Tauri capability and the currently managed gateway origin.
/// A random localhost page must not gain access just because it uses the same port.
fn authorize(window: &WebviewWindow, app: &AppHandle) -> Result<(), String> {
    let url = window.url().map_err(|_| "无法验证窗口来源".to_string())?;
    let launcher = window.label() == "main" && windows::navigation::is_launcher_url(&url);
    let dashboard = window.label().starts_with("dashboard-")
        && app
            .try_state::<Arc<Gateway>>()
            .and_then(|gateway| gateway.status(false).ok())
            .is_some_and(|status| {
                url.scheme() == "http"
                    && url.host_str() == Some("127.0.0.1")
                    && url.port_or_known_default() == Some(status.port)
                    && url.username().is_empty()
                    && url.password().is_none()
            });
    if !launcher && !dashboard {
        return Err("此窗口没有桌面管理权限".into());
    }
    Ok(())
}

fn status(app: &AppHandle, gateway: &Gateway) -> Result<Status, String> {
    let enabled = app
        .autolaunch()
        .is_enabled()
        .map_err(|error| format!("读取登录自启设置失败：{error}"))?;
    gateway.status(enabled)
}

async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|_| "桌面操作中断，请查看日志后重试".to_string())?
}

#[tauri::command]
pub async fn desktop_status(
    window: WebviewWindow,
    app: AppHandle,
    gateway: State<'_, Arc<Gateway>>,
) -> Result<Status, String> {
    authorize(&window, &app)?;
    let gateway = gateway.inner().clone();
    blocking(move || status(&app, &gateway)).await
}

#[tauri::command]
pub async fn desktop_start(
    window: WebviewWindow,
    app: AppHandle,
    gateway: State<'_, Arc<Gateway>>,
) -> Result<Status, String> {
    authorize(&window, &app)?;
    let gateway = gateway.inner().clone();
    blocking(move || {
        gateway.start()?;
        status(&app, &gateway)
    })
    .await
}

#[tauri::command]
pub async fn desktop_stop(
    window: WebviewWindow,
    app: AppHandle,
    gateway: State<'_, Arc<Gateway>>,
) -> Result<Status, String> {
    authorize(&window, &app)?;
    let gateway = gateway.inner().clone();
    blocking(move || {
        gateway.stop_with(|| {
            windows::close_dashboard(&app)?;
            windows::show_launcher(&app)
        })?;
        status(&app, &gateway)
    })
    .await
}

#[tauri::command]
pub async fn desktop_restart(
    window: WebviewWindow,
    app: AppHandle,
    gateway: State<'_, Arc<Gateway>>,
) -> Result<Status, String> {
    authorize(&window, &app)?;
    let gateway = gateway.inner().clone();
    blocking(move || {
        if let Err(error) = gateway
            .restart_with(|| windows::close_dashboard(&app))
            .and_then(|_| windows::open_dashboard(&app))
        {
            gateway.record_error(error.clone());
            let _ = windows::show_launcher(&app);
            return Err(error);
        }
        status(&app, &gateway)
    })
    .await
}

#[tauri::command]
pub async fn desktop_set_port(
    window: WebviewWindow,
    app: AppHandle,
    gateway: State<'_, Arc<Gateway>>,
    port: u16,
) -> Result<Status, String> {
    authorize(&window, &app)?;
    let gateway = gateway.inner().clone();
    blocking(move || {
        let was_running = gateway.status(false)?.pid.is_some();
        if was_running {
            gateway.stop_with(|| windows::close_dashboard(&app))?;
        }
        if let Err(error) = gateway.set_port(port) {
            let _ = windows::show_launcher(&app);
            return Err(error);
        }
        if was_running {
            if let Err(error) = gateway.start().and_then(|_| windows::open_dashboard(&app)) {
                gateway.record_error(error.clone());
                let _ = windows::show_launcher(&app);
                return Err(error);
            }
        }
        status(&app, &gateway)
    })
    .await
}

#[tauri::command]
pub async fn desktop_set_autostart(
    window: WebviewWindow,
    app: AppHandle,
    gateway: State<'_, Arc<Gateway>>,
    enabled: bool,
) -> Result<Status, String> {
    authorize(&window, &app)?;
    let gateway = gateway.inner().clone();
    blocking(move || {
        let launch = app.autolaunch();
        if enabled {
            launch.enable()
        } else {
            launch.disable()
        }
        .map_err(|error| format!("更新登录自启失败：{error}"))?;
        status(&app, &gateway)
    })
    .await
}

#[tauri::command]
pub async fn desktop_open_dashboard(window: WebviewWindow, app: AppHandle) -> Result<(), String> {
    authorize(&window, &app)?;
    blocking(move || windows::open_dashboard(&app)).await
}

#[tauri::command]
pub fn desktop_open_data_dir(
    window: WebviewWindow,
    app: AppHandle,
    gateway: State<'_, Arc<Gateway>>,
) -> Result<(), String> {
    authorize(&window, &app)?;
    app.opener()
        .open_path(gateway.paths.data.display().to_string(), None::<&str>)
        .map_err(|error| format!("打开数据目录失败：{error}"))
}

#[tauri::command]
pub fn desktop_open_log_dir(
    window: WebviewWindow,
    app: AppHandle,
    gateway: State<'_, Arc<Gateway>>,
) -> Result<(), String> {
    authorize(&window, &app)?;
    app.opener()
        .open_path(gateway.paths.logs.display().to_string(), None::<&str>)
        .map_err(|error| format!("打开日志目录失败：{error}"))
}

#[tauri::command]
pub fn desktop_logs(
    window: WebviewWindow,
    app: AppHandle,
    gateway: State<'_, Arc<Gateway>>,
) -> Result<Vec<String>, String> {
    authorize(&window, &app)?;
    gateway
        .logs
        .lock()
        .map(|lines| lines.iter().cloned().collect())
        .map_err(|_| "无法读取诊断日志".into())
}

#[tauri::command]
pub fn desktop_quit(window: WebviewWindow, app: AppHandle) -> Result<(), String> {
    authorize(&window, &app)?;
    windows::request_quit(&app);
    Ok(())
}
