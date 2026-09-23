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
fn ready_initializes_one_complete_v2_set_under_the_fixed_lock() {
    let temp = Temp::new();
    let root = temp.0.join("config");
    let directory = acquire(Some(root.clone())).unwrap();
    assert_eq!(directory.root(), Some(root.as_path()));
    assert_eq!(directory.diagnostic(), None);
    let selected = directory
        .path(DataFile::Desktop)
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    assert_eq!(selected.parent(), Some(root.join("data-sets").as_path()));
    for (file, name) in [
        (DataFile::Desktop, "desktop-state.json"),
        (DataFile::Characters, "character-preferences.json"),
        (DataFile::Reminders, "reminders.json"),
        (DataFile::Focus, "focus.json"),
    ] {
        assert_eq!(directory.path(file), Some(selected.join(name)));
        assert!(selected.join(name).is_file());
        assert!(!root.join(name).exists());
    }
    assert_eq!(fs::read_dir(&root).unwrap().count(), 4);
    drop(directory);
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
        focus: serde_json::to_vec(&march_7th_app_lib::focus::model::Data::default()).unwrap(),
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
fn prepared_import_switches_all_four_paths_only_on_next_acquisition() {
    let temp = Temp::new();
    let root = temp.0.join("config");
    fs::create_dir(&root).unwrap();
    let legacy = root.join("character-preferences.json");
    fs::write(&legacy, &valid_import().characters).unwrap();
    let directory = acquire(Some(root.clone())).unwrap();
    let original_path = directory.path(DataFile::Characters).unwrap();
    let original_pointer = fs::read(root.join("active-data-set.json")).unwrap();
    let transaction = directory.prepare_import(valid_import()).unwrap();
    assert_eq!(
        directory.path(DataFile::Characters),
        Some(original_path.clone())
    );
    assert_eq!(fs::read(&legacy).unwrap(), valid_import().characters);
    assert_eq!(
        fs::read(root.join("active-data-set.json")).unwrap(),
        original_pointer
    );
    drop(directory);

    let switched = acquire(Some(root.clone())).unwrap();
    assert_eq!(switched.diagnostic(), None);
    let character_path = switched.path(DataFile::Characters).unwrap();
    assert_ne!(character_path, original_path);
    let dataset = character_path.parent().unwrap();
    assert_eq!(
        switched.path(DataFile::Focus),
        Some(dataset.join("focus.json"))
    );
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
    assert_eq!(fs::read(&legacy).unwrap(), valid_import().characters);
    assert!(!root.join("pending-import.json").exists());
    assert!(!transaction.is_empty());
}

#[test]
fn invalid_import_never_schedules_or_changes_legacy_data() {
    let temp = Temp::new();
    let root = temp.0.join("config");
    let directory = acquire(Some(root.clone())).unwrap();
    let before = fs::read(root.join("active-data-set.json")).unwrap();
    let before_path = directory.path(DataFile::Desktop);
    let mut invalid = valid_import();
    invalid.characters =
        br#"{"version":1,"selectedCharacterId":"march-7th","future":true}"#.to_vec();
    assert!(directory.prepare_import(invalid).is_err());
    assert!(!root.join("pending-import.json").exists());
    assert_eq!(fs::read(root.join("active-data-set.json")).unwrap(), before);
    assert_eq!(directory.path(DataFile::Desktop), before_path);
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
    let before = fs::read(root.join("active-data-set.json")).unwrap();
    fs::rename(root.join("data-sets"), root.join("held-sets")).unwrap();
    fs::write(root.join("data-sets"), b"block directory creation").unwrap();
    assert!(directory.prepare_import(valid_import()).is_err());
    assert!(!root.join("pending-import.json").exists());
    assert!(root.join("data-set-activated.json").exists());
    drop(directory);
    fs::remove_file(root.join("data-sets")).unwrap();
    fs::rename(root.join("held-sets"), root.join("data-sets")).unwrap();
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
    assert_eq!(fs::read(root.join("active-data-set.json")).unwrap(), before);
    assert!(root.join("pending-import.json").exists());
}

#[test]
fn invalid_or_future_pointer_protects_all_stores_without_legacy_fallback() {
    for pointer in [
        "{broken",
        r#"{"version":3,"setId":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","transactionId":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}"#,
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
    fs::write(
        root.join("data-set-activated.json"),
        br#"{"version":1,"activated":true}"#,
    )
    .unwrap();
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
    let before = fs::read(root.join("active-data-set.json")).unwrap();
    let recovered = acquire(Some(root.clone())).unwrap();
    assert!(recovered
        .path(DataFile::Reminders)
        .unwrap()
        .starts_with(root.join("data-sets")));
    assert!(root.join("active-data-set.json").exists());
    assert!(!root.join("pending-import.json").exists());
    assert_ne!(fs::read(root.join("active-data-set.json")).unwrap(), before);
}

#[test]
fn committed_pointer_loss_never_falls_back_to_legacy_files() {
    let temp = Temp::new();
    let root = temp.0.join("config");
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("desktop-state.json"),
        br#"{"version":1,"placement":null}"#,
    )
    .unwrap();
    let directory = acquire(Some(root.clone())).unwrap();
    directory.prepare_import(valid_import()).unwrap();
    drop(directory);
    let committed = acquire(Some(root.clone())).unwrap();
    assert!(root.join("data-set-activated.json").exists());
    drop(committed);
    assert!(!root.join("pending-import.json").exists());
    fs::remove_file(root.join("active-data-set.json")).unwrap();
    let protected = acquire(Some(root.clone())).unwrap();
    assert_eq!(protected.diagnostic(), Some("activePointerMissing"));
    for file in [DataFile::Desktop, DataFile::Characters, DataFile::Reminders] {
        assert!(protected.path(file).is_none());
    }
    assert_eq!(
        fs::read(root.join("desktop-state.json")).unwrap(),
        br#"{"version":1,"placement":null}"#
    );
}

#[test]
fn marker_before_first_pointer_can_finish_pending_but_missing_pending_protects() {
    let temp = Temp::new();
    let root = temp.0.join("config");
    let directory = acquire(Some(root.clone())).unwrap();
    directory.prepare_import(valid_import()).unwrap();
    drop(directory);
    fs::write(
        root.join("data-set-activated.json"),
        br#"{"version":1,"activated":true}"#,
    )
    .unwrap();
    let resumed = acquire(Some(root.clone())).unwrap();
    assert_eq!(resumed.diagnostic(), None);
    assert!(root.join("active-data-set.json").exists());
    drop(resumed);
    fs::remove_file(root.join("active-data-set.json")).unwrap();
    assert_eq!(
        acquire(Some(root.clone())).unwrap().diagnostic(),
        Some("activePointerMissing")
    );
}

#[test]
fn second_import_keeps_activation_history_and_first_set() {
    let temp = Temp::new();
    let root = temp.0.join("config");
    let first = acquire(Some(root.clone())).unwrap();
    first.prepare_import(valid_import()).unwrap();
    drop(first);
    let first_active = acquire(Some(root.clone())).unwrap();
    let old_set = first_active
        .path(DataFile::Reminders)
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    first_active.prepare_import(valid_import()).unwrap();
    drop(first_active);
    let second_active = acquire(Some(root.clone())).unwrap();
    assert_eq!(second_active.diagnostic(), None);
    assert_ne!(
        second_active.path(DataFile::Reminders).unwrap().parent(),
        Some(old_set.as_path())
    );
    assert!(old_set.join("reminders.json").exists());
    assert!(root.join("data-set-activated.json").exists());
}

#[test]
fn invalid_activation_marker_never_becomes_a_legacy_session() {
    for marker in [
        b"broken".as_slice(),
        br#"{"version":2,"activated":true}"#,
        br#"{"version":1,"activated":false}"#,
        br#"{"version":1,"activated":true,"future":1}"#,
    ] {
        let temp = Temp::new();
        let root = temp.0.join("config");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("data-set-activated.json"), marker).unwrap();
        fs::write(root.join("reminders.json"), b"legacy unchanged").unwrap();
        let protected = acquire(Some(root.clone())).unwrap();
        assert_eq!(protected.diagnostic(), Some("activationMarkerInvalid"));
        assert!(protected.path(DataFile::Reminders).is_none());
        assert_eq!(
            fs::read(root.join("reminders.json")).unwrap(),
            b"legacy unchanged"
        );
    }
}

