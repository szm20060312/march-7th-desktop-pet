use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub monitor_name: Option<String>,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub name: Option<String>,
    pub origin: (i32, i32),
    pub size: (u32, u32),
    pub scale: f64,
    pub primary: bool,
}

fn valid(m: &Monitor) -> bool {
    m.scale.is_finite()
        && m.scale > 0.0
        && m.size.0 > 0
        && m.size.1 > 0
        && i64::from(m.origin.0) + i64::from(m.size.0) <= i64::from(i32::MAX)
        && i64::from(m.origin.1) + i64::from(m.size.1) <= i64::from(i32::MAX)
}

pub fn capture(position: (i32, i32), monitor: &Monitor) -> Option<Placement> {
    if !valid(monitor) {
        return None;
    }
    let x = (i64::from(position.0) - i64::from(monitor.origin.0)) as f64 / monitor.scale;
    let y = (i64::from(position.1) - i64::from(monitor.origin.1)) as f64 / monitor.scale;
    (x.is_finite() && y.is_finite()).then(|| Placement {
        monitor_name: monitor.name.clone(),
        x,
        y,
    })
}

fn axis(origin: i32, extent: u32, window: u32, offset: Option<i64>) -> Option<i32> {
    let free = i64::from(extent.saturating_sub(window));
    let relative = offset.unwrap_or(free / 2).clamp(0, free);
    i32::try_from(i64::from(origin).checked_add(relative)?).ok()
}

fn physical_offset(value: f64, scale: f64) -> Option<i64> {
    let value = (value * scale).round();
    // Any native physical coordinate difference fits in this range. Checking before
    // the float-to-integer cast avoids Rust's saturating cast hiding invalid data.
    if value.is_finite() && value.abs() <= f64::from(u32::MAX) {
        Some(value as i64)
    } else {
        None
    }
}

pub fn after_scale(
    previous: Option<&Placement>,
    current: &Monitor,
    position: (i32, i32),
    monitors: &[Monitor],
) -> Option<Placement> {
    // Preserve logical offset for a DPI change on the same identified display.
    // Crossing to another/ambiguous display must not snap back to the old screen.
    if let Some(previous) = previous {
        if current.name.is_some()
            && previous.monitor_name == current.name
            && monitors.iter().filter(|m| m.name == current.name).count() == 1
        {
            return Some(previous.clone());
        }
    }
    capture(position, current)
}

pub fn restore(
    placement: Option<&Placement>,
    monitors: &[Monitor],
    size: (u32, u32),
) -> Option<(i32, i32)> {
    let monitor = select_monitor(placement, monitors)?;
    restore_on_monitor(placement, monitor, size)
}

pub fn select_monitor<'a>(
    placement: Option<&Placement>,
    monitors: &'a [Monitor],
) -> Option<&'a Monitor> {
    let valid_monitors: Vec<_> = monitors.iter().filter(|m| valid(m)).collect();
    let unique = placement
        .and_then(|p| p.monitor_name.as_ref())
        .and_then(|name| {
            let mut matches = valid_monitors
                .iter()
                .copied()
                .filter(|m| m.name.as_ref() == Some(name));
            let first = matches.next()?;
            matches.next().is_none().then_some(first)
        });
    unique
        .or_else(|| valid_monitors.iter().copied().find(|m| m.primary))
        .or_else(|| valid_monitors.first().copied())
}

fn restore_on_monitor(
    placement: Option<&Placement>,
    monitor: &Monitor,
    size: (u32, u32),
) -> Option<(i32, i32)> {
    let offsets = placement.and_then(|p| {
        Some((
            physical_offset(p.x, monitor.scale)?,
            physical_offset(p.y, monitor.scale)?,
        ))
    });
    Some((
        axis(
            monitor.origin.0,
            monitor.size.0,
            size.0,
            offsets.map(|o| o.0),
        )?,
        axis(
            monitor.origin.1,
            monitor.size.1,
            size.1,
            offsets.map(|o| o.1),
        )?,
    ))
}

pub fn rescaled_size(size: (u32, u32), source_scale: f64, target_scale: f64) -> Option<(u32, u32)> {
    if !source_scale.is_finite()
        || source_scale <= 0.0
        || !target_scale.is_finite()
        || target_scale <= 0.0
    {
        return None;
    }
    let axis = |value: u32| {
        let physical = (f64::from(value) / source_scale * target_scale).round();
        if physical.is_finite() && physical >= 1.0 && physical <= f64::from(u32::MAX) {
            Some(physical as u32)
        } else {
            None
        }
    };
    Some((axis(size.0)?, axis(size.1)?))
}

