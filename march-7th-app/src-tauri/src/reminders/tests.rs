use super::model::*;
fn time(ms: i64) -> Time {
    Time {
        utc_ms: ms,
        local_minute: 600,
        monotonic_ms: ms as u64,
    }
}
fn enabled() -> Settings {
    let mut s = Settings {
        active_hours: ActiveHours::AllDay {},
        ..Settings::default()
    };
    for i in &mut s.items {
        i.enabled = true;
        i.interval_minutes = 1;
    }
    s
}
fn engine() -> Engine {
    let mut e = Engine::new(Data::default(), false).unwrap();
    e.step(
        Some(Command::UpdateSettings {
            settings: enabled(),
        }),
        time(0),
    )
    .unwrap();
    e
}
#[test]
fn simultaneous_due_is_one_batch_and_never_repeats() {
    let mut e = engine();
    e.step(None, time(MINUTE)).unwrap();
    assert!(e.data.progress.iter().all(|p| p.pending && p.auto_handled));
    assert_eq!(e.presentation.as_ref().unwrap().items, IDS);
    e.step(None, time(MINUTE + 10_000)).unwrap();
    assert!(e.presentation.is_none());
    e.step(None, time(99 * MINUTE)).unwrap();
    assert!(e.presentation.is_none());
    let mut restart = Engine::new(e.data, false).unwrap();
    restart.step(None, time(100 * MINUTE)).unwrap();
    assert!(restart.presentation.is_none());
}
#[test]
fn snooze_defers_all_pending_across_pause_and_consumes_once() {
    let mut e = engine();
    e.step(None, time(MINUTE)).unwrap();
    e.step(Some(Command::SnoozeAll {}), time(MINUTE)).unwrap();
    e.step(Some(Command::SetPaused { paused: true }), time(2 * MINUTE))
        .unwrap();
    e.step(None, time(12 * MINUTE)).unwrap();
    assert!(e.data.snooze_pending);
    assert!(e.presentation.is_none());
    e.step(
        Some(Command::SetPaused { paused: false }),
        time(13 * MINUTE),
    )
    .unwrap();
    assert_eq!(e.presentation.as_ref().unwrap().items, IDS);
    assert!(!e.data.snooze_pending);
    e.step(None, time(14 * MINUTE)).unwrap();
    assert!(e.presentation.is_none());
}