#[test]
fn losing_activation_marker_with_a_valid_pointer_is_protected() {
    let temp = Temp::new();
    let root = temp.0.join("config");
    let first = acquire(Some(root.clone())).unwrap();
    first.prepare_import(valid_import()).unwrap();
    drop(first);
    let active = acquire(Some(root.clone())).unwrap();
    drop(active);
    fs::remove_file(root.join("data-set-activated.json")).unwrap();
    let protected = acquire(Some(root.clone())).unwrap();
    assert_eq!(protected.diagnostic(), Some("activationMarkerMissing"));
    assert!(protected.path(DataFile::Reminders).is_none());
    assert!(root.join("active-data-set.json").exists());
}

#[test]
fn v2_migration_preserves_legacy_flat_bytes_and_adds_idle_focus_in_selected_set() {
    let temp = Temp::new();
    let old = valid_import();
    fs::write(temp.0.join("character-preferences.json"), &old.characters).unwrap();
    fs::write(temp.0.join("reminders.json"), &old.reminders).unwrap();
    let directory = acquire(Some(temp.0.clone())).unwrap();
    assert_eq!(directory.diagnostic(), None);
    let selected = directory.path(DataFile::Reminders).unwrap();
    assert!(selected.starts_with(temp.0.join("data-sets")));
    let focus = selected.parent().unwrap().join("focus.json");
    let data: serde_json::Value = serde_json::from_slice(&fs::read(focus).unwrap()).unwrap();
    assert_eq!(
        data,
        serde_json::json!({"version":1,"session":{"status":"idle"}})
    );
    assert!(!temp.0.join("focus.json").exists());
    assert_eq!(fs::read(selected).unwrap(), old.reminders);
    assert_eq!(
        fs::read(temp.0.join("reminders.json")).unwrap(),
        old.reminders
    );
    let pointer: serde_json::Value =
        serde_json::from_slice(&fs::read(temp.0.join("active-data-set.json")).unwrap()).unwrap();
    assert_eq!(pointer["version"], 2);
    drop(directory);
    let repeated = acquire(Some(temp.0.clone())).unwrap();
    assert_eq!(repeated.diagnostic(), None);
    let after: serde_json::Value =
        serde_json::from_slice(&fs::read(temp.0.join("active-data-set.json")).unwrap()).unwrap();
    assert_eq!(after, pointer);
}

