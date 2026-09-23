use super::model::*;

fn time(utc_ms: i64, monotonic_ms: u64) -> Time {
    Time {
        utc_ms,
        monotonic_ms,
    }
}

fn started(duration_ms: u64) -> Engine {
    let mut engine = Engine::new();
    assert!(
        !engine
            .step(Command::Start { duration_ms }, time(1_000_000, 100))
            .unwrap()
            .completed_now
    );
    engine
}

#[test]
fn start_validates_duration_and_is_explicit() {
    let mut engine = Engine::new();
    for duration_ms in [0, MIN_DURATION_MS - 1, MAX_DURATION_MS + 1, u64::MAX] {
        let before = engine.clone();
        assert_eq!(
            engine
                .step(Command::Start { duration_ms }, time(1_000_000, 100))
                .unwrap_err()
                .code,
            "invalidDuration"
        );
        assert_eq!(engine, before);
    }
    engine
        .step(
            Command::Start {
                duration_ms: MIN_DURATION_MS,
            },
            time(1_000_000, 100),
        )
        .unwrap();
    assert_eq!(
        engine
            .step(
                Command::Start {
                    duration_ms: MIN_DURATION_MS,
                },
                time(1_000_001, 101),
            )
            .unwrap_err()
            .code,
        "alreadyActive"
    );
}

#[test]
fn pause_freezes_remaining_and_resume_reanchors() {
    let mut engine = started(120_000);
    engine.step(Command::Tick, time(1_030_000, 30_100)).unwrap();
    engine
        .step(Command::Pause, time(1_030_000, 30_100))
        .unwrap();
    assert!(matches!(
        engine.data.session,
        Session::Paused {
            remaining_ms: 90_000,
            ..
        }
    ));
    let paused = engine.clone();
    engine.step(Command::Tick, time(1_090_000, 90_100)).unwrap();
    engine
        .step(Command::Pause, time(1_090_000, 90_100))
        .unwrap();
    assert_eq!(engine, paused);
    engine
        .step(Command::Resume, time(1_090_000, 90_100))
        .unwrap();
    engine
        .step(Command::Resume, time(1_090_000, 90_100))
        .unwrap();
    engine
        .step(Command::Tick, time(1_100_000, 100_100))
        .unwrap();
    assert!(matches!(
        engine.data.session,
        Session::Running {
            remaining_ms: 80_000,
            ..
        }
    ));
}

#[test]
fn natural_completion_emits_once_and_dismissal_survives_ticks() {
    let mut engine = started(MIN_DURATION_MS);
    assert!(
        !engine
            .step(Command::Tick, time(1_059_999, 60_099))
            .unwrap()
            .completed_now
    );
    assert!(
        engine
            .step(Command::Tick, time(1_060_000, 60_100))
            .unwrap()
            .completed_now
    );
    assert_eq!(
        engine.data.session,
        Session::Finished {
            duration_ms: MIN_DURATION_MS,
            outcome: Outcome::Natural,
            feedback: Feedback::Pending,
        }
    );
    assert!(
        !engine
            .step(Command::Tick, time(2_000_000, 100_100))
            .unwrap()
            .completed_now
    );
    engine
        .step(Command::DismissFeedback, time(2_000_000, 100_100))
        .unwrap();
    engine
        .step(Command::DismissFeedback, time(2_000_000, 100_100))
        .unwrap();
    assert!(matches!(
        engine.data.session,
        Session::Finished {
            feedback: Feedback::Dismissed,
            ..
        }
    ));
    let restored = Engine::restore(engine.data.clone(), time(2_100_000, 0)).unwrap();
    assert_eq!(restored.data, engine.data);
}

#[test]
fn natural_deadline_wins_over_a_late_end_command_once() {
    let mut engine = started(MIN_DURATION_MS);
    assert!(
        engine
            .step(Command::EndEarly, time(1_060_000, 60_100))
            .unwrap()
            .completed_now
    );
    assert!(matches!(
        engine.data.session,
        Session::Finished {
            outcome: Outcome::Natural,
            feedback: Feedback::Pending,
            ..
        }
    ));
    assert!(
        !engine
            .step(Command::EndEarly, time(1_060_001, 60_101))
            .unwrap()
            .completed_now
    );
}

