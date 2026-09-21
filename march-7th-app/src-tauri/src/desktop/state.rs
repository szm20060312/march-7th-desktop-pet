use super::geometry::{Monitor, Placement};
use std::time::{Duration, Instant};

pub const SAVE_DELAY: Duration = Duration::from_millis(500);

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
    pub scale_pending: Option<Option<Placement>>,
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
        }
    }
    pub fn schedule(&mut self, now: Instant, placement: Placement) {
        if self.lifecycle != Lifecycle::Running {
            return;
        }
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
            self.revision += 1;
            self.pending.cancel();
            self.availability = Availability::Unavailable;
        }
    }
    pub fn stop(&mut self, latest: Option<Placement>, code: i32) {
        if self.lifecycle != Lifecycle::Running {
            return;
        }
        self.lifecycle = Lifecycle::Stopping;
        self.scale_pending = None;
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
        session.scale_pending = Some(None);
        session.stop(None, 0);
        assert!(session.scale_pending.is_none());
        assert!(session.quit.as_ref().unwrap().0.is_none());
    }
}