#[test]
fn v2_empty_root_is_complete_and_bad_flat_bytes_are_never_defaulted() {
    let temp = Temp::new();
    let ready = acquire(Some(temp.0.clone())).unwrap();
    let selected = ready.path(DataFile::Reminders).unwrap();
    assert!(selected.parent().unwrap().join("focus.json").is_file());
    drop(ready);
    let broken = Temp::new();
    let bytes = br#"{"version":99,"future":"preserve"}"#;
    fs::write(broken.0.join("reminders.json"), bytes).unwrap();
    let guarded = acquire(Some(broken.0.clone())).unwrap();
    assert!(guarded.diagnostic().is_some());
    assert!(guarded.path(DataFile::Reminders).is_none());
    assert_eq!(fs::read(broken.0.join("reminders.json")).unwrap(), bytes);
    assert!(!broken.0.join("active-data-set.json").exists());
}

// Deliberately construct v1 fixtures without the current staging/encoding path.
fn legacy_set(root: &std::path::Path, set_id: &str, files: &ImportFiles) -> String {
    use sha2::{Digest, Sha256};
    let dir = root.join("data-sets").join(set_id);
    fs::create_dir_all(&dir).unwrap();
    let mut digest = Sha256::new();
    for (name, bytes) in [
        (
            "desktop-state.json",
            files
                .desktop
                .as_deref()
                .unwrap_or(br#"{"version":1,"placement":null}"#),
        ),
        ("character-preferences.json", files.characters.as_slice()),
        ("reminders.json", files.reminders.as_slice()),
    ] {
        fs::write(dir.join(name), bytes).unwrap();
        digest.update(name.as_bytes());
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
    format!("{:x}", digest.finalize())
}
fn legacy_pointer(root: &std::path::Path, set: &str, transaction: &str) -> Vec<u8> {
    let bytes = serde_json::to_vec(
        &serde_json::json!({"version":1,"setId":set,"transactionId":transaction}),
    )
    .unwrap();
    fs::write(root.join("active-data-set.json"), &bytes).unwrap();
    fs::write(
        root.join("data-set-activated.json"),
        br#"{"version":1,"activated":true}"#,
    )
    .unwrap();
    bytes
}

#[test]
fn v1_active_migrates_exact_bytes_preserves_retired_role_and_pointer_backup() {
    let temp = Temp::new();
    let set = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let mut files = valid_import();
    files.characters = br#"{ "version":1, "selectedCharacterId":"retired-character" }"#.to_vec();
    legacy_set(&temp.0, set, &files);
    let before = legacy_pointer(&temp.0, set, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let directory = acquire(Some(temp.0.clone())).unwrap();
    assert_eq!(directory.diagnostic(), None);
    assert_eq!(
        fs::read(directory.path(DataFile::Characters).unwrap()).unwrap(),
        files.characters
    );
    assert_eq!(
        fs::read(temp.0.join("active-data-set.json.bak")).unwrap(),
        before
    );
    assert_eq!(
        fs::read(
            temp.0
                .join("data-sets")
                .join(set)
                .join("character-preferences.json")
        )
        .unwrap(),
        files.characters
    );
    assert!(!temp
        .0
        .join("data-sets")
        .join(set)
        .join("focus.json")
        .exists());
    assert!(directory.path(DataFile::Focus).unwrap().is_file());
}

#[test]
fn v1_pending_finishes_before_upgrade_with_its_original_three_file_digest() {
    for has_base in [false, true] {
        let temp = Temp::new();
        let base = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let target = "cccccccccccccccccccccccccccccccc";
        let base_tx = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        if has_base {
            legacy_set(&temp.0, base, &valid_import());
            legacy_pointer(&temp.0, base, base_tx);
        }
        let mut imported = valid_import();
        imported.characters = br#"{"version":1,"selectedCharacterId":"raiden-shogun"}"#.to_vec();
        let digest = legacy_set(&temp.0, target, &imported);
        fs::write(temp.0.join("pending-import.json"), serde_json::to_vec(&serde_json::json!({
            "version":1,"setId":target,"transactionId":"dddddddddddddddddddddddddddddddd",
            "baseSetId":if has_base {Some(base)} else {None},"baseTransactionId":if has_base {Some(base_tx)} else {None},"digest":digest
        })).unwrap()).unwrap();
        let directory = acquire(Some(temp.0.clone())).unwrap();
        assert_eq!(directory.diagnostic(), None);
        assert_eq!(
            fs::read(directory.path(DataFile::Characters).unwrap()).unwrap(),
            imported.characters
        );
        assert!(!temp.0.join("pending-import.json").exists());
        let selected: serde_json::Value =
            serde_json::from_slice(&fs::read(temp.0.join("active-data-set.json")).unwrap())
                .unwrap();
        assert_eq!(selected["version"], 2);
        assert_ne!(selected["setId"], target);
        assert!(!temp
            .0
            .join("data-sets")
            .join(target)
            .join("focus.json")
            .exists());
        assert_eq!(
            fs::read(directory.path(DataFile::Focus).unwrap()).unwrap(),
            valid_import().focus
        );
    }
}

#[test]
fn v2_missing_corrupt_or_future_focus_blocks_all_stores_without_fallback() {
    for bytes in [
        None,
        Some(b"broken".as_slice()),
        Some(br#"{"version":2,"session":{"status":"idle"}}"#.as_slice()),
        Some(br#"{"version":1,"session":{"status":"idle","task":"secret"}}"#.as_slice()),
    ] {
        let temp = Temp::new();
        let directory = acquire(Some(temp.0.clone())).unwrap();
        let focus = directory.path(DataFile::Focus).unwrap();
        let pointer = fs::read(temp.0.join("active-data-set.json")).unwrap();
        drop(directory);
        match bytes {
            Some(bytes) => fs::write(&focus, bytes).unwrap(),
            None => fs::remove_file(&focus).unwrap(),
        }
        let protected = acquire(Some(temp.0.clone())).unwrap();
        assert_eq!(protected.diagnostic(), Some("dataSetInvalid"));
        for file in [
            DataFile::Desktop,
            DataFile::Characters,
            DataFile::Reminders,
            DataFile::Focus,
        ] {
            assert!(protected.path(file).is_none());
        }
        assert_eq!(
            fs::read(temp.0.join("active-data-set.json")).unwrap(),
            pointer
        );
        if let Some(bytes) = bytes {
            assert_eq!(fs::read(&focus).unwrap(), bytes);
        }
    }
}

#[test]
fn staged_focus_is_checked_before_commit_and_real_atomic_failure_is_restartable() {
    let temp = Temp::new();
    let directory = acquire(Some(temp.0.clone())).unwrap();
    let before = fs::read(temp.0.join("active-data-set.json")).unwrap();
    let mut imported = valid_import();
    imported.focus =
        br#"{ "version":1,"session":{"status":"paused","duration_ms":60000,"remaining_ms":12000}}"#
            .to_vec();
    directory.prepare_import(imported.clone()).unwrap();
    assert_eq!(
        fs::read(directory.path(DataFile::Focus).unwrap()).unwrap(),
        valid_import().focus
    );
    drop(directory);
    let pending = fs::read(temp.0.join("pending-import.json")).unwrap();
    let record: serde_json::Value = serde_json::from_slice(&pending).unwrap();
    let staged = temp
        .0
        .join("data-sets")
        .join(record["setId"].as_str().unwrap())
        .join("focus.json");
    fs::write(&staged, &valid_import().focus).unwrap();
    let guarded = acquire(Some(temp.0.clone())).unwrap();
    assert_eq!(guarded.diagnostic(), Some("pendingImportChanged"));
    assert_eq!(
        fs::read(temp.0.join("active-data-set.json")).unwrap(),
        before
    );
    drop(guarded);
    fs::write(&staged, &imported.focus).unwrap();
    let backup = temp.0.join("active-data-set.json.bak");
    fs::create_dir(&backup).unwrap(); // deterministic failure at the pointer backup write
    let failed = acquire(Some(temp.0.clone())).unwrap();
    assert_eq!(failed.diagnostic(), Some("activePointerWriteFailed"));
    assert_eq!(
        fs::read(temp.0.join("active-data-set.json")).unwrap(),
        before
    );
    assert_eq!(
        fs::read(temp.0.join("pending-import.json")).unwrap(),
        pending
    );
    drop(failed);
    fs::remove_dir(&backup).unwrap();
    let recovered = acquire(Some(temp.0.clone())).unwrap();
    assert_eq!(recovered.diagnostic(), None);
    assert_eq!(
        fs::read(recovered.path(DataFile::Focus).unwrap()).unwrap(),
        imported.focus
    );
    assert!(!temp.0.join("pending-import.json").exists());
    let chosen = recovered.path(DataFile::Focus).unwrap();
    drop(recovered);
    let again = acquire(Some(temp.0.clone())).unwrap();
    assert_eq!(again.path(DataFile::Focus), Some(chosen));
}

#[test]
fn v1_unowned_focus_is_preserved_and_never_silently_dropped_by_migration() {
    for flat in [false, true] {
        let temp = Temp::new();
        let dir = if flat {
            temp.0.clone()
        } else {
            let set = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
            legacy_set(&temp.0, set, &valid_import());
            legacy_pointer(&temp.0, set, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
            temp.0.join("data-sets").join(set)
        };
        let future = br#"{"version":99,"future":"preserve"}"#;
        fs::write(dir.join("focus.json"), future).unwrap();
        let guarded = acquire(Some(temp.0.clone())).unwrap();
        assert_eq!(guarded.diagnostic(), Some("dataSetInvalid"));
        assert!(guarded.path(DataFile::Focus).is_none());
        assert_eq!(fs::read(dir.join("focus.json")).unwrap(), future);
    }
}

#[test]
fn valid_v1_at_original_size_limit_can_upgrade_without_losing_source_bytes() {
    let temp = Temp::new();
    let mut files = valid_import();
    let desktop = br#"{"version":1,"placement":null}"#;
    let total = desktop.len() + files.characters.len() + files.reminders.len();
    files.reminders.extend(vec![b' '; 1024 * 1024 - total]);
    legacy_set(&temp.0, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", &files);
    legacy_pointer(
        &temp.0,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );
    let directory = acquire(Some(temp.0.clone())).unwrap();
    assert_eq!(directory.diagnostic(), None);
    assert_eq!(
        fs::read(directory.path(DataFile::Reminders).unwrap()).unwrap(),
        files.reminders
    );
    assert_eq!(
        fs::read(directory.path(DataFile::Focus).unwrap()).unwrap(),
        files.focus
    );
}
