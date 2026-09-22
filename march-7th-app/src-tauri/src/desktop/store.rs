use super::geometry::Placement;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    pub placement: Placement,
}
impl Config {
    pub fn new(placement: Placement) -> Self {
        Self {
            version: 1,
            placement,
        }
    }
}
pub struct Store {
    path: PathBuf,
    pub writable: bool,
}
impl Store {
    pub fn load(path: PathBuf) -> (Self, Option<Placement>, Option<String>) {
        let mut store = Self {
            path,
            writable: true,
        };
        match fs::read(&store.path) {
            Ok(bytes) => match parse(&bytes) {
                Ok(config) => (store, Some(config.placement), None),
                Err(error) => {
                    store.writable = false;
                    (store, None, Some(error))
                }
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => (store, None, None),
            Err(error) => (store, None, Some(format!("read configuration: {error}"))),
        }
    }
    pub fn save(&mut self, placement: &Placement) -> Result<(), String> {
        if !self.writable {
            return Err("configuration is read-only for this session".into());
        }
        if !placement.x.is_finite() || !placement.y.is_finite() {
            return Err("nonfinite placement cannot be saved".into());
        }
        let bytes = serde_json::to_vec_pretty(&Config::new(placement.clone()))
            .map_err(|_| "serialize configuration failed")?;
        let parent = self
            .path
            .parent()
            .ok_or("configuration has no parent directory")?;
        fs::create_dir_all(parent).map_err(|e| format!("create configuration directory: {e}"))?;
        let previous = match fs::read(&self.path) {
            Ok(bytes) => {
                if let Err(error) = parse(&bytes) {
                    self.writable = false;
                    return Err(error);
                }
                Some(bytes)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(format!("read previous configuration: {error}")),
        };
        let pending = TempFile::write(parent, &bytes)
            .map_err(|e| format!("write temporary configuration: {e}"))?;
        if let Some(previous) = previous {
            let backup =
                TempFile::write(parent, &previous).map_err(|e| format!("write backup: {e}"))?;
            fs::rename(&backup.0, self.path.with_extension("json.bak"))
                .map_err(|e| format!("replace backup: {e}"))?;
        }
        // On Windows std::fs::rename uses MoveFileExW(REPLACE_EXISTING); on Unix
        // rename replaces atomically. Never remove the destination first.
        fs::rename(&pending.0, &self.path).map_err(|e| format!("replace configuration: {e}"))?;
        Ok(())
    }
}

fn parse(bytes: &[u8]) -> Result<Config, String> {
    #[derive(Deserialize)]
    struct Header {
        version: u32,
    }
    fn json_error(error: serde_json::Error) -> String {
        format!(
            "invalid configuration JSON at line {}, column {} ({:?})",
            error.line(),
            error.column(),
            error.classify()
        )
    }
    let header: Header = serde_json::from_slice(bytes).map_err(json_error)?;
    if header.version != 1 {
        return Err(format!(
            "unsupported desktop configuration version {}",
            header.version
        ));
    }
    serde_json::from_slice(bytes).map_err(json_error)
}

struct TempFile(PathBuf);
impl TempFile {
    fn write(directory: &Path, bytes: &[u8]) -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = directory.join(format!(
                ".desktop-state-{}-{}.tmp",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    let temp = Self(path);
                    let result = file.write_all(bytes).and_then(|_| file.sync_all());
                    drop(file);
                    result?;
                    return Ok(temp);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }
}
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "march7-store-{}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn file(&self) -> PathBuf {
            self.0.join("desktop-state.json")
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn placement(x: f64) -> Placement {
        Placement {
            monitor_name: Some("test".into()),
            x,
            y: 20.0,
        }
    }

    #[test]
    fn missing_file_is_normal_and_round_trips_only_placement() {
        let temp = Temp::new();
        let (mut store, loaded, error) = Store::load(temp.file());
        assert!(store.writable);
        assert!(loaded.is_none());
        assert!(error.is_none());
        store.save(&placement(10.0)).unwrap();
        let (_, loaded, error) = Store::load(temp.file());
        assert_eq!(loaded, Some(placement(10.0)));
        assert!(error.is_none());
    }
    #[test]
    fn corrupt_and_future_configs_are_preserved_read_only() {
        for raw in [
            "{broken",
            r#"{"version":2,"future":"keep"}"#,
            r#"{"version":1}"#,
        ] {
            let temp = Temp::new();
            fs::write(temp.file(), raw).unwrap();
            let (mut store, loaded, error) = Store::load(temp.file());
            assert!(!store.writable);
            assert!(loaded.is_none());
            assert!(error.is_some());
            assert!(store.save(&placement(1.0)).is_err());
            assert_eq!(fs::read_to_string(temp.file()).unwrap(), raw);
        }
    }
    #[test]
    fn replacement_retains_previous_valid_config_as_backup() {
        let temp = Temp::new();
        let (mut store, _, _) = Store::load(temp.file());
        store.save(&placement(10.0)).unwrap();
        let old = fs::read(temp.file()).unwrap();
        store.save(&placement(30.0)).unwrap();
        assert_eq!(
            fs::read(temp.0.join("desktop-state.json.bak")).unwrap(),
            old
        );
        assert_eq!(Store::load(temp.file()).1, Some(placement(30.0)));
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 2);
    }
    #[test]
    fn failed_backup_preserves_destination_and_later_save_recovers() {
        let temp = Temp::new();
        let (mut store, _, _) = Store::load(temp.file());
        store.save(&placement(10.0)).unwrap();
        let old = fs::read(temp.file()).unwrap();
        fs::create_dir(temp.0.join("desktop-state.json.bak")).unwrap();
        assert!(store.save(&placement(30.0)).is_err());
        assert_eq!(fs::read(temp.file()).unwrap(), old);
        assert!(store.writable);
        fs::remove_dir(temp.0.join("desktop-state.json.bak")).unwrap();
        store.save(&placement(30.0)).unwrap();
        assert_eq!(Store::load(temp.file()).1, Some(placement(30.0)));
    }
    #[test]
    fn refuses_invalid_values_and_external_future_replacement() {
        let temp = Temp::new();
        let (mut store, _, _) = Store::load(temp.file());
        assert!(store.save(&placement(f64::NAN)).is_err());
        assert!(!temp.file().exists());
        fs::write(temp.file(), r#"{"version":999}"#).unwrap();
        assert!(store.save(&placement(3.0)).is_err());
        assert!(!store.writable);
        assert_eq!(
            fs::read_to_string(temp.file()).unwrap(),
            r#"{"version":999}"#
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_destination_replace_failure_keeps_original_and_cleans_temps() {
        use std::os::windows::fs::OpenOptionsExt;
        let temp = Temp::new();
        let (mut store, _, _) = Store::load(temp.file());
        store.save(&placement(10.0)).unwrap();
        let old = fs::read(temp.file()).unwrap();
        // Deny delete sharing: Windows must reject the final atomic rename.
        let held = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(temp.file())
            .unwrap();
        assert!(store.save(&placement(30.0)).is_err());
        assert_eq!(fs::read(temp.file()).unwrap(), old);
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 2);
        drop(held);
        store.save(&placement(30.0)).unwrap();
        assert_eq!(Store::load(temp.file()).1, Some(placement(30.0)));
    }
}
