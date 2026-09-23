use super::geometry::{Monitor, Placement};
use std::time::{Duration, Instant};

pub const SAVE_DELAY: Duration = Duration::from_millis(500);
#[cfg(any(target_os = "macos", test))]
pub const SPACE_SWITCH_DELAY: Duration = Duration::from_millis(80);

#[derive(Clone)]
pub struct StartupRestore {
    pub original: Option<Placement>,
    pub source_inner: (u32, u32),
    pub source_scale: f64,
    pub show: bool,
    pub fallback: bool,
}
pub struct WindowGeometry {
    pub monitor: Option<Monitor>,
    pub scale: f64,
    pub inner: (u32, u32),
    pub outer: (u32, u32),
    pub position: (i32, i32),
}
#[derive(Debug, PartialEq)]
pub enum StartupAction {
    Move((i32, i32), f64),
    Resize((u32, u32)),
    Fallback((i32, i32), f64),
    Wait,
    Ready,
}
impl StartupRestore {
    #[cfg(test)]
    pub fn step(&self, monitors: &[Monitor], window: &WindowGeometry) -> Option<StartupAction> {
        self.step_with_coordinates(super::coordinates::Coordinates::Physical, monitors, window)
    }
    pub fn step_with_coordinates(
        &self,
        coordinates: super::coordinates::Coordinates,
        monitors: &[Monitor],
        window: &WindowGeometry,
    ) -> Option<StartupAction> {
        use super::geometry;
        let requested = geometry::select_monitor(self.original.as_ref(), monitors)?;
        let current =
            geometry::select_monitor(None, std::slice::from_ref(window.monitor.as_ref()?))?;
        let target = if self.fallback { current } else { requested };
        if !window.scale.is_finite() || window.scale <= 0.0 {
            return Some(StartupAction::Wait);
        }
        let at_target_scale = (window.scale - target.scale).abs() <= 1e-6
            && (current.scale - target.scale).abs() <= 1e-6;
        let (target_position, target_outer) =
            coordinates.rect(window.position, window.outer, window.scale, target.scale)?;
        if !at_target_scale {
            if self.fallback || (current.name == target.name && current.origin == target.origin) {
                return Some(StartupAction::Wait);
            }
            let staging = geometry::restore(None, std::slice::from_ref(target), target_outer)?;
            let oversized = target_outer.0 > target.size.0 || target_outer.1 > target.size.1;
            if oversized && target_position == staging {
                // The commanded anchor is already reached, yet the neighbor owns
                // the larger intersection. Repeating it cannot acquire target DPI.
                let fallback = monitors
                    .iter()
                    .find(|m| m.name == current.name && m.origin == current.origin)
                    .and_then(|m| geometry::select_monitor(None, std::slice::from_ref(m)))
                    .or_else(|| geometry::select_monitor(None, monitors))?;
                let (_, outer) = coordinates.rect(
                    window.position,
                    window.outer,
                    window.scale,
                    fallback.scale,
                )?;
                return geometry::restore(None, std::slice::from_ref(fallback), outer)
                    .map(|p| StartupAction::Fallback(p, fallback.scale));
            }
            return Some(StartupAction::Move(staging, target.scale));
        }
        let expected_inner =
            geometry::rescaled_size(self.source_inner, self.source_scale, target.scale)?;
        if window.inner != expected_inner {
            return Some(StartupAction::Resize(expected_inner));
        }
        if self.fallback {
            // After the explicit fallback move, require actual settled DPI/size
            // and a reachable rectangle. Dominant monitor identity can differ for
            // any oversized window; do not oscillate between small displays.
            if coordinates.reachable(window.position, window.outer, window.scale, monitors) {
                return Some(StartupAction::Ready);
            }
            return geometry::restore(None, std::slice::from_ref(target), target_outer)
                .map(|p| StartupAction::Move(p, target.scale));
        }
        // Matching actual DPI/size makes monitor ownership irrelevant: the
        // whole rectangle (or oversized-axis anchor) is checked against target.
        let position = geometry::restore(
            self.original.as_ref(),
            std::slice::from_ref(target),
            target_outer,
        )?;
        Some(if position == target_position {
            StartupAction::Ready
        } else {
            StartupAction::Move(position, target.scale)
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lifecycle {
    Running,
    Stopping,
    ExitReady,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Availability {
    Saved,
    Pending,
    Unavailable,
}
impl Availability {
    pub fn label(self) -> &'static str {
        match self {
            Self::Saved => "位置：已保存",
            Self::Pending => "位置：待保存",
            Self::Unavailable => "位置：不可用",
        }
    }
}
#[derive(Clone)]
pub struct Save {
    pub revision: u64,
    pub placement: Placement,
}
#[derive(Clone)]
pub struct ScaleRequest {
    pub position_revision: u64,
}
pub struct Session {
    pub mode: Mode,
    pub lifecycle: Lifecycle,
    pub latest: Option<Placement>,
    pub topology: Vec<Monitor>,
    pub pending: Pending<Save>,
    pub quit: Option<(Option<Save>, i32)>,
    pub revision: u64,
    pub availability: Availability,
    pub writable: bool,
    pub scale_pending: Option<ScaleRequest>,
    pub scale_due: Option<Instant>,
    pub scale_needed: bool,
    pub position_revision: u64,
    pub startup: Option<StartupRestore>,
    pub startup_due: Option<Instant>,
    pub space_switch_due: Option<Instant>,
    space_switch_generation: u64,
    space_switch_active: Option<u64>,
}
impl Session {
    pub fn new(
        latest: Option<Placement>,
        topology: Vec<Monitor>,
        availability: Availability,
        writable: bool,
    ) -> Self {
        Self {
            mode: Mode::default(),
            lifecycle: Lifecycle::Running,
            latest,
            topology,
            pending: Pending::default(),
            quit: None,
            revision: 0,
            availability,
            writable,
            scale_pending: None,
            scale_due: None,
            scale_needed: false,
            position_revision: 0,
            startup: None,
            startup_due: None,
            space_switch_due: None,
            space_switch_generation: 0,
            space_switch_active: None,
        }
    }
    #[cfg(any(target_os = "macos", test))]
    pub fn begin_space_switch(&mut self, now: Instant) -> Option<u64> {
        if self.lifecycle != Lifecycle::Running {
            return None;
        }
        self.space_switch_generation = self.space_switch_generation.wrapping_add(1);
        let generation = self.space_switch_generation;
        self.space_switch_active = Some(generation);
        self.space_switch_due = Some(now + SPACE_SWITCH_DELAY);
        if let Some(startup) = self.startup.as_mut() {
            startup.show = false;
        }
        Some(generation)
    }
    pub fn take_space_switch_due(&mut self, now: Instant) -> Option<u64> {
        if self.space_switch_due.is_some_and(|due| due <= now) {
            self.space_switch_due = None;
            self.space_switch_active
        } else {
            None
        }
    }
    pub fn space_switch_is_current(&self, generation: u64) -> bool {
        self.lifecycle == Lifecycle::Running && self.space_switch_active == Some(generation)
    }
    pub fn cancel_space_switch(&mut self) {
        self.space_switch_generation = self.space_switch_generation.wrapping_add(1);
        self.space_switch_active = None;
        self.space_switch_due = None;
    }
    pub fn schedule(&mut self, now: Instant, placement: Placement) {
        if self.lifecycle != Lifecycle::Running || self.startup.is_some() {
            return;
        }
        self.revise_position(now);
        self.latest = Some(placement.clone());
        if self.writable {
            self.revision += 1;
            self.pending.schedule(
                now,
                Save {
                    revision: self.revision,
                    placement,
                },
            );
            self.availability = Availability::Pending;
        }
    }
    pub fn request_scale(&mut self, now: Instant) {
        if self.startup.is_some() || self.lifecycle != Lifecycle::Running {
            return;
        }
        self.scale_needed = true;
        self.revise_position(now);
    }
    pub fn take_scale_due(&mut self, now: Instant) -> Option<ScaleRequest> {
        if self.scale_due.is_some_and(|due| due <= now) {
            self.scale_due = None;
            self.scale_pending.take()
        } else {
            None
        }
    }
    pub fn complete_scale(&mut self, request: &ScaleRequest) -> bool {
        if !self.scale_is_current(request) {
            return false;
        }
        self.scale_needed = false;
        self.scale_pending = None;
        self.scale_due = None;
        true
    }
    pub fn begin_startup(&mut self, startup: StartupRestore, now: Instant) {
        if self.lifecycle != Lifecycle::Running {
            return;
        }
        self.cancel_space_switch();
        self.position_revision += 1;
        self.revision += 1;
        self.pending.cancel();
        self.scale_needed = false;
        self.scale_pending = None;
        self.scale_due = None;
        if self.writable {
            self.availability = Availability::Pending;
        }
        self.startup = Some(startup);
        self.startup_due = Some(now);
    }
    pub fn scale_is_current(&self, request: &ScaleRequest) -> bool {
        self.lifecycle == Lifecycle::Running
            && self.startup.is_none()
            && self.scale_needed
            && self.position_revision == request.position_revision
    }
    pub fn position_intent(&mut self) {
        self.revise_position(Instant::now());
    }
    fn revise_position(&mut self, now: Instant) {
        self.position_revision += 1;
        if self.scale_needed {
            // Native DPI movement and user movement both invalidate the old
            // snapshot, but the settled current rectangle still needs checking.
            self.scale_pending = Some(ScaleRequest {
                position_revision: self.position_revision,
            });
            self.scale_due = Some(now + SAVE_DELAY);
        }
    }
    pub fn finish_save(&mut self, revision: u64, success: bool, writable: bool) {
        self.writable = writable;
        if self.lifecycle == Lifecycle::Running && self.revision == revision {
            self.availability = if success {
                Availability::Saved
            } else {
                Availability::Unavailable
            };
        }
        if !writable {
            self.availability = Availability::Unavailable;
            self.pending.stop(None);
        }
    }
    pub fn capture_failed(&mut self) {
        if self.lifecycle == Lifecycle::Running {
            self.position_intent();
            self.revision += 1;
            self.pending.cancel();
            self.availability = Availability::Unavailable;
        }
    }
    pub fn stop(&mut self, latest: Option<Placement>, code: i32) {
        if self.lifecycle != Lifecycle::Running {
            return;
        }
        self.cancel_space_switch();
        self.lifecycle = Lifecycle::Stopping;
        self.scale_pending = None;
        self.scale_due = None;
        self.scale_needed = false;
        self.startup_due = None;
        if self.startup.take().is_some() {
            self.pending.stop(None);
            self.quit = Some((None, code));
            return;
        }
        let latest = latest.or_else(|| self.latest.clone());
        let save = latest.filter(|_| self.writable).map(|placement| Save {
            revision: self.revision,
            placement,
        });
        self.quit = Some((self.pending.stop(save), code));
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Mode {
    #[default]
    Interaction,
    ClickThrough,
}

impl Mode {
    pub fn change(
        &mut self,
        next: Self,
        host: impl FnOnce(bool) -> Result<(), String>,
    ) -> Result<(), String> {
        host(next == Self::ClickThrough)?;
        *self = next;
        Ok(())
    }
}

pub struct Pending<T> {
    entry: Option<(Instant, T)>,
    stopped: bool,
}
impl<T> Default for Pending<T> {
    fn default() -> Self {
        Self {
            entry: None,
            stopped: false,
        }
    }
}
impl<T> Pending<T> {
    pub fn schedule(&mut self, now: Instant, value: T) {
        if !self.stopped {
            self.entry = Some((now + SAVE_DELAY, value));
        }
    }
    pub fn take_due(&mut self, now: Instant) -> Option<T> {
        if self.deadline().is_some_and(|deadline| deadline <= now) {
            self.entry.take().map(|e| e.1)
        } else {
            None
        }
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.entry.as_ref().map(|e| e.0)
    }
    pub fn cancel(&mut self) {
        self.entry = None;
    }
    pub fn stop(&mut self, latest: Option<T>) -> Option<T> {
        if self.stopped {
            return None;
        }
        self.stopped = true;
        let pending = self.entry.take().map(|e| e.1);
        latest.or(pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn space_switch_waits_then_only_current_visible_intent_can_show() {
        let now = Instant::now();
        let mut session = Session::new(None, vec![], Availability::Pending, true);
        let first = session.begin_space_switch(now).unwrap();
        assert_eq!(
            session.take_space_switch_due(now + Duration::from_millis(79)),
            None
        );
        assert_eq!(
            session.take_space_switch_due(now + Duration::from_millis(80)),
            Some(first)
        );
        assert!(session.space_switch_is_current(first));
        session.cancel_space_switch();
        assert!(!session.space_switch_is_current(first));
        let second = session.begin_space_switch(now).unwrap();
        assert_ne!(first, second);
        assert!(!session.space_switch_is_current(first));
        assert!(session.space_switch_is_current(second));
    }
    #[test]
    fn space_switch_cannot_resurrect_after_hide_reset_or_quit() {
        let now = Instant::now();
        let mut session = Session::new(None, vec![], Availability::Pending, true);
        let hidden = session.begin_space_switch(now).unwrap();
        session.cancel_space_switch();
        assert_eq!(
            session.take_space_switch_due(now + Duration::from_secs(1)),
            None
        );
        assert!(!session.space_switch_is_current(hidden));
        let reset = session.begin_space_switch(now).unwrap();
        session.begin_startup(
            StartupRestore {
                original: None,
                source_inner: (240, 260),
                source_scale: 1.0,
                show: true,
                fallback: false,
            },
            now,
        );
        assert!(!session.space_switch_is_current(reset));
        let quit = session.begin_space_switch(now).unwrap();
        session.stop(None, 0);
        assert!(!session.space_switch_is_current(quit));
        assert_eq!(
            session.take_space_switch_due(now + Duration::from_secs(1)),
            None
        );
        assert!(session.begin_space_switch(now).is_none());
    }
    #[test]
    fn mode_commits_only_after_host_success_and_recovers() {
        let mut mode = Mode::default();
        assert!(mode
            .change(Mode::ClickThrough, |_| Err("host refused".into()))
            .is_err());
        assert_eq!(mode, Mode::Interaction);
        mode.change(Mode::ClickThrough, |ignore| {
            assert!(ignore);
            Ok(())
        })
        .unwrap();
        assert_eq!(mode, Mode::ClickThrough);
        assert!(mode
            .change(Mode::Interaction, |_| Err("host refused".into()))
            .is_err());
        assert_eq!(mode, Mode::ClickThrough);
        mode.change(Mode::Interaction, |ignore| {
            assert!(!ignore);
            Ok(())
        })
        .unwrap();
        assert_eq!(mode, Mode::Interaction);
    }
    #[test]
    fn moves_replace_one_deadline_and_one_value() {
        let now = Instant::now();
        let mut pending = Pending::default();
        pending.schedule(now, 1);
        pending.schedule(now + Duration::from_millis(400), 2);
        assert_eq!(pending.take_due(now + SAVE_DELAY), None);
        assert_eq!(pending.take_due(now + Duration::from_millis(900)), Some(2));
        assert_eq!(pending.deadline(), None);
    }
    #[test]
    fn shutdown_flushes_latest_cancels_and_ignores_late_callbacks() {
        let now = Instant::now();
        let mut pending = Pending::default();
        pending.schedule(now, 1);
        assert_eq!(pending.stop(Some(3)), Some(3));
        pending.schedule(now, 4);
        assert_eq!(pending.take_due(now + SAVE_DELAY), None);
        assert_eq!(pending.stop(Some(5)), None);
        let mut fallback = Pending::default();
        fallback.schedule(now, 2);
        assert_eq!(fallback.stop(None), Some(2));
    }
    #[test]
    fn old_completion_cannot_mark_new_placement_saved() {
        let mut session = Session::new(None, vec![], Availability::Pending, true);
        let p = Placement {
            monitor_name: None,
            x: 1.0,
            y: 2.0,
        };
        session.schedule(Instant::now(), p.clone());
        session.schedule(Instant::now(), p.clone());
        session.finish_save(1, true, true);
        assert_eq!(session.availability, Availability::Pending);
        session.finish_save(2, false, true);
        assert_eq!(session.availability, Availability::Unavailable);
        session.schedule(Instant::now(), p.clone());
        session.finish_save(3, true, true);
        assert_eq!(session.availability, Availability::Saved);
        session.schedule(Instant::now(), p.clone());
        session.capture_failed();
        session.finish_save(4, true, true);
        assert_eq!(session.availability, Availability::Unavailable);
        assert!(session.pending.deadline().is_none());
        session.stop(Some(p.clone()), 7);
        session.schedule(
            Instant::now(),
            Placement {
                x: 99.0,
                ..p.clone()
            },
        );
        assert_eq!(session.latest, Some(p));
        assert!(session.pending.deadline().is_none());
        assert_eq!(session.quit.as_ref().unwrap().1, 7);
    }

    #[test]
    fn read_only_failure_cancels_newer_pending_save_and_stop_cancels_scale() {
        let mut session = Session::new(None, vec![], Availability::Pending, true);
        let p = Placement {
            monitor_name: None,
            x: 1.0,
            y: 2.0,
        };
        session.schedule(Instant::now(), p.clone());
        session.schedule(Instant::now(), p.clone());
        session.finish_save(1, false, false);
        assert!(session.pending.deadline().is_none());
        session.schedule(Instant::now(), p);
        assert!(session.pending.deadline().is_none());
        assert_eq!(session.availability, Availability::Unavailable);
        session.request_scale(Instant::now());
        session.stop(None, 0);
        assert!(session.scale_pending.is_none());
        assert!(session.quit.as_ref().unwrap().0.is_none());
    }
    #[test]
    fn delayed_dpi_correction_cannot_undo_later_move_or_reset_even_read_only() {
        let old = Placement {
            monitor_name: Some("same".into()),
            x: 10.0,
            y: 20.0,
        };
        for writable in [true, false] {
            let mut session =
                Session::new(Some(old.clone()), vec![], Availability::Saved, writable);
            session.request_scale(Instant::now());
            let delayed = session.scale_pending.take().unwrap();
            assert!(session.scale_is_current(&delayed));
            session.schedule(
                Instant::now(),
                Placement {
                    x: 200.0,
                    ..old.clone()
                },
            );
            assert!(
                !session.scale_is_current(&delayed),
                "movement must invalidate a dequeued callback"
            );
            session.request_scale(Instant::now());
            let delayed = session.scale_pending.take().unwrap();
            session.position_intent();
            assert!(
                !session.scale_is_current(&delayed),
                "reset intent must invalidate before native movement"
            );
        }
    }
    #[test]
    fn dpi_native_move_keeps_current_geometry_check_after_movement_settles() {
        let now = Instant::now();
        let p = Placement {
            monitor_name: Some("current".into()),
            x: 1900.0,
            y: 20.0,
        };
        for writable in [true, false] {
            let mut session = Session::new(None, vec![], Availability::Saved, writable);
            session.request_scale(now);
            // Native Moved emitted by DPI's SetWindowPos is indistinguishable from
            // user movement. It invalidates old intent, not the need to clamp.
            session.schedule(now + Duration::from_millis(100), p.clone());
            assert!(
                session.scale_pending.is_some(),
                "native Moved must retain a current geometry check"
            );
            assert!(session
                .take_scale_due(now + Duration::from_millis(599))
                .is_none());
            let delayed = session
                .take_scale_due(now + Duration::from_millis(600))
                .unwrap();
            assert!(session.scale_is_current(&delayed));
            // A later user move invalidates the queued callback and reschedules
            // the check against the latest actual position, not the old logical one.
            session.schedule(
                now + Duration::from_millis(650),
                Placement {
                    x: 1800.0,
                    ..p.clone()
                },
            );
            assert!(!session.scale_is_current(&delayed));
            for step in 7..=12 {
                let time = now + Duration::from_millis(step * 100);
                session.schedule(
                    time,
                    Placement {
                        x: 1700.0,
                        ..p.clone()
                    },
                );
                assert!(
                    session
                        .take_scale_due(time + Duration::from_millis(99))
                        .is_none(),
                    "do not fight continuous movement"
                );
            }
            let settled = session
                .take_scale_due(now + Duration::from_millis(1700))
                .unwrap();
            assert!(session.scale_is_current(&settled));
            session.complete_scale(&settled);
            session.schedule(
                now + Duration::from_millis(1701),
                Placement {
                    x: 1440.0,
                    ..p.clone()
                },
            );
            assert!(
                session
                    .take_scale_due(now + Duration::from_secs(3))
                    .is_none(),
                "successful clamp must not loop"
            );
            session.request_scale(now + Duration::from_secs(4));
            session.stop(None, 0);
            assert!(session
                .take_scale_due(now + Duration::from_secs(5))
                .is_none());
        }
    }
    fn startup_fixture(
        source_scale: f64,
        target_scale: f64,
    ) -> (StartupRestore, Monitor, WindowGeometry) {
        let target = Monitor {
            name: Some("target".into()),
            origin: (1920, 0),
            size: (1920, 1080),
            scale: target_scale,
            primary: false,
        };
        let source_inner = if source_scale == 2.0 {
            (480, 520)
        } else {
            (240, 260)
        };
        let startup = StartupRestore {
            original: Some(Placement {
                monitor_name: target.name.clone(),
                x: 1600.0,
                y: 100.0,
            }),
            source_inner,
            source_scale,
            show: true,
            fallback: false,
        };
        let window = WindowGeometry {
            monitor: Some(Monitor {
                name: Some("source".into()),
                origin: (0, 0),
                scale: source_scale,
                primary: true,
                ..target.clone()
            }),
            scale: source_scale,
            inner: source_inner,
            outer: source_inner,
            position: (0, 0),
        };
        (startup, target, window)
    }
    #[test]
    fn startup_high_to_low_preserves_unclamped_target_and_waits_for_real_size() {
        let (startup, target, mut window) = startup_fixture(2.0, 1.0);
        let monitors = [target.clone()];
        assert!(matches!(
            startup.step(&monitors, &window),
            Some(StartupAction::Move(_, _))
        ));
        window.monitor = Some(target.clone());
        assert_eq!(startup.step(&monitors, &window), Some(StartupAction::Wait));
        window.scale = 1.0;
        assert_eq!(
            startup.step(&monitors, &window),
            Some(StartupAction::Resize((240, 260)))
        );
        window.inner = (240, 260);
        window.outer = (240, 260);
        assert_eq!(
            startup.step(&monitors, &window),
            Some(StartupAction::Move((3520, 100), 1.0))
        );
        assert_eq!(startup.original.as_ref().unwrap().x, 1600.0);
        window.position = (3520, 100);
        assert_eq!(startup.step(&monitors, &window), Some(StartupAction::Ready));
    }
    #[test]
    fn startup_low_to_high_clamps_with_final_actual_outer_before_ready() {
        let (startup, target, mut window) = startup_fixture(1.0, 2.0);
        let monitors = [target.clone()];
        assert!(matches!(
            startup.step(&monitors, &window),
            Some(StartupAction::Move(_, _))
        ));
        window.monitor = Some(target);
        window.scale = 2.0;
        assert_eq!(
            startup.step(&monitors, &window),
            Some(StartupAction::Resize((480, 520)))
        );
        window.inner = (480, 520);
        window.outer = (480, 520);
        assert_eq!(
            startup.step(&monitors, &window),
            Some(StartupAction::Move((3360, 200), 2.0))
        );
        window.position = (3360, 200);
        assert_eq!(startup.step(&monitors, &window), Some(StartupAction::Ready));
    }
    #[test]
    fn startup_intermediate_events_and_quit_cannot_save_over_original_target() {
        let (startup, _, _) = startup_fixture(2.0, 1.0);
        let original = startup.original.clone();
        let mut session = Session::new(original.clone(), vec![], Availability::Saved, true);
        session.begin_startup(startup, Instant::now());
        session.schedule(
            Instant::now(),
            Placement {
                monitor_name: None,
                x: 1440.0,
                y: 100.0,
            },
        );
        assert_eq!(session.latest, original);
        assert!(session.pending.deadline().is_none());
        session.request_scale(Instant::now());
        assert!(session.scale_pending.is_none());
        session.stop(
            Some(Placement {
                monitor_name: None,
                x: 1440.0,
                y: 100.0,
            }),
            0,
        );
        assert!(session.quit.as_ref().unwrap().0.is_none());
    }

    fn oversized_startup(target_scale: f64) -> (StartupRestore, Vec<Monitor>, WindowGeometry) {
        let target = Monitor {
            name: Some("narrow".into()),
            origin: (-200, 0),
            size: (200, 1080),
            scale: target_scale,
            primary: false,
        };
        let current = Monitor {
            name: Some("main".into()),
            origin: (0, 0),
            size: (1920, 1080),
            scale: 1.0,
            primary: true,
        };
        let startup = StartupRestore {
            original: Some(Placement {
                monitor_name: target.name.clone(),
                x: 0.0,
                y: 100.0,
            }),
            source_inner: (480, 260),
            source_scale: 1.0,
            show: true,
            fallback: false,
        };
        let window = WindowGeometry {
            monitor: Some(current.clone()),
            scale: 1.0,
            inner: (480, 260),
            outer: (480, 260),
            position: (-200, 100),
        };
        (startup, vec![target, current], window)
    }
    #[test]
    fn startup_oversized_same_dpi_accepts_visible_target_despite_dominant_neighbor() {
        let (startup, monitors, window) = oversized_startup(1.0);
        assert_eq!(startup.step(&monitors, &window), Some(StartupAction::Ready));
    }
    #[test]
    fn startup_oversized_mixed_dpi_fixed_point_degrades_then_converges() {
        let (mut startup, monitors, mut window) = oversized_startup(2.0);
        // Source-sized staging anchor was applied, but the 280px overlap in main
        // exceeds the narrow target's 200px and it cannot acquire target DPI.
        window.position = (-200, 410);
        assert_eq!(
            startup.step(&monitors, &window),
            Some(StartupAction::Fallback((720, 410), 1.0))
        );
        // A stale window scale equal to the requested target is not evidence
        // of settlement when the actual dominant display has another DPI.
        window.scale = 2.0;
        assert_eq!(
            startup.step(&monitors, &window),
            Some(StartupAction::Fallback((720, 410), 1.0))
        );
        window.scale = 1.0;
        startup.fallback = true;
        window.position = (720, 410);
        assert_eq!(startup.step(&monitors, &window), Some(StartupAction::Ready));
        // Explicit fallback still requires a valid scale and observed actual size.
        window.scale = f64::NAN;
        assert_ne!(startup.step(&monitors, &window), Some(StartupAction::Ready));
        window.scale = 1.0;
        window.inner = (240, 130);
        assert_eq!(
            startup.step(&monitors, &window),
            Some(StartupAction::Resize((480, 260)))
        );
        assert!(startup.step(&[], &window).is_none());
        let mut session = Session::new(
            startup.original.clone(),
            monitors,
            Availability::Saved,
            true,
        );
        session.begin_startup(startup.clone(), Instant::now());
        session.stop(None, 7);
        session.begin_startup(startup, Instant::now());
        session.request_scale(Instant::now());
        assert!(session.startup.is_none());
        assert!(session.startup_due.is_none());
        assert!(session.scale_pending.is_none());
        assert!(session.quit.as_ref().unwrap().0.is_none());
        assert_eq!(session.quit.as_ref().unwrap().1, 7);
    }
    #[test]
    fn mac_adapter_startup_and_reset_stage_in_destination_units() {
        use super::super::coordinates::Coordinates;
        for (source_scale, target_scale, origin, expected) in [
            (2.0, 1.0, (1440, 40), (2280, 450)),
            (1.0, 2.0, (-3840, -200), (-3120, 80)),
        ] {
            let (mut startup, mut target, mut window) = startup_fixture(source_scale, target_scale);
            target.origin = origin;
            startup.original.as_mut().unwrap().x = 100.0;
            for reset in [false, true] {
                if reset {
                    startup.original = None;
                    target.primary = true;
                }
                assert_eq!(
                    startup.step_with_coordinates(Coordinates::Mac, &[target.clone()], &window),
                    Some(StartupAction::Move(expected, target.scale))
                );
                window.position = (0, 0);
            }
        }
    }
    #[test]
    fn relocation_cancels_pending_save_and_old_dpi_before_settlement() {
        let (startup, _, _) = startup_fixture(2.0, 1.0);
        let mut session = Session::new(None, vec![], Availability::Saved, true);
        session.schedule(Instant::now(), startup.original.clone().unwrap());
        let revision = session.revision;
        session.request_scale(Instant::now());
        session.begin_startup(startup, Instant::now());
        assert!(session.pending.deadline().is_none());
        assert!(session.scale_pending.is_none());
        assert!(session.revision > revision);
    }
}
