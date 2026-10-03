//! A single presentation opportunity, not a second reminder queue or focus store.
use super::model::Mode;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusState {
    Unavailable,
    Running,
    Available,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Reminder(u64),
    Focus(u64),
}
#[derive(Clone, Copy)]
pub struct Input {
    pub reminder: Option<(u64, Mode)>,
    pub focus: FocusState,
    pub automatic_allowed: bool,
    pub now_ms: u64,
    pub utc_ms: i64,
}
#[derive(Default)]
pub struct Coordinator {
    source: Option<Source>,
    id: u64,
    suppressed_through: u64,
    completion_seen: u64,
    completion: Option<(u64, u64, i64)>,
    was_running: bool,
    response_seen: u64,
    dismissed: Option<Source>,
}
impl Coordinator {
    pub fn update(&mut self, input: Input, completion: Option<u64>) -> Option<(u64, Source)> {
        let manual = input.reminder.filter(|(_, mode)| *mode == Mode::Manual);
        let allowed =
            input.focus == FocusState::Available && input.automatic_allowed && manual.is_none();
        if let Some(revision) = completion.filter(|r| *r > self.completion_seen) {
            self.completion_seen = revision;
            self.completion = allowed.then_some((
                revision,
                input.now_ms.saturating_add(10_000),
                input.utc_ms.saturating_add(10_000),
            ));
        }
        // Expired/blocked opportunities are consumed, never held for a quieter time.
        if !allowed
            || self.completion.is_some_and(|(_, until, utc_until)| {
                input.now_ms >= until || input.utc_ms >= utc_until
            })
        {
            self.completion = None;
        }
        if input.focus == FocusState::Running
            || self.was_running
            || self.completion.is_some()
            || completion.is_some()
        {
            if let Some((id, Mode::Automatic)) = input.reminder {
                self.suppressed_through = self.suppressed_through.max(id);
            }
        }
        self.was_running = input.focus == FocusState::Running;
        let next = if let Some((id, _)) = manual {
            Some(Source::Reminder(id))
        } else if let Some((revision, _, _)) = self.completion {
            Some(Source::Focus(revision))
        } else if input.focus != FocusState::Running && input.automatic_allowed {
            input
                .reminder
                .filter(|(id, _)| *id > self.suppressed_through)
                .map(|(id, _)| Source::Reminder(id))
        } else {
            None
        };
        let next = next.filter(|source| Some(*source) != self.dismissed);
        if next != self.source {
            self.id = self.id.saturating_add(1);
            self.source = next;
        }
        self.source.map(|source| (self.id, source))
    }
    pub fn dismiss(&mut self) {
        self.dismissed = self.source;
        if let Some(Source::Reminder(id)) = self.source {
            self.suppressed_through = self.suppressed_through.max(id);
        }
        self.completion = None;
        self.source = None;
    }
    pub fn presented(&mut self, id: u64) -> Option<u64> {
        if id != self.id {
            return None;
        }
        if let Some(Source::Focus(revision)) = self.source {
            self.respond(revision)
        } else {
            None
        }
    }
    pub fn respond(&mut self, revision: u64) -> Option<u64> {
        if revision <= self.response_seen {
            return None;
        }
        self.response_seen = revision;
        Some(revision)
    }
    pub fn retain_completion(&mut self, revision: Option<u64>) {
        if self.completion.is_some_and(|(r, _, _)| Some(r) != revision) {
            self.completion = None;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Input {
        Input {
            reminder: Some((3, Mode::Automatic)),
            focus: FocusState::Available,
            automatic_allowed: true,
            now_ms: 1_000,
            utc_ms: 1_000,
        }
    }
    #[test]
    fn focus_blocks_automatic_but_never_manual_or_replays_suppressed() {
        let mut c = Coordinator::default();
        let mut i = input();
        assert!(c.update(i, None).is_some());
        i.focus = FocusState::Running;
        assert_eq!(c.update(i, None), None);
        i.focus = FocusState::Available;
        assert_eq!(c.update(i, None), None);
        i.reminder = Some((4, Mode::Manual));
        i.focus = FocusState::Running;
        assert!(matches!(c.update(i, None), Some((_, Source::Reminder(4)))));
    }
    #[test]
    fn live_completion_wins_once_and_never_replays_after_deadline() {
        let mut c = Coordinator::default();
        let mut i = input();
        let (id, source) = c.update(i, Some(5)).unwrap();
        assert_eq!(source, Source::Focus(5));
        assert_eq!(c.presented(id), Some(5));
        assert_eq!(c.presented(id), None);
        assert_eq!(c.update(i, Some(5)), Some((id, source)));
        i.now_ms = 11_000;
        assert_eq!(c.update(i, None), None);
        assert_eq!(c.update(i, Some(5)), None);
    }
    #[test]
    fn quiet_manual_unavailable_and_running_consume_not_queue_completion() {
        for kind in 0..4 {
            let mut c = Coordinator::default();
            let mut i = input();
            match kind {
                0 => i.automatic_allowed = false,
                1 => i.reminder = Some((4, Mode::Manual)),
                2 => i.focus = FocusState::Unavailable,
                _ => i.focus = FocusState::Running,
            }
            assert!(!matches!(c.update(i, Some(7)), Some((_, Source::Focus(_)))));
            i = input();
            assert!(!matches!(c.update(i, Some(7)), Some((_, Source::Focus(_)))));
        }
    }
    #[test]
    fn manual_preempts_completion_and_dismiss_does_not_resurrect_it() {
        let mut c = Coordinator::default();
        let mut i = input();
        let (old, _) = c.update(i, Some(8)).unwrap();
        i.reminder = Some((4, Mode::Manual));
        assert!(matches!(c.update(i, None), Some((_, Source::Reminder(4)))));
        assert_eq!(c.presented(old), None);
        c.dismiss();
        i.reminder = None;
        assert_eq!(c.update(i, Some(8)), None);
    }
    #[test]
    fn unavailable_focus_does_not_disable_independent_reminders() {
        let mut c = Coordinator::default();
        let mut i = input();
        i.focus = FocusState::Unavailable;
        assert!(matches!(c.update(i, None), Some((_, Source::Reminder(3)))));
    }
    #[test]
    fn completion_boundary_suppresses_a_new_arrival_before_live_event_callback() {
        let mut c = Coordinator::default();
        let mut i = input();
        i.reminder = None;
        i.focus = FocusState::Running;
        c.update(i, None);
        i = input();
        assert_eq!(c.update(i, None), None);
    }
    #[test]
    fn sleep_expiry_uses_wall_time_even_if_monotonic_did_not_advance() {
        let mut c = Coordinator::default();
        let mut i = input();
        let (id, _) = c.update(i, Some(5)).unwrap();
        i.utc_ms = 100_000;
        assert_eq!(c.update(i, None), None);
        assert_eq!(c.presented(id), None);
    }
    #[test]
    fn restored_finished_state_without_live_event_never_creates_feedback() {
        let mut c = Coordinator::default();
        let mut i = input();
        i.reminder = None;
        assert_eq!(c.update(i, None), None);
    }
}
