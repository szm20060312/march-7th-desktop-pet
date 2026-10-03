use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

// Shared mechanics only. Callers validate their own schema before replacing.
pub fn replace(path: &Path, bytes: &[u8], previous: Option<&[u8]>) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or("configuration has no parent directory")?;
    fs::create_dir_all(parent).map_err(|e| format!("create configuration directory: {e}"))?;
    let pending = TempFile::write(parent, bytes)
        .map_err(|e| format!("write temporary configuration: {e}"))?;
    if let Some(previous) = previous {
        let backup = TempFile::write(parent, previous).map_err(|e| format!("write backup: {e}"))?;
        fs::rename(&backup.0, path.with_extension("json.bak"))
            .map_err(|e| format!("replace backup: {e}"))?;
    }
    // Windows MoveFileExW(REPLACE_EXISTING), Unix rename: never remove first.
    fs::rename(&pending.0, path).map_err(|e| format!("replace configuration: {e}"))?;
    Ok(())
}
struct TempFile(PathBuf);
impl TempFile {
    fn write(directory: &Path, bytes: &[u8]) -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = directory.join(format!(
                ".app-config-{}-{}.tmp",
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
