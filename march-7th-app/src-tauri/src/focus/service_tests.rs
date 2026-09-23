use super::*;
use crate::focus::model::{Session, MIN_DURATION_MS};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
struct Memory {
    bytes: Vec<u8>,
    fail: Arc<AtomicBool>,
}
impl Storage for Memory {
    fn load(&mut self) -> Result<Vec<u8>, Error> {
        Ok(self.bytes.clone())
    }
    fn save(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(Error {
                code: "writeFailed",
            });
        }
        self.bytes = bytes.to_vec();
        Ok(())
    }
}
fn time(ms: u64) -> Time {
    Time {
        utc_ms: ms as i64,
        monotonic_ms: ms,
    }
}
fn memory(data: Data) -> (Memory, Arc<AtomicBool>) {
    let fail = Arc::new(AtomicBool::new(false));
    (
        Memory {
            bytes: serde_json::to_vec(&data).unwrap(),
            fail: fail.clone(),
        },
        fail,
    )
}
#[test]
fn failed_write_keeps_engine_snapshot_and_export_bytes() {
    let (store, fail) = memory(Data::default());
    let mut core = Core::load(store);
    core.pump(Command::Tick, time(0)).unwrap();
    let before = core.snapshot.clone();
    let bytes = core.bytes.clone();
    fail.store(true, Ordering::SeqCst);
    assert_eq!(
        core.pump(
            Command::Start {
                duration_ms: MIN_DURATION_MS
            },
            time(0)
        )
        .unwrap_err()
        .code,
        "writeFailed"
    );
    assert_eq!(core.snapshot, before);
    assert_eq!(core.bytes, bytes);
    fail.store(false, Ordering::SeqCst);
    core.pump(
        Command::Start {
            duration_ms: MIN_DURATION_MS,
        },
        time(0),
    )
    .unwrap();
    assert!(matches!(
        core.snapshot.data.unwrap().session,
        Session::Running { .. }
    ));
}
#[test]
fn restore_is_durable_without_live_completion_and_repeated_pause_is_idempotent() {
    let data = Data {
        version: 1,
        session: Session::Running {
            duration_ms: MIN_DURATION_MS,
            remaining_ms: MIN_DURATION_MS,
            anchor_utc_ms: 0,
        },
    };
    let (store, _) = memory(data);
    let mut core = Core::load(store);
    core.pump(Command::Tick, time(MIN_DURATION_MS)).unwrap();
    assert!(matches!(
        core.snapshot.data.as_ref().unwrap().session,
        Session::Finished { .. }
    ));
    assert_eq!(
        serde_json::from_slice::<Data>(&core.bytes).unwrap(),
        core.snapshot.data.clone().unwrap()
    );
    core.pump(
        Command::Start {
            duration_ms: MIN_DURATION_MS,
        },
        time(MIN_DURATION_MS),
    )
    .unwrap();
    core.pump(Command::Pause, time(MIN_DURATION_MS + 5))
        .unwrap();
    let paused = core.snapshot.clone();
    assert!(!core
        .pump(Command::Pause, time(MIN_DURATION_MS + 10))
        .unwrap());
    assert_eq!(core.snapshot, paused);
}

