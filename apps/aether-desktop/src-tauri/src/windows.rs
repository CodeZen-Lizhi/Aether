mod menu;
pub mod navigation;

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use sha2::{Digest, Sha256};
use tauri::{
    AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

use crate::{
    commands,
    config::Paths,
    gateway::{Gateway, Phase},
    secrets,
};

#[derive(Default)]
struct ExitState {
    ready: AtomicBool,
}

#[derive(Default)]
struct DashboardState {
    current: Mutex<Option<DashboardWindow>>,
}

struct DashboardWindow {
    generation: uuid::Uuid,
    label: String,
}

struct StartupState {
    // Some records whether startup should open the dashboard; None means the
    // initial start has finished and later activation can open it immediately.
    pending_dashboard: Mutex<Option<bool>>,
}

impl StartupState {
    fn new(open_dashboard: bool) -> Self {
        Self {
            pending_dashboard: Mutex::new(Some(open_dashboard)),
        }
    }

    fn defer_activation(&self) -> bool {
        let mut pending = self
            .pending_dashboard
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(open_dashboard) = pending.as_mut() {
            *open_dashboard = true;
            true
        } else {
            false
        }
    }

    fn complete(&self) -> bool {
        self.pending_dashboard
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
            .unwrap_or(false)
    }
}

pub fn show_launcher(app: &AppHandle) -> Result<(), String> {
    // The standalone recovery page was removed. Keep this compatibility hook
    // for older error paths, but never surface the hidden launcher window.
    let _ = app;
    Ok(())
}

fn show(window: &WebviewWindow) -> Result<(), String> {
    window
        .show()
        .and_then(|_| window.unminimize())
        .and_then(|_| window.set_focus())
        .map_err(|error| format!("打开窗口失败：{error}"))
}

pub fn close_dashboard(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<DashboardState>();
    let mut current = state
        .current
        .lock()
        .map_err(|_| "管理窗口状态不可用".to_string())?;
    if let Some(window) = current
        .as_ref()
        .and_then(|current| app.get_webview_window(&current.label))
    {
        window
            .destroy()
            .map_err(|error| format!("关闭管理窗口失败：{error}"))?;
    }
    *current = None;
    Ok(())
}

pub fn open_dashboard(app: &AppHandle) -> Result<(), String> {
    // A Dock/menu activation can arrive before the startup worker acquires the
    // gateway lifecycle lock. Remember it instead of showing settings because
    // the gateway is still in Setup/Stopped. Do not hold this lock across UI work.
    if app.state::<StartupState>().defer_activation() {
        return Ok(());
    }
    let gateway = app.try_state::<Arc<Gateway>>().ok_or("客户端仍在初始化")?;
    gateway.with_dashboard(|connection| {
        let state = app.state::<DashboardState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "管理窗口状态不可用".to_string())?;
        let existing = current
            .as_ref()
            .and_then(|current| app.get_webview_window(&current.label));
        if let Some(window) = existing.as_ref().filter(|_| {
            current
                .as_ref()
                .is_some_and(|current| current.generation == connection.generation)
        }) {
            show(window)?;
        } else {
            if let Some(window) = existing {
                window
                    .destroy()
                    .map_err(|error| format!("更新管理窗口失败：{error}"))?;
            }
            *current = None;
            let port = connection.port;
            let url = url::Url::parse(&format!("http://127.0.0.1:{port}/admin/dashboard"))
                .map_err(|error| error.to_string())?;
            let navigation_app = app.clone();
            let new_window_app = app.clone();
            let downloads = navigation::Downloads::new(app.clone(), port);
            let digest = Sha256::digest(gateway.paths.data.to_string_lossy().as_bytes());
            let mut store_id = [0; 16];
            store_id.copy_from_slice(&digest[..16]);
            // Native destroy is queued. A new label avoids racing the old label's
            // removal from Tauri's window manager during immediate recovery.
            let label = format!("dashboard-{}", uuid::Uuid::new_v4().simple());
            WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
                .title("Aether")
                .inner_size(1200.0, 820.0)
                .min_inner_size(840.0, 620.0)
                .data_store_identifier(store_id)
                .initialization_script(connection.initialization_script)
                .on_navigation(move |url| {
                    navigation::allow_dashboard_navigation(&navigation_app, url, port)
                })
                .on_new_window(move |url, _| {
                    navigation::open_external(&new_window_app, &url);
                    tauri::webview::NewWindowResponse::Deny
                })
                .on_download(move |_, event| downloads.handle(event))
                .build()
                .map_err(|error| format!("创建管理窗口失败：{error}"))?;
            *current = Some(DashboardWindow {
                generation: connection.generation,
                label,
            });
        }
        if let Some(window) = app.get_webview_window("main") {
            window.hide().map_err(|error| error.to_string())?;
        }
        Ok(())
    })
}

