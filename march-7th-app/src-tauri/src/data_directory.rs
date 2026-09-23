//! One participating writer per configuration root, and one selected data set
//! for all three stores. No pointer means the original flat files remain live.
pub use crate::data_lock::AlreadyRunning;
use crate::data_lock::LockedRoot;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

const ACTIVE: &str = "active-data-set.json";
const PENDING: &str = "pending-import.json";
const ACTIVATED: &str = "data-set-activated.json";
const SETS: &str = "data-sets";
const MAX_SET_BYTES: usize = 1024 * 1024;
const MAX_RECORD_BYTES: usize = 4096;
const ABSENT_DESKTOP: &[u8] = br#"{"version":1,"placement":null}"#;

#[derive(Clone, Copy)]
pub enum DataFile {
    Desktop,
    Characters,
    Reminders,
}
impl DataFile {
    fn name(self) -> &'static str {
        match self {
            Self::Desktop => "desktop-state.json",
            Self::Characters => "character-preferences.json",
            Self::Reminders => "reminders.json",
        }
    }
}

/// An internal, already parsed candidate. B3 will own the external backup
/// format; this type deliberately cannot carry a source path or file names.
pub struct ImportFiles {
    pub desktop: Option<Vec<u8>>,
    pub characters: Vec<u8>,
    pub reminders: Vec<u8>,
}
impl ImportFiles {
    fn desktop_bytes(&self) -> &[u8] {
        self.desktop.as_deref().unwrap_or(ABSENT_DESKTOP)
    }
    fn validate(&self) -> Result<(), &'static str> {
        let total = self
            .desktop_bytes()
            .len()
            .checked_add(self.characters.len())
            .and_then(|v| v.checked_add(self.reminders.len()))
            .ok_or("dataSetTooLarge")?;
        if total > MAX_SET_BYTES {
            return Err("dataSetTooLarge");
        }
        crate::desktop::validate_imported_desktop(self.desktop_bytes())
            .map_err(|_| "dataSetInvalid")?;
        crate::characters::validate_imported_characters(&self.characters)
            .map_err(|_| "dataSetInvalid")?;
        crate::reminders::validate_imported_reminders(&self.reminders)
            .map_err(|_| "dataSetInvalid")?;
        Ok(())
    }
    fn digest(&self) -> String {
        let mut digest = Sha256::new();
        for (file, bytes) in [
            (DataFile::Desktop, self.desktop_bytes()),
            (DataFile::Characters, self.characters.as_slice()),
            (DataFile::Reminders, self.reminders.as_slice()),
        ] {
            digest.update(file.name().as_bytes());
            digest.update((bytes.len() as u64).to_le_bytes());
            digest.update(bytes);
        }
        hex(&digest.finalize())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pointer {
    version: u8,
    set_id: String,
    transaction_id: String,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pending {
    version: u8,
    set_id: String,
    transaction_id: String,
    base_set_id: Option<String>,
    base_transaction_id: Option<String>,
    digest: String,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Activation {
    version: u8,
    activated: bool,
}
impl Pointer {
    fn valid(&self) -> bool {
        self.version == 1 && valid_id(&self.set_id) && valid_id(&self.transaction_id)
    }
}
impl Pending {
    fn valid(&self) -> bool {
        self.version == 1
            && valid_id(&self.set_id)
            && valid_id(&self.transaction_id)
            && self.base_set_id.as_deref().is_none_or(valid_id)
            && self.base_transaction_id.as_deref().is_none_or(valid_id)
            && self.base_set_id.is_some() == self.base_transaction_id.is_some()
            && self.digest.len() == 64
            && self.digest.bytes().all(|b| b.is_ascii_hexdigit())
    }
    fn target(&self) -> Pointer {
        Pointer {
            version: 1,
            set_id: self.set_id.clone(),
            transaction_id: self.transaction_id.clone(),
        }
    }
    fn matches_base(&self, active: Option<&Pointer>) -> bool {
        self.base_set_id.as_deref() == active.map(|p| p.set_id.as_str())
            && self.base_transaction_id.as_deref() == active.map(|p| p.transaction_id.as_str())
    }
}

enum Access {
    Ready {
        root: PathBuf,
        selected: Option<Pointer>,
        _lock: File,
    },
    // Retain the lock while the app is open, but hand no writable path to any
    // store when the pointer/pending protocol is uncertain.
    Protected {
        _lock: File,
        code: &'static str,
    },
    Unavailable(&'static str),
}
// Deliberately not Clone: managed application state owns the only lock handle.
pub struct DataDirectory(Access);

pub fn acquire(root: Option<PathBuf>) -> Result<DataDirectory, AlreadyRunning> {
    match crate::data_lock::acquire(root)? {
        LockedRoot::Ready { root, lock } => match resolve_locked(&root) {
            Ok(selected) => Ok(DataDirectory(Access::Ready {
                root,
                selected,
                _lock: lock,
            })),
            Err(code) => Ok(DataDirectory(Access::Protected { _lock: lock, code })),
        },
        LockedRoot::Unavailable(code) => Ok(DataDirectory(Access::Unavailable(code))),
    }
}

impl DataDirectory {
    pub fn root(&self) -> Option<&Path> {
        match &self.0 {
            Access::Ready { root, .. } => Some(root),
            Access::Protected { .. } | Access::Unavailable(_) => None,
        }
    }

    pub fn path(&self, file: DataFile) -> Option<PathBuf> {
        match &self.0 {
            Access::Ready { root, selected, .. } => {
                let selected_root = selected
                    .as_ref()
                    .map(|p| set_dir(root, &p.set_id))
                    .unwrap_or_else(|| root.clone());
                Some(selected_root.join(file.name()))
            }
            Access::Protected { .. } | Access::Unavailable(_) => None,
        }
    }

    pub fn diagnostic(&self) -> Option<&'static str> {
        match &self.0 {
            Access::Ready { .. } => None,
            Access::Protected { code, .. } | Access::Unavailable(code) => Some(code),
        }
    }

    /// Schedules an internally validated complete set for the next startup.
    /// It does not change this process's selected paths or restart the app.
    pub fn prepare_import(&self, files: ImportFiles) -> Result<String, &'static str> {
        let Access::Ready { root, selected, .. } = &self.0 else {
            return Err("directoryUnavailable");
        };
        files.validate()?;
        if read_record::<Pending>(&root.join(PENDING), "pendingImportInvalid")?.is_some() {
            return Err("pendingImportExists");
        }
        let current = read_pointer(root)?;
        if &current != selected {
            return Err("activePointerChanged");
        }
        if read_activation(root)? != selected.is_some() {
            return Err("activationMarkerChanged");
        }
        let mut random = [0_u8; 32];
        getrandom::fill(&mut random).map_err(|_| "randomUnavailable")?;
        let set_id = hex(&random[..16]);
        let transaction_id = hex(&random[16..]);
        let sets = root.join(SETS);
        // create_dir_all would follow an existing symlink/junction; reject it.
        match fs::symlink_metadata(&sets) {
            Ok(metadata) if !metadata.is_dir() || metadata.is_symlink() => {
                return Err("dataSetsUnavailable")
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&sets).map_err(|_| "dataSetsUnavailable")?;
            }
            Err(_) => return Err("dataSetsUnavailable"),
        }
        let new_set = set_dir(root, &set_id);
        fs::create_dir(&new_set).map_err(|_| "dataSetCreateFailed")?;
        for (file, bytes) in [
            (DataFile::Desktop, files.desktop_bytes()),
            (DataFile::Characters, files.characters.as_slice()),
            (DataFile::Reminders, files.reminders.as_slice()),
        ] {
            let mut target = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(new_set.join(file.name()))
                .map_err(|_| "dataSetWriteFailed")?;
            target
                .write_all(bytes)
                .and_then(|_| target.sync_all())
                .map_err(|_| "dataSetWriteFailed")?;
        }
        let prepared = read_set(root, &set_id)?;
        if prepared.digest() != files.digest() {
            return Err("dataSetChanged");
        }
        let pending = Pending {
            version: 1,
            set_id,
            transaction_id: transaction_id.clone(),
            base_set_id: selected.as_ref().map(|p| p.set_id.clone()),
            base_transaction_id: selected.as_ref().map(|p| p.transaction_id.clone()),
            digest: files.digest(),
        };
        let bytes = serde_json::to_vec(&pending).map_err(|_| "pendingImportWriteFailed")?;
        crate::atomic_file::replace(&root.join(PENDING), &bytes, None)
            .map_err(|_| "pendingImportWriteFailed")?;
        Ok(transaction_id)
    }
}

fn resolve_locked(root: &Path) -> Result<Option<Pointer>, &'static str> {
    resolve_locked_with(
        root,
        |path, bytes| crate::atomic_file::replace(path, bytes, None),
        crate::atomic_file::replace,
        |path| fs::remove_file(path),
    )
}
fn resolve_locked_with(
    root: &Path,
    write_activation: impl FnOnce(&Path, &[u8]) -> Result<(), String>,
    replace_pointer: impl FnOnce(&Path, &[u8], Option<&[u8]>) -> Result<(), String>,
    remove_pending: impl Fn(&Path) -> io::Result<()>,
) -> Result<Option<Pointer>, &'static str> {
    let activated = read_activation(root)?;
    let active = read_pointer(root)?;
    if active.is_some() && !activated {
        return Err("activationMarkerMissing");
    }
    if let Some(pointer) = &active {
        read_set(root, &pointer.set_id)?;
    }
    let Some(pending) = read_record::<Pending>(&root.join(PENDING), "pendingImportInvalid")? else {
        if activated && active.is_none() {
            return Err("activePointerMissing");
        }
        return Ok(active);
    };
    if !pending.valid() {
        return Err("pendingImportInvalid");
    }
    let target = pending.target();
    if active.as_ref() == Some(&target) {
        // The previous launch already committed. Data may have advanced since
        // then, so do not compare its old digest or rewrite any data file.
        let _ = remove_pending(&root.join(PENDING));
        return Ok(active);
    }
    if !pending.matches_base(active.as_ref()) {
        return Err("pendingImportConflict");
    }
    let staged = read_set(root, &pending.set_id)?;
    if staged.digest() != pending.digest {
        return Err("pendingImportChanged");
    }
    if !activated {
        let bytes = serde_json::to_vec(&Activation {
            version: 1,
            activated: true,
        })
        .map_err(|_| "activationMarkerWriteFailed")?;
        write_activation(&root.join(ACTIVATED), &bytes)
            .map_err(|_| "activationMarkerWriteFailed")?;
        if !read_activation(root)? {
            return Err("activationMarkerWriteFailed");
        }
    }
    let bytes = serde_json::to_vec(&target).map_err(|_| "activePointerWriteFailed")?;
    let previous = if active.is_some() {
        let raw = read_limited(&root.join(ACTIVE), MAX_RECORD_BYTES)
            .map_err(|_| "activePointerWriteFailed")?;
        let now: Pointer = serde_json::from_slice(&raw).map_err(|_| "activePointerWriteFailed")?;
        if Some(now) != active {
            return Err("activePointerChanged");
        }
        Some(raw)
    } else {
        None
    };
    replace_pointer(&root.join(ACTIVE), &bytes, previous.as_deref())
        .map_err(|_| "activePointerWriteFailed")?;
    let _ = remove_pending(&root.join(PENDING));
    Ok(Some(target))
}

