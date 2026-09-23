//! Frozen, one-use import previews for the settings window.
use crate::data_directory::{
    backup_codec::{self, DecodedBackup, Preview},
    DataDirectory, ImportFiles,
};
use serde::Serialize;
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportPreview {
    pub ticket: u64,
    pub preview: Preview,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportConfirmation {
    pub transaction_id: String,
    pub restart_required: bool,
}

enum Selection {
    Loading {
        session: (u64, u64),
        ticket: u64,
    },
    Ready {
        session: (u64, u64),
        ticket: u64,
        files: ImportFiles,
        preview: Box<Preview>,
    },
}

#[derive(Default)]
pub(crate) struct ImportSession {
    next: u64,
    selection: Option<Selection>,
}

impl ImportSession {
    /// A new selection immediately invalidates the prior preview and any late callback.
    pub fn begin_selection(&mut self, session: (u64, u64)) -> Result<u64, &'static str> {
        self.selection = None;
        self.next = self.next.checked_add(1).ok_or("backupUnavailable")?;
        self.selection = Some(Selection::Loading {
            session,
            ticket: self.next,
        });
        Ok(self.next)
    }

    pub fn finish_selection(
        &mut self,
        ticket: u64,
        current: (u64, u64),
        loaded: Result<DecodedBackup, &'static str>,
    ) -> Result<ImportPreview, &'static str> {
        let Some(Selection::Loading {
            session,
            ticket: active,
        }) = self.selection.as_ref()
        else {
            return Err("backupStale");
        };
        if *active != ticket {
            return Err("backupStale");
        }
        if *session != current {
            self.selection = None;
            return Err("backupStale");
        }
        let decoded = match loaded {
            Ok(decoded) => decoded,
            Err(code) => {
                self.selection = None;
                return Err(code);
            }
        };
        let response = ImportPreview {
            ticket,
            preview: decoded.preview.clone(),
        };
        self.selection = Some(Selection::Ready {
            session: current,
            ticket,
            files: decoded.files,
            preview: Box::new(decoded.preview),
        });
        Ok(response)
    }

    pub fn confirm(
        &mut self,
        ticket: u64,
        current: (u64, u64),
        directory: &DataDirectory,
    ) -> Result<ImportConfirmation, &'static str> {
        let Some(Selection::Ready {
            session,
            ticket: active,
            files,
            ..
        }) = self.selection.as_ref()
        else {
            return Err("backupStale");
        };
        if *active != ticket {
            return Err("backupStale");
        }
        if *session != current {
            self.selection = None;
            return Err("backupStale");
        }
        // B2 consumes its argument. Keep the verified candidate until a successful
        // pending transaction so a failed write can be retried without file I/O.
        let transaction_id = directory.prepare_import(files.clone())?;
        self.selection = None;
        Ok(ImportConfirmation {
            transaction_id,
            restart_required: true,
        })
    }

    pub fn cancel(&mut self) {
        self.selection = None;
    }
    pub fn invalidate(&mut self) {
        self.selection = None;
    }

    /// Re-display an existing frozen summary after a recoverable prepare error.
    pub fn current_preview(&self, current: (u64, u64)) -> Option<ImportPreview> {
        match self.selection.as_ref() {
            Some(Selection::Ready {
                session,
                ticket,
                preview,
                ..
            }) if *session == current => Some(ImportPreview {
                ticket: *ticket,
                preview: preview.as_ref().clone(),
            }),
            _ => None,
        }
    }
}

/// Accept only a native picker path. The caller must never expose this path to
/// the WebView; decoded state and errors contain no paths or source bytes.
pub(crate) fn read_selected(path: &Path) -> Result<DecodedBackup, &'static str> {
    if !path.is_absolute() || !local_path(path) {
        return Err("backupReadFailed");
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| "backupReadFailed")?;
    if !metadata.is_file() || metadata.is_symlink() {
        return Err("backupReadFailed");
    }
    if metadata.len() > backup_codec::MAX_BACKUP_BYTES as u64 {
        return Err("backupTooLarge");
    }
    let file = File::open(path).map_err(|_| "backupReadFailed")?;
    if !file.metadata().map_err(|_| "backupReadFailed")?.is_file() {
        return Err("backupReadFailed");
    }
    let mut bytes = Vec::new();
    file.take(backup_codec::MAX_BACKUP_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "backupReadFailed")?;
    if bytes.len() > backup_codec::MAX_BACKUP_BYTES {
        return Err("backupTooLarge");
    }
    backup_codec::decode(&bytes)
}

