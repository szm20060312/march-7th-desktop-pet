use super::model::{Data, Error};
use serde::Serialize;
use std::{fs, io::ErrorKind, path::PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SaveStatus {
    Loading,
    Default,
    Saved,
    Unsaved,
    ReadOnly,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Persistence {
    pub status: SaveStatus,
    pub code: Option<&'static str>,
}
impl Persistence {
    pub fn new(status: SaveStatus, code: Option<&'static str>) -> Self {
        Self { status, code }
    }
    pub fn protected(&self) -> bool {
        self.status == SaveStatus::ReadOnly
    }
}
pub trait Storage: Send {
    fn load(&mut self) -> (Data, Persistence);
    fn save(&mut self, data: &Data) -> Persistence;
}
pub struct Store {
    path: Option<PathBuf>,
    blocked: Option<&'static str>,
}
impl Store {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            blocked: None,
        }
    }
}
// v1 is the first format. No invented migration path; all schema validation stays here/model.
pub(crate) fn decode(bytes: &[u8]) -> Result<Data, Error> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| Error::new("invalidFile"))?;
    let version = value
        .get("version")
        .and_then(|v| v.as_u64())
        .ok_or(Error::new("invalidFile"))?;
    if version != 1 {
        return Err(Error::new("unsupportedVersion"));
    }
    let data: Data = serde_json::from_value(value).map_err(|_| Error::new("invalidFile"))?;
    data.validate().map_err(|_| Error::new("invalidFile"))?;
    Ok(data)
}
pub(crate) fn validate_import(bytes: &[u8]) -> Result<(), Error> {
    decode(bytes).map(|_| ())
}
impl Storage for Store {
    fn load(&mut self) -> (Data, Persistence) {
        let result = self
            .path
            .as_ref()
            .ok_or(Error::new("directoryUnavailable"))
            .and_then(|p| match fs::read(p) {
                Ok(bytes) => decode(&bytes).map(Some),
                Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
                Err(_) => Err(Error::new("readFailed")),
            });
        match result {
            Ok(Some(data)) => (data, Persistence::new(SaveStatus::Saved, None)),
            Ok(None) => (Data::default(), Persistence::new(SaveStatus::Default, None)),
            Err(error) => {
                self.blocked = Some(error.code);
                (
                    Data::default(),
                    Persistence::new(SaveStatus::ReadOnly, Some(error.code)),
                )
            }
        }
    }
    fn save(&mut self, data: &Data) -> Persistence {
        if let Some(code) = self.blocked {
            return Persistence::new(SaveStatus::ReadOnly, Some(code));
        }
        if data.validate().is_err() {
            return Persistence::new(SaveStatus::Unsaved, Some("invalidState"));
        }
        let Some(path) = &self.path else {
            self.blocked = Some("directoryUnavailable");
            return Persistence::new(SaveStatus::ReadOnly, self.blocked);
        };
        let previous = match fs::read(path) {
            Ok(bytes) => match decode(&bytes) {
                Ok(_) => Some(bytes),
                Err(e) => {
                    self.blocked = Some(e.code);
                    return Persistence::new(SaveStatus::ReadOnly, self.blocked);
                }
            },
            Err(e) if e.kind() == ErrorKind::NotFound => None,
            Err(_) => {
                self.blocked = Some("readFailed");
                return Persistence::new(SaveStatus::ReadOnly, self.blocked);
            }
        };
        let Ok(bytes) = serde_json::to_vec_pretty(data) else {
            return Persistence::new(SaveStatus::Unsaved, Some("encodeFailed"));
        };
        match crate::atomic_file::replace(path, &bytes, previous.as_deref()) {
            Ok(()) => Persistence::new(SaveStatus::Saved, None),
            Err(_) => Persistence::new(SaveStatus::Unsaved, Some("writeFailed")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_directory_keeps_reminders_disabled_and_read_only() {
        use crate::data_directory::{acquire, DataFile};
        let directory = acquire(None).unwrap();
        let mut store = Store::new(directory.path(DataFile::Reminders));
        let (data, persistence) = store.load();
        assert!(data.settings.items.iter().all(|item| !item.enabled));
        assert_eq!(persistence.status, SaveStatus::ReadOnly);
        assert_eq!(persistence.code, Some("directoryUnavailable"));
        assert_eq!(store.save(&data), persistence);
    }
    #[test]
    fn fixround1_all_day_unknown_fields_protect_real_file_on_load_and_save() {
        let temp = Temp::new();
        let path = temp.0.join("reminders.json");
        let mut value = serde_json::to_value(Data::default()).unwrap();
        value["settings"]["activeHours"] =
            serde_json::json!({"kind":"allDay","start":540,"end":1320,"unexpected":"retain-me"});
        let bytes = serde_json::to_vec(&value).unwrap();
        fs::write(&path, &bytes).unwrap();
        let mut store = Store::new(Some(path.clone()));
        let (data, persistence) = store.load();
        assert_eq!(persistence.status, SaveStatus::ReadOnly);
        assert_eq!(persistence.code, Some("invalidFile"));
        assert!(data.settings.items.iter().all(|s| !s.enabled));
        assert_eq!(store.save(&Data::default()).status, SaveStatus::ReadOnly);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fs::write(&path, serde_json::to_vec(&Data::default()).unwrap()).unwrap();
        let mut store = Store::new(Some(path.clone()));
        assert_eq!(store.load().1.status, SaveStatus::Saved);
        fs::write(&path, &bytes).unwrap();
        assert_eq!(store.save(&Data::default()).status, SaveStatus::ReadOnly);
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    use std::sync::atomic::{AtomicU64, Ordering};
    pub struct Temp(pub PathBuf);
    impl Temp {
        pub fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let p = std::env::temp_dir().join(format!(
                "march-reminders-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn missing_roundtrip_backup_and_cross_state_validation() {
        let temp = Temp::new();
        let path = temp.0.join("reminders.json");
        let mut store = Store::new(Some(path.clone()));
        assert_eq!(store.load().1.status, SaveStatus::Default);
        assert!(!path.exists());
        let data = Data::default();
        assert_eq!(store.save(&data).status, SaveStatus::Saved);
        let old = fs::read(&path).unwrap();
        let mut changed = data.clone();
        changed.paused = true;
        assert_eq!(store.save(&changed).status, SaveStatus::Saved);
        assert_eq!(fs::read(path.with_extension("json.bak")).unwrap(), old);
        assert_eq!(Store::new(Some(path.clone())).load().0, changed);
        changed.progress[0].pending = true;
        assert_eq!(store.save(&changed).status, SaveStatus::Unsaved);
        assert!(Store::new(Some(path)).load().0.paused);
    }
    #[test]
    fn invalid_future_duplicate_and_read_failure_are_preserved_and_locked() {
        let temp = Temp::new();
        let path = temp.0.join("reminders.json");
        let mut duplicate = Data::default();
        duplicate.progress[1].id = super::super::model::Id::Water;
        for bytes in [
            b"broken".to_vec(),
            br#"{"version":2}"#.to_vec(),
            serde_json::to_vec(&duplicate).unwrap(),
        ] {
            fs::write(&path, &bytes).unwrap();
            let mut store = Store::new(Some(path.clone()));
            let (data, status) = store.load();
            assert_eq!(status.status, SaveStatus::ReadOnly);
            assert!(data.settings.items.iter().all(|i| !i.enabled));
            assert_eq!(store.save(&Data::default()).status, SaveStatus::ReadOnly);
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        let mut store = Store::new(Some(temp.0.clone()));
        assert_eq!(store.load().1.code, Some("readFailed"));
        assert_eq!(Store::new(None).load().1.code, Some("directoryUnavailable"));
    }
    #[test]
    fn real_atomic_failure_preserves_old_file_and_can_recover() {
        let temp = Temp::new();
        let path = temp.0.join("reminders.json");
        let mut store = Store::new(Some(path.clone()));
        store.load();
        let data = Data::default();
        assert_eq!(store.save(&data).status, SaveStatus::Saved);
        let original = fs::read(&path).unwrap();
        // A directory at the backup destination makes replace fail on both platforms.
        let backup = path.with_extension("json.bak");
        fs::create_dir(&backup).unwrap();
        let mut changed = data.clone();
        changed.paused = true;
        assert_eq!(store.save(&changed).status, SaveStatus::Unsaved);
        assert_eq!(fs::read(&path).unwrap(), original);
        fs::remove_dir(&backup).unwrap();
        assert_eq!(store.save(&changed).status, SaveStatus::Saved);
        assert_eq!(store.load().0, changed);
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 2);
    }
    #[test]
    fn runtime_external_corruption_is_not_overwritten() {
        let temp = Temp::new();
        let path = temp.0.join("reminders.json");
        let mut store = Store::new(Some(path.clone()));
        store.load();
        assert_eq!(store.save(&Data::default()).status, SaveStatus::Saved);
        fs::write(&path, b"invalid").unwrap();
        assert_eq!(store.save(&Data::default()).status, SaveStatus::ReadOnly);
        assert_eq!(fs::read(&path).unwrap(), b"invalid");
    }
}
