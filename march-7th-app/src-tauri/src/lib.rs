mod platform;

use serde::Serialize;
use tauri::{
    menu::{MenuBuilder, MenuEvent, MenuItem},
    tray::TrayIconBuilder,
    App, AppHandle, Manager, Runtime,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CursorSample {
    x: f64,
    y: f64,
    window_x: f64,
    window_y: f64,
}

#[tauri::command]
fn cursor_relative_to_window(window: tauri::WebviewWindow) -> Result<CursorSample, String> {
    let cursor = platform::cursor_position()?;
    let window_origin = window.outer_position().map_err(|error| error.to_string())?;
    let scale_factor = window.scale_factor().map_err(|error| error.to_string())?;
    let relative = platform::relative_to_window(cursor, window_origin, scale_factor);
    let logical_origin = window_origin.to_logical::<f64>(scale_factor);

    Ok(CursorSample {
        x: relative.x,
        y: relative.y,
        window_x: logical_origin.x,
        window_y: logical_origin.y,
    })
}

fn setup_status_bar<R: Runtime>(app: &mut App<R>) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示三月七", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "隐藏三月七", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 March 7th", true, None::<&str>)?;
    let menu = MenuBuilder::new(app)
        .items(&[&show, &hide, &quit])
        .build()?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default tray icon".into()))?;

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .on_menu_event(handle_status_bar_menu)
        .build(app)?;
    Ok(())
}

fn handle_status_bar_menu<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        "show" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
            }
        }
        "hide" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        }
        "quit" => app.exit(0),
        _ => {}
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| Ok(setup_status_bar(app)?))
        .invoke_handler(tauri::generate_handler![cursor_relative_to_window])
        .run(tauri::generate_context!())
        .expect("error while running March 7th");
}