pub fn activate(app: &AppHandle) {
    // Reopen can run on the UI thread. Never wait for the lifecycle mutex there:
    // its owner can be waiting for that same UI thread to create/destroy a view.
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = open_dashboard(&app) {
            if let Some(gateway) = app.try_state::<Arc<Gateway>>() {
                if gateway
                    .status(false)
                    .is_ok_and(|status| status.phase == Phase::Running)
                {
                    gateway.record_error(error);
                }
            }
            let _ = show_launcher(&app);
        }
    });
}

pub fn request_quit(app: &AppHandle) {
    let Some(gateway) = app.try_state::<Arc<Gateway>>() else {
        app.state::<ExitState>()
            .ready
            .store(true, Ordering::Release);
        app.exit(1);
        return;
    };
    if gateway.quitting.swap(true, Ordering::AcqRel) {
        return;
    }
    let gateway = gateway.inner().clone();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = gateway.stop_with(|| close_dashboard(&app));
        if let Err(error) = &result {
            gateway.record_error(error.clone());
        }
        app.state::<ExitState>()
            .ready
            .store(true, Ordering::Release);
        app.exit(if result.is_ok() { 0 } else { 1 });
    });
}

fn data_override() -> Result<Option<PathBuf>, String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--data-dir" {
            return args
                .next()
                .map(PathBuf::from)
                .map(Some)
                .ok_or("--data-dir 缺少绝对路径".into());
        }
    }
    Ok(std::env::var_os("AETHER_DESKTOP_DATA_DIR").map(PathBuf::from))
}

fn paths(app: &AppHandle, data: Option<PathBuf>) -> Result<Paths, String> {
    let data = data
        .map(Ok)
        .unwrap_or_else(|| app.path().app_data_dir().map_err(|error| error.to_string()))?;
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    // Bundled builds must never fall back to a checkout or the user's PATH.
    let (gateway, web) = if cfg!(debug_assertions)
        && !executable
            .components()
            .any(|part| part.as_os_str().to_string_lossy().ends_with(".app"))
    {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        (
            manifest.join(format!(
                "binaries/aether-gateway-{}",
                env!("AETHER_DESKTOP_TARGET")
            )),
            manifest.join("../../../frontend/dist"),
        )
    } else {
        (
            executable
                .parent()
                .ok_or("无法定位网关程序")?
                .join("aether-gateway"),
            app.path()
                .resource_dir()
                .map_err(|error| error.to_string())?
                .join("web"),
        )
    };
    Paths::new(data, gateway, web)
}

