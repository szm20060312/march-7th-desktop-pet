use super::geometry::{self, Monitor, Placement};
use tauri::{LogicalPosition, PhysicalPosition, Position};

/// macOS monitor rectangles use each monitor's scale; window coordinates use
/// the window's current scale. Windows uses one global physical desktop.
#[derive(Clone, Copy)]
pub enum Coordinates {
    Physical,
    Mac,
}
pub const NATIVE: Coordinates = if cfg!(target_os = "macos") {
    Coordinates::Mac
} else {
    Coordinates::Physical
};

type Rectangle = ((i32, i32), (u32, u32));

fn valid_scale(scale: f64) -> bool {
    scale.is_finite() && scale > 0.0
}

impl Coordinates {
    pub fn rect(
        self,
        position: (i32, i32),
        size: (u32, u32),
        source: f64,
        target: f64,
    ) -> Option<Rectangle> {
        if !valid_scale(source) || !valid_scale(target) {
            return None;
        }
        if matches!(self, Self::Physical) || source == target {
            return Some((position, size));
        }
        let ratio = target / source;
        let point = |value: i32| {
            let v = (f64::from(value) * ratio).round();
            if v.is_finite() && v >= f64::from(i32::MIN) && v <= f64::from(i32::MAX) {
                Some(v as i32)
            } else {
                None
            }
        };
        // Conservative outer bounds when backing scales have a fractional ratio.
        let extent = |value: u32| {
            let v = (f64::from(value) * ratio).ceil();
            if v.is_finite() && v >= 1.0 && v <= f64::from(u32::MAX) {
                Some(v as u32)
            } else {
                None
            }
        };
        Some((
            (point(position.0)?, point(position.1)?),
            (extent(size.0)?, extent(size.1)?),
        ))
    }
    pub fn position(self, position: (i32, i32), scale: f64) -> Option<Position> {
        if !valid_scale(scale) {
            return None;
        }
        Some(match self {
            Self::Physical => PhysicalPosition::new(position.0, position.1).into(),
            Self::Mac => {
                LogicalPosition::new(f64::from(position.0) / scale, f64::from(position.1) / scale)
                    .into()
            }
        })
    }
    pub fn reachable(
        self,
        position: (i32, i32),
        size: (u32, u32),
        scale: f64,
        monitors: &[Monitor],
    ) -> bool {
        if !valid_scale(scale) {
            return false;
        }
        match self {
            Self::Physical => geometry::reachable(position, size, monitors),
            Self::Mac => monitors
                .iter()
                .filter(|m| geometry::select_monitor(None, std::slice::from_ref(*m)).is_some())
                .any(|m| {
                    // Compare unrounded global logical bounds. Per-monitor physical
                    // origins must never be treated as a shared physical desktop.
                    let axis = |p: i32, s: u32, o: i32, extent: u32| {
                        let offset = f64::from(p) / scale - f64::from(o) / m.scale;
                        let free = (f64::from(extent) / m.scale - f64::from(s) / scale).max(0.0);
                        offset >= 0.0 && offset <= free
                    };
                    axis(position.0, size.0, m.origin.0, m.size.0)
                        && axis(position.1, size.1, m.origin.1, m.size.1)
                }),
        }
    }
    pub fn capture(self, position: (i32, i32), scale: f64, monitor: &Monitor) -> Option<Placement> {
        // Capturing only settled observations keeps v1 monitor-relative logical
        // offsets exact, including half-points on Retina. Never save a mixture.
        if !valid_scale(scale) || (scale - monitor.scale).abs() > 1e-6 {
            return None;
        }
        geometry::capture(position, monitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn screen(
        name: &str,
        origin: (i32, i32),
        size: (u32, u32),
        scale: f64,
        primary: bool,
    ) -> Monitor {
        Monitor {
            name: Some(name.into()),
            origin,
            size,
            scale,
            primary,
        }
    }
    // Emulates Tao's locked macOS setter: physical inputs are divided by the
    // CURRENT window scale. Logical inputs are global Cocoa points directly.
    fn cocoa_position(position: Position, current_scale: f64) -> (f64, f64) {
        let p = position.to_logical::<f64>(current_scale);
        (p.x, p.y)
    }
    #[test]
    fn mac_adapter_setter_crosses_both_scale_directions_and_negative_origins() {
        for (current, target, physical, logical) in [
            (2.0, 1.0, (2280, 100), (2280.0, 100.0)),
            (1.0, 2.0, (4080, -200), (2040.0, -100.0)),
            (2.0, 1.0, (-1800, -100), (-1800.0, -100.0)),
        ] {
            assert_eq!(
                cocoa_position(
                    Coordinates::Mac.position(physical, target).unwrap(),
                    current
                ),
                logical
            );
        }
    }
    #[test]
    fn mac_adapter_reachability_and_live_clamp_use_per_target_projection() {
        let retina = screen("retina", (0, 0), (2880, 1800), 2.0, true);
        let external = screen("external", (1440, 0), (1920, 1080), 1.0, false);
        // Actual global logical x=1300 extends past retina's 1440 edge. Comparing
        // raw physical x=2600 against external's rect would incorrectly accept it.
        assert!(!Coordinates::Mac.reachable(
            (2600, 100),
            (480, 520),
            2.0,
            &[retina.clone(), external]
        ));
        let (p, s) = Coordinates::Mac
            .rect((2600, 100), (480, 520), 2.0, retina.scale)
            .unwrap();
        assert_eq!(geometry::clamp_to_monitor(p, s, &retina), Some((2400, 100)));
        let left = screen("left", (-3840, -200), (3840, 2160), 2.0, false);
        assert!(Coordinates::Mac.reachable((-1700, -50), (240, 260), 1.0, &[left]));
    }
    #[test]
    fn mac_adapter_capture_preserves_v1_offsets_and_rejects_unsettled_scale() {
        let monitor = screen("retina", (-3840, -200), (3840, 2160), 2.0, false);
        let p = Coordinates::Mac
            .capture((-3639, -99), 2.0, &monitor)
            .unwrap();
        assert_eq!((p.x, p.y), (100.5, 50.5));
        assert_eq!(
            geometry::restore(Some(&p), std::slice::from_ref(&monitor), (480, 520)),
            Some((-3639, -99))
        );
        assert!(Coordinates::Mac
            .capture((-1800, -50), 1.0, &monitor)
            .is_none());
    }
    #[test]
    fn adapter_windows_keeps_global_physical_and_checked_invalid_conversions() {
        assert_eq!(
            Coordinates::Physical.rect((-1700, 90), (300, 325), 1.25, 2.0),
            Some(((-1700, 90), (300, 325)))
        );
        assert!(matches!(
            Coordinates::Physical.position((-1700, 90), 2.0),
            Some(Position::Physical(_))
        ));
        assert!(Coordinates::Mac
            .rect((i32::MAX, 0), (240, 260), 1.0, 2.0)
            .is_none());
        assert!(Coordinates::Mac.position((0, 0), f64::NAN).is_none());
    }
    #[test]
    fn mac_adapter_native_setter_model_settles_restore_and_reset_round_trips() {
        use super::super::state::{StartupAction, StartupRestore, WindowGeometry};
        for (source_scale, target_scale, target_x) in
            [(2.0, 1.0, 1440), (1.0, 2.0, 1440), (1.0, 2.0, -1920)]
        {
            for reset in [false, true] {
                let source = screen(
                    "source",
                    (0, 0),
                    (
                        (1440.0 * source_scale) as u32,
                        (900.0 * source_scale) as u32,
                    ),
                    source_scale,
                    false,
                );
                let target = screen(
                    "target",
                    (
                        (f64::from(target_x) * target_scale) as i32,
                        (-100.0 * target_scale) as i32,
                    ),
                    (
                        (1920.0 * target_scale) as u32,
                        (1080.0 * target_scale) as u32,
                    ),
                    target_scale,
                    true,
                );
                let inner = ((240.0 * source_scale) as u32, (260.0 * source_scale) as u32);
                let startup = StartupRestore {
                    original: (!reset).then(|| Placement {
                        monitor_name: target.name.clone(),
                        x: 1600.5,
                        y: 100.5,
                    }),
                    source_inner: inner,
                    source_scale,
                    show: true,
                    fallback: false,
                };
                let mut window = WindowGeometry {
                    monitor: Some(source.clone()),
                    scale: source_scale,
                    inner,
                    outer: inner,
                    position: (0, 0),
                };
                let monitors = [source.clone(), target.clone()];
                let mut ready = false;
                for _ in 0..10 {
                    match startup
                        .step_with_coordinates(Coordinates::Mac, &monitors, &window)
                        .unwrap()
                    {
                        StartupAction::Move(p, target_units) => {
                            let logical = cocoa_position(
                                Coordinates::Mac.position(p, target_units).unwrap(),
                                window.scale,
                            );
                            // The controlled host selects by actual global logical
                            // center, not by manually assigning the requested monitor.
                            let center = logical.0 + f64::from(window.outer.0) / window.scale / 2.0;
                            let owner = if center >= f64::from(target_x)
                                && center < f64::from(target_x + 1920)
                            {
                                &target
                            } else {
                                &source
                            };
                            window.monitor = Some(owner.clone());
                            window.scale = owner.scale;
                            window.position = (
                                (logical.0 * owner.scale).round() as i32,
                                (logical.1 * owner.scale).round() as i32,
                            );
                        }
                        StartupAction::Resize(size) => {
                            window.inner = size;
                            window.outer = size;
                        }
                        StartupAction::Ready => {
                            ready = true;
                            break;
                        }
                        other => panic!("unexpected stalled/fallback restore: {other:?}"),
                    }
                }
                assert!(
                    ready,
                    "native setter model must converge without retry timing assumptions"
                );
                assert_eq!(window.monitor.as_ref().unwrap().name, target.name);
                assert!(Coordinates::Mac.reachable(
                    window.position,
                    window.outer,
                    window.scale,
                    &monitors
                ));
                let saved = Coordinates::Mac
                    .capture(window.position, window.scale, &target)
                    .unwrap();
                if reset {
                    assert_eq!((saved.x, saved.y), (840.0, 410.0));
                } else {
                    let tolerance = 0.5 / target_scale;
                    assert!(
                        (saved.x - 1600.5).abs() <= tolerance
                            && (saved.y - 100.5).abs() <= tolerance
                    );
                }
                assert_eq!(
                    geometry::restore(Some(&saved), std::slice::from_ref(&target), window.outer),
                    Some(window.position)
                );
            }
        }
    }
}