#[test]
fn early_end_and_abandon_remain_distinct_and_do_not_emit_completion() {
    let mut ended = started(120_000);
    assert!(
        !ended
            .step(Command::EndEarly, time(1_020_000, 20_100))
            .unwrap()
            .completed_now
    );
    assert!(matches!(
        ended.data.session,
        Session::Finished {
            outcome: Outcome::EndedEarly,
            feedback: Feedback::None,
            ..
        }
    ));
    let snapshot = ended.clone();
    ended
        .step(Command::EndEarly, time(1_030_000, 30_100))
        .unwrap();
    assert_eq!(ended, snapshot);

    let mut abandoned = started(120_000);
    abandoned
        .step(Command::Pause, time(1_000_000, 100))
        .unwrap();
    assert!(
        !abandoned
            .step(Command::Abandon, time(1_050_000, 50_100))
            .unwrap()
            .completed_now
    );
    assert!(matches!(
        abandoned.data.session,
        Session::Finished {
            outcome: Outcome::Abandoned,
            feedback: Feedback::None,
            ..
        }
    ));
    let snapshot = abandoned.clone();
    abandoned
        .step(Command::Abandon, time(1_060_000, 60_100))
        .unwrap();
    assert_eq!(abandoned, snapshot);
}

#[test]
fn clock_anomalies_interrupt_without_extending_time() {
    for anomalous in [
        time(999_000, 101),
        time(1_001_000, 99),
        time(1_001_000, MAX_SAFE + 1),
        time(1_001_000, MAX_TRUSTED_GAP_MS + 101),
        time(1_360_001, 101),
    ] {
        let mut engine = started(120_000);
        let before = engine.clone();
        let result = engine.step(Command::Tick, anomalous);
        if anomalous.monotonic_ms > MAX_SAFE {
            assert_eq!(result.unwrap_err().code, "invalidTime");
            assert_eq!(engine, before);
        } else {
            assert!(!result.unwrap().completed_now);
            assert!(matches!(
                engine.data.session,
                Session::Interrupted {
                    remaining_ms: 120_000,
                    ..
                }
            ));
        }
    }
}

#[test]
fn short_sleep_does_not_discard_elapsed_wall_time_when_monotonic_stalls() {
    let mut engine = started(120_000);
    let result = engine.step(Command::Tick, time(1_180_000, 100)).unwrap();
    assert!(!result.completed_now);
    assert!(matches!(
        engine.data.session,
        Session::Interrupted {
            remaining_ms: 120_000,
            ..
        }
    ));
}

#[test]
fn small_clock_sample_jitter_does_not_accumulate_as_extra_session_time() {
    let mut engine = started(120_000);
    engine.step(Command::Tick, time(1_010_500, 10_100)).unwrap();
    assert!(matches!(
        engine.data.session,
        Session::Running {
            remaining_ms: 110_000,
            anchor_utc_ms: 1_010_000,
            ..
        }
    ));
    engine.step(Command::Tick, time(1_020_500, 20_100)).unwrap();
    assert!(matches!(
        engine.data.session,
        Session::Running {
            remaining_ms: 100_000,
            anchor_utc_ms: 1_020_000,
            ..
        }
    ));
}

#[test]
fn recovery_uses_bounded_utc_and_never_replays_completion_event() {
    let data = started(120_000).data;
    let restored = Engine::restore(data.clone(), time(1_030_000, 5)).unwrap();
    assert!(matches!(
        restored.data.session,
        Session::Running {
            remaining_ms: 90_000,
            anchor_utc_ms: 1_030_000,
            ..
        }
    ));
    let mut expired = Engine::restore(data.clone(), time(1_120_000, 5)).unwrap();
    assert!(matches!(
        expired.data.session,
        Session::Finished {
            outcome: Outcome::Natural,
            feedback: Feedback::Pending,
            ..
        }
    ));
    assert!(
        !expired
            .step(Command::Tick, time(1_130_000, 15))
            .unwrap()
            .completed_now
    );
    for uncertain in [
        time(999_999, 5),
        time(1_000_000 + MAX_TRUSTED_GAP_MS as i64 + 1, 5),
    ] {
        let interrupted = Engine::restore(data.clone(), uncertain).unwrap();
        assert!(matches!(
            interrupted.data.session,
            Session::Interrupted {
                remaining_ms: 120_000,
                ..
            }
        ));
    }
}

