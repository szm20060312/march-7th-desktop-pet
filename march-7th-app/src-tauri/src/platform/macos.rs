use std::ffi::c_void;

use tauri::PhysicalPosition;

use super::ScreenPoint;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreate(source: *const c_void) -> *mut c_void;
    fn CGEventGetLocation(event: *mut c_void) -> CGPoint;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: *const c_void);
}

pub fn cursor_position() -> Result<ScreenPoint, String> {
    // Quartz reports global cursor coordinates in logical screen points.
    let event = unsafe { CGEventCreate(std::ptr::null()) };
    if event.is_null() {
        return Err("CoreGraphics could not create a cursor event".into());
    }

    let location = unsafe { CGEventGetLocation(event) };
    unsafe { CFRelease(event) };

    Ok(ScreenPoint {
        x: location.x,
        y: location.y,
    })
}

pub fn relative_to_window(
    cursor: ScreenPoint,
    window_origin: PhysicalPosition<i32>,
    scale_factor: f64,
) -> ScreenPoint {
    ScreenPoint {
        x: cursor.x - f64::from(window_origin.x) / scale_factor,
        y: cursor.y - f64::from(window_origin.y) / scale_factor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_physical_window_origin_to_logical_points() {
        let relative = relative_to_window(
            ScreenPoint { x: 500.0, y: 400.0 },
            PhysicalPosition::new(400, 300),
            2.0,
        );

        assert_eq!(relative.x, 300.0);
        assert_eq!(relative.y, 250.0);
    }
}