fn read_pointer(root: &Path) -> Result<Option<Pointer>, &'static str> {
    let pointer = read_record::<Pointer>(&root.join(ACTIVE), "activePointerInvalid")?;
    if pointer.as_ref().is_some_and(|p| !p.valid()) {
        return Err("activePointerInvalid");
    }
    Ok(pointer)
}
fn read_activation(root: &Path) -> Result<bool, &'static str> {
    let activation = read_record::<Activation>(&root.join(ACTIVATED), "activationMarkerInvalid")?;
    match activation {
        None => Ok(false),
        Some(Activation {
            version: 1,
            activated: true,
        }) => Ok(true),
        Some(_) => Err("activationMarkerInvalid"),
    }
}
fn read_record<T: DeserializeOwned>(
    path: &Path,
    error_code: &'static str,
) -> Result<Option<T>, &'static str> {
    read_record_with_reader(path, error_code, read_limited)
}
fn read_record_with_reader<T: DeserializeOwned>(
    path: &Path,
    error_code: &'static str,
    reader: impl FnOnce(&Path, usize) -> io::Result<Vec<u8>>,
) -> Result<Option<T>, &'static str> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || metadata.is_symlink() => return Err(error_code),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(error_code),
    }
    match reader(path, MAX_RECORD_BYTES) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| error_code),
        Err(_) => Err(error_code),
    }
}
fn read_set(root: &Path, id: &str) -> Result<ImportFiles, &'static str> {
    if !valid_id(id) {
        return Err("dataSetInvalid");
    }
    let sets = root.join(SETS);
    if !real_directory(&sets) || !real_directory(&sets.join(id)) {
        return Err("dataSetInvalid");
    }
    let dir = set_dir(root, id);
    let read = |file: DataFile| -> Result<Vec<u8>, &'static str> {
        let path = dir.join(file.name());
        let metadata = fs::symlink_metadata(&path).map_err(|_| "dataSetInvalid")?;
        if !metadata.is_file() || metadata.is_symlink() {
            return Err("dataSetInvalid");
        }
        read_limited(&path, MAX_SET_BYTES).map_err(|_| "dataSetInvalid")
    };
    let files = ImportFiles {
        desktop: Some(read(DataFile::Desktop)?),
        characters: read(DataFile::Characters)?,
        reminders: read(DataFile::Reminders)?,
    };
    files.validate()?;
    Ok(files)
}
fn read_limited(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "size limit"));
    }
    Ok(bytes)
}
fn real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir() && !metadata.is_symlink())
}
fn set_dir(root: &Path, id: &str) -> PathBuf {
    root.join(SETS).join(id)
}
fn valid_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(DIGITS[(byte >> 4) as usize] as char);
        text.push(DIGITS[(byte & 15) as usize] as char);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "march7-transfer-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
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
    fn candidate() -> ImportFiles {
        ImportFiles {
            desktop: None,
            characters: br#"{"version":1,"selectedCharacterId":"march-7th"}"#.to_vec(),
            reminders: br#"{"version":1,"settings":{"items":[{"id":"water","enabled":false,"intervalMinutes":60},{"id":"move","enabled":false,"intervalMinutes":60},{"id":"eyes","enabled":false,"intervalMinutes":30}],"activeHours":{"kind":"daily","start":540,"end":1320},"snoozeMinutes":10},"progress":[{"id":"water","nextDueAt":null,"pending":false,"autoHandled":false},{"id":"move","nextDueAt":null,"pending":false,"autoHandled":false},{"id":"eyes","nextDueAt":null,"pending":false,"autoHandled":false}],"paused":false,"quiet":null,"snoozePending":false}"#.to_vec(),
        }
    }

    #[test]
    fn pointer_replace_failure_keeps_pending_and_legacy_for_retry() {
        let temp = Temp::new();
        let root = &temp.0;
        fs::write(root.join("desktop-state.json"), b"legacy").unwrap();
        let directory = acquire(Some(root.clone())).unwrap();
        directory.prepare_import(candidate()).unwrap();
        drop(directory);
        assert_eq!(
            resolve_locked_with(
                root,
                |path, bytes| crate::atomic_file::replace(path, bytes, None),
                |_path, _bytes, _previous| Err("injected replace failure".into()),
                |_path| Ok(())
            ),
            Err("activePointerWriteFailed")
        );
        assert!(!root.join(ACTIVE).exists());
        assert!(root.join(ACTIVATED).exists());
        assert!(root.join(PENDING).exists());
        assert_eq!(
            fs::read(root.join("desktop-state.json")).unwrap(),
            b"legacy"
        );
        let recovered = acquire(Some(root.clone())).unwrap();
        assert_eq!(recovered.diagnostic(), None);
        assert_ne!(
            recovered.path(DataFile::Desktop),
            Some(root.join("desktop-state.json"))
        );
    }

    #[test]
    fn cleanup_failure_after_commit_does_not_replay_new_saves() {
        let temp = Temp::new();
        let root = &temp.0;
        let directory = acquire(Some(root.clone())).unwrap();
        directory.prepare_import(candidate()).unwrap();
        drop(directory);
        resolve_locked_with(
            root,
            |path, bytes| crate::atomic_file::replace(path, bytes, None),
            crate::atomic_file::replace,
            |_path| Err(io::Error::other("injected cleanup failure")),
        )
        .unwrap();
        assert!(root.join(ACTIVE).exists());
        assert!(root.join(PENDING).exists());
        let pointer = read_pointer(root).unwrap().unwrap();
        let reminders = set_dir(root, &pointer.set_id).join(DataFile::Reminders.name());
        let mut data: serde_json::Value =
            serde_json::from_slice(&fs::read(&reminders).unwrap()).unwrap();
        data["paused"] = serde_json::json!(true);
        fs::write(&reminders, serde_json::to_vec(&data).unwrap()).unwrap();
        let restarted = acquire(Some(root.clone())).unwrap();
        assert_eq!(restarted.diagnostic(), None);
        assert_eq!(restarted.path(DataFile::Reminders), Some(reminders.clone()));
        assert!(!root.join(PENDING).exists());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&fs::read(reminders).unwrap()).unwrap()
                ["paused"],
            true
        );
    }

    #[test]
    fn record_seen_then_not_found_during_read_is_protected() {
        let temp = Temp::new();
        let path = temp.0.join(ACTIVE);
        fs::write(&path, b"seen before read").unwrap();
        let result =
            read_record_with_reader::<Pointer>(&path, "activePointerInvalid", |_path, _limit| {
                Err(io::Error::from(io::ErrorKind::NotFound))
            });
        assert!(matches!(result, Err("activePointerInvalid")));
    }

    #[test]
    fn activation_write_failure_keeps_pending_for_retry_without_legacy_loss() {
        let temp = Temp::new();
        let root = &temp.0;
        fs::write(root.join("desktop-state.json"), b"legacy").unwrap();
        let directory = acquire(Some(root.clone())).unwrap();
        directory.prepare_import(candidate()).unwrap();
        drop(directory);
        assert_eq!(
            resolve_locked_with(
                root,
                |_path, _bytes| Err("injected marker failure".into()),
                crate::atomic_file::replace,
                |_path| Ok(())
            ),
            Err("activationMarkerWriteFailed")
        );
        assert!(!root.join(ACTIVATED).exists());
        assert!(!root.join(ACTIVE).exists());
        assert!(root.join(PENDING).exists());
        assert_eq!(
            fs::read(root.join("desktop-state.json")).unwrap(),
            b"legacy"
        );
        let restarted = acquire(Some(root.clone())).unwrap();
        assert_eq!(restarted.diagnostic(), None);
        assert!(root.join(ACTIVATED).exists());
        assert!(root.join(ACTIVE).exists());
    }
}
