mod platform;

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CursorPosition {
    x: f64,
    y: f64,
}

#[tauri::command]
fn cursor_relative_to_window(window: tauri::WebviewWindow) -> Result<CursorPosition, String> {
    let cursor = platform::cursor_position()?;
    let window_origin = window.outer_position().map_err(|error| error.to_string())?;
    let scale_factor = window.scale_factor().map_err(|error| error.to_string())?;
    let relative = platform::relative_to_window(cursor, window_origin, scale_factor);

    Ok(CursorPosition {
        x: relative.x,
        y: relative.y,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![cursor_relative_to_window])
        .run(tauri::generate_context!())
        .expect("error while running March 7th");
}
