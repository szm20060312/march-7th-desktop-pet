use crate::desktop::{coordinates::Coordinates, geometry::Monitor};

#[derive(Clone, Debug, PartialEq)]
pub struct Observation {
    pub position: (i32, i32),
    pub inner: (u32, u32),
    pub outer: (u32, u32),
    pub scale: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub position: (i32, i32),
    pub inner: (u32, u32),
    pub scale: f64,
}
pub fn fully_reachable(
    coordinates: Coordinates,
    window: &Observation,
    monitors: &[Monitor],
) -> bool {
    monitors.iter().any(|m| {
        let Some((p, s)) = coordinates.rect(window.position, window.outer, window.scale, m.scale)
        else {
            return false;
        };
        let axis = |p: i32, s: u32, origin: i32, size: u32| {
            s <= size
                && i64::from(p) >= i64::from(origin)
                && i64::from(p) + i64::from(s) <= i64::from(origin) + i64::from(size)
        };
        axis(p.0, s.0, m.origin.0, m.size.0) && axis(p.1, s.1, m.origin.1, m.size.1)
    })
}
fn reminder_height(rows: usize) -> f64 {
    220.0 + (rows.saturating_sub(1).min(2) as f64) * 40.0
}
pub fn place(
    coordinates: Coordinates,
    monitor: &Monitor,
    anchor: &Observation,
    window: &Observation,
    settings: bool,
    reminder_rows: usize,
    choices: bool,
) -> Option<Placement> {
    if !window.scale.is_finite()
        || window.scale <= 0.0
        || !monitor.scale.is_finite()
        || monitor.scale <= 0.0
    {
        return None;
    }
    let (anchor_position, anchor_size) =
        coordinates.rect(anchor.position, anchor.outer, anchor.scale, monitor.scale)?;
    let frame = |outer: u32, inner: u32| {
        ((outer.saturating_sub(inner) as f64) * monitor.scale / window.scale).ceil() as u32
    };
    let frame = (
        frame(window.outer.0, window.inner.0),
        frame(window.outer.1, window.inner.1),
    );
    let available = (
        monitor.size.0.checked_sub(frame.0)?,
        monitor.size.1.checked_sub(frame.1)?,
    );
    if available.0 == 0 || available.1 == 0 {
        return None;
    }
    let desired = if settings {
        (460.0, 620.0)
    } else if choices {
        (220.0, 70.0)
    } else {
        (320.0, reminder_height(reminder_rows))
    };
    let inner = (
        ((desired.0 * monitor.scale).round() as u32).min(available.0),
        ((desired.1 * monitor.scale).round() as u32).min(available.1),
    );
    let outer = (inner.0 + frame.0, inner.1 + frame.1);
    let origin = (i64::from(monitor.origin.0), i64::from(monitor.origin.1));
    let max = (
        origin.0 + i64::from(monitor.size.0 - outer.0),
        origin.1 + i64::from(monitor.size.1 - outer.1),
    );
    let gap = ((if choices { 4.0 } else { 12.0 }) * monitor.scale).round() as i64;
    let (x, y) = if settings {
        ((origin.0 + max.0) / 2, (origin.1 + max.1) / 2)
    } else {
        let above = i64::from(anchor_position.1) - i64::from(outer.1) - gap;
        let below = i64::from(anchor_position.1) + i64::from(anchor_size.1) + gap;
        (
            i64::from(anchor_position.0) + (i64::from(anchor_size.0) - i64::from(outer.0)) / 2,
            if choices && below <= max.1 {
                below
            } else if above >= origin.1 {
                above
            } else {
                below
            },
        )
    };
    Some(Placement {
        position: (
            i32::try_from(x.clamp(origin.0, max.0)).ok()?,
            i32::try_from(y.clamp(origin.1, max.1)).ok()?,
        ),
        inner,
        scale: monitor.scale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn speech_bubble_height_tracks_visible_rows_without_growing_unbounded() {
        assert_eq!(reminder_height(0), 220.0);
        assert_eq!(reminder_height(1), 220.0);
        assert_eq!(reminder_height(2), 260.0);
        assert_eq!(reminder_height(3), 300.0);
        assert_eq!(reminder_height(8), 300.0);
    }
    #[test]
    fn transparent_choices_stay_beside_the_character_and_flip_above_near_screen_bottom() {
        let m = screen((0, 0), (1920, 1080), 1.0);
        let w = window((0, 0), (320, 220), 1.0);
        let mut anchor = window((500, 400), (240, 260), 1.0);
        let below = place(Coordinates::Physical, &m, &anchor, &w, false, 1, true).unwrap();
        assert_eq!(below.inner, (220, 70));
        assert_eq!(below.position, (510, 664));
        anchor.position.1 = 900;
        let above = place(Coordinates::Physical, &m, &anchor, &w, false, 1, true).unwrap();
        assert_eq!(above.position, (510, 826));
    }
    fn screen(origin: (i32, i32), size: (u32, u32), scale: f64) -> Monitor {
        Monitor {
            name: None,
            origin,
            size,
            scale,
            primary: true,
        }
    }
    fn window(position: (i32, i32), size: (u32, u32), scale: f64) -> Observation {
        Observation {
            position,
            inner: size,
            outer: size,
            scale,
        }
    }
    #[test]
    fn bubble_prefers_above_then_below_and_clamps_negative_desktop() {
        let m = screen((-1920, -100), (1920, 1080), 1.0);
        let w = window((0, 0), (320, 300), 1.0);
        let mut anchor = window((-1000, 500), (240, 260), 1.0);
        let above = place(Coordinates::Physical, &m, &anchor, &w, false, 3, false).unwrap();
        assert_eq!(above.position, (-1040, 188));
        anchor.position = (-1900, -90);
        let below = place(Coordinates::Physical, &m, &anchor, &w, false, 3, false).unwrap();
        assert_eq!(below.position, (-1920, 182));
    }
    #[test]
    fn mac_projects_anchor_from_its_scale_into_target_monitor_units() {
        let m = screen((-3840, -200), (3840, 2160), 2.0);
        let anchor = window((-1700, 500), (240, 260), 1.0);
        let w = window((0, 0), (320, 300), 1.0);
        let p = place(Coordinates::Mac, &m, &anchor, &w, false, 3, false).unwrap();
        assert_eq!(p.position, (-3480, 376));
        assert_eq!(p.inner, (640, 600));
        assert!(Coordinates::Mac.reachable(p.position, p.inner, 2.0, &[m]));
    }
    #[test]
    fn tiny_work_area_shrinks_both_viewports_using_actual_outer_frame() {
        let m = screen((-300, 20), (300, 200), 2.0);
        let anchor = window((-200, 20), (240, 260), 2.0);
        let w = Observation {
            position: (0, 0),
            inner: (460, 620),
            outer: (476, 659),
            scale: 1.0,
        };
        for settings in [false, true] {
            let p = place(Coordinates::Physical, &m, &anchor, &w, settings, 3, false).unwrap();
            assert_eq!(p.inner, (268, 122));
            assert_eq!(p.position, (-300, 20));
        }
    }
    #[test]
    fn settings_high_dpi_is_centered_and_frame_remains_reachable() {
        let m = screen((0, 0), (1200, 900), 2.0);
        let anchor = window((200, 300), (480, 520), 2.0);
        let w = Observation {
            position: (0, 0),
            inner: (920, 1240),
            outer: (936, 1279),
            scale: 2.0,
        };
        let p = place(Coordinates::Physical, &m, &anchor, &w, true, 0, false).unwrap();
        assert_eq!(p.inner, (920, 861));
        assert_eq!(p.position, (132, 0));
    }
    #[test]
    fn invalid_scale_or_unusable_frame_fails_without_showing() {
        let m = screen((0, 0), (10, 10), 1.0);
        let w = Observation {
            position: (0, 0),
            inner: (100, 100),
            outer: (120, 140),
            scale: 1.0,
        };
        assert!(place(Coordinates::Physical, &m, &w, &w, true, 0, false).is_none());
    }
    #[test]
    fn topology_shrink_does_not_accept_an_oversized_window_at_work_area_origin() {
        let m = screen((-300, 20), (300, 200), 1.0);
        let w = window((-300, 20), (320, 300), 1.0);
        assert!(!fully_reachable(
            Coordinates::Physical,
            &w,
            std::slice::from_ref(&m)
        ));
        assert!(!fully_reachable(Coordinates::Mac, &w, &[m]));
    }
}
