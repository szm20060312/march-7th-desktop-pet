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
fn pause_during_new_clock_anomaly_preserves_interrupted_state() {
    let mut engine = started(120_000);
    assert!(
        !engine
            .step(Command::Pause, time(1_180_000, 100))
            .unwrap()
            .completed_now
    );
    assert!(matches!(
        engine.data.session,
        Session::Interrupted {
            remaining_ms: 120_000,
            ..
        }
    ));
    engine.step(Command::Pause, time(1_180_001, 101)).unwrap();
    assert!(matches!(engine.data.session, Session::Interrupted { .. }));
}

#[test]
fn duplicate_resume_during_new_clock_anomaly_does_not_confirm_it() {
    let mut engine = started(120_000);
    engine.step(Command::Resume, time(1_180_000, 100)).unwrap();
    assert!(matches!(
        engine.data.session,
        Session::Interrupted {
            remaining_ms: 120_000,
            ..
        }
    ));
    engine.step(Command::Resume, time(1_180_001, 101)).unwrap();
    assert!(matches!(engine.data.session, Session::Running { .. }));
}

#[test]
fn explicit_termination_during_clock_anomaly_keeps_requested_outcome() {
    for (command, outcome) in [
        (Command::EndEarly, Outcome::EndedEarly),
        (Command::Abandon, Outcome::Abandoned),
    ] {
        let mut engine = started(120_000);
        assert!(
            !engine
                .step(command, time(1_180_000, 100))
                .unwrap()
                .completed_now
        );
        assert!(matches!(
            engine.data.session,
            Session::Finished {
                outcome: actual,
                feedback: Feedback::None,
                ..
            } if actual == outcome
        ));
    }
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
fn negative_wall_clock_jitter_does_not_interrupt_successive_short_ticks() {
    let mut engine = started(120_000);
    engine.step(Command::Tick, time(1_009_500, 10_100)).unwrap();
    assert!(matches!(
        engine.data.session,
        Session::Running {
            remaining_ms: 110_000,
            anchor_utc_ms: 1_010_000,
            ..
        }
    ));
    for (utc_ms, monotonic_ms, remaining_ms, anchor_utc_ms) in [
        (1_009_600, 10_200, 109_900, 1_010_100),
        (1_009_700, 10_300, 109_800, 1_010_200),
        (1_009_800, 10_400, 109_700, 1_010_300),
    ] {
        assert!(
            !engine
                .step(Command::Tick, time(utc_ms, monotonic_ms))
                .unwrap()
                .completed_now
        );
        assert!(matches!(
            engine.data.session,
            Session::Running {
                remaining_ms: actual_remaining,
                anchor_utc_ms: actual_anchor,
                ..
            } if actual_remaining == remaining_ms && actual_anchor == anchor_utc_ms
        ));
    }
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
        version: crate::focus::model::VERSION,
        task: None,
        session: Session::Finished {
            duration_ms: MIN_DURATION_MS,
            outcome: Outcome::Abandoned,
            feedback: Feedback::Pending,
        },
    };
    assert_eq!(invalid.validate().unwrap_err().code, "invalidState");
    let invalid = Data {
        version: crate::focus::model::VERSION,
        task: None,
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
    value["version"] = serde_json::json!(3);
    let future: Data = serde_json::from_value(value).unwrap();
    assert_eq!(future.validate().unwrap_err().code, "unsupportedVersion");
    assert_eq!(
        Engine::restore(future, time(1_000_000, 0))
            .unwrap_err()
            .code,
        "unsupportedVersion"
    );
}
#[test]
fn m5b_v1_migrates_without_losing_finished_feedback() {
    let old = br#"{"version":1,"session":{"status":"finished","duration_ms":60000,"outcome":"natural","feedback":"pending"}}"#;
    let data = super::store::decode(old).unwrap();
    let value = serde_json::to_value(data).unwrap();
    assert_eq!(value["version"], 2);
    assert_eq!(value["task"], serde_json::Value::Null);
    assert_eq!(value["session"]["feedback"], "pending");
}
#[test]
fn m5b_task_name_boundaries_and_mutual_exclusion() {
    use super::model::{normalize_task_name, TaskStatus};
    assert_eq!(
        normalize_task_name("  阅读  ").unwrap().as_deref(),
        Some("阅读")
    );
    assert_eq!(normalize_task_name("　 ").unwrap(), None);
    for name in [
        "x".repeat(81),
        "😀".repeat(65),
        "a\nb".into(),
        "\t".into(),
        "\u{85}".into(),
    ] {
        assert_eq!(
            normalize_task_name(&name).unwrap_err().code,
            "invalidTaskName"
        );
    }
    for name in ["中".repeat(80), "😀".repeat(64), "a".repeat(80)] {
        assert!(normalize_task_name(&name).unwrap().is_some());
    }
    for (command, status, opposite) in [
        (
            Command::CompleteTask,
            TaskStatus::Completed,
            Command::AbandonTask,
        ),
        (
            Command::AbandonTask,
            TaskStatus::Abandoned,
            Command::CompleteTask,
        ),
    ] {
        let mut engine = Engine::new();
        assert_eq!(
            engine.step(command.clone(), time(0, 0)).unwrap_err().code,
            "noTask"
        );
        engine
            .step(
                Command::StartWithTask {
                    duration_ms: MIN_DURATION_MS,
                    task_name: "  读书  ".into(),
                },
                time(0, 0),
            )
            .unwrap();
        let result = engine.step(command.clone(), time(1, 1)).unwrap();
        assert_eq!(result.task_response, Some(status));
        assert!(!result.completed_now);
        assert!(matches!(engine.data.session, Session::Running { .. }));
        assert_eq!(engine.data.task.as_ref().unwrap().name, "读书");
        assert_eq!(
            engine.step(command, time(1, 1)).unwrap().task_response,
            None
        );
        let previous = engine.clone();
        assert_eq!(
            engine.step(opposite, time(1, 1)).unwrap_err().code,
            "taskResolved"
        );
        assert_eq!(engine, previous);
        engine.step(Command::Pause, time(1, 1)).unwrap();
        engine.step(Command::Resume, time(1, 1)).unwrap();
        engine
            .step(Command::Tick, time(MIN_DURATION_MS as i64, MIN_DURATION_MS))
            .unwrap();
        assert_eq!(engine.data.task.as_ref().unwrap().status, status);
        engine
            .step(
                Command::Start {
                    duration_ms: MIN_DURATION_MS,
                },
                time(MIN_DURATION_MS as i64, MIN_DURATION_MS),
            )
            .unwrap();
        assert!(engine.data.task.is_none());
    }
}
#[test]
fn m5b_natural_end_does_not_complete_task_and_restore_never_replays_it() {
    use super::model::TaskStatus;
    let mut engine = Engine::new();
    engine
        .step(
            Command::StartWithTask {
                duration_ms: MIN_DURATION_MS,
                task_name: "一件事".into(),
            },
            time(0, 0),
        )
        .unwrap();
    let result = engine
        .step(Command::Tick, time(MIN_DURATION_MS as i64, MIN_DURATION_MS))
        .unwrap();
    assert!(result.completed_now);
    assert!(result.task_response.is_none());
    assert_eq!(
        engine.data.task.as_ref().unwrap().status,
        TaskStatus::Active
    );
    let mut restored = Engine::restore(engine.data, time(MIN_DURATION_MS as i64, 0)).unwrap();
    assert_eq!(
        restored
            .step(Command::CompleteTask, time(MIN_DURATION_MS as i64, 0))
            .unwrap()
            .task_response,
        Some(TaskStatus::Completed)
    );
    let mut restarted = Engine::restore(restored.data, time(MIN_DURATION_MS as i64, 0)).unwrap();
    assert!(restarted
        .step(Command::Tick, time(MIN_DURATION_MS as i64, 0))
        .unwrap()
        .task_response
        .is_none());
}
#[test]
fn m5b_decode_v1_running_and_v2_are_strict_and_bounded() {
    let old=br#"{"version":1,"session":{"status":"running","duration_ms":60000,"remaining_ms":32000,"anchor_utc_ms":12000}}"#;
    let data = super::store::decode(old).unwrap();
    assert_eq!(data.version, 2);
    assert!(data.task.is_none());
    assert_eq!(
        data.session,
        Session::Running {
            duration_ms: 60000,
            remaining_ms: 32000,
            anchor_utc_ms: 12000
        }
    );
    for bad in [
        br#"{"version":1,"task":null,"session":{"status":"idle"}}"#.as_slice(),
        br#"{"version":2,"session":{"status":"idle"}}"#,
        br#"{"version":3,"session":{"status":"idle"},"task":null}"#,
    ] {
        assert!(super::store::decode(bad).is_err());
    }
    let mut bytes = serde_json::to_vec(&data).unwrap();
    bytes.resize(4096, b' ');
    assert!(super::store::decode(&bytes).is_ok());
    bytes.push(b' ');
    assert!(super::store::decode(&bytes).is_err());
}
