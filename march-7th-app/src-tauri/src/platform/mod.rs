#[derive(Clone, Copy, Debug)]
pub struct ScreenPoint {
    pub x: f64,
    pub y: f64,
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{cursor_position, relative_to_window};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::{cursor_position, relative_to_window};

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
compile_error!("March 7th currently supports macOS and Windows only");
