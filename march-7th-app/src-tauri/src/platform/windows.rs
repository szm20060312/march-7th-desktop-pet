use tauri::PhysicalPosition;

use super::ScreenPoint;

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetCursorPos(point: *mut Point) -> i32;
}

pub fn cursor_position() -> Result<ScreenPoint, String> {
    let mut point = Point { x: 0, y: 0 };
    let succeeded = unsafe { GetCursorPos(&mut point) };

    if succeeded == 0 {
        return Err("GetCursorPos failed".into());
    }

    Ok(ScreenPoint {
        x: f64::from(point.x),
        y: f64::from(point.y),
    })
}

pub fn relative_to_window(
    cursor: ScreenPoint,
    window_origin: PhysicalPosition<i32>,
    scale_factor: f64,
) -> ScreenPoint {
    ScreenPoint {
        x: (cursor.x - f64::from(window_origin.x)) / scale_factor,
        y: (cursor.y - f64::from(window_origin.y)) / scale_factor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_physical_delta_to_logical_points() {
        let relative = relative_to_window(
            ScreenPoint { x: 500.0, y: 400.0 },
            PhysicalPosition::new(200, 100),
            1.5,
        );

        assert_eq!(relative.x, 200.0);
        assert_eq!(relative.y, 200.0);
    }
}
