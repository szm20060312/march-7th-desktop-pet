//! Persistence port for the focus service. The selected v2 set must already exist.
use super::model::{Data, Error};
use std::{fs, io::Read, path::PathBuf};
const LIMIT: u64 = crate::data_directory::MAX_FOCUS_BYTES as u64;
pub trait Storage: Send {
    fn load(&mut self) -> Result<Vec<u8>, Error>;
    fn save(&mut self, bytes: &[u8]) -> Result<(), Error>;
}
pub struct Store {
    path: Option<PathBuf>,
    previous: Option<Vec<u8>>,
    blocked: bool,
}
impl Store {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            previous: None,
            blocked: false,
        }
    }
    fn read(&self) -> Result<Vec<u8>, Error> {
        let path = self.path.as_ref().ok_or(Error {
            code: "directoryUnavailable",
        })?;
        let metadata = fs::symlink_metadata(path).map_err(|_| Error { code: "readFailed" })?;
        if !metadata.is_file() || metadata.is_symlink() || metadata.len() > LIMIT {
            return Err(Error {
                code: "invalidFile",
            });
        }
        let mut bytes = Vec::new();
        fs::File::open(path)
            .and_then(|f| f.take(LIMIT + 1).read_to_end(&mut bytes))
            .map_err(|_| Error { code: "readFailed" })?;
        decode(&bytes)?;
        Ok(bytes)
    }
}
pub(crate) fn decode(bytes: &[u8]) -> Result<Data, Error> {
    if bytes.len() as u64 > LIMIT {
        return Err(Error {
            code: "invalidFile",
        });
    }
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| Error {
        code: "invalidFile",
    })?;
    match value.get("version").and_then(|v| v.as_u64()) {
        Some(1) => (),
        Some(_) => {
            return Err(Error {
                code: "unsupportedVersion",
            })
        }
        None => {
            return Err(Error {
                code: "invalidFile",
            })
        }
    }
    let data: Data = serde_json::from_value(value).map_err(|_| Error {
        code: "invalidFile",
    })?;
    data.validate()?;
    Ok(data)
}
impl Storage for Store {
    fn load(&mut self) -> Result<Vec<u8>, Error> {
        match self.read() {
            Ok(bytes) => {
                self.previous = Some(bytes.clone());
                Ok(bytes)
            }
            Err(error) => {
                self.blocked = true;
                Err(error)
            }
        }
    }
    fn save(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if self.blocked || self.previous.is_none() {
            return Err(Error { code: "readOnly" });
        }
        decode(bytes)?;
        let current = match self.read() {
            Ok(bytes) => bytes,
            Err(error) => {
                self.blocked = true;
                return Err(error);
            }
        };
        if self.previous.as_ref() != Some(&current) {
            self.blocked = true;
            return Err(Error {
                code: "fileChanged",
            });
        }
        let path = self.path.as_ref().ok_or(Error {
            code: "directoryUnavailable",
        })?;
        if fs::metadata(path)
            .map_err(|_| Error { code: "readFailed" })?
            .permissions()
            .readonly()
        {
            return Err(Error { code: "readOnly" });
        }
        crate::atomic_file::replace(path, bytes, Some(&current)).map_err(|_| Error {
            code: "writeFailed",
        })?;
        self.previous = Some(bytes.to_vec());
        Ok(())
    }
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    pub(crate) struct Temp(pub PathBuf);
    impl Temp {
        pub(crate) fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "march7-focus-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn missing_corrupt_future_oversized_and_unavailable_stores_never_get_defaults_written() {
        let temp = Temp::new();
        let path = temp.0.join("focus.json");
        let valid = serde_json::to_vec(&Data::default()).unwrap();
        let mut missing = Store::new(Some(path.clone()));
        assert!(missing.load().is_err());
        assert!(missing.save(&valid).is_err());
        assert!(!path.exists());
        for bytes in [
            b"invalid".to_vec(),
            br#"{"version":2}"#.to_vec(),
            vec![b' '; LIMIT as usize + 1],
        ] {
            fs::write(&path, &bytes).unwrap();
            let mut store = Store::new(Some(path.clone()));
            assert!(store.load().is_err());
            assert!(store.save(&valid).is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        assert_eq!(
            Store::new(None).load().unwrap_err().code,
            "directoryUnavailable"
        );
    }
    #[test]
    fn actual_atomic_failure_and_readonly_preserve_old_bytes_then_recover() {
        let temp = Temp::new();
        let path = temp.0.join("focus.json");
        let original = serde_json::to_vec(&Data::default()).unwrap();
        fs::write(&path, &original).unwrap();
        let mut store = Store::new(Some(path.clone()));
        store.load().unwrap();
        let next = br#"{"version":1,"session":{"status":"paused","duration_ms":60000,"remaining_ms":12000}}"#;
        let backup = path.with_extension("json.bak");
        fs::create_dir(&backup).unwrap();
        assert_eq!(store.save(next).unwrap_err().code, "writeFailed");
        assert_eq!(fs::read(&path).unwrap(), original);
        fs::remove_dir(&backup).unwrap();
        let permissions = fs::metadata(&path).unwrap().permissions();
        let mut readonly = permissions.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&path, readonly).unwrap();
        assert_eq!(store.save(next).unwrap_err().code, "readOnly");
        assert_eq!(fs::read(&path).unwrap(), original);
        fs::set_permissions(&path, permissions).unwrap();
        store.save(next).unwrap();
        assert_eq!(fs::read(&path).unwrap(), next);
        assert_eq!(fs::read(&backup).unwrap(), original);
        assert_eq!(Store::new(Some(path)).load().unwrap(), next);
    }
    #[test]
    fn runtime_external_replacement_is_not_overwritten_even_when_valid() {
        let temp = Temp::new();
        let path = temp.0.join("focus.json");
        let original = serde_json::to_vec(&Data::default()).unwrap();
        fs::write(&path, &original).unwrap();
        let mut store = Store::new(Some(path.clone()));
        store.load().unwrap();
        let changed = serde_json::to_vec_pretty(&Data::default()).unwrap();
        fs::write(&path, &changed).unwrap();
        assert_eq!(store.save(&original).unwrap_err().code, "fileChanged");
        assert_eq!(fs::read(&path).unwrap(), changed);
    }
}
