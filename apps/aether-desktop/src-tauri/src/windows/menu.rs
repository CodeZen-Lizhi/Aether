use std::sync::Arc;

use tauri::{
    menu::{MenuBuilder, MenuItem, SubmenuBuilder},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};
use tauri_plugin_opener::OpenerExt;

use crate::gateway::{Gateway, Phase};

pub fn phase_label(phase: Phase) -> &'static str {
    match phase {
        Phase::Setup => "等待初始化",
        Phase::Starting => "启动中",
        Phase::Running => "运行中",
        Phase::Stopping => "停止中",
        Phase::Stopped => "已停止",
        Phase::Failed => "需要处理",
    }
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let quit = MenuItem::with_id(app, "quit", "退出 Aether", true, Some("CmdOrCtrl+Q"))?;
    let application = SubmenuBuilder::new(app, "Aether")
        .hide_with_text("隐藏 Aether")
        .hide_others_with_text("隐藏其他")
        .show_all_with_text("全部显示")
        .separator()
        .item(&quit)
        .build()?;
    let edit = SubmenuBuilder::new(app, "编辑")
        .undo_with_text("撤销")
        .redo_with_text("重做")
        .separator()
        .cut_with_text("剪切")
        .copy_with_text("拷贝")
        .paste_with_text("粘贴")
        .select_all_with_text("全选")
        .build()?;
    let gateway = SubmenuBuilder::new(app, "网关")
        .text("dashboard", "打开管理后台")
        .separator()
        .text("start", "启动网关")
        .text("stop", "停止网关")
        .text("restart", "重启网关")
        .separator()
        .text("data", "打开数据目录")
        .text("logs", "打开日志目录")
        .build()?;
    let window = SubmenuBuilder::new(app, "窗口")
        .minimize_with_text("最小化")
        .fullscreen_with_text("切换全屏")
        .build()?;
    app.set_menu(
        MenuBuilder::new(app)
            .items(&[&application, &edit, &gateway, &window])
            .build()?,
    )?;
    let tray_menu = MenuBuilder::new(app)
        .text("dashboard", "打开管理后台")
        .separator()
        .text("start", "启动网关")
        .text("stop", "停止网关")
        .text("restart", "重启网关")
        .separator()
        .text("data", "打开数据目录")
        .text("logs", "打开日志目录")
        .separator()
        .text("quit", "退出 Aether")
        .build()?;
    let tray = TrayIconBuilder::with_id("aether")
        .menu(&tray_menu)
        .tooltip("Aether")
        .icon(tauri::image::Image::from_bytes(include_bytes!(
            "../../icons/tray.png"
        ))?)
        .icon_as_template(true)
        .show_menu_on_left_click(true);
    tray.build(app)?;
    app.on_menu_event(|app, event| {
        let app = app.clone();
        let id = event.id().as_ref().to_string();
        if id == "quit" {
            super::request_quit(&app);
            return;
        }
        tauri::async_runtime::spawn_blocking(move || {
            let gateway = app.state::<Arc<Gateway>>();
            let result = match id.as_str() {
                "dashboard" => super::open_dashboard(&app),
                "start" => gateway.start().and_then(|_| super::open_dashboard(&app)),
                "stop" => gateway.stop_with(|| super::close_dashboard(&app)),
                "restart" => gateway
                    .restart_with(|| super::close_dashboard(&app))
                    .and_then(|_| super::open_dashboard(&app)),
                "data" => app
                    .opener()
                    .open_path(gateway.paths.data.display().to_string(), None::<&str>)
                    .map_err(|error| error.to_string()),
                "logs" => app
                    .opener()
                    .open_path(gateway.paths.logs.display().to_string(), None::<&str>)
                    .map_err(|error| error.to_string()),
                _ => Ok(()),
            };
            if let Err(error) = result {
                gateway.record_error(error);
                let _ = super::show_launcher(&app);
            }
        });
    });
    Ok(())
}