#[test]
fn first_boot_off_and_invalid_changes_are_transactional() {
    let mut e = Engine::new(Data::default(), false).unwrap();
    assert!(e.data.settings.items.iter().all(|i| !i.enabled));
    assert_eq!(
        e.data
            .settings
            .items
            .iter()
            .map(|i| i.interval_minutes)
            .collect::<Vec<_>>(),
        vec![60, 60, 30]
    );
    e.step(None, time(100 * MINUTE)).unwrap();
    assert!(e.presentation.is_none());
    for invalid in [0, 1441, u16::MAX] {
        let before = e.clone();
        let mut s = enabled();
        s.items[0].interval_minutes = invalid;
        assert_eq!(
            e.step(Some(Command::UpdateSettings { settings: s }), time(0))
                .unwrap_err()
                .code,
            "invalidSettings"
        );
        assert_eq!(e, before);
    }
    for t in [
        Time {
            utc_ms: -1,
            ..time(0)
        },
        Time {
            utc_ms: i64::MAX,
            ..time(0)
        },
        Time {
            local_minute: -1,
            ..time(0)
        },
        Time {
            local_minute: 1440,
            ..time(0)
        },
        Time {
            monotonic_ms: u64::MAX,
            ..time(0)
        },
    ] {
        let before = e.clone();
        assert!(e.step(None, t).is_err());
        assert_eq!(e, before);
    }
    let before = e.clone();
    assert!(e
        .step(
            Some(Command::UpdateSettings {
                settings: enabled()
            }),
            Time {
                utc_ms: MAX_UTC,
                ..time(0)
            }
        )
        .is_err());
    assert_eq!(e, before);
}
#[test]
fn new_due_extends_batch_without_extending_deadline_and_does_not_readd_old() {
    let mut s = enabled();
    s.items[1].interval_minutes = 2;
    s.items[2].enabled = false;
    let mut e = Engine::new(Data::default(), false).unwrap();
    e.step(
        Some(Command::UpdateSettings {
            settings: s.clone(),
        }),
        time(0),
    )
    .unwrap();
    e.step(None, time(MINUTE)).unwrap();
    let id = e.presentation.as_ref().unwrap().id;
    e.step(
        Some(Command::Dismiss {
            presentation_id: id,
        }),
        time(MINUTE + 1),
    )
    .unwrap();
    e.step(None, time(2 * MINUTE)).unwrap();
    assert_eq!(e.presentation.as_ref().unwrap().items, vec![Id::Move]);
    // Arrange another independent due time through a genuine later enable action.
    s.items[2].enabled = true;
    e.step(
        Some(Command::UpdateSettings { settings: s }),
        time(2 * MINUTE + 1),
    )
    .unwrap();
    e.step(
        Some(Command::Complete { id: Id::Water }),
        time(2 * MINUTE + 5),
    )
    .unwrap();
    e.step(None, time(3 * MINUTE + 1)).unwrap();
    let p = e.presentation.clone().unwrap();
    assert_eq!(p.items, vec![Id::Eyes]);
    e.step(None, time(3 * MINUTE + 5)).unwrap();
    let joined = e.presentation.as_ref().unwrap();
    assert_eq!(joined.id, p.id);
    assert_eq!(joined.closes_at, p.closes_at);
    assert_eq!(joined.items, vec![Id::Eyes, Id::Water]);
}
#[test]
fn snooze_covers_new_due_and_remembers_applied_duration_after_settings_change() {
    let mut e = engine();
    let mut s = enabled();
    s.items[2].interval_minutes = 5;
    e.step(
        Some(Command::UpdateSettings {
            settings: s.clone(),
        }),
        time(0),
    )
    .unwrap();
    e.step(None, time(MINUTE)).unwrap();
    e.step(Some(Command::SnoozeAll {}), time(MINUTE)).unwrap();
    s.snooze_minutes = 1;
    e.step(
        Some(Command::UpdateSettings { settings: s }),
        time(2 * MINUTE),
    )
    .unwrap();
    e.step(None, time(6 * MINUTE)).unwrap();
    assert!(e.data.progress[2].pending);
    assert!(e.presentation.is_none());
    assert_eq!(e.data.quiet.as_ref().unwrap().until, 11 * MINUTE);
    let mut restored = Engine::new(e.data, false).unwrap();
    restored.step(None, time(7 * MINUTE)).unwrap();
    assert!(restored.presentation.is_none());
    restored.step(None, time(11 * MINUTE)).unwrap();
    assert_eq!(restored.presentation.as_ref().unwrap().items, IDS);
}
#[test]
fn midnight_hours_start_included_end_excluded_and_pause_does_not_freeze() {
    let mut s = enabled();
    s.active_hours = ActiveHours::Daily {
        start: 1320,
        end: 120,
    };
    let mut e = Engine::new(Data::default(), false).unwrap();
    e.step(Some(Command::UpdateSettings { settings: s }), time(0))
        .unwrap();
    e.step(
        None,
        Time {
            local_minute: 1319,
            ..time(MINUTE)
        },
    )
    .unwrap();
    assert!(e.data.progress[0].pending);
    assert!(e.presentation.is_none());
    e.step(
        None,
        Time {
            local_minute: 1320,
            ..time(2 * MINUTE)
        },
    )
    .unwrap();
    assert!(e.presentation.is_some());
    e.step(
        None,
        Time {
            local_minute: 120,
            ..time(2 * MINUTE + 1)
        },
    )
    .unwrap();
    assert!(e.presentation.is_none());
    e.step(
        None,
        Time {
            local_minute: 0,
            ..time(3 * MINUTE)
        },
    )
    .unwrap();
    assert!(e.presentation.is_none());
    e.step(Some(Command::SetPaused { paused: true }), time(4 * MINUTE))
        .unwrap();
    e.step(Some(Command::Complete { id: Id::Water }), time(4 * MINUTE))
        .unwrap();
    e.step(None, time(6 * MINUTE)).unwrap();
    assert!(e.data.progress[0].pending);
    assert!(e.presentation.is_none());
    e.step(
        Some(Command::SetPaused { paused: false }),
        Time {
            local_minute: 119,
            ..time(7 * MINUTE)
        },
    )
    .unwrap();
    assert_eq!(e.presentation.unwrap().items, vec![Id::Water]);
}
#[test]
fn manual_view_preserves_controls_empty_state_and_generation() {
    let mut e = engine();
    e.step(None, time(MINUTE)).unwrap();
    let old = e.presentation.as_ref().unwrap().id;
    e.step(Some(Command::SnoozeAll {}), time(MINUTE)).unwrap();
    e.step(Some(Command::SetPaused { paused: true }), time(MINUTE))
        .unwrap();
    e.step(Some(Command::ShowPending {}), time(MINUTE)).unwrap();
    let manual = e.presentation.clone().unwrap();
    assert!(manual.id > old);
    assert_eq!(manual.mode, Mode::Manual);
    assert_eq!(manual.closes_at, None);
    e.step(
        Some(Command::Dismiss {
            presentation_id: old,
        }),
        time(2 * MINUTE),
    )
    .unwrap();
    assert_eq!(e.presentation, Some(manual));
    assert!(e.data.paused);
    assert!(e.data.quiet.is_some());
    assert!(e.data.progress.iter().all(|p| p.pending));
    for id in IDS {
        e.step(Some(Command::Complete { id }), time(2 * MINUTE))
            .unwrap();
    }
    assert!(e.presentation.unwrap().items.is_empty());
}
#[test]
fn completion_is_local_and_duplicate_is_noop_settings_only_reset_changed_item() {
    let mut e = engine();
    e.step(None, time(MINUTE)).unwrap();
    assert_eq!(
        e.step(Some(Command::Complete { id: Id::Water }), time(2 * MINUTE))
            .unwrap(),
        Some(Response::Complete { id: Id::Water })
    );
    assert!(e.data.progress[1].pending);
    assert_eq!(e.data.progress[0].next_due_at, Some(3 * MINUTE));
    assert_eq!(
        e.step(
            Some(Command::Complete { id: Id::Water }),
            time(2 * MINUTE + 1)
        )
        .unwrap(),
        None
    );
    assert_eq!(e.data.progress[0].next_due_at, Some(3 * MINUTE));
    let before = e.data.clone();
    e.step(
        Some(Command::UpdateSettings {
            settings: before.settings.clone(),
        }),
        time(2 * MINUTE + 2),
    )
    .unwrap();
    assert_eq!(e.data, before);
    let mut s = e.data.settings.clone();
    s.items[1].enabled = false;
    s.items[2].interval_minutes = 2;
    e.step(
        Some(Command::UpdateSettings {
            settings: s.clone(),
        }),
        time(2 * MINUTE + 3),
    )
    .unwrap();
    assert_eq!(e.data.progress[0], before.progress[0]);
    assert_eq!(e.data.progress[1].next_due_at, None);
    assert!(!e.data.progress[1].pending);
    assert_eq!(e.data.progress[2].next_due_at, Some(4 * MINUTE + 3));
    s.items[1].enabled = true;
    e.step(
        Some(Command::UpdateSettings { settings: s }),
        time(2 * MINUTE + 4),
    )
    .unwrap();
    assert_eq!(e.data.progress[1].next_due_at, Some(3 * MINUTE + 4));
}
#[test]
fn last_auto_completion_closes_and_wall_or_monotonic_expiry_each_suffices() {
    let mut e = engine();
    e.step(None, time(MINUTE)).unwrap();
    for id in IDS {
        e.step(Some(Command::Complete { id }), time(MINUTE))
            .unwrap();
    }
    assert!(e.presentation.is_none());
    e.step(None, time(2 * MINUTE)).unwrap();
    e.step(
        None,
        Time {
            utc_ms: 0,
            local_minute: 600,
            monotonic_ms: 2 * MINUTE as u64 + 10_000,
        },
    )
    .unwrap();
    assert!(e.presentation.is_none());
    let mut e = engine();
    e.step(None, time(MINUTE)).unwrap();
    e.step(
        None,
        Time {
            utc_ms: 20 * MINUTE,
            local_minute: 600,
            monotonic_ms: MINUTE as u64 + 1,
        },
    )
    .unwrap();
    assert!(e.presentation.is_none());
}
#[test]
fn backwards_restart_caps_due_and_quiet_forward_jump_coalesces() {
    let mut e = engine();
    e.step(
        Some(Command::Complete { id: Id::Water }),
        time(100 * MINUTE),
    )
    .unwrap();
    e.step(Some(Command::SnoozeAll {}), time(100 * MINUTE))
        .unwrap();
    let mut restart = Engine::new(e.data, false).unwrap();
    restart.step(None, time(0)).unwrap();
    assert_eq!(restart.data.progress[0].next_due_at, Some(MINUTE));
    assert_eq!(restart.data.quiet.unwrap().until, 10 * MINUTE);
    let mut jump = engine();
    jump.step(None, time(10000 * MINUTE)).unwrap();
    assert_eq!(jump.presentation.unwrap().items, IDS);
    assert!(jump
        .data
        .progress
        .iter()
        .all(|p| p.pending && p.next_due_at.is_none()));
}
#[test]
fn protected_state_rejects_configuration_but_manual_controls_work() {
    let mut e = engine();
    e.step(None, time(MINUTE)).unwrap();
    e.protected = true;
    assert_eq!(
        e.step(
            Some(Command::UpdateSettings {
                settings: enabled()
            }),
            time(MINUTE)
        )
        .unwrap_err()
        .code,
        "readOnly"
    );
    e.step(Some(Command::SetPaused { paused: true }), time(MINUTE))
        .unwrap();
    assert!(e.presentation.is_none());
    e.step(Some(Command::ShowPending {}), time(MINUTE)).unwrap();
    let id = e.presentation.as_ref().unwrap().id;
    e.step(
        Some(Command::Dismiss {
            presentation_id: id,
        }),
        time(MINUTE),
    )
    .unwrap();
    assert!(e.presentation.is_none());
}
#[test]
fn dto_rejects_unknown_ids_extra_fields_fractional_and_out_of_range_values() {
    for value in [
        r#"{"type":"complete","id":"unknown"}"#,
        r#"{"type":"snoozeAll","now":0}"#,
        r#"{"type":"dismiss","presentationId":1.5}"#,
        r#"{"type":"setPaused","paused":"true"}"#,
        r#"{"type":"showPending","utcMs":0}"#,
    ] {
        assert!(serde_json::from_str::<Command>(value).is_err(), "{value}");
    }
    let mut e = engine();
    for id in [0, MAX_SAFE + 1, u64::MAX] {
        assert!(e
            .step(
                Some(Command::Dismiss {
                    presentation_id: id
                }),
                time(0)
            )
            .is_err());
    }
    let mut data = Data::default();
    data.progress[0].pending = true;
    assert!(Engine::new(data, false).is_err());
}
#[test]
fn manual_in_pause_handles_ordinary_intent_including_new_arrivals() {
    let mut e = engine();
    let mut s = enabled();
    s.items[2].interval_minutes = 2;
    e.step(Some(Command::UpdateSettings { settings: s }), time(0))
        .unwrap();
    e.step(Some(Command::SetPaused { paused: true }), time(0))
        .unwrap();
    e.step(None, time(MINUTE)).unwrap();
    e.step(Some(Command::ShowPending {}), time(MINUTE)).unwrap();
    let id = e.presentation.as_ref().unwrap().id;
    e.step(None, time(2 * MINUTE)).unwrap();
    assert_eq!(e.presentation.as_ref().unwrap().id, id);
    assert_eq!(e.presentation.as_ref().unwrap().mode, Mode::Manual);
    assert!(e.data.progress.iter().all(|p| p.auto_handled));
    e.step(
        Some(Command::Dismiss {
            presentation_id: id,
        }),
        time(2 * MINUTE),
    )
    .unwrap();
    e.step(Some(Command::SetPaused { paused: false }), time(2 * MINUTE))
        .unwrap();
    assert!(e.presentation.is_none());
    assert!(e.data.progress.iter().all(|p| p.pending));
}
#[test]
fn manual_during_snooze_preserves_explicit_reprompt() {
    let mut e = engine();
    e.step(Some(Command::SnoozeAll {}), time(0)).unwrap();
    e.step(None, time(MINUTE)).unwrap();
    e.step(Some(Command::ShowPending {}), time(MINUTE)).unwrap();
    let id = e.presentation.as_ref().unwrap().id;
    assert!(e.data.progress.iter().all(|p| p.auto_handled));
    assert!(e.data.snooze_pending);
    e.step(
        Some(Command::Dismiss {
            presentation_id: id,
        }),
        time(MINUTE),
    )
    .unwrap();
    e.step(None, time(10 * MINUTE)).unwrap();
    assert_eq!(e.presentation.as_ref().unwrap().mode, Mode::Automatic);
    assert_eq!(e.presentation.as_ref().unwrap().items, IDS);
    assert!(!e.data.snooze_pending);
}
#[test]
fn empty_snooze_expires_without_empty_auto_and_hour_changes_keep_progress() {
    let mut e = Engine::new(Data::default(), false).unwrap();
    e.step(Some(Command::SnoozeAll {}), time(0)).unwrap();
    e.step(None, time(10 * MINUTE)).unwrap();
    assert!(!e.data.snooze_pending);
    assert!(e.presentation.is_none());
    e.step(
        Some(Command::UpdateSettings {
            settings: enabled(),
        }),
        time(10 * MINUTE),
    )
    .unwrap();
    let before = e.data.progress.clone();
    let mut settings = e.data.settings.clone();
    settings.active_hours = ActiveHours::Daily { start: 0, end: 1 };
    settings.snooze_minutes = 120;
    e.step(
        Some(Command::UpdateSettings { settings }),
        time(10 * MINUTE + 1),
    )
    .unwrap();
    assert_eq!(e.data.progress, before);
}
#[test]
fn interval_and_snooze_upper_bounds_valid_hour_and_set_cross_state_invalid() {
    let mut settings = enabled();
    settings.items[0].interval_minutes = 1440;
    settings.snooze_minutes = 120;
    settings.validate().unwrap();
    for hours in [
        ActiveHours::Daily { start: 1, end: 1 },
        ActiveHours::Daily {
            start: 1440,
            end: 1,
        },
        ActiveHours::Daily {
            start: 1,
            end: 1440,
        },
    ] {
        settings.active_hours = hours;
        assert!(settings.validate().is_err());
    }
    let invalid = Data {
        quiet: Some(Quiet {
            until: 0,
            duration_minutes: 10,
        }),
        ..Data::default()
    };
    assert!(invalid.validate().is_err());
    let mut invalid = Data::default();
    invalid.progress[0].auto_handled = true;
    assert!(invalid.validate().is_err());
}