fn watch_gateway(app: AppHandle, gateway: Arc<Gateway>) {
    thread::spawn(move || {
        let mut previous = Phase::Setup;
        while !gateway.quitting.load(Ordering::Acquire) {
            if let Ok(status) = gateway.status(false) {
                if status.phase != previous {
                    if let Some(tray) = app.tray_by_id("aether") {
                        let _ = tray.set_tooltip(Some(format!(
                            "Aether · {}",
                            menu::phase_label(status.phase)
                        )));
                    }
                    if status.phase == Phase::Failed {
                        let _ = gateway.with_failure(|| close_dashboard(&app));
                    }
                    previous = status.phase;
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let Some(mut instance) = crate::instance::Instance::acquire()? else {
        return Ok(());
    };
    let data = data_override()?;
    if data.as_ref().is_some_and(|path| !path.is_absolute()) {
        return Err("数据目录必须是绝对路径".into());
    }
    let background = std::env::args().any(|arg| arg == "--background");
    let mut autostart = tauri_plugin_autostart::Builder::new()
        .app_name("Aether")
        .args(["--background"]);
    if let Some(data) = &data {
        // Diagnostic profiles cannot change the regular application's login item.
        autostart = autostart
            .app_name(format!(
                "Aether-{}",
                secrets::service_name(data)
                    .rsplit('.')
                    .next()
                    .unwrap_or("qa")
            ))
            .args(["--data-dir".to_string(), data.display().to_string()]);
    }
    let app = tauri::Builder::default()
        .plugin(autostart.build())
        .plugin(tauri_plugin_opener::init())
        .manage(ExitState::default())
        .manage(DashboardState::default())
        .manage(StartupState::new(!background))
        .invoke_handler(tauri::generate_handler![
            commands::desktop_status,
            commands::desktop_start,
            commands::desktop_stop,
            commands::desktop_restart,
            commands::desktop_set_port,
            commands::desktop_set_autostart,
            commands::desktop_open_dashboard,
            commands::desktop_open_data_dir,
            commands::desktop_open_log_dir,
            commands::desktop_logs,
            commands::desktop_quit,
        ])
        .setup(move |app| {
            let gateway = Gateway::new(paths(app.handle(), data.clone())?);
            app.manage(gateway.clone());
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("desktop.html".into()))
                .title("Aether")
                .inner_size(960.0, 760.0)
                .min_inner_size(740.0, 580.0)
                .visible(false)
                .focused(false)
                .on_navigation(navigation::is_launcher_url)
                .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
                .build()?;
            menu::install(app.handle())?;
            let activation_handle = app.handle().clone();
            instance.listen(move || activate(&activation_handle));
            app.manage(instance);
            watch_gateway(app.handle().clone(), gateway.clone());
            let handle = app.handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                let result = gateway.start();
                let open_requested = handle.state::<StartupState>().complete();
                if gateway.quitting.load(Ordering::Acquire) {
                    return;
                }
                match result {
                    Ok(()) if open_requested => {
                        if let Err(error) = open_dashboard(&handle) {
                            gateway.record_error(error);
                            let _ = show_launcher(&handle);
                        }
                    }
                    Ok(()) => (),
                    Err(error) => {
                        gateway.record_error(error);
                        let _ = show_launcher(&handle);
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())?;
    app.run(|app, event| match event {
        RunEvent::Exit => app.state::<crate::instance::Instance>().cleanup_socket(),
        RunEvent::ExitRequested { api, .. }
            if !app.state::<ExitState>().ready.load(Ordering::Acquire) =>
        {
            api.prevent_exit();
            request_quit(app);
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen {
            has_visible_windows: false,
            ..
        } => activate(app),
        _ => (),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::StartupState;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn foreground_start_opens_dashboard_when_ready() {
        let startup = StartupState::new(true);
        assert!(startup.complete());
        assert!(!startup.complete());
        assert!(!startup.defer_activation());
    }

    #[test]
    fn background_start_stays_hidden_without_user_activation() {
        let startup = StartupState::new(false);
        assert!(!startup.complete());
        assert!(!startup.defer_activation());
    }

    #[test]
    fn activation_during_startup_waits_for_dashboard_instead_of_showing_settings() {
        for foreground in [false, true] {
            let startup = StartupState::new(foreground);
            assert!(startup.defer_activation());
            assert!(startup.defer_activation());
            assert!(startup.complete());
            assert!(!startup.defer_activation());
        }
    }

    #[test]
    fn activation_racing_startup_completion_is_never_lost() {
        for _ in 0..64 {
            let startup = Arc::new(StartupState::new(false));
            let barrier = Arc::new(Barrier::new(2));
            let worker_startup = startup.clone();
            let worker_barrier = barrier.clone();
            let activation = thread::spawn(move || {
                worker_barrier.wait();
                worker_startup.defer_activation()
            });
            barrier.wait();
            let open_on_completion = startup.complete();
            let deferred = activation.join().unwrap();
            // Either startup receives the request or activation opens directly.
            assert_eq!(open_on_completion, deferred);
        }
    }
}