#[cfg(windows)]
fn local_path(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    matches!(path.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
}
#[cfg(not(windows))]
fn local_path(_path: &Path) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_directory::{acquire, backup_codec, DataFile, ImportFiles};
    use base64::{engine::general_purpose::STANDARD, Engine as _};
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
                "march7-import-session-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn backup(&self) -> PathBuf {
            self.0.join("backup.json")
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn candidate() -> ImportFiles {
        ImportFiles {
            desktop: None,
            characters: br#"{"version":1,"selectedCharacterId":"march-7th"}"#.to_vec(),
            reminders: br#"{"version":1,"settings":{"items":[{"id":"water","enabled":false,"intervalMinutes":60},{"id":"move","enabled":false,"intervalMinutes":60},{"id":"eyes","enabled":false,"intervalMinutes":30}],"activeHours":{"kind":"daily","start":540,"end":1320},"snoozeMinutes":10},"progress":[{"id":"water","nextDueAt":null,"pending":false,"autoHandled":false},{"id":"move","nextDueAt":null,"pending":false,"autoHandled":false},{"id":"eyes","nextDueAt":null,"pending":false,"autoHandled":false}],"paused":false,"quiet":null,"snoozePending":false}"#.to_vec(),
        }
    }
    fn package() -> Vec<u8> {
        backup_codec::encode(candidate(), 100).unwrap()
    }

    #[test]
    fn preview_freezes_bytes_and_confirm_switches_only_on_next_launch() {
        let temp = Temp::new();
        fs::write(temp.backup(), package()).unwrap();
        let directory = acquire(Some(temp.0.clone())).unwrap();
        let mut session = ImportSession::default();
        let ticket = session.begin_selection((7, 9)).unwrap();
        let selected = read_selected(&temp.backup()).unwrap();
        let shown = session
            .finish_selection(ticket, (7, 9), Ok(selected))
            .unwrap();
        assert_eq!(shown.ticket, ticket);
        assert_eq!(shown.preview.selected_character_id, "march-7th");
        assert_eq!(
            directory.path(DataFile::Characters).unwrap(),
            temp.0.join("character-preferences.json")
        );
        fs::write(temp.backup(), b"changed after preview").unwrap();
        let committed = session.confirm(ticket, (7, 9), &directory).unwrap();
        assert_eq!(committed.transaction_id.len(), 32);
        assert!(committed.restart_required);
        assert_eq!(
            session.confirm(ticket, (7, 9), &directory).err(),
            Some("backupStale")
        );
        assert_eq!(
            directory.path(DataFile::Characters).unwrap(),
            temp.0.join("character-preferences.json")
        );
        drop(directory);
        let next = acquire(Some(temp.0.clone())).unwrap();
        assert_eq!(
            fs::read(next.path(DataFile::Characters).unwrap()).unwrap(),
            candidate().characters
        );
    }

    #[test]
    fn replacement_cancellation_and_window_generation_fence_old_async_results() {
        let mut session = ImportSession::default();
        let first = session.begin_selection((1, 1)).unwrap();
        let second = session.begin_selection((1, 1)).unwrap();
        assert_eq!(
            session
                .finish_selection(first, (1, 1), backup_codec::decode(&package()))
                .err(),
            Some("backupStale")
        );
        let shown = session
            .finish_selection(second, (1, 1), backup_codec::decode(&package()))
            .unwrap();
        assert_eq!(shown.ticket, second);
        session.cancel();
        assert_eq!(
            session
                .confirm(second, (1, 1), &acquire(None).unwrap())
                .err(),
            Some("backupStale")
        );
        let third = session.begin_selection((1, 1)).unwrap();
        assert_eq!(
            session
                .finish_selection(third, (1, 2), backup_codec::decode(&package()))
                .err(),
            Some("backupStale")
        );
        let fourth = session.begin_selection((2, 3)).unwrap();
        session.invalidate();
        assert_eq!(
            session
                .finish_selection(fourth, (2, 3), backup_codec::decode(&package()))
                .err(),
            Some("backupStale")
        );
        let fifth = session.begin_selection((2, 4)).unwrap();
        session
            .finish_selection(fifth, (2, 4), backup_codec::decode(&package()))
            .unwrap();
        assert_eq!(
            session
                .confirm(fifth, (2, 5), &acquire(None).unwrap())
                .err(),
            Some("backupStale")
        );
    }

    #[test]
    fn bounded_reader_rejects_non_file_oversize_and_bad_content_without_pending() {
        let temp = Temp::new();
        let directory = acquire(Some(temp.0.clone())).unwrap();
        assert_eq!(read_selected(&temp.0).err(), Some("backupReadFailed"));
        assert_eq!(
            read_selected(Path::new("backup.json")).err(),
            Some("backupReadFailed")
        );
        assert_eq!(
            read_selected(Path::new(r"\\server\share\backup.json")).err(),
            Some("backupReadFailed")
        );
        fs::write(
            temp.backup(),
            vec![b'x'; backup_codec::MAX_BACKUP_BYTES + 1],
        )
        .unwrap();
        assert_eq!(read_selected(&temp.backup()).err(), Some("backupTooLarge"));
        let mut session = ImportSession::default();
        for bad in [
            b"not json".to_vec(),
            {
                let mut v: serde_json::Value = serde_json::from_slice(&package()).unwrap();
                v["schemaVersion"] = 2.into();
                serde_json::to_vec(&v).unwrap()
            },
            {
                let mut v: serde_json::Value = serde_json::from_slice(&package()).unwrap();
                v["sha256"] = "0".repeat(64).into();
                serde_json::to_vec(&v).unwrap()
            },
        ] {
            fs::write(temp.backup(), bad).unwrap();
            let ticket = session.begin_selection((1, 1)).unwrap();
            assert!(session
                .finish_selection(ticket, (1, 1), read_selected(&temp.backup()))
                .is_err());
            assert_eq!(
                session.confirm(ticket, (1, 1), &directory).err(),
                Some("backupStale")
            );
            assert!(!temp.0.join("pending-import.json").exists());
        }
        let missing = temp.0.join("missing.json");
        assert_eq!(read_selected(&missing).err(), Some("backupReadFailed"));
    }

    #[test]
    fn pending_or_failed_prepare_preserves_candidate_and_original_data() {
        let temp = Temp::new();
        let original = temp.0.join("character-preferences.json");
        fs::write(&original, b"original").unwrap();
        let directory = acquire(Some(temp.0.clone())).unwrap();
        let mut session = ImportSession::default();
        let ticket = session.begin_selection((1, 1)).unwrap();
        session
            .finish_selection(ticket, (1, 1), backup_codec::decode(&package()))
            .unwrap();
        fs::write(temp.0.join("data-sets"), b"obstacle").unwrap();
        assert_eq!(
            session.confirm(ticket, (1, 1), &directory).err(),
            Some("dataSetsUnavailable")
        );
        assert_eq!(session.current_preview((1, 1)).unwrap().ticket, ticket);
        assert_eq!(fs::read(&original).unwrap(), b"original");
        assert!(!temp.0.join("pending-import.json").exists());
        fs::remove_file(temp.0.join("data-sets")).unwrap();
        assert!(session.confirm(ticket, (1, 1), &directory).is_ok());
        let existing_pending = fs::read(temp.0.join("pending-import.json")).unwrap();
        let next = session.begin_selection((1, 1)).unwrap();
        session
            .finish_selection(next, (1, 1), backup_codec::decode(&package()))
            .unwrap();
        assert_eq!(
            session.confirm(next, (1, 1), &directory).err(),
            Some("pendingImportExists")
        );
        assert_eq!(fs::read(&original).unwrap(), b"original");
        assert_eq!(
            fs::read(temp.0.join("pending-import.json")).unwrap(),
            existing_pending
        );
    }

    #[test]
    fn unknown_role_or_invalid_reminder_never_prepares_pending() {
        let temp = Temp::new();
        let directory = acquire(Some(temp.0.clone())).unwrap();
        let mut session = ImportSession::default();
        for bad_files in [
            {
                let mut files = candidate();
                files.characters =
                    br#"{"version":1,"selectedCharacterId":"retired-character"}"#.to_vec();
                files
            },
            {
                let mut files = candidate();
                files.reminders = String::from_utf8(files.reminders)
                    .unwrap()
                    .replacen("\"intervalMinutes\":60", "\"intervalMinutes\":0", 1)
                    .into_bytes();
                files
            },
        ] {
            let mut doc: serde_json::Value = serde_json::from_slice(&package()).unwrap();
            doc["files"]["charactersJsonBase64"] = STANDARD.encode(&bad_files.characters).into();
            doc["files"]["remindersJsonBase64"] = STANDARD.encode(&bad_files.reminders).into();
            fs::write(temp.backup(), serde_json::to_vec(&doc).unwrap()).unwrap();
            let ticket = session.begin_selection((3, 4)).unwrap();
            assert_eq!(
                session
                    .finish_selection(ticket, (3, 4), read_selected(&temp.backup()))
                    .err(),
                Some("dataSetInvalid")
            );
            assert_eq!(
                session.confirm(ticket, (3, 4), &directory).err(),
                Some("backupStale")
            );
            assert!(!temp.0.join("pending-import.json").exists());
        }
    }
}
