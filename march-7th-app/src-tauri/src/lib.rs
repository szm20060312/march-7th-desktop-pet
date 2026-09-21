mod desktop;
mod platform;

use serde::Serialize;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    desktop::configure(tauri::Builder::default())
        .invoke_handler(tauri::generate_handler![cursor_relative_to_window])
        .run(tauri::generate_context!())
        .expect("error while running March 7th");
}