#[test]
fn interrupted_session_needs_explicit_resume_and_keeps_last_known_remaining() {
    let mut engine = started(120_000);
    engine.step(Command::Tick, time(1_020_000, 20_100)).unwrap();
    engine.step(Command::Tick, time(999_000, 20_101)).unwrap();
    assert!(matches!(
        engine.data.session,
        Session::Interrupted {
            remaining_ms: 100_000,
            ..
        }
    ));
    engine.step(Command::Tick, time(1_060_000, 60_100)).unwrap();
    assert!(matches!(engine.data.session, Session::Interrupted { .. }));
    engine
        .step(Command::Resume, time(1_060_000, 60_100))
        .unwrap();
    engine
        .step(Command::Tick, time(1_160_000, 160_100))
        .unwrap();
    assert!(matches!(
        engine.data.session,
        Session::Finished {
            outcome: Outcome::Natural,
            ..
        }
    ));
}

#[test]
fn invalid_serialized_combinations_and_deadline_overflow_are_rejected() {
    let mut engine = Engine::new();
    let before = engine.clone();
    assert_eq!(
        engine
            .step(
                Command::Start {
                    duration_ms: MIN_DURATION_MS,
                },
                time(MAX_UTC, 0),
            )
            .unwrap_err()
            .code,
        "invalidTime"
    );
    assert_eq!(engine, before);

    let invalid = Data {
        version: VERSION,
        session: Session::Finished {
            duration_ms: MIN_DURATION_MS,
            outcome: Outcome::Abandoned,
            feedback: Feedback::Pending,
        },
    };
    assert_eq!(invalid.validate().unwrap_err().code, "invalidState");
    let invalid = Data {
        version: VERSION,
        session: Session::Running {
            duration_ms: MIN_DURATION_MS,
            remaining_ms: MIN_DURATION_MS,
            anchor_utc_ms: MAX_UTC,
        },
    };
    assert_eq!(invalid.validate().unwrap_err().code, "invalidState");
}

#[test]
fn invalid_existing_state_cannot_emit_completion_or_mutate_engine() {
    let mut engine = started(MIN_DURATION_MS);
    engine.data.session = Session::Running {
        duration_ms: MIN_DURATION_MS,
        remaining_ms: 0,
        anchor_utc_ms: 1_000_000,
    };
    let before = engine.clone();
    assert_eq!(
        engine
            .step(Command::Tick, time(1_000_000, 100))
            .unwrap_err()
            .code,
        "invalidState"
    );
    assert_eq!(engine, before);
}

#[test]
fn transitions_are_atomic_and_versioned_state_is_rejected() {
    let mut engine = started(MIN_DURATION_MS);
    let before = engine.clone();
    assert_eq!(
        engine
            .step(Command::DismissFeedback, time(1_000_000, 100))
            .unwrap_err()
            .code,
        "invalidTransition"
    );
    assert_eq!(engine, before);
    assert_eq!(
        engine.step(Command::Tick, time(-1, 101)).unwrap_err().code,
        "invalidTime"
    );
    assert_eq!(engine, before);

    let mut value = serde_json::to_value(&engine.data).unwrap();
    value["version"] = serde_json::json!(2);
    let future: Data = serde_json::from_value(value).unwrap();
    assert_eq!(future.validate().unwrap_err().code, "unsupportedVersion");
    assert_eq!(
        Engine::restore(future, time(1_000_000, 0))
            .unwrap_err()
            .code,
        "unsupportedVersion"
    );
}