fn manual_snooze() -> Engine {
    let mut e = engine();
    e.step(Some(Command::SnoozeAll {}), time(0)).unwrap();
    e.step(Some(Command::ShowPending {}), time(MINUTE)).unwrap();
    e
}
#[test]
fn fixround1_manual_consumes_due_snooze_without_changing_view_or_close_bounce() {
    let mut e = manual_snooze();
    let view = e.presentation.clone().unwrap();
    assert!(e.data.snooze_pending);
    e.step(None, time(10 * MINUTE)).unwrap();
    assert!(!e.data.snooze_pending);
    assert!(e.data.quiet.is_none());
    assert_eq!(e.presentation, Some(view.clone()));
    e.step(
        Some(Command::Dismiss {
            presentation_id: view.id,
        }),
        time(10 * MINUTE + 1),
    )
    .unwrap();
    assert!(e.presentation.is_none());
    assert!(e.data.progress.iter().all(|p| p.pending));
}
#[test]
fn fixround1_dismiss_as_first_expired_step_consumes_using_previous_manual_view() {
    let mut e = manual_snooze();
    let id = e.presentation.as_ref().unwrap().id;
    e.step(
        Some(Command::Dismiss {
            presentation_id: id,
        }),
        time(10 * MINUTE),
    )
    .unwrap();
    assert!(e.presentation.is_none());
    assert!(!e.data.snooze_pending);
    assert!(e.data.progress.iter().all(|p| p.auto_handled));
}
#[test]
fn fixround1_expired_manual_snooze_waits_for_pause_and_hours_then_consumes_in_place() {
    let mut e = manual_snooze();
    let view = e.presentation.clone().unwrap();
    e.step(Some(Command::SetPaused { paused: true }), time(2 * MINUTE))
        .unwrap();
    e.step(None, time(10 * MINUTE)).unwrap();
    assert!(e.data.snooze_pending);
    assert_eq!(e.presentation, Some(view.clone()));
    let mut settings = e.data.settings.clone();
    settings.active_hours = ActiveHours::Daily {
        start: 1000,
        end: 1200,
    };
    e.step(
        Some(Command::UpdateSettings { settings }),
        time(10 * MINUTE),
    )
    .unwrap();
    e.step(
        Some(Command::SetPaused { paused: false }),
        time(11 * MINUTE),
    )
    .unwrap();
    assert!(e.data.snooze_pending);
    e.step(
        None,
        Time {
            local_minute: 1000,
            ..time(12 * MINUTE)
        },
    )
    .unwrap();
    assert!(!e.data.snooze_pending);
    assert_eq!(e.presentation, Some(view));
}
#[test]
fn fixround1_new_snooze_at_previous_expiry_keeps_new_future_request() {
    let mut e = manual_snooze();
    e.step(Some(Command::SnoozeAll {}), time(10 * MINUTE))
        .unwrap();
    assert!(e.presentation.is_none());
    assert!(e.data.snooze_pending);
    assert_eq!(e.data.quiet.as_ref().unwrap().until, 20 * MINUTE);
    e.step(None, time(20 * MINUTE)).unwrap();
    assert_eq!(e.presentation.as_ref().unwrap().mode, Mode::Automatic);
    assert!(!e.data.snooze_pending);
}
#[test]
fn fixround1_manual_dismiss_before_expiry_does_not_cancel_future_snooze() {
    let mut e = manual_snooze();
    let id = e.presentation.as_ref().unwrap().id;
    e.step(
        Some(Command::Dismiss {
            presentation_id: id,
        }),
        time(2 * MINUTE),
    )
    .unwrap();
    assert!(e.data.snooze_pending);
    assert!(e.data.quiet.is_some());
    e.step(None, time(10 * MINUTE)).unwrap();
    assert_eq!(e.presentation.as_ref().unwrap().mode, Mode::Automatic);
}
