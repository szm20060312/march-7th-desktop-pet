//! Std-only ownership of the participating process lock. Kept independent of
//! data-set parsing so the existing real-process regression can compile it.
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    path::PathBuf,
};

pub enum LockedRoot {
    Ready { root: PathBuf, lock: File },
    Unavailable(&'static str),
}
#[derive(Debug)]
pub struct AlreadyRunning;

pub fn acquire(root: Option<PathBuf>) -> Result<LockedRoot, AlreadyRunning> {
    let unavailable = |code| Ok(LockedRoot::Unavailable(code));
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
        Ok(()) => Ok(LockedRoot::Ready { root, lock }),
        Err(TryLockError::WouldBlock) => Err(AlreadyRunning),
        Err(TryLockError::Error(_)) => unavailable("directoryLockFailed"),
    }
}
