//! Presentation lifecycle only. No business clock, storage, or native effects.
#[derive(Default)]
pub struct SettingsUi {
    pub token: u64,
    pub creating: bool,
    pub open: bool,
    /// Identity of the displayed settings/backup session, retained on visible focus.
    pub generation: u64,
    /// Presentation intent sequence; independent of the backup session generation.
    pub intent: u64,
    displayed_generation: Option<u64>,
    stopped: bool,
}
impl SettingsUi {
    pub fn request_open(&mut self, exists: bool, already_visible: bool) -> Option<u64> {
        if self.stopped {
            return None;
        }
        self.intent += 1;
        if !exists || !already_visible || self.displayed_generation.is_none() {
            self.generation += 1;
            self.displayed_generation = None;
        }
        self.open = true;
        if exists || self.creating {
            return None;
        }
        self.token += 1;
        self.creating = true;
        Some(self.token)
    }
    pub fn close(&mut self) {
        self.open = false;
        self.intent += 1;
        self.displayed_generation = None;
    }
    pub fn created(&mut self, token: u64) -> bool {
        if self.stopped || token != self.token {
            return false;
        }
        self.creating = false;
        true
    }
    pub fn may_show(&self, intent: u64) -> bool {
        !self.stopped && self.open && !self.creating && intent == self.intent
    }
    /// Called only after the native window has shown or finished a visible reflow.
    pub fn presented(&mut self, intent: u64) -> bool {
        if self.stopped || self.creating || intent != self.intent {
            return false;
        }
        self.open = false;
        self.displayed_generation = Some(self.generation);
        true
    }
    pub fn export_session(&self, reflow: bool) -> Option<(u64, u64)> {
        // An open intent can locate an already displayed window. That intent must
        // not suspend its session while a native file picker is returning.
        (!self.stopped
            && !self.creating
            && !reflow
            && self.displayed_generation == Some(self.generation))
        .then_some((self.token, self.generation))
    }
    pub fn destroyed(&mut self, token: u64) -> bool {
        if self.stopped || token != self.token {
            return false;
        }
        self.creating = false;
        self.close();
        true
    }
    pub fn stop(&mut self) {
        self.stopped = true;
        self.close();
        self.token += 1;
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub token: u64,
    pub generation: u64,
    pub presentation: u64,
}

#[derive(Default)]
pub struct PresentationUi {
    pub revision: u64,
    pub stopped: bool,
    pub presentation: Option<u64>,
    pub token: Option<u64>,
    pub creating: bool,
    pub shown: bool,
    pub failed: bool,
    generation: u64,
    next_token: u64,
    ready: Option<u64>,
    dismissed: Option<u64>,
}
impl PresentationUi {
    pub fn update(&mut self, revision: u64, presentation: Option<u64>, stopped: bool) -> bool {
        if self.stopped || revision < self.revision {
            return false;
        }
        self.revision = revision;
        if stopped {
            self.stopped = true;
            self.token = None;
            self.creating = false;
        }
        let presentation = if stopped { None } else { presentation };
        if self.presentation != presentation || stopped {
            self.presentation = presentation;
            self.generation += 1;
            self.ready = None;
            self.shown = false;
            self.failed = false;
            self.dismissed = None;
        }
        true
    }
    pub fn begin_create(&mut self) -> Option<u64> {
        if self.stopped || self.presentation.is_none() || self.token.is_some() || self.failed {
            return None;
        }
        self.next_token = self
            .next_token
            .checked_add(1)
            .filter(|t| *t <= super::model::MAX_SAFE)?;
        self.token = Some(self.next_token);
        self.creating = true;
        self.token
    }
    pub fn created(&mut self, token: u64) -> bool {
        if self.stopped || self.token != Some(token) {
            return false;
        }
        self.creating = false;
        true
    }
    pub fn ticket(&self) -> Option<Ticket> {
        if self.stopped || self.failed || self.dismissed == self.presentation {
            return None;
        }
        Some(Ticket {
            token: self.token?,
            generation: self.generation,
            presentation: self.presentation?,
        })
    }
    pub fn accepts(&self, ticket: Ticket) -> bool {
        self.ticket() == Some(ticket)
    }
    pub fn ready(&mut self, token: u64, presentation: u64) -> bool {
        if !self
            .ticket()
            .is_some_and(|t| t.token == token && t.presentation == presentation)
        {
            return false;
        }
        self.ready = Some(presentation);
        true
    }
    pub fn can_show(&self, ticket: Ticket) -> bool {
        self.accepts(ticket)
            && !self.creating
            && !self.shown
            && self.ready == Some(ticket.presentation)
    }
    pub fn finish_show(&mut self, ticket: Ticket, succeeded: bool) {
        if !self.accepts(ticket) {
            return;
        }
        self.shown = succeeded;
        self.failed = !succeeded;
    }
    pub fn dismiss(&mut self) -> Option<u64> {
        let id = self.presentation?;
        self.dismissed = Some(id);
        self.ready = None;
        self.shown = false;
        self.generation += 1;
        Some(id)
    }
    pub fn destroyed(&mut self, token: u64) {
        if self.token != Some(token) {
            return;
        }
        self.token = None;
        self.creating = false;
        self.shown = false;
        self.ready = None;
        self.failed = true;
        self.generation += 1;
    }
    pub fn reload(&mut self, token: u64) {
        if self.token != Some(token) || self.stopped {
            return;
        }
        self.ready = None;
        self.shown = false;
        self.generation += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_location_intents_supersede_each_other_without_replacing_session() {
        let mut ui = SettingsUi::default();
        let token = ui.request_open(false, false).unwrap();
        assert!(ui.created(token));
        assert!(ui.presented(ui.intent));
        let session = ui.export_session(false);
        ui.request_open(true, true);
        let old_intent = ui.intent;
        ui.request_open(true, true);
        assert!(ui.intent > old_intent);
        assert!(!ui.may_show(old_intent));
        assert!(!ui.presented(old_intent));
        assert_eq!(ui.export_session(false), session);
        let closing_intent = ui.intent;
        ui.close();
        assert!(!ui.presented(closing_intent));
        assert_eq!(ui.export_session(false), None);
        ui.request_open(true, false);
        assert!(!ui.presented(closing_intent));
        assert!(ui.presented(ui.intent));
        assert_ne!(ui.export_session(false), session);
    }
    #[test]
    fn settings_focus_requires_current_explicit_open_and_close_cancels_it() {
        let mut ui = SettingsUi::default();
        assert!(!ui.may_show(0));
        let token = ui.request_open(false, false).unwrap();
        let generation = ui.intent;
        assert!(!ui.may_show(generation));
        assert!(ui.created(token));
        assert!(ui.may_show(generation));
        ui.close();
        assert!(!ui.may_show(generation));
        ui.request_open(true, false);
        assert!(!ui.may_show(generation));
        assert!(ui.may_show(ui.intent));
    }
    #[test]
    fn settings_stop_rejects_late_build_and_no_open_revives_it() {
        let mut ui = SettingsUi::default();
        let token = ui.request_open(false, false).unwrap();
        ui.stop();
        assert!(!ui.created(token));
        assert!(ui.request_open(false, false).is_none());
        assert!(!ui.may_show(ui.intent));
    }
    #[test]
    fn shown_settings_can_export_after_open_intent_clears_but_old_generation_cannot() {
        let mut ui = SettingsUi::default();
        let token = ui.request_open(false, false).unwrap();
        let generation = ui.intent;
        assert!(ui.created(token));
        assert!(ui.may_show(generation));
        assert_eq!(ui.export_session(false), None);
        assert!(ui.presented(generation)); // native show and focus completed
        assert!(!ui.open);
        assert_eq!(ui.export_session(false), Some((token, generation)));
        assert_eq!(ui.export_session(true), None); // display reflow is unsettled
        ui.close();
        assert_eq!(ui.export_session(false), None);
        ui.request_open(true, false);
        assert_eq!(ui.export_session(false), None);
        assert!(ui.presented(ui.intent));
        assert_ne!(ui.export_session(false), Some((token, generation)));
    }
    #[test]
    fn destroyed_window_cancels_only_matching_settings_session() {
        let mut ui = SettingsUi::default();
        let old = ui.request_open(false, false).unwrap();
        assert!(ui.created(old));
        assert!(ui.presented(ui.intent));
        assert!(ui.destroyed(old));
        assert_eq!(ui.export_session(false), None);
        let new = ui.request_open(false, false).unwrap();
        assert_ne!(new, old);
        assert!(!ui.destroyed(old));
        assert!(ui.created(new));
        assert!(ui.presented(ui.intent));
        assert_eq!(ui.export_session(false), Some((new, ui.generation)));
    }
    #[test]
    fn failed_native_show_cannot_claim_visible_or_reopen_without_new_intent() {
        let (mut ui, ticket) = active();
        ui.finish_show(ticket, false);
        assert!(!ui.shown);
        assert!(ui.failed);
        assert!(!ui.can_show(ticket));
        ui.update(3, Some(11), false);
        ui.ready(ticket.token, 11);
        let manual = ui.ticket().unwrap();
        ui.finish_show(ticket, true); // late successful effect report from old view
        assert!(!ui.shown);
        assert!(ui.can_show(manual));
        ui.finish_show(manual, true);
        assert!(ui.shown);
    }
    fn active() -> (PresentationUi, Ticket) {
        let mut ui = PresentationUi::default();
        assert!(ui.update(2, Some(10), false));
        let token = ui.begin_create().unwrap();
        assert!(ui.created(token));
        assert!(ui.ready(token, 10));
        let ticket = ui.ticket().unwrap();
        (ui, ticket)
    }
    #[test]
    fn old_snapshot_cannot_reopen_and_auto_null_hides() {
        let (mut ui, ticket) = active();
        ui.shown = true;
        assert!(ui.update(3, None, false));
        assert!(!ui.update(2, Some(10), false));
        assert!(!ui.can_show(ticket));
        assert!(!ui.shown);
    }
    #[test]
    fn same_id_update_keeps_ready_and_does_not_repeat_show() {
        let (mut ui, ticket) = active();
        assert!(ui.can_show(ticket));
        ui.shown = true;
        ui.update(3, Some(10), false);
        assert!(ui.accepts(ticket));
        assert!(!ui.can_show(ticket));
    }
    #[test]
    fn manual_generation_rejects_old_ready_and_position_callbacks() {
        let (mut ui, old) = active();
        ui.update(3, Some(11), false);
        assert!(!ui.accepts(old));
        assert!(!ui.ready(old.token, 10));
        assert!(ui.ready(old.token, 11));
        assert!(ui.can_show(ui.ticket().unwrap()));
    }
    #[test]
    fn close_suppresses_only_current_id_and_is_not_completion() {
        let (mut ui, old) = active();
        assert_eq!(ui.dismiss(), Some(10));
        ui.update(3, Some(10), false); // service command still in flight
        assert!(!ui.can_show(old));
        ui.update(4, Some(11), false);
        assert!(ui.ready(old.token, 11));
        assert!(ui.can_show(ui.ticket().unwrap()));
    }
    #[test]
    fn stop_rejects_construction_ready_and_old_snapshots() {
        let mut ui = PresentationUi::default();
        ui.update(2, Some(10), false);
        let token = ui.begin_create().unwrap();
        ui.update(3, None, true);
        assert!(!ui.created(token));
        assert!(!ui.ready(token, 10));
        assert!(!ui.update(4, Some(11), false));
        assert!(ui.begin_create().is_none());
    }
    #[test]
    fn failure_does_not_retry_forever_and_recovery_gets_new_token() {
        let (mut ui, old) = active();
        ui.destroyed(old.token);
        assert!(ui.failed);
        assert!(ui.begin_create().is_none());
        ui.update(3, Some(11), false); // explicit ShowPending creates a fresh ID
        let token = ui.begin_create().unwrap();
        assert_ne!(token, old.token);
        assert!(!ui.created(old.token));
        assert!(!ui.ready(old.token, 11));
        assert!(ui.created(token));
    }
    #[test]
    fn page_reload_requires_new_ready_before_show() {
        let (mut ui, ticket) = active();
        ui.shown = true;
        ui.reload(ticket.token);
        assert!(!ui.accepts(ticket));
        assert!(!ui.can_show(ui.ticket().unwrap()));
        assert!(ui.ready(ticket.token, 10));
        assert!(ui.can_show(ui.ticket().unwrap()));
    }
    #[test]
    fn ready_before_construction_completion_is_retained_but_cannot_show_early() {
        let mut ui = PresentationUi::default();
        ui.update(2, Some(10), false);
        let token = ui.begin_create().unwrap();
        assert!(ui.ready(token, 10));
        let ticket = ui.ticket().unwrap();
        assert!(!ui.can_show(ticket));
        assert!(ui.created(token));
        assert!(ui.can_show(ticket));
    }
}
