//! Converts current authoritative snapshots into the one bubble's presentation.
use super::{
    model::{Command, Error, Id, Mode, Presentation},
    presentation_policy::{Coordinator, FocusState, Input, Source},
    service::Snapshot,
    store::SaveStatus,
};
use crate::focus::{
    model::{Feedback, Outcome, Session},
    service::Change,
};
use serde_json::Value;
#[derive(Clone, Copy, PartialEq, Eq)]
struct Signature {
    revision: u64,
    selected: Option<(u64, Source)>,
    choices_open: Option<u64>,
    stopped: bool,
    water_only_source: Option<u64>,
}
#[derive(Default)]
pub struct Bubble {
    pub policy: Coordinator,
    pub selected: Option<(u64, Source)>,
    choices_open: Option<u64>,
    signature: Option<Signature>,
    pub revision: u64,
    water_only_source: Option<u64>,
}
pub fn natural_revision(focus: &Change) -> Option<u64> {
    (!focus.snapshot.stopped
        && focus.snapshot.error.is_none()
        && focus.error.is_none()
        && matches!(
            focus.snapshot.data.as_ref().map(|d| d.session),
            Some(Session::Finished {
                outcome: Outcome::Natural,
                feedback: Feedback::Pending,
                ..
            })
        ))
    .then_some(focus.snapshot.revision)
}
impl Bubble {
    fn visible_items(&self, presentation: &Presentation) -> Vec<Id> {
        presentation
            .items
            .iter()
            .copied()
            .filter(|id| {
                self.water_only_source != Some(presentation.id)
                    || presentation.mode != Mode::Automatic
                    || *id == Id::Water
            })
            .collect()
    }
    pub fn visible_item_count(&self, raw: &Snapshot) -> usize {
        let Some((_, Source::Reminder(source))) = self.selected else {
            return 0;
        };
        raw.presentation
            .as_ref()
            .filter(|presentation| presentation.id == source)
            .map_or(0, |presentation| self.visible_items(presentation).len())
    }
    pub fn automatic_prompt(&self, raw: &Snapshot, presentation_id: u64) -> Option<Vec<Id>> {
        let (id, Source::Reminder(source)) = self.selected? else {
            return None;
        };
        let presentation = raw.presentation.as_ref()?;
        if id != presentation_id
            || source != presentation.id
            || presentation.mode != Mode::Automatic
        {
            return None;
        }
        let items = self.visible_items(presentation);
        (!items.is_empty()).then_some(items)
    }
    pub fn open_choices(&mut self, raw: &Snapshot, presentation_id: u64) -> bool {
        if self.automatic_prompt(raw, presentation_id).is_none() {
            return false;
        }
        self.choices_open = Some(presentation_id);
        true
    }
    pub fn choices_open_for(&self, presentation_id: u64) -> bool {
        self.choices_open == Some(presentation_id)
    }
    pub fn window_presentation(&self, raw: &Snapshot) -> Option<u64> {
        let id = self.selected?.0;
        (self.automatic_prompt(raw, id).is_none() || self.choices_open_for(id)).then_some(id)
    }
    #[cfg(test)]
    pub fn update(
        &mut self,
        reminder: &Snapshot,
        focus: Option<&Change>,
        completion: Option<u64>,
        time: super::model::Time,
    ) -> bool {
        self.update_with_policy(reminder, focus, completion, time, false)
    }
    pub fn update_with_policy(
        &mut self,
        reminder: &Snapshot,
        focus: Option<&Change>,
        completion: Option<u64>,
        time: super::model::Time,
        water_during_focus: bool,
    ) -> bool {
        let focus_state = match focus {
            // A transient failed write leaves the committed Running session authoritative.
            Some(f)
                if !f.snapshot.stopped
                    && f.snapshot.error.is_none()
                    && matches!(
                        f.snapshot.data.as_ref().map(|d| d.session),
                        Some(Session::Running { .. })
                    ) =>
            {
                FocusState::Running
            }
            Some(f) if !f.snapshot.stopped && f.snapshot.error.is_none() && f.error.is_none() => {
                match f.snapshot.data.as_ref().map(|d| d.session) {
                    Some(Session::Running { .. }) => FocusState::Running,
                    Some(_) => FocusState::Available,
                    None => FocusState::Unavailable,
                }
            }
            _ => FocusState::Unavailable,
        };
        let automatic = reminder
            .presentation
            .as_ref()
            .filter(|p| p.mode == Mode::Automatic);
        if !water_during_focus || self.water_only_source != automatic.map(|p| p.id) {
            self.water_only_source = None;
        }
        if water_during_focus && focus_state == FocusState::Running {
            if let Some(p) = automatic.filter(|p| p.items.contains(&Id::Water)) {
                // Keep the water-only projection for this entire batch, including
                // after focus pauses or water completes. Other items stay pending.
                self.water_only_source = Some(p.id);
            }
        }
        let focus_exception = water_during_focus
            && automatic.is_some_and(|p| {
                self.water_only_source == Some(p.id) && p.items.contains(&Id::Water)
            });
        let natural = focus.and_then(natural_revision);
        self.policy.retain_completion(natural);
        let completion = completion.filter(|r| Some(*r) == natural);
        self.selected = self.policy.update(
            Input {
                reminder: if reminder.stopped {
                    None
                } else {
                    reminder
                        .presentation
                        .as_ref()
                        .filter(|p| {
                            self.water_only_source != Some(p.id)
                                || p.mode != Mode::Automatic
                                || p.items.contains(&Id::Water)
                        })
                        .map(|p| (p.id, p.mode))
                },
                focus: focus_state,
                water_during_focus: focus_exception,
                automatic_allowed: !reminder.stopped
                    && !reminder.paused
                    && reminder.quiet.is_none()
                    && reminder.runtime_error.is_none()
                    && !matches!(
                        reminder.persistence.status,
                        SaveStatus::Loading | SaveStatus::ReadOnly
                    )
                    && reminder.settings.active_hours.contains(time.local_minute),
                now_ms: time.monotonic_ms,
                utc_ms: time.utc_ms,
            },
            completion,
        );
        if self.choices_open != self.selected.map(|(id, _)| id) {
            self.choices_open = None;
        }
        let signature = Signature {
            revision: reminder.revision,
            selected: self.selected,
            choices_open: self.choices_open,
            stopped: reminder.stopped,
            water_only_source: self.water_only_source,
        };
        if self.signature == Some(signature) {
            return false;
        }
        self.signature = Some(signature);
        self.revision += 1;
        true
    }
    pub fn snapshot(&self, raw: &Snapshot) -> Result<Value, Error> {
        let mut projected = raw.clone();
        projected.revision = self.revision;
        projected.presentation = match self.selected {
            Some((id, Source::Reminder(source))) => raw
                .presentation
                .clone()
                .filter(|p| p.id == source)
                .map(|mut p| {
                    p.items = self.visible_items(&p);
                    p.id = id;
                    p
                }),
            Some((id, Source::Focus(_))) => Some(Presentation {
                id,
                mode: Mode::Automatic,
                items: vec![],
                closes_at: None,
                monotonic_deadline: None,
            }),
            None => None,
        };
        let mut value = serde_json::to_value(projected).map_err(|_| Error::new("invalidState"))?;
        if let Some((id, Source::Reminder(_))) = self.selected {
            if value["presentation"]["mode"] == "automatic" {
                value["presentation"]["choicesOpen"] = Value::Bool(self.choices_open_for(id));
            }
        }
        if matches!(self.selected, Some((_, Source::Focus(_)))) {
            value["presentation"]["focusCompleted"] = Value::Bool(true);
        }
        Ok(value)
    }
    pub fn command(&mut self, command: Command) -> Result<Option<Command>, Error> {
        match command {
            Command::Dismiss { presentation_id } => {
                let (id, source) = self.selected.ok_or(Error::new("stalePresentation"))?;
                if id != presentation_id {
                    return Err(Error::new("stalePresentation"));
                }
                self.policy.dismiss();
                self.choices_open = None;
                Ok(match source {
                    Source::Reminder(presentation_id) => Some(Command::Dismiss { presentation_id }),
                    Source::Focus(_) => None,
                })
            }
            Command::ShowPending {} => {
                self.policy.dismiss();
                self.choices_open = None;
                Ok(Some(command))
            }
            _ => Ok(Some(command)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reminders::{
        model::{ActiveHours, Data, Quiet},
        store::Persistence,
        ui_policy::PresentationUi,
    };
    fn reminder() -> Snapshot {
        let d = Data::default();
        Snapshot {
            revision: 1,
            settings: d.settings,
            progress: d.progress,
            paused: false,
            quiet: None,
            snooze_pending: false,
            presentation: None,
            persistence: Persistence::new(SaveStatus::Saved, None),
            runtime_error: None,
            stopped: false,
        }
    }
    fn focus(session: Session) -> Change {
        Change {
            snapshot: crate::focus::service::Snapshot {
                revision: 5,
                data: Some(crate::focus::model::Data {
                    version: 2,
                    task: None,
                    session,
                }),
                error: None,
                stopped: false,
            },
            completed_now: true,
            task_response: None,
            error: None,
        }
    }
    fn finished() -> Change {
        focus(Session::Finished {
            duration_ms: 60_000,
            outcome: Outcome::Natural,
            feedback: Feedback::Pending,
        })
    }
    fn pending(r: &mut Snapshot, id: u64, mode: Mode) {
        for p in &mut r.progress {
            p.pending = true;
            p.auto_handled = true;
        }
        r.presentation = Some(Presentation {
            id,
            mode,
            items: r.progress.iter().map(|p| p.id).collect(),
            closes_at: None,
            monotonic_deadline: None,
        });
        r.revision += 1;
    }
    fn update(b: &mut Bubble, r: &Snapshot, f: &Change, event: Option<u64>, now: u64) {
        b.update(
            r,
            Some(f),
            event,
            super::super::model::Time {
                utc_ms: now as i64 + 1000,
                monotonic_ms: now,
                local_minute: 600,
            },
        );
    }
    fn session(b: &Bubble, ui: &mut PresentationUi, r: &Snapshot) {
        ui.update(b.revision, b.window_presentation(r), r.stopped);
    }
    #[test]
    fn only_a_selected_automatic_reminder_can_prompt_the_character() {
        let mut r = reminder();
        pending(&mut r, 9, Mode::Automatic);
        let mut b = Bubble {
            selected: Some((12, Source::Reminder(9))),
            ..Default::default()
        };
        assert_eq!(b.visible_item_count(&r), 3);
        assert_eq!(
            b.automatic_prompt(&r, 12),
            Some(r.presentation.as_ref().unwrap().items.clone())
        );
        assert_eq!(b.automatic_prompt(&r, 13), None);
        b.selected = Some((12, Source::Focus(5)));
        assert_eq!(b.visible_item_count(&r), 0);
        assert_eq!(b.automatic_prompt(&r, 12), None);
        b.selected = Some((12, Source::Reminder(9)));
        r.presentation.as_mut().unwrap().mode = Mode::Manual;
        assert_eq!(b.visible_item_count(&r), 3);
        assert_eq!(b.automatic_prompt(&r, 12), None);
    }
    fn water_policy_update(b: &mut Bubble, r: &Snapshot, f: &Change, allowed: bool, now: u64) {
        b.update_with_policy(
            r,
            Some(f),
            None,
            super::super::model::Time {
                utc_ms: now as i64 + 1000,
                monotonic_ms: now,
                local_minute: 600,
            },
            allowed,
        );
    }
    fn running_focus() -> Change {
        focus(Session::Running {
            duration_ms: 1_500_000,
            remaining_ms: 900_000,
            anchor_utc_ms: 1000,
        })
    }
    #[test]
    fn water_opt_in_shows_only_water_from_a_mixed_batch_during_focus() {
        let mut r = reminder();
        pending(&mut r, 9, Mode::Automatic);
        let mut b = Bubble::default();
        water_policy_update(&mut b, &r, &running_focus(), true, 0);
        let id = b.selected.expect("water must be selected during focus").0;
        assert_eq!(b.automatic_prompt(&r, id), Some(vec![Id::Water]));
        assert_eq!(b.visible_item_count(&r), 1);
        assert_eq!(
            b.snapshot(&r).unwrap()["presentation"]["items"],
            serde_json::json!(["water"])
        );
        assert_eq!(r.presentation.as_ref().unwrap().items.len(), 3);
        assert!(b.open_choices(&r, id));
        pending(&mut r, 10, Mode::Manual);
        water_policy_update(&mut b, &r, &running_focus(), true, 1);
        assert_eq!(b.visible_item_count(&r), 3);
    }
    #[test]
    fn water_opt_in_never_enables_legacy_or_non_water_focus_prompts() {
        let mut r = reminder();
        pending(&mut r, 9, Mode::Automatic);
        let mut legacy = Bubble::default();
        water_policy_update(&mut legacy, &r, &running_focus(), false, 0);
        assert!(legacy.selected.is_none());
        r.presentation.as_mut().unwrap().items = vec![Id::Move, Id::Eyes];
        let mut b = Bubble::default();
        water_policy_update(&mut b, &r, &running_focus(), true, 0);
        assert!(b.selected.is_none());
    }
    #[test]
    fn water_opt_in_does_not_release_the_other_items_when_focus_pauses_or_water_completes() {
        let mut r = reminder();
        pending(&mut r, 9, Mode::Automatic);
        let mut b = Bubble::default();
        water_policy_update(&mut b, &r, &running_focus(), true, 0);
        let id = b.selected.expect("water must be selected during focus").0;
        let paused = focus(Session::Paused {
            duration_ms: 1_500_000,
            remaining_ms: 900_000,
        });
        water_policy_update(&mut b, &r, &paused, true, 1);
        assert_eq!(b.selected.unwrap().0, id);
        assert_eq!(b.automatic_prompt(&r, id), Some(vec![Id::Water]));
        r.presentation.as_mut().unwrap().items = vec![Id::Move, Id::Eyes];
        r.revision += 1;
        water_policy_update(&mut b, &r, &paused, true, 2);
        assert!(b.selected.is_none());
    }
    #[test]
    fn water_opt_in_still_respects_pause_quiet_hours_and_dismissal() {
        for blocked in 0..3 {
            let mut r = reminder();
            pending(&mut r, 9, Mode::Automatic);
            match blocked {
                0 => r.paused = true,
                1 => {
                    r.quiet = Some(Quiet {
                        until: 5000,
                        duration_minutes: 1,
                    })
                }
                _ => r.settings.active_hours = ActiveHours::Daily { start: 0, end: 1 },
            }
            let mut b = Bubble::default();
            water_policy_update(&mut b, &r, &running_focus(), true, 0);
            assert!(b.selected.is_none());
        }
        let mut r = reminder();
        pending(&mut r, 9, Mode::Automatic);
        let mut b = Bubble::default();
        water_policy_update(&mut b, &r, &running_focus(), true, 0);
        let id = b.selected.expect("water must be selected during focus").0;
        b.command(Command::Dismiss {
            presentation_id: id,
        })
        .unwrap();
        water_policy_update(&mut b, &r, &running_focus(), true, 1);
        assert!(b.selected.is_none());
    }
    #[test]
    fn cup_choices_require_the_live_automatic_presentation_and_clear_when_it_ends() {
        let mut r = reminder();
        pending(&mut r, 9, Mode::Automatic);
        let f = focus(Session::Idle {});
        let mut b = Bubble::default();
        update(&mut b, &r, &f, None, 0);
        let id = b.selected.unwrap().0;
        assert_eq!(b.window_presentation(&r), None);
        let mut ui = PresentationUi::default();
        session(&b, &mut ui, &r);
        assert!(ui.begin_create().is_none());
        assert!(!b.choices_open_for(id));
        assert!(!b.open_choices(&r, id + 1));
        assert!(b.open_choices(&r, id));
        update(&mut b, &r, &f, None, 1);
        assert_eq!(b.snapshot(&r).unwrap()["presentation"]["choicesOpen"], true);
        assert!(b.choices_open_for(id));
        assert_eq!(b.window_presentation(&r), Some(id));
        session(&b, &mut ui, &r);
        assert!(ui.begin_create().is_some());
        r.presentation = None;
        r.revision += 1;
        update(&mut b, &r, &f, None, 2);
        assert!(!b.choices_open_for(id));
        assert_eq!(b.window_presentation(&r), None);
    }
    #[test]
    fn committed_running_still_silences_reminders_during_transient_focus_write_failure() {
        let mut r = reminder();
        pending(&mut r, 3, Mode::Automatic);
        let mut f = focus(Session::Running {
            duration_ms: 60_000,
            remaining_ms: 60_000,
            anchor_utc_ms: 1000,
        });
        f.error = Some("writeFailed");
        let mut b = Bubble::default();
        update(&mut b, &r, &f, None, 0);
        assert!(b.selected.is_none());
    }
    #[test]
    fn simultaneous_three_pending_and_completion_only_selects_one_preserving_all_business_state() {
        let mut r = reminder();
        pending(&mut r, 9, Mode::Automatic);
        let original = r.clone();
        let f = finished();
        let mut b = Bubble::default();
        update(&mut b, &r, &f, Some(5), 0);
        let view = b.snapshot(&r).unwrap();
        assert_eq!(view["presentation"]["focusCompleted"], true);
        assert_eq!(
            view["progress"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|p| p["pending"] == true)
                .count(),
            3
        );
        assert_eq!(r, original);
        assert!(matches!(b.selected, Some((_, Source::Focus(5)))));
        update(&mut b, &r, &f, None, 10_000);
        assert_eq!(b.selected, None);
        assert_eq!(r, original);
    }
    #[test]
    fn quiet_active_hours_pause_readonly_and_runtime_error_do_not_queue_completion() {
        for case in 0..5 {
            let mut r = reminder();
            let mut b = Bubble::default();
            let f = finished();
            match case {
                0 => {
                    r.quiet = Some(Quiet {
                        until: 99999,
                        duration_minutes: 10,
                    })
                }
                1 => {
                    r.settings.active_hours = ActiveHours::Daily {
                        start: 700,
                        end: 800,
                    }
                }
                2 => r.paused = true,
                3 => r.persistence.status = SaveStatus::ReadOnly,
                _ => r.runtime_error = Some("invalidTime"),
            }
            update(&mut b, &r, &f, Some(5), 0);
            assert!(b.selected.is_none());
            r = reminder();
            update(&mut b, &r, &f, Some(5), 1);
            assert!(b.selected.is_none());
        }
    }
    #[test]
    fn manual_view_stays_open_running_and_on_completion_and_keeps_its_ticket() {
        let mut r = reminder();
        pending(&mut r, 3, Mode::Manual);
        let mut b = Bubble::default();
        let mut ui = PresentationUi::default();
        let running = focus(Session::Running {
            duration_ms: 60_000,
            remaining_ms: 60_000,
            anchor_utc_ms: 1000,
        });
        update(&mut b, &r, &running, None, 0);
        session(&b, &mut ui, &r);
        let token = ui.begin_create().unwrap();
        ui.created(token);
        let ticket = ui.ticket().unwrap();
        ui.ready(token, ticket.presentation);
        ui.finish_show(ticket, true);
        update(&mut b, &r, &finished(), Some(5), 1);
        session(&b, &mut ui, &r);
        assert_eq!(ui.ticket(), Some(ticket));
        assert!(ui.shown);
        assert_eq!(b.policy.presented(ticket.presentation), None);
    }
    #[test]
    fn running_fences_already_visible_and_late_ready_show_without_marking_pause() {
        for already_shown in [false, true] {
            let mut r = reminder();
            pending(&mut r, 3, Mode::Automatic);
            let mut b = Bubble::default();
            let mut ui = PresentationUi::default();
            update(&mut b, &r, &focus(Session::Idle {}), None, 0);
            assert!(b.open_choices(&r, b.selected.unwrap().0));
            update(&mut b, &r, &focus(Session::Idle {}), None, 0);
            session(&b, &mut ui, &r);
            let token = ui.begin_create().unwrap();
            ui.created(token);
            let ticket = ui.ticket().unwrap();
            ui.ready(token, ticket.presentation);
            if already_shown {
                ui.finish_show(ticket, true);
            }
            let running = focus(Session::Running {
                duration_ms: 60_000,
                remaining_ms: 60_000,
                anchor_utc_ms: 1000,
            });
            update(&mut b, &r, &running, None, 1);
            session(&b, &mut ui, &r);
            assert!(!ui.can_show(ticket));
            assert!(!ui.ready(token, ticket.presentation));
            assert!(!ui.shown);
            assert!(!r.paused);
            assert!(r.progress.iter().all(|p| p.pending));
        }
    }
    #[test]
    fn delayed_creation_expiry_reload_destroy_and_failed_show_never_replay_response() {
        let r = reminder();
        let f = finished();
        let mut b = Bubble::default();
        let mut ui = PresentationUi::default();
        update(&mut b, &r, &f, Some(5), 0);
        session(&b, &mut ui, &r);
        let token = ui.begin_create().unwrap();
        let id = b.selected.unwrap().0;
        update(&mut b, &r, &f, None, 10000);
        session(&b, &mut ui, &r);
        assert!(ui.created(token));
        assert!(!ui.ready(token, id));
        assert!(ui.ticket().is_none());
        assert_eq!(b.policy.presented(id), None);
        // A new live session may reuse the native window but has a distinct ticket.
        let mut f = finished();
        f.snapshot.revision = 6;
        update(&mut b, &r, &f, Some(6), 11000);
        session(&b, &mut ui, &r);
        let ticket = ui.ticket().unwrap();
        assert!(ui.ready(token, ticket.presentation));
        ui.finish_show(ticket, false);
        assert!(ui.ticket().is_none());
        assert_eq!(b.policy.respond(5), Some(5)); // failed show never consumed it
        ui.destroyed(token);
        assert!(ui.begin_create().is_none());
    }
    #[test]
    fn stale_dismiss_cannot_close_new_manual_view_and_focus_dismiss_keeps_durable_feedback() {
        let mut r = reminder();
        let f = finished();
        let mut b = Bubble::default();
        update(&mut b, &r, &f, Some(5), 0);
        let old = b.selected.unwrap().0;
        assert!(b
            .command(Command::Dismiss {
                presentation_id: old
            })
            .unwrap()
            .is_none());
        update(&mut b, &r, &f, None, 1);
        assert!(b.selected.is_none());
        assert_eq!(natural_revision(&f), Some(5));
        pending(&mut r, 9, Mode::Manual);
        update(&mut b, &r, &f, None, 2);
        assert_eq!(
            b.command(Command::Dismiss {
                presentation_id: old
            })
            .unwrap_err()
            .code,
            "stalePresentation"
        );
        let id = b.selected.unwrap().0;
        assert!(matches!(
            b.command(Command::Dismiss {
                presentation_id: id
            })
            .unwrap(),
            Some(Command::Dismiss { presentation_id: 9 })
        ));
    }
    #[test]
    fn restart_early_abandon_interrupted_errors_and_stale_event_cannot_create_completion() {
        let r = reminder();
        let mut b = Bubble::default();
        let f = finished();
        update(&mut b, &r, &f, None, 0);
        assert!(b.selected.is_none());
        for session in [
            Session::Idle {},
            Session::Interrupted {
                duration_ms: 60000,
                remaining_ms: 1000,
            },
            Session::Finished {
                duration_ms: 60000,
                outcome: Outcome::EndedEarly,
                feedback: Feedback::None,
            },
            Session::Finished {
                duration_ms: 60000,
                outcome: Outcome::Abandoned,
                feedback: Feedback::None,
            },
        ] {
            update(&mut b, &r, &focus(session), Some(5), 1);
            assert!(b.selected.is_none());
        }
        let mut error = finished();
        error.error = Some("writeFailed");
        update(&mut b, &r, &error, Some(5), 2);
        assert!(b.selected.is_none());
        let mut newer = finished();
        newer.snapshot.revision = 6;
        update(&mut b, &r, &newer, Some(5), 3);
        assert!(b.selected.is_none());
    }
    #[test]
    fn completion_response_is_consumed_once_across_reload_and_explicit_view() {
        let r = reminder();
        let f = finished();
        let mut b = Bubble::default();
        let mut ui = PresentationUi::default();
        update(&mut b, &r, &f, Some(5), 0);
        session(&b, &mut ui, &r);
        let token = ui.begin_create().unwrap();
        ui.created(token);
        let ticket = ui.ticket().unwrap();
        ui.ready(token, ticket.presentation);
        ui.finish_show(ticket, true);
        assert_eq!(b.policy.presented(ticket.presentation), Some(5));
        ui.reload(token);
        assert!(!ui.can_show(ticket));
        assert_eq!(b.policy.respond(5), None);
        update(&mut b, &r, &f, Some(5), 1);
        assert_eq!(b.policy.presented(ticket.presentation), None);
    }
}