pub fn reachable(position: (i32, i32), size: (u32, u32), monitors: &[Monitor]) -> bool {
    monitors.iter().filter(|m| valid(m)).any(|m| {
        let x = i64::from(position.0) - i64::from(m.origin.0);
        let y = i64::from(position.1) - i64::from(m.origin.1);
        (0..=i64::from(m.size.0.saturating_sub(size.0))).contains(&x)
            && (0..=i64::from(m.size.1.saturating_sub(size.1))).contains(&y)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(name: &str, origin: (i32, i32), scale: f64, primary: bool) -> Monitor {
        Monitor {
            name: Some(name.into()),
            origin,
            size: (1920, 1040),
            scale,
            primary,
        }
    }

    #[test]
    fn negative_origin_and_mixed_scale_round_trip() {
        let source = screen("left", (-1920, -100), 1.5, false);
        let saved = capture((-1770, 50), &source).unwrap();
        assert_eq!((saved.x, saved.y), (100.0, 100.0));
        let target = screen("left", (-2560, 50), 2.0, false);
        assert_eq!(
            restore(Some(&saved), &[target], (240, 260)),
            Some((-2360, 250))
        );
    }

    #[test]
    fn dpi_change_preserves_offsets_but_cross_screen_move_uses_destination() {
        let left = screen("left", (-1920, 0), 1.0, false);
        let main = screen("main", (0, 40), 2.0, true);
        let saved = capture((-1820, 100), &left).unwrap();
        let current = after_scale(
            Some(&saved),
            &main,
            (200, 240),
            &[left.clone(), main.clone()],
        )
        .unwrap();
        assert_eq!(current.monitor_name, main.name);
        assert_eq!((current.x, current.y), (100.0, 100.0));
        let scaled = Monitor { scale: 2.0, ..left };
        assert_eq!(
            after_scale(
                Some(&saved),
                &scaled,
                (-1800, 200),
                std::slice::from_ref(&scaled)
            ),
            Some(saved)
        );
    }

    #[test]
    fn identity_requires_unique_match_otherwise_primary_then_first() {
        let saved = Placement {
            monitor_name: Some("left".into()),
            x: 100.0,
            y: 50.0,
        };
        let left = screen("left", (-1920, 0), 1.0, false);
        let primary = screen("main", (0, 40), 1.0, true);
        assert_eq!(
            restore(Some(&saved), &[left.clone(), primary.clone()], (240, 260)),
            Some((-1820, 50))
        );
        assert_eq!(
            restore(Some(&saved), std::slice::from_ref(&primary), (240, 260)),
            Some((100, 90))
        );
        assert_eq!(
            restore(
                Some(&saved),
                &[left.clone(), primary.clone(), left.clone()],
                (240, 260)
            ),
            Some((100, 90))
        );
        let unnamed = Placement {
            monitor_name: None,
            ..saved
        };
        assert_eq!(
            restore(Some(&unnamed), &[left.clone(), primary], (240, 260)),
            Some((100, 90))
        );
        assert_eq!(
            restore(Some(&unnamed), &[left], (240, 260)),
            Some((-1820, 50))
        );
    }

    #[test]
    fn clamps_whole_rectangle_and_anchors_oversized_axes() {
        let monitor = screen("main", (-400, 40), 1.0, true);
        let far = Placement {
            monitor_name: None,
            x: 9999.0,
            y: -100.0,
        };
        assert_eq!(
            restore(Some(&far), std::slice::from_ref(&monitor), (240, 260)),
            Some((1280, 40))
        );
        assert_eq!(
            restore(None, std::slice::from_ref(&monitor), (3000, 2000)),
            Some((-400, 40))
        );
        assert!(reachable(
            (-400, 40),
            (3000, 2000),
            std::slice::from_ref(&monitor)
        ));
        assert!(!reachable((-401, 40), (3000, 2000), &[monitor]));
    }

    #[test]
    fn invalid_and_overflow_offsets_center_safely() {
        let monitor = screen("main", (-1920, 40), 2.0, true);
        for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX, 1e20] {
            let saved = Placement {
                monitor_name: None,
                x,
                y: 1.0,
            };
            assert_eq!(
                restore(Some(&saved), std::slice::from_ref(&monitor), (240, 260)),
                Some((-1080, 430))
            );
        }
        assert!(restore(None, &[], (240, 260)).is_none());
        let invalid = Monitor {
            scale: 0.0,
            ..monitor
        };
        assert!(capture((0, 0), &invalid).is_none());
        assert!(restore(None, &[invalid], (240, 260)).is_none());
    }

    #[test]
    fn extreme_native_coordinates_never_narrow_unchecked() {
        let monitor = Monitor {
            origin: (i32::MAX - 20, i32::MIN),
            size: (u32::MAX, u32::MAX),
            ..screen("extreme", (0, 0), 1.0, true)
        };
        assert!(restore(None, &[monitor], (1, 1)).is_none());
    }
}