struct BlockedStore {
    bytes: Vec<u8>,
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}
impl Storage for BlockedStore {
    fn load(&mut self) -> Result<Vec<u8>, Error> {
        Ok(self.bytes.clone())
    }
    fn save(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.entered.send(()).unwrap();
        self.release.recv_timeout(Duration::from_secs(5)).unwrap();
        self.bytes = bytes.to_vec();
        Ok(())
    }
}
#[test]
fn save_in_flight_is_invisible_to_snapshot_and_export_then_publishes_together() {
    let original = serde_json::to_vec_pretty(&Data::default()).unwrap();
    let (entered_send, entered) = mpsc::channel();
    let (release, release_receive) = mpsc::channel();
    let (events, receive) = mpsc::channel();
    let service = Service::start(
        BlockedStore {
            bytes: original.clone(),
            entered: entered_send,
            release: release_receive,
        },
        || Ok(time(0)),
        move |s| {
            events.send(s).unwrap();
        },
    )
    .unwrap();
    receive.recv_timeout(Duration::from_secs(5)).unwrap();
    let before = service.snapshot();
    let response = service
        .command(Command::Start {
            duration_ms: MIN_DURATION_MS,
        })
        .unwrap();
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(service.snapshot(), before);
    assert_eq!(service.export_bytes().unwrap(), original);
    assert!(response.try_recv().is_err());
    assert!(receive.try_recv().is_err());
    release.send(()).unwrap();
    let committed = response
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    assert_eq!(service.snapshot(), committed);
    assert_eq!(
        serde_json::from_slice::<Data>(&service.export_bytes().unwrap()).unwrap(),
        committed.data.unwrap()
    );
}
#[test]
fn stop_during_save_rejects_inflight_and_queued_replies_without_late_publication() {
    let (entered_send, entered) = mpsc::channel();
    let (release, release_receive) = mpsc::channel();
    let (events, receive) = mpsc::channel();
    let service = Service::start(
        BlockedStore {
            bytes: serde_json::to_vec(&Data::default()).unwrap(),
            entered: entered_send,
            release: release_receive,
        },
        || Ok(time(0)),
        move |s| {
            let _ = events.send(s);
        },
    )
    .unwrap();
    receive.recv_timeout(Duration::from_secs(5)).unwrap();
    let first = service
        .command(Command::Start {
            duration_ms: MIN_DURATION_MS,
        })
        .unwrap();
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    let second = service.command(Command::Pause).unwrap();
    service.stop();
    let stopped = service.snapshot();
    assert_eq!(
        second
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap_err()
            .code,
        "stopped"
    );
    assert_eq!(service.export_bytes().unwrap_err().code, "stopped");
    release.send(()).unwrap();
    assert_eq!(
        first
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap_err()
            .code,
        "stopped"
    );
    assert_eq!(service.snapshot(), stopped);
    assert!(receive.try_recv().is_err());
    assert_eq!(
        service.command(Command::Resume).unwrap_err().code,
        "stopped"
    );
}
#[test]
fn failed_restore_keeps_the_original_session_until_a_successful_retry() {
    let data = Data {
        version: 1,
        session: Session::Running {
            duration_ms: MIN_DURATION_MS,
            remaining_ms: MIN_DURATION_MS,
            anchor_utc_ms: 0,
        },
    };
    let (store, fail) = memory(data.clone());
    let mut core = Core::load(store);
    let bytes = core.bytes.clone();
    fail.store(true, Ordering::SeqCst);
    assert_eq!(
        core.pump(Command::Tick, time(MIN_DURATION_MS))
            .unwrap_err()
            .code,
        "writeFailed"
    );
    assert_eq!(core.snapshot.data, Some(data));
    assert_eq!(core.bytes, bytes);
    assert!(core.engine.is_none());
    fail.store(false, Ordering::SeqCst);
    core.pump(Command::Tick, time(MIN_DURATION_MS)).unwrap();
    assert!(matches!(
        core.snapshot.data.unwrap().session,
        Session::Finished { .. }
    ));
}
#[test]
fn command_sequence_persists_every_transition_and_restart_preserves_dismissal() {
    let (store, _) = memory(Data::default());
    let mut core = Core::load(store);
    for (command, ms) in [
        (Command::Tick, 0),
        (
            Command::Start {
                duration_ms: MIN_DURATION_MS,
            },
            0,
        ),
        (Command::Pause, 1000),
        (Command::Resume, 2000),
        (Command::Tick, 61000),
        (Command::DismissFeedback, 61000),
    ] {
        core.pump(command, time(ms)).unwrap();
        assert_eq!(
            decode(&core.bytes).unwrap(),
            core.snapshot.data.clone().unwrap()
        );
    }
    let saved = core.snapshot.data.clone().unwrap();
    let mut restarted = Core::load(core.store);
    restarted.pump(Command::Tick, time(100000)).unwrap();
    assert_eq!(restarted.snapshot.data, Some(saved));
    assert!(!restarted
        .pump(Command::DismissFeedback, time(100000))
        .unwrap());
    restarted
        .pump(
            Command::Start {
                duration_ms: MIN_DURATION_MS,
            },
            time(100000),
        )
        .unwrap();
    assert_eq!(
        restarted
            .pump(
                Command::Start {
                    duration_ms: MIN_DURATION_MS
                },
                time(100000)
            )
            .unwrap_err()
            .code,
        "alreadyActive"
    );
    restarted.pump(Command::Abandon, time(100000)).unwrap();
    assert!(!restarted.pump(Command::Abandon, time(100000)).unwrap());
    restarted
        .pump(
            Command::Start {
                duration_ms: MIN_DURATION_MS,
            },
            time(100000),
        )
        .unwrap();
    restarted.pump(Command::EndEarly, time(100000)).unwrap();
    assert!(!restarted.pump(Command::EndEarly, time(100000)).unwrap());
}
#[test]
fn actual_v2_service_commit_roundtrips_four_file_backup_and_import_activates_only_on_restart() {
    use crate::data_directory::{acquire, backup_codec, DataFile, ImportFiles};
    use crate::focus::store::{tests::Temp, Store};
    let temp = Temp::new();
    let directory = acquire(Some(temp.0.clone())).unwrap();
    assert!(acquire(Some(temp.0.clone())).is_err());
    let path = directory.path(DataFile::Focus).unwrap();
    let (send, receive) = mpsc::channel();
    let service = Service::start(
        Store::new(Some(path.clone())),
        || Ok(time(0)),
        move |s| {
            let _ = send.send(s);
        },
    )
    .unwrap();
    receive.recv_timeout(Duration::from_secs(5)).unwrap();
    let saved = service
        .command(Command::Start {
            duration_ms: MIN_DURATION_MS,
        })
        .unwrap()
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    let focus = service.export_bytes().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), focus);
    assert_eq!(decode(&focus).unwrap(), saved.data.unwrap());
    let files = ImportFiles {
        desktop: None,
        characters: std::fs::read(directory.path(DataFile::Characters).unwrap()).unwrap(),
        reminders: std::fs::read(directory.path(DataFile::Reminders).unwrap()).unwrap(),
        focus: focus.clone(),
    };
    let decoded =
        backup_codec::decode(&backup_codec::encode(files.clone(), 1000).unwrap()).unwrap();
    assert_eq!(decoded.files.focus, focus);
    assert_eq!(decoded.files.characters, files.characters);
    assert_eq!(decoded.files.reminders, files.reminders);
    assert_eq!(decoded.files.desktop, files.desktop);
    let mut imported = decoded.files;
    imported.focus = serde_json::to_vec(&Data::default()).unwrap();
    directory.prepare_import(imported).unwrap();
    assert_eq!(service.export_bytes().unwrap(), focus);
    assert_eq!(std::fs::read(&path).unwrap(), focus);
    service.stop();
    drop(service);
    drop(directory);
    let restarted = acquire(Some(temp.0.clone())).unwrap();
    assert_eq!(
        decode(&std::fs::read(restarted.path(DataFile::Focus).unwrap()).unwrap()).unwrap(),
        Data::default()
    );
}

