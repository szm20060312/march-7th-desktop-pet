use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "march7-characters-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn file(&self) -> PathBuf {
        self.0.join("character-preferences.json")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn catalog() -> Catalog {
    Catalog::builtin().unwrap()
}
#[test]
fn unavailable_directory_keeps_character_selection_session_only() {
    use crate::data_directory::{acquire, DataFile};
    let directory = acquire(None).unwrap();
    let service = Service::start(directory.path(DataFile::Characters), |_| {}).unwrap();
    assert_eq!(service.snapshot().persistence, Persistence::SessionOnly);
    let id = catalog().characters[1].id.clone();
    let selected = service.select(id.clone()).unwrap().recv().unwrap().unwrap();
    assert_eq!(selected.selected_character_id, id);
    assert_eq!(selected.persistence, Persistence::SessionOnly);
    service.stop();
}
#[test]
fn default_unknown_and_explicit_normalization() {
    let temp = Temp::new();
    let catalog = catalog();
    let (mut store, initial) = Store::load(Some(temp.file()), &catalog);
    assert_eq!(initial.selected_character_id, catalog.default_id);
    assert_eq!(initial.persistence, Persistence::Default);
    let raw = r#"{"version":1,"selectedCharacterId":"removed"}"#;
    fs::write(temp.file(), raw).unwrap();
    let (_, initial) = Store::load(Some(temp.file()), &catalog);
    assert_eq!(initial.persistence, Persistence::Fallback);
    assert_eq!(fs::read_to_string(temp.file()).unwrap(), raw);
    store.save(&catalog.default_id).unwrap();
    assert_eq!(
        Store::load(Some(temp.file()), &catalog).1.persistence,
        Persistence::Saved
    );
}
#[test]
fn corrupt_future_and_read_errors_are_session_only_without_content_leaks() {
    let temp = Temp::new();
    let catalog = catalog();
    for raw in [
        "private broken content",
        r#"{"version":99,"private":"secret"}"#,
        r#"{"version":1,"selectedCharacterId":"march-7th","future":"keep"}"#,
    ] {
        fs::write(temp.file(), raw).unwrap();
        let (mut store, initial) = Store::load(Some(temp.file()), &catalog);
        assert_eq!(initial.persistence, Persistence::SessionOnly);
        assert_eq!(
            store.save(&catalog.default_id).unwrap(),
            Persistence::SessionOnly
        );
        assert_eq!(fs::read_to_string(temp.file()).unwrap(), raw);
    }
    let (_, initial) = Store::load(Some(temp.0.clone()), &catalog);
    assert_eq!(initial.persistence, Persistence::SessionOnly);
}
#[test]
fn failed_backup_preserves_selection_and_retries() {
    let temp = Temp::new();
    let service = Service::start(Some(temp.file()), |_| {}).unwrap();
    let ids = catalog().characters;
    service
        .select(ids[0].id.clone())
        .unwrap()
        .recv()
        .unwrap()
        .unwrap();
    let previous = fs::read(temp.file()).unwrap();
    fs::create_dir(temp.file().with_extension("json.bak")).unwrap();
    assert!(service
        .select(ids[1].id.clone())
        .unwrap()
        .recv()
        .unwrap()
        .is_err());
    assert_eq!(service.snapshot().selected_character_id, ids[0].id);
    assert_eq!(service.snapshot().persistence, Persistence::SaveFailed);
    assert_eq!(fs::read(temp.file()).unwrap(), previous);
    fs::remove_dir(temp.file().with_extension("json.bak")).unwrap();
    let result = service
        .select(ids[1].id.clone())
        .unwrap()
        .recv()
        .unwrap()
        .unwrap();
    assert_eq!(result.selected_character_id, ids[1].id);
    service.stop();
}
#[test]
fn queued_selection_is_serial_unknown_is_rejected_and_stop_rejects_new_work() {
    let temp = Temp::new();
    let service = Service::start(Some(temp.file()), |_| {}).unwrap();
    let ids = catalog().characters;
    assert!(service.select("unknown".into()).is_err());
    let a = service.select(ids[1].id.clone()).unwrap();
    let b = service.select(ids[0].id.clone()).unwrap();
    let a = a.recv().unwrap().unwrap();
    let b = b.recv().unwrap().unwrap();
    assert!(a.revision < b.revision);
    assert_eq!(service.snapshot(), b);
    service.stop();
    assert!(service.select(ids[1].id.clone()).is_err());
}
#[test]
fn readonly_session_selection_is_effective_without_replacing_file() {
    let temp = Temp::new();
    let raw = r#"{"version":999}"#;
    fs::write(temp.file(), raw).unwrap();
    let service = Service::start(Some(temp.file()), |_| {}).unwrap();
    let id = catalog().characters[1].id.clone();
    let result = service.select(id.clone()).unwrap().recv().unwrap().unwrap();
    assert_eq!(result.selected_character_id, id);
    assert_eq!(result.persistence, Persistence::SessionOnly);
    assert_eq!(fs::read_to_string(temp.file()).unwrap(), raw);
    service.stop();
}

#[test]
fn shutdown_during_io_discards_queued_work_and_late_commit_without_holding_snapshot_lock() {
    use std::sync::mpsc;
    let catalog = catalog();
    let (_, snapshot) = Store::load(None, &catalog);
    let state = Arc::new(Mutex::new(State {
        snapshot: snapshot.clone(),
        stopped: false,
    }));
    let (sender, receiver) = mpsc::channel();
    let service = Service {
        catalog,
        state: state.clone(),
        sender,
    };
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (notifications_tx, notifications_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        run_worker(
            state,
            receiver,
            |_| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(Persistence::Saved)
            },
            |snapshot| {
                notifications_tx.send(snapshot).unwrap();
            },
        )
    });
    let first = service
        .select(service.catalog.characters[1].id.clone())
        .unwrap();
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    let second = service.select(service.catalog.default_id.clone()).unwrap();
    assert_eq!(service.snapshot(), snapshot); // IO has begun, but no provisional selection.
    service.stop(); // Must complete while IO is still blocked.
    release_tx.send(()).unwrap();
    assert!(first.recv().unwrap().is_err());
    assert!(second.recv().unwrap().is_err());
    worker.join().unwrap();
    assert_eq!(service.snapshot(), snapshot);
    assert!(notifications_rx.try_recv().is_err());
    assert!(entered_rx.try_recv().is_err()); // Queued selection never saved.
}
#[test]
fn external_future_config_turns_next_selection_into_session_only() {
    let temp = Temp::new();
    let service = Service::start(Some(temp.file()), |_| {}).unwrap();
    let raw = r#"{"version":900,"private":"retain"}"#;
    fs::write(temp.file(), raw).unwrap();
    let next = service
        .select(catalog().characters[1].id.clone())
        .unwrap()
        .recv()
        .unwrap()
        .unwrap();
    assert_eq!(next.persistence, Persistence::SessionOnly);
    assert_eq!(fs::read_to_string(temp.file()).unwrap(), raw);
    service.stop();
}
#[test]
fn native_snapshot_uses_frontend_contract_and_repeat_selection_has_retry_revision() {
    let temp = Temp::new();
    let service = Service::start(Some(temp.file()), |_| {}).unwrap();
    let id = catalog().default_id;
    let first = service.select(id.clone()).unwrap().recv().unwrap().unwrap();
    let second = service.select(id.clone()).unwrap().recv().unwrap().unwrap();
    assert!(second.revision > first.revision);
    let value = serde_json::to_value(&second).unwrap();
    assert_eq!(value["selectedCharacterId"], id);
    assert_eq!(value["persistence"], "saved");
    service.stop();
}
