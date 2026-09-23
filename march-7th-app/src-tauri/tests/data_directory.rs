use march_7th_app_lib::data_directory::{acquire, DataFile, ImportFiles};
use std::{
    fs,
    path::PathBuf,
    process::Command,
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

fn valid_import() -> ImportFiles {
    ImportFiles {
        desktop: None,
        characters: br#"{"version":1,"selectedCharacterId":"march-7th"}"#.to_vec(),
        reminders: serde_json::to_vec(&serde_json::json!({
            "version": 1,
            "settings": {
                "items": [
                    {"id":"water","enabled":false,"intervalMinutes":60},
                    {"id":"move","enabled":false,"intervalMinutes":60},
                    {"id":"eyes","enabled":false,"intervalMinutes":30}
                ],
                "activeHours":{"kind":"daily","start":540,"end":1320},
                "snoozeMinutes":10
            },
            "progress": [
                {"id":"water","nextDueAt":null,"pending":false,"autoHandled":false},
                {"id":"move","nextDueAt":null,"pending":false,"autoHandled":false},
                {"id":"eyes","nextDueAt":null,"pending":false,"autoHandled":false}
            ],
            "paused":false,"quiet":null,"snoozePending":false
        }))
        .unwrap(),
    }
}

#[test]
fn prepared_import_switches_all_three_paths_only_on_next_acquisition() {
    let temp = Temp::new();
    let root = temp.0.join("config");
    fs::create_dir(&root).unwrap();
    let legacy = root.join("character-preferences.json");
    fs::write(&legacy, b"legacy bytes remain exact").unwrap();
    let directory = acquire(Some(root.clone())).unwrap();
    let transaction = directory.prepare_import(valid_import()).unwrap();
    assert_eq!(directory.path(DataFile::Characters), Some(legacy.clone()));
    assert_eq!(fs::read(&legacy).unwrap(), b"legacy bytes remain exact");
    assert!(!root.join("active-data-set.json").exists());
    drop(directory);

    let switched = acquire(Some(root.clone())).unwrap();
    assert_eq!(switched.diagnostic(), None);
    let character_path = switched.path(DataFile::Characters).unwrap();
    let dataset = character_path.parent().unwrap();
    assert_eq!(dataset.parent(), Some(root.join("data-sets").as_path()));
    assert_eq!(
        switched.path(DataFile::Desktop),
        Some(dataset.join("desktop-state.json"))
    );
    assert_eq!(
        switched.path(DataFile::Reminders),
        Some(dataset.join("reminders.json"))
    );
    assert!(fs::read_to_string(dataset.join("desktop-state.json"))
        .unwrap()
        .contains("null"));
    assert_eq!(fs::read(&legacy).unwrap(), b"legacy bytes remain exact");
    assert!(!root.join("pending-import.json").exists());
    assert!(!transaction.is_empty());
}

#[test]
fn invalid_import_never_schedules_or_changes_legacy_data() {
    let temp = Temp::new();
    let root = temp.0.join("config");
    let directory = acquire(Some(root.clone())).unwrap();
    let mut invalid = valid_import();
    invalid.characters =
        br#"{"version":1,"selectedCharacterId":"march-7th","future":true}"#.to_vec();
    assert!(directory.prepare_import(invalid).is_err());
    assert!(!root.join("pending-import.json").exists());
    assert!(!root.join("active-data-set.json").exists());
    assert_eq!(
        directory.path(DataFile::Desktop),
        Some(root.join("desktop-state.json"))
    );
}

#[test]
fn later_save_is_not_replayed_even_if_old_pending_record_survives() {
    let temp = Temp::new();
    let root = temp.0.join("config");
    let directory = acquire(Some(root.clone())).unwrap();
    directory.prepare_import(valid_import()).unwrap();
    let old_pending = fs::read(root.join("pending-import.json")).unwrap();
    drop(directory);
    let first = acquire(Some(root.clone())).unwrap();
    let reminders = first.path(DataFile::Reminders).unwrap();
    let mut changed: serde_json::Value =
        serde_json::from_slice(&fs::read(&reminders).unwrap()).unwrap();
    changed["paused"] = serde_json::json!(true);
    fs::write(&reminders, serde_json::to_vec(&changed).unwrap()).unwrap();
    drop(first);
    // A crash after pointer commit but before pending removal leaves this
    // exact old record. Recreating it models the same on-disk state.
    fs::write(root.join("pending-import.json"), old_pending).unwrap();
    let second = acquire(Some(root.clone())).unwrap();
    assert_eq!(second.diagnostic(), None);
    let after: serde_json::Value = serde_json::from_slice(&fs::read(&reminders).unwrap()).unwrap();
    assert_eq!(after["paused"], true);
    assert_eq!(second.path(DataFile::Reminders), Some(reminders));
    assert!(!root.join("pending-import.json").exists());
}

#[test]
fn incomplete_preparation_and_changed_staging_never_change_active_pointer() {
    let temp = Temp::new();
    let root = temp.0.join("config");
    let directory = acquire(Some(root.clone())).unwrap();
    fs::write(root.join("data-sets"), b"block directory creation").unwrap();
    assert!(directory.prepare_import(valid_import()).is_err());
    assert!(!root.join("pending-import.json").exists());
    drop(directory);
    fs::remove_file(root.join("data-sets")).unwrap();
    let directory = acquire(Some(root.clone())).unwrap();
    directory.prepare_import(valid_import()).unwrap();
    drop(directory);
    let pending: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("pending-import.json")).unwrap()).unwrap();
    let set_id = pending["setId"].as_str().unwrap();
    let path = root
        .join("data-sets")
        .join(set_id)
        .join("character-preferences.json");
    fs::write(
        &path,
        br#"{"version":1,"selectedCharacterId":"raiden-shogun"}"#,
    )
    .unwrap();
    let protected = acquire(Some(root.clone())).unwrap();
    assert_eq!(protected.diagnostic(), Some("pendingImportChanged"));
    assert!(protected.path(DataFile::Desktop).is_none());
    assert!(!root.join("active-data-set.json").exists());
    assert!(root.join("pending-import.json").exists());
}

