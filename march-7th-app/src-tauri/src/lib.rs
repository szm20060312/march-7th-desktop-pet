mod atomic_file;
pub mod build_info;
mod characters;
mod data_directory;
mod desktop;
mod platform;
mod reminders;

use serde::Serialize;
use tauri::Manager;

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

fn app_context() -> tauri::Context<tauri::Wry> {
    tauri::generate_context!()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = desktop::configure(tauri::Builder::default())
        .setup(|app| {
            let characters = characters::setup(app)?;
            let reminders = reminders::setup(app)?;
            desktop::setup(app, &characters, &reminders)
        })
        .invoke_handler(tauri::generate_handler![
            build_info::get_build_info,
            cursor_relative_to_window,
            characters::native::get_selected_character,
            characters::native::select_character,
            reminders::native::get_reminders,
            reminders::native::reminder_command,
            reminders::ui::reminder_ui_ready,
            reminders::ui::hide_reminder_settings
        ])
        .build(app_context())
        .expect("error while building March 7th");

    // Tauri 2 runs setup on the event loop's Ready event, not in build().
    // Qualify here so a duplicate never enters that loop or starts any service.
    let directory = match data_directory::acquire(app.path().app_config_dir().ok()) {
        Ok(directory) => directory,
        Err(data_directory::AlreadyRunning) => return,
    };
    if let Some(code) = directory.diagnostic() {
        eprintln!("March 7th configuration unavailable ({code}); session-only/read-only mode");
    }
    app.manage(directory);
    app.run(|app, event| {
        if matches!(
            &event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            characters::stop(app);
            reminders::ui::stop(app);
            reminders::stop(app);
        }
        desktop::on_run_event(app, event);
    });
}
