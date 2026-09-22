#[path = "../src/data_directory.rs"]
mod data_directory;

use data_directory::{acquire, DataFile};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "march7-directory-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn ready_creates_only_the_fixed_lock_and_preserves_flat_paths() {
    let temp = Temp::new();
    let root = temp.0.join("config");
    let directory = acquire(Some(root.clone())).unwrap();
    assert_eq!(directory.root(), Some(root.as_path()));
    assert_eq!(directory.diagnostic(), None);
    assert_eq!(
        directory.path(DataFile::Desktop),
        Some(root.join("desktop-state.json"))
    );
    assert_eq!(
        directory.path(DataFile::Characters),
        Some(root.join("character-preferences.json"))
    );
    assert_eq!(
        directory.path(DataFile::Reminders),
        Some(root.join("reminders.json"))
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    drop(directory); // Windows denies reading locked bytes through another handle.
    assert_eq!(fs::read(root.join("instance.lock")).unwrap(), b"");
}

#[test]
fn lock_lifetime_prevents_second_start_then_allows_reacquisition() {
    let temp = Temp::new();
    let directory = acquire(Some(temp.0.clone())).unwrap();
    assert!(acquire(Some(temp.0.clone())).is_err());
    drop(directory);
    assert!(acquire(Some(temp.0.clone())).unwrap().root().is_some());
}

#[test]
fn stale_lock_is_not_a_process_marker_and_is_never_truncated_or_removed() {
    let temp = Temp::new();
    fs::write(temp.0.join("instance.lock"), "existing bytes").unwrap();
    let directory = acquire(Some(temp.0.clone())).unwrap();
    assert!(directory.root().is_some());
    drop(directory);
    assert_eq!(
        fs::read_to_string(temp.0.join("instance.lock")).unwrap(),
        "existing bytes"
    );
}

#[test]
fn unavailable_root_gives_none_to_every_store() {
    let directory = acquire(None).unwrap();
    assert_eq!(directory.diagnostic(), Some("directoryUnavailable"));
    assert!(directory.root().is_none());
    assert!(directory.path(DataFile::Desktop).is_none());
    assert!(directory.path(DataFile::Characters).is_none());
    assert!(directory.path(DataFile::Reminders).is_none());
}

#[test]
fn file_root_and_child_of_file_are_unavailable_without_altering_existing_data() {
    let temp = Temp::new();
    let file = temp.0.join("keep");
    fs::write(&file, "original").unwrap();
    for root in [file.clone(), file.join("config")] {
        let directory = acquire(Some(root)).unwrap();
        assert_eq!(directory.diagnostic(), Some("directoryCreateFailed"));
        assert!(directory.root().is_none());
    }
    assert_eq!(fs::read_to_string(file).unwrap(), "original");
    assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 1);
}

#[test]
fn lock_open_failure_is_unavailable_not_duplicate_and_preserves_data() {
    let temp = Temp::new();
    fs::create_dir(temp.0.join("instance.lock")).unwrap();
    fs::write(temp.0.join("reminders.json"), "preserve even invalid bytes").unwrap();
    let directory = acquire(Some(temp.0.clone())).unwrap();
    assert_eq!(directory.diagnostic(), Some("directoryLockOpenFailed"));
    assert!(directory.path(DataFile::Reminders).is_none());
    assert_eq!(
        fs::read_to_string(temp.0.join("reminders.json")).unwrap(),
        "preserve even invalid bytes"
    );
    assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 2);
}