#[test]
fn recovering_initial_clock_failure_advances_revision_even_when_data_is_unchanged() {
    let (store, _) = memory(Data::default());
    let mut core = Core::load(store);
    core.snapshot.error = Some("invalidTime");
    let old = core.snapshot.clone();
    assert!(core.pump(Command::Tick, time(0)).unwrap());
    assert_eq!(core.snapshot.error, None);
    assert!(core.snapshot.revision > old.revision);
}

#[test]
fn failed_pause_does_not_advance_hidden_clock_and_completion_retries_once() {
    let (store, fail) = memory(Data::default());
    let mut core = Core::load(store);
    core.pump(
        Command::Start {
            duration_ms: MIN_DURATION_MS,
        },
        time(0),
    )
    .unwrap();
    let old = core.snapshot.clone();
    fail.store(true, Ordering::SeqCst);
    assert!(core.pump(Command::Pause, time(1000)).is_err());
    assert_eq!(core.snapshot, old);
    fail.store(false, Ordering::SeqCst);
    core.pump(Command::Tick, time(2000)).unwrap();
    assert!(matches!(
        core.snapshot.data.as_ref().unwrap().session,
        Session::Running {
            remaining_ms: 58000,
            ..
        }
    ));
    let running = core.snapshot.clone();
    fail.store(true, Ordering::SeqCst);
    assert!(core.pump(Command::Tick, time(MIN_DURATION_MS)).is_err());
    assert_eq!(core.snapshot, running);
    fail.store(false, Ordering::SeqCst);
    core.pump(Command::Tick, time(MIN_DURATION_MS + 1000))
        .unwrap();
    assert!(matches!(
        core.snapshot.data.as_ref().unwrap().session,
        Session::Finished { .. }
    ));
    assert!(!core
        .pump(Command::Tick, time(MIN_DURATION_MS + 2000))
        .unwrap());
}
#[test]
fn stopping_during_initial_read_does_not_sample_clock_or_publish_or_save() {
    struct Loading {
        entered: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    }
    impl Storage for Loading {
        fn load(&mut self) -> Result<Vec<u8>, Error> {
            self.entered.send(()).unwrap();
            self.release.recv_timeout(Duration::from_secs(5)).unwrap();
            Ok(serde_json::to_vec(&Data::default()).unwrap())
        }
        fn save(&mut self, _: &[u8]) -> Result<(), Error> {
            panic!("save after stop");
        }
    }
    let (entered_send, entered) = mpsc::channel();
    let (release, release_receive) = mpsc::channel();
    let (events, receive) = mpsc::channel();
    let service = Service::start(
        Loading {
            entered: entered_send,
            release: release_receive,
        },
        || panic!("clock after stop"),
        move |snapshot| {
            let _ = events.send(snapshot);
        },
    )
    .unwrap();
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    let pending = service
        .command(Command::Start {
            duration_ms: MIN_DURATION_MS,
        })
        .unwrap();
    service.stop();
    release.send(()).unwrap();
    assert_eq!(
        pending
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap_err()
            .code,
        "stopped"
    );
    assert_eq!(
        receive.recv_timeout(Duration::from_secs(5)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    );
    assert!(service.snapshot().stopped);
}

#[test]
fn completion_signal_only_follows_a_successful_live_commit_never_restore_or_retry() {
    let running = Data {
        version: 1,
        session: Session::Running {
            duration_ms: MIN_DURATION_MS,
            remaining_ms: MIN_DURATION_MS,
            anchor_utc_ms: 0,
        },
    };
    let (store, _) = memory(running);
    let mut restored = Core::load(store);
    restored.pump(Command::Tick, time(MIN_DURATION_MS)).unwrap();
    assert!(!restored.completed_now);
    let (store, fail) = memory(Data::default());
    let mut core = Core::load(store);
    core.pump(
        Command::Start {
            duration_ms: MIN_DURATION_MS,
        },
        time(0),
    )
    .unwrap();
    fail.store(true, Ordering::SeqCst);
    assert!(core.pump(Command::Tick, time(MIN_DURATION_MS)).is_err());
    assert!(!core.completed_now);
    fail.store(false, Ordering::SeqCst);
    core.pump(Command::Tick, time(MIN_DURATION_MS)).unwrap();
    assert!(core.completed_now);
    core.pump(Command::Tick, time(MIN_DURATION_MS + 1)).unwrap();
    assert!(!core.completed_now);
}

#[test]
fn live_completion_event_is_committed_once_and_suppressed_if_stop_wins_during_io() {
    use std::sync::atomic::AtomicU64;
    for stop in [false, true] {
        let current = Arc::new(AtomicU64::new(0));
        let clock = current.clone();
        let (entered_send, entered) = mpsc::channel();
        let (release, release_receive) = mpsc::channel();
        let (events, receive) = mpsc::channel();
        let running = Data {
            version: 1,
            session: Session::Running {
                duration_ms: MIN_DURATION_MS,
                remaining_ms: MIN_DURATION_MS,
                anchor_utc_ms: 0,
            },
        };
        let service = Service::start(
            BlockedStore {
                bytes: serde_json::to_vec(&running).unwrap(),
                entered: entered_send,
                release: release_receive,
            },
            move || Ok(time(clock.load(Ordering::SeqCst))),
            move |s| {
                let _ = events.send(s);
            },
        )
        .unwrap();
        assert!(
            !receive
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .completed_now
        );
        current.store(MIN_DURATION_MS, Ordering::SeqCst);
        let reply = service.command(Command::Tick).unwrap();
        entered.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(service.snapshot().data, Some(running));
        assert!(receive.try_recv().is_err());
        if stop {
            service.stop();
        }
        release.send(()).unwrap();
        let result = reply.recv_timeout(Duration::from_secs(5)).unwrap();
        if stop {
            assert_eq!(result.unwrap_err().code, "stopped");
            assert!(receive.try_recv().is_err());
        } else {
            let snapshot = result.unwrap();
            let event = receive.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(event.completed_now);
            assert_eq!(event.snapshot, snapshot);
            assert_eq!(
                decode(&service.export_bytes().unwrap()).unwrap(),
                snapshot.data.unwrap()
            );
            service
                .command(Command::Tick)
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap();
            assert!(receive.try_recv().is_err());
        }
    }
}
#[test]
fn protected_initial_file_never_publishes_an_editable_default() {
    for bytes in [b"broken".to_vec(), br#"{"version":2}"#.to_vec()] {
        let (events, receive) = mpsc::channel();
        let service = Service::start(
            Memory {
                bytes,
                fail: Arc::new(AtomicBool::new(false)),
            },
            || Ok(time(0)),
            move |s| {
                let _ = events.send(s);
            },
        )
        .unwrap();
        let initial = receive
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .snapshot;
        assert!(initial.data.is_none());
        assert!(initial.error.is_some());
        assert!(service.export_bytes().is_err());
        assert!(service
            .command(Command::Start {
                duration_ms: MIN_DURATION_MS
            })
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .is_err());
        assert_eq!(service.snapshot(), initial);
        assert!(receive.try_recv().is_err());
    }
}

#[test]
fn automatic_save_failure_reports_an_error_without_publishing_uncommitted_progress() {
    use std::sync::atomic::AtomicU64;
    let current = Arc::new(AtomicU64::new(0));
    let clock = current.clone();
    let samples = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let sampled = samples.clone();
    let (store, fail) = memory(Data {
        version: 1,
        session: Session::Running {
            duration_ms: MIN_DURATION_MS,
            remaining_ms: 100,
            anchor_utc_ms: 0,
        },
    });
    let (events, receive) = mpsc::channel();
    let service = Service::start(
        store,
        move || {
            sampled.fetch_add(1, Ordering::SeqCst);
            Ok(time(clock.load(Ordering::SeqCst)))
        },
        move |change| {
            let _ = events.send(change);
        },
    )
    .unwrap();
    let committed = receive
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .snapshot;
    let bytes = service.export_bytes().unwrap();
    fail.store(true, Ordering::SeqCst);
    current.store(MIN_DURATION_MS, Ordering::SeqCst);
    let failure = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(failure.error, Some("writeFailed"));
    assert_eq!(service.current().error, Some("writeFailed"));
    assert_eq!(service.current().error, Some("writeFailed")); // reads do not consume diagnostics
    assert!(!failure.completed_now);
    assert_eq!(failure.snapshot, committed);
    assert_eq!(service.snapshot(), committed);
    assert_eq!(service.export_bytes().unwrap(), bytes);
    let count = samples.load(Ordering::SeqCst);
    assert_eq!(
        receive.recv_timeout(Duration::from_millis(250)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );
    assert_eq!(
        samples.load(Ordering::SeqCst),
        count,
        "failed near-deadline writes must not spin"
    );
    fail.store(false, Ordering::SeqCst);
    service
        .command(Command::Tick)
        .unwrap()
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    let recovery = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(recovery.error, None);
    assert_eq!(service.current().error, None);
    assert!(!service.current().completed_now);
    assert!(recovery.completed_now);
    service
        .command(Command::Tick)
        .unwrap()
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    assert!(receive.try_recv().is_err());
}

#[test]
fn stopping_during_background_failed_save_suppresses_late_error_notification() {
    use std::sync::atomic::AtomicU64;
    struct BlockedFailure(mpsc::Sender<()>, mpsc::Receiver<()>, Vec<u8>);
    impl Storage for BlockedFailure {
        fn load(&mut self) -> Result<Vec<u8>, Error> {
            Ok(self.2.clone())
        }
        fn save(&mut self, _: &[u8]) -> Result<(), Error> {
            self.0.send(()).unwrap();
            self.1.recv_timeout(Duration::from_secs(5)).unwrap();
            Err(Error {
                code: "writeFailed",
            })
        }
    }
    let (entered_send, entered) = mpsc::channel();
    let (release, release_receive) = mpsc::channel();
    let (events, receive) = mpsc::channel();
    let current = Arc::new(AtomicU64::new(0));
    let clock = current.clone();
    let running = Data {
        version: 1,
        session: Session::Running {
            duration_ms: MIN_DURATION_MS,
            remaining_ms: 100,
            anchor_utc_ms: 0,
        },
    };
    let service = Service::start(
        BlockedFailure(
            entered_send,
            release_receive,
            serde_json::to_vec(&running).unwrap(),
        ),
        move || Ok(time(clock.load(Ordering::SeqCst))),
        move |change| {
            let _ = events.send(change);
        },
    )
    .unwrap();
    receive.recv_timeout(Duration::from_secs(5)).unwrap();
    current.store(MIN_DURATION_MS, Ordering::SeqCst);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    service.stop();
    release.send(()).unwrap();
    assert_eq!(
        receive.recv_timeout(Duration::from_secs(5)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    );
}

#[test]
fn background_checkpoint_does_not_save_or_publish_each_second_but_pause_is_immediate() {
    struct Counted {
        inner: Memory,
        saves: Arc<std::sync::atomic::AtomicUsize>,
    }
    impl Storage for Counted {
        fn load(&mut self) -> Result<Vec<u8>, Error> {
            self.inner.load()
        }
        fn save(&mut self, bytes: &[u8]) -> Result<(), Error> {
            self.saves.fetch_add(1, Ordering::SeqCst);
            self.inner.save(bytes)
        }
    }
    let (inner, _) = memory(Data::default());
    let saves = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let store = Counted {
        inner,
        saves: saves.clone(),
    };
    let (send, events) = mpsc::channel();
    let started = std::time::Instant::now();
    let service = Service::start(
        store,
        move || Ok(time(started.elapsed().as_millis() as u64)),
        move |event| {
            let _ = send.send(event);
        },
    )
    .unwrap();
    events.recv_timeout(Duration::from_secs(5)).unwrap();
    let running = service
        .command(Command::Start {
            duration_ms: 25 * 60_000,
        })
        .unwrap()
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    events.recv_timeout(Duration::from_secs(5)).unwrap();
    let bytes = service.export_bytes().unwrap();
    assert_eq!(
        events.recv_timeout(Duration::from_millis(1200)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );
    assert_eq!(saves.load(Ordering::SeqCst), 1); // only Start committed
    assert_eq!(service.snapshot(), running);
    assert_eq!(service.export_bytes().unwrap(), bytes);
    let paused = service
        .command(Command::Pause)
        .unwrap()
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    assert_eq!(saves.load(Ordering::SeqCst), 2); // Pause commits immediately
    assert!(matches!(
        paused.data.as_ref().unwrap().session,
        Session::Paused { .. }
    ));
    assert_eq!(
        decode(&service.export_bytes().unwrap()).unwrap(),
        paused.data.unwrap()
    );
}

#[test]
fn checkpoint_wait_is_bounded_by_one_minute_and_remaining_time_with_exact_completion() {
    let (store, _) = memory(Data::default());
    let mut core = Core::load(store);
    assert_eq!(core.checkpoint_delay(), None);
    core.pump(
        Command::Start {
            duration_ms: 25 * 60_000,
        },
        time(0),
    )
    .unwrap();
    let mut now = 0;
    let mut checkpoints = 0;
    let mut completions = 0;
    while let Some(delay) = core.checkpoint_delay() {
        assert!(delay <= Duration::from_secs(60));
        now += delay.as_millis() as u64;
        core.pump(Command::Tick, time(now)).unwrap();
        checkpoints += 1;
        completions += usize::from(core.completed_now);
        assert_eq!(
            decode(&core.bytes).unwrap(),
            core.snapshot.data.clone().unwrap()
        );
    }
    assert_eq!(now, 25 * 60_000);
    assert_eq!(checkpoints, 25);
    assert_eq!(completions, 1);
    core.pump(
        Command::Start {
            duration_ms: 65_000,
        },
        time(now),
    )
    .unwrap();
    core.pump(Command::Tick, time(now + 60_000)).unwrap();
    assert_eq!(core.checkpoint_delay(), Some(Duration::from_secs(5)));
    core.pump(Command::Pause, time(now + 61_000)).unwrap();
    assert_eq!(core.checkpoint_delay(), None);
    core.pump(Command::Resume, time(now + 62_000)).unwrap();
    assert_eq!(core.checkpoint_delay(), Some(Duration::from_secs(4)));
}

#[test]
fn short_remaining_deadline_is_not_delayed_by_invalid_commands_or_spurious_wakes() {
    let data = Data {
        version: 1,
        session: Session::Running {
            duration_ms: MIN_DURATION_MS,
            remaining_ms: 150,
            anchor_utc_ms: 0,
        },
    };
    let (store, _) = memory(data);
    let (send, events) = mpsc::channel();
    let started = Instant::now();
    let service = Service::start(
        store,
        move || Ok(time(started.elapsed().as_millis() as u64)),
        move |event| {
            let _ = send.send(event);
        },
    )
    .unwrap();
    let initial = events.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(!initial.completed_now);
    while started.elapsed() < Duration::from_millis(200) {
        service.shared.wake.notify_one();
        assert_eq!(
            service
                .command(Command::Start { duration_ms: 1 })
                .unwrap()
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .unwrap_err()
                .code,
            "invalidDuration"
        );
    }
    let completion = events.recv_timeout(Duration::from_millis(500)).unwrap();
    assert!(completion.completed_now);
    assert!(matches!(
        completion.snapshot.data.unwrap().session,
        Session::Finished { .. }
    ));
    assert!(events.try_recv().is_err());
}
#[test]
fn checkpoint_gaps_restore_elapsed_time_and_detect_untrusted_sleep_without_replay() {
    let (store, _) = memory(Data::default());
    let mut core = Core::load(store);
    core.pump(
        Command::Start {
            duration_ms: 150_000,
        },
        time(0),
    )
    .unwrap();
    core.pump(Command::Tick, time(60_000)).unwrap();
    let mut restored = Core::load(core.store);
    restored.pump(Command::Tick, time(100_000)).unwrap();
    assert!(matches!(
        restored.snapshot.data.as_ref().unwrap().session,
        Session::Running {
            remaining_ms: 50_000,
            ..
        }
    ));
    assert_eq!(restored.checkpoint_delay(), Some(Duration::from_secs(50)));
    assert!(!restored.completed_now);
    restored
        .pump(
            Command::Tick,
            Time {
                utc_ms: 200_000,
                monotonic_ms: 150_000,
            },
        )
        .unwrap();
    assert!(matches!(
        restored.snapshot.data.as_ref().unwrap().session,
        Session::Interrupted {
            remaining_ms: 50_000,
            ..
        }
    ));
    assert_eq!(restored.checkpoint_delay(), None);
    assert!(!restored.completed_now);
    restored.pump(Command::Resume, time(210_000)).unwrap();
    restored.pump(Command::Tick, time(300_000)).unwrap();
    assert!(restored.completed_now);
    assert!(!restored.pump(Command::Tick, time(310_000)).unwrap());
    assert!(!restored.completed_now);
}