#[test]
fn invalid_or_future_pointer_protects_all_stores_without_legacy_fallback() {
    for pointer in [
        "{broken",
        r#"{"version":2,"setId":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","transactionId":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}"#,
        r#"{"version":1,"setId":"../../other","transactionId":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}"#,
        r#"{"version":1,"setId":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","transactionId":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}"#,
    ] {
        let temp = Temp::new();
        let root = temp.0.join("config");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("desktop-state.json"), b"legacy stays").unwrap();
        fs::write(root.join("active-data-set.json"), pointer).unwrap();
        let directory = acquire(Some(root.clone())).unwrap();
        assert!(directory.diagnostic().is_some());
        for file in [DataFile::Desktop, DataFile::Characters, DataFile::Reminders] {
            assert!(directory.path(file).is_none());
        }
        assert_eq!(
            fs::read(root.join("desktop-state.json")).unwrap(),
            b"legacy stays"
        );
        assert_eq!(
            fs::read_to_string(root.join("active-data-set.json")).unwrap(),
            pointer
        );
    }
}

#[test]
fn base_mismatch_keeps_both_records_and_blocks_auto_apply() {
    let temp = Temp::new();
    let root = temp.0.join("config");
    let directory = acquire(Some(root.clone())).unwrap();
    directory.prepare_import(valid_import()).unwrap();
    drop(directory);
    let pending: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("pending-import.json")).unwrap()).unwrap();
    let staged = root
        .join("data-sets")
        .join(pending["setId"].as_str().unwrap());
    let other = root
        .join("data-sets")
        .join("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    fs::create_dir(&other).unwrap();
    for file in [
        "desktop-state.json",
        "character-preferences.json",
        "reminders.json",
    ] {
        fs::copy(staged.join(file), other.join(file)).unwrap();
    }
    let other_pointer = r#"{"version":1,"setId":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","transactionId":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}"#;
    fs::write(root.join("active-data-set.json"), other_pointer).unwrap();
    let protected = acquire(Some(root.clone())).unwrap();
    assert_eq!(protected.diagnostic(), Some("pendingImportConflict"));
    assert!(root.join("pending-import.json").exists());
    assert_eq!(
        fs::read_to_string(root.join("active-data-set.json")).unwrap(),
        other_pointer
    );
}

#[test]
fn unknown_fields_and_future_versions_in_candidate_never_create_pending() {
    let temp = Temp::new();
    let root = temp.0.join("config");
    let directory = acquire(Some(root.clone())).unwrap();
    let mut with_desktop_future = valid_import();
    with_desktop_future.desktop = Some(
        br#"{"version":1,"placement":{"monitor_name":null,"x":1,"y":2,"future":true}}"#.to_vec(),
    );
    assert!(directory.prepare_import(with_desktop_future).is_err());
    let mut with_future_character = valid_import();
    with_future_character.characters =
        br#"{"version":2,"selectedCharacterId":"march-7th"}"#.to_vec();
    assert!(directory.prepare_import(with_future_character).is_err());
    let mut with_unknown_reminder = valid_import();
    let mut reminder: serde_json::Value =
        serde_json::from_slice(&with_unknown_reminder.reminders).unwrap();
    reminder["newProgress"] = serde_json::json!(true);
    with_unknown_reminder.reminders = serde_json::to_vec(&reminder).unwrap();
    assert!(directory.prepare_import(with_unknown_reminder).is_err());
    assert!(!root.join("pending-import.json").exists());
}

#[test]
fn child_prepare_only() {
    let Some(root) = std::env::var_os("MARCH7_TRANSFER_TEST_ROOT") else {
        return;
    };
    let directory = acquire(Some(PathBuf::from(root))).unwrap();
    directory.prepare_import(valid_import()).unwrap();
    // The parent must recover using only on-disk state after this process ends.
    std::process::exit(0);
}

#[test]
fn process_exit_after_prepare_is_recovered_by_next_process() {
    let temp = Temp::new();
    let root = temp.0.join("config");
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_prepare_only"])
        .env("MARCH7_TRANSFER_TEST_ROOT", &root)
        .status()
        .unwrap();
    assert!(result.success());
    assert!(root.join("pending-import.json").exists());
    assert!(!root.join("active-data-set.json").exists());
    let recovered = acquire(Some(root.clone())).unwrap();
    assert!(recovered
        .path(DataFile::Reminders)
        .unwrap()
        .starts_with(root.join("data-sets")));
    assert!(root.join("active-data-set.json").exists());
    assert!(!root.join("pending-import.json").exists());
}
