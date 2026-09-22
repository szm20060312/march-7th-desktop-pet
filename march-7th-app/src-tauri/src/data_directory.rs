//! One participating writer per configuration root. The OS releases the lock
//! when this owner (or the process) goes away; the lock file is never a PID file.
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    path::{Path, PathBuf},
};

pub enum DataFile {
    Desktop,
    Characters,
    Reminders,
}

enum Access {
    Ready { root: PathBuf, _lock: File },
    Unavailable(&'static str),
}

// Deliberately not Clone: managed application state owns the only lock handle.
pub struct DataDirectory(Access);

#[derive(Debug)]
pub struct AlreadyRunning;

pub fn acquire(root: Option<PathBuf>) -> Result<DataDirectory, AlreadyRunning> {
    let unavailable = |code| Ok(DataDirectory(Access::Unavailable(code)));
    let Some(root) = root else {
        return unavailable("directoryUnavailable");
    };
    if fs::create_dir_all(&root).is_err() {
        return unavailable("directoryCreateFailed");
    }
    let lock = match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("instance.lock"))
    {
        Ok(lock) => lock,
        Err(_) => return unavailable("directoryLockOpenFailed"),
    };
    match lock.try_lock() {
        Ok(()) => Ok(DataDirectory(Access::Ready { root, _lock: lock })),
        Err(TryLockError::WouldBlock) => Err(AlreadyRunning),
        Err(TryLockError::Error(_)) => unavailable("directoryLockFailed"),
    }
}

impl DataDirectory {
    pub fn root(&self) -> Option<&Path> {
        match &self.0 {
            Access::Ready { root, .. } => Some(root),
            Access::Unavailable(_) => None,
        }
    }

    pub fn path(&self, file: DataFile) -> Option<PathBuf> {
        self.root().map(|root| {
            root.join(match file {
                DataFile::Desktop => "desktop-state.json",
                DataFile::Characters => "character-preferences.json",
                DataFile::Reminders => "reminders.json",
            })
        })
    }

    pub fn diagnostic(&self) -> Option<&'static str> {
        match &self.0 {
            Access::Ready { .. } => None,
            Access::Unavailable(code) => Some(code),
        }
    }
}
