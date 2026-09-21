use super::*;
use crate::reminders::store::Store;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
fn time(ms: i64) -> Time {
    Time {
        utc_ms: ms,
        local_minute: 600,
        monotonic_ms: ms as u64,
    }
}
fn enabled() -> Settings {
    let mut s = Settings {
        active_hours: ActiveHours::AllDay,
        ..Settings::default()
    };
    for item in &mut s.items {
        item.enabled = true;
        item.interval_minutes = 1;
    }
    s
}
struct MemoryStore {
    data: Data,
    writes: Arc<AtomicUsize>,
    fail: Arc<AtomicUsize>,
}
impl Storage for MemoryStore {
    fn load(&mut self) -> (Data, Persistence) {
        (
            self.data.clone(),
            Persistence::new(SaveStatus::Default, None),
        )
    }
    fn save(&mut self, data: &Data) -> Persistence {
        self.writes.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) > 0 {
            return Persistence::new(SaveStatus::Unsaved, Some("writeFailed"));
        }
        self.data = data.clone();
        Persistence::new(SaveStatus::Saved, None)
    }
}
fn core() -> (Core<MemoryStore>, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let writes = Arc::new(AtomicUsize::new(0));
    let fail = Arc::new(AtomicUsize::new(0));
    (
        Core::load(MemoryStore {
            data: Data::default(),
            writes: writes.clone(),
            fail: fail.clone(),
        }),
        writes,
        fail,
    )
}
#[test]
fn saves_only_actual_data_changes_and_noop_retry_recovers_unsaved() {
    let (mut c, writes, fail) = core();
    c.pump(None, Ok(time(0))).unwrap();
    assert_eq!(writes.load(Ordering::SeqCst), 0);
    fail.store(1, Ordering::SeqCst);
    let result = c
        .pump(
            Some(Command::UpdateSettings {
                settings: enabled(),
            }),
            Ok(time(0)),
        )
        .unwrap()
        .unwrap();
    assert_eq!(result.snapshot.persistence.status, SaveStatus::Unsaved);
    assert!(result.snapshot.settings.items[0].enabled);
    assert_eq!(c.store.data, Data::default());
    let rev = result.snapshot.revision;
    assert!(c.pump(None, Ok(time(1))).unwrap().is_none());
    assert_eq!(writes.load(Ordering::SeqCst), 1);
    assert_eq!(c.snapshot.revision, rev);
    fail.store(0, Ordering::SeqCst);
    c.pump(
        Some(Command::UpdateSettings {
            settings: enabled(),
        }),
        Ok(time(2)),
    )
    .unwrap();
    assert_eq!(c.snapshot.persistence.status, SaveStatus::Saved);
    assert_eq!(writes.load(Ordering::SeqCst), 2);
    c.pump(None, Ok(time(MINUTE))).unwrap();
    let writes_before = writes.load(Ordering::SeqCst);
    let id = c.snapshot.presentation.as_ref().unwrap().id;
    c.pump(
        Some(Command::Dismiss {
            presentation_id: id,
        }),
        Ok(time(MINUTE + 1)),
    )
    .unwrap();
    assert_eq!(writes.load(Ordering::SeqCst), writes_before);
    assert!(c.pump(None, Ok(time(MINUTE + 2))).unwrap().is_none());
}
#[test]
fn response_event_only_on_applied_action_and_invalid_input_has_no_effect() {
    let (mut c, _, _) = core();
    let before = c.snapshot.clone();
    let mut invalid = enabled();
    invalid.items[0].interval_minutes = 0;
    assert!(c
        .pump(
            Some(Command::UpdateSettings { settings: invalid }),
            Ok(time(0))
        )
        .is_err());
    assert_eq!(c.snapshot, before);
    c.pump(
        Some(Command::UpdateSettings {
            settings: enabled(),
        }),
        Ok(time(0)),
    )
    .unwrap();
    c.pump(None, Ok(time(MINUTE))).unwrap();
    assert_eq!(
        c.pump(Some(Command::Complete { id: Id::Water }), Ok(time(MINUTE)))
            .unwrap()
            .unwrap()
            .response,
        Some(Response::Complete { id: Id::Water })
    );
    assert!(c
        .pump(
            Some(Command::Complete { id: Id::Water }),
            Ok(time(MINUTE + 1))
        )
        .unwrap()
        .is_none());
    assert_eq!(
        c.pump(Some(Command::SnoozeAll {}), Ok(time(MINUTE + 1)))
            .unwrap()
            .unwrap()
            .response,
        Some(Response::SnoozeAll)
    );
}
#[test]
fn invalid_clock_report_is_stable_and_recovers_without_mutating_progress() {
    let (mut c, _, _) = core();
    let data = c.engine.data.clone();
    let change = c
        .pump(None, Err(Error::new("clockUnavailable")))
        .unwrap()
        .unwrap();
    assert_eq!(change.snapshot.runtime_error, Some("clockUnavailable"));
    assert!(c.pump(None, Err(Error::new("clockUnavailable"))).is_err());
    assert_eq!(c.engine.data, data);
    c.pump(None, Ok(time(0))).unwrap();
    assert_eq!(c.snapshot.runtime_error, None);
    c.snapshot.revision = MAX_SAFE;
    assert_eq!(
        c.pump(None, Ok(time(0))).err().unwrap().code,
        "sequenceExhausted"
    );
    assert_eq!(c.snapshot.revision, MAX_SAFE);
}
struct BlockingStore {
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
    saved: Arc<AtomicUsize>,
}
impl Storage for BlockingStore {
    fn load(&mut self) -> (Data, Persistence) {
        (Data::default(), Persistence::new(SaveStatus::Default, None))
    }
    fn save(&mut self, _: &Data) -> Persistence {
        self.entered.send(()).unwrap();
        self.release.recv().unwrap();
        self.saved.fetch_add(1, Ordering::SeqCst);
        Persistence::new(SaveStatus::Saved, None)
    }
}
#[test]
fn stopping_during_io_keeps_snapshot_available_cancels_queue_and_no_late_notification() {
    let (entered, blocking) = mpsc::channel();
    let (release, hold) = mpsc::channel();
    let saved = Arc::new(AtomicUsize::new(0));
    let (events, received) = mpsc::channel();
    let service = Service::start(
        BlockingStore {
            entered,
            release: hold,
            saved: saved.clone(),
        },
        || Ok(time(0)),
        move |c| {
            let _ = events.send(c.snapshot);
        },
    )
    .unwrap();
    received.recv_timeout(Duration::from_secs(5)).unwrap();
    let first = service
        .command(Command::UpdateSettings {
            settings: enabled(),
        })
        .unwrap();
    blocking.recv_timeout(Duration::from_secs(5)).unwrap();
    let queued = service
        .command(Command::SetPaused { paused: true })
        .unwrap();
    assert_eq!(service.snapshot().persistence.status, SaveStatus::Default);
    service.stop();
    assert!(service.snapshot().stopped);
    assert_eq!(queued.recv().unwrap().unwrap_err().code, "stopped");
    assert!(service.command(Command::ShowPending {}).is_err());
    release.send(()).unwrap();
    assert_eq!(
        first
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap_err()
            .code,
        "stopped"
    );
    assert_eq!(saved.load(Ordering::SeqCst), 1);
    assert!(received.try_recv().is_err());
}
#[test]
fn worker_serial_commands_use_injected_clock_and_snapshot_revision_order() {
    let (core, writes, _) = core();
    let now = Arc::new(AtomicU64::new(0));
    let worker_now = now.clone();
    let (events, received) = mpsc::channel();
    let service = Service::start(
        core.store,
        move || Ok(time(worker_now.load(Ordering::SeqCst) as i64)),
        move |c| {
            let _ = events.send(c.snapshot);
        },
    )
    .unwrap();
    received.recv_timeout(Duration::from_secs(5)).unwrap();
    let enabled = service
        .command(Command::UpdateSettings {
            settings: enabled(),
        })
        .unwrap()
        .recv()
        .unwrap()
        .unwrap();
    now.store(MINUTE as u64, Ordering::SeqCst);
    let a = service.command(Command::ShowPending {}).unwrap();
    let b = service
        .command(Command::Complete { id: Id::Water })
        .unwrap();
    let c = service
        .command(Command::SetPaused { paused: true })
        .unwrap();
    let a = a.recv().unwrap().unwrap();
    let b = b.recv().unwrap().unwrap();
    let c = c.recv().unwrap().unwrap();
    assert!(enabled.revision < a.revision && a.revision < b.revision && b.revision < c.revision);
    assert!(a.progress[0].pending);
    assert!(!b.progress[0].pending);
    assert!(c.paused);
    assert_eq!(writes.load(Ordering::SeqCst), 4);
    service.stop();
}
#[test]
fn real_store_startup_protection_blocks_auto_and_settings() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "reminder-core-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("reminders.json");
    std::fs::write(&path, b"broken").unwrap();
    let mut c = Core::load(Store::new(Some(path.clone())));
    assert!(c.snapshot.persistence.protected());
    assert!(c
        .pump(
            Some(Command::UpdateSettings {
                settings: enabled()
            }),
            Ok(time(0))
        )
        .is_err());
    c.pump(Some(Command::SetPaused { paused: true }), Ok(time(0)))
        .unwrap();
    c.pump(Some(Command::ShowPending {}), Ok(time(MINUTE)))
        .unwrap();
    assert_eq!(c.snapshot.presentation.as_ref().unwrap().mode, Mode::Manual);
    assert_eq!(std::fs::read(path).unwrap(), b"broken");
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn arithmetic_limit_is_visible_without_mutating_due_state() {
    let (mut c, _, _) = core();
    let before = c.engine.data.clone();
    let change = c
        .pump(
            Some(Command::UpdateSettings {
                settings: enabled(),
            }),
            Ok(Time {
                utc_ms: MAX_UTC,
                ..time(0)
            }),
        )
        .unwrap()
        .unwrap();
    assert_eq!(change.snapshot.runtime_error, Some("invalidTime"));
    assert_eq!(c.engine.data, before);
}
