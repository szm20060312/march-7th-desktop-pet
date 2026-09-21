use std::{
    error::Error,
    io::{Error as IoError, ErrorKind},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use tauri::{
    menu::{MenuBuilder, MenuEvent},
    tray::TrayIconBuilder,
    App, AppHandle, Builder, Manager, PhysicalPosition, Runtime, WebviewWindow, Window,
    WindowEvent,
};

const MAIN_WINDOW_LABEL: &str = "main";
const SHOW_ID: &str = "show";
const HIDE_ID: &str = "hide";
const RESET_POSITION_ID: &str = "reset-position";
const QUIT_ID: &str = "quit";

pub fn configure<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    let tray_ready = Arc::new(AtomicBool::new(false));
    let setup_ready = Arc::clone(&tray_ready);

    builder
        .setup(move |app| {
            setup_tray(app)?;
            setup_ready.store(true, Ordering::Release);
            Ok(())
        })
        .on_window_event(move |window, event| {
            if tray_ready.load(Ordering::Acquire) {
                handle_window_event(window, event);
            }
        })
}

fn setup_tray<R: Runtime>(app: &mut App<R>) -> Result<(), Box<dyn Error>> {
    let icon = app.default_window_icon().cloned().ok_or_else(|| {
        IoError::new(
            ErrorKind::NotFound,
            "the configured default window icon is unavailable for the tray",
        )
    })?;
    let menu = MenuBuilder::new(app)
        .text(SHOW_ID, "显示")
        .text(HIDE_ID, "隐藏")
        .text(RESET_POSITION_ID, "重置位置")
        .separator()
        .text(QUIT_ID, "退出")
        .build()?;

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .on_menu_event(handle_menu_event)
        .build(app)?;

    Ok(())
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    let operation = if event.id() == SHOW_ID {
        Some(("show", show_window(app)))
    } else if event.id() == HIDE_ID {
        Some(("hide", hide_window(app)))
    } else if event.id() == RESET_POSITION_ID {
        Some(("reset-position", reset_position(app)))
    } else if event.id() == QUIT_ID {
        app.exit(0);
        None
    } else {
        None
    };

    if let Some((operation, Err(error))) = operation {
        report_failure(operation, error);
    }
}

fn handle_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if window.label() != MAIN_WINDOW_LABEL {
        return;
    }

    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        if let Err(error) = window.hide() {
            report_failure("close-hide", error);
        }
    }
}

fn show_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = main_window(app)?;
    show_non_focusable_window(&window)
}

fn hide_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    main_window(app)?.hide().map_err(|error| error.to_string())
}

fn reset_position<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = main_window(app)?;
    let monitor = match window.primary_monitor() {
        Ok(Some(monitor)) => monitor,
        Ok(None) => window
            .current_monitor()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "neither a primary nor current monitor is available".to_string())?,
        Err(primary_error) => window
            .current_monitor()
            .map_err(|current_error| {
                format!(
                    "primary monitor query failed ({primary_error}); current monitor query failed ({current_error})"
                )
            })?
            .ok_or_else(|| {
                format!(
                    "primary monitor query failed ({primary_error}) and the current monitor is unavailable"
                )
            })?,
    };
    let window_size = window.outer_size().map_err(|error| error.to_string())?;
    let monitor_origin = monitor.position();
    let monitor_size = monitor.size();
    let position = PhysicalPosition::new(
        centered_axis(monitor_origin.x, monitor_size.width, window_size.width),
        centered_axis(monitor_origin.y, monitor_size.height, window_size.height),
    );

    window
        .set_position(position)
        .map_err(|error| error.to_string())?;
    show_non_focusable_window(&window)
}

fn show_non_focusable_window<R: Runtime>(window: &WebviewWindow<R>) -> Result<(), String> {
    window
        .set_focusable(false)
        .map_err(|error| format!("could not keep the main window non-focusable: {error}"))?;
    window.show().map_err(|error| error.to_string())
}

fn main_window<R: Runtime>(app: &AppHandle<R>) -> Result<WebviewWindow<R>, String> {
    app.get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| format!("window `{MAIN_WINDOW_LABEL}` is unavailable"))
}

fn report_failure(operation: &str, error: impl std::fmt::Display) {
    eprintln!("March 7th desktop operation `{operation}` failed: {error}");
}

fn centered_axis(origin: i32, monitor_extent: u32, window_extent: u32) -> i32 {
    let centered = i64::from(origin) + (i64::from(monitor_extent) - i64::from(window_extent)) / 2;
    centered.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centers_on_a_normal_monitor() {
        assert_eq!(centered_axis(0, 1920, 240), 840);
        assert_eq!(centered_axis(0, 1080, 260), 410);
    }

    #[test]
    fn centers_relative_to_a_nonzero_origin() {
        assert_eq!(centered_axis(1920, 2560, 240), 3080);
        assert_eq!(centered_axis(180, 1440, 260), 770);
    }

    #[test]
    fn centers_on_a_negative_origin_monitor() {
        assert_eq!(centered_axis(-1920, 1920, 240), -1080);
        assert_eq!(centered_axis(-1080, 1080, 260), -670);
    }

    #[test]
    fn oversized_windows_remain_centered_without_underflow() {
        assert_eq!(centered_axis(0, 200, 240), -20);
        assert_eq!(centered_axis(-500, 100, 300), -600);
    }

    #[test]
    fn extreme_inputs_saturate_instead_of_overflowing() {
        assert_eq!(centered_axis(i32::MIN, 0, u32::MAX), i32::MIN);
        assert_eq!(centered_axis(i32::MAX, u32::MAX, 0), i32::MAX);
    }

    #[test]
    fn main_window_is_configured_not_to_take_focus() {
        let context = crate::app_context();
        let main_window = context
            .config()
            .app
            .windows
            .iter()
            .find(|window| window.label == MAIN_WINDOW_LABEL)
            .expect("main window config should exist");

        assert!(!main_window.focus, "main window must not start focused");
        assert!(
            !main_window.focusable,
            "main window must remain non-focusable when shown from the tray"
        );
    }
}
