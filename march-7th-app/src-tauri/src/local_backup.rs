//! Explicit, single-file local export. The dialog never grants WebView file access.
use crate::{
    characters,
    data_directory::{backup_codec, export_snapshot, DataDirectory},
    desktop, reminders,
};
use serde::Serialize;
use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::Path,
    sync::{mpsc, Mutex},
};
use tauri::{AppHandle, Manager, Runtime, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, FilePath};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ticket {
    session: (u64, u64),
    sequence: u64,
}
type DialogReply = Result<(Ticket, Vec<u8>, Option<FilePath>), ExportError>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Dialog,
    Saving,
}
struct Active {
    ticket: Ticket,
    phase: Phase,
    cancel: Option<mpsc::Sender<DialogReply>>,
}
#[derive(Default)]
pub(crate) struct ExportGate {
    next: u64,
    active: Option<Active>,
}
impl ExportGate {
    fn begin(
        &mut self,
        session: (u64, u64),
        cancel: mpsc::Sender<DialogReply>,
    ) -> Result<Ticket, &'static str> {
        if self.active.is_some() {
            return Err("backupBusy");
        }
        self.next = self.next.wrapping_add(1);
        let ticket = Ticket {
            session,
            sequence: self.next,
        };
        self.active = Some(Active {
            ticket,
            phase: Phase::Dialog,
            cancel: Some(cancel),
        });
        Ok(ticket)
    }
    fn authorize(&mut self, ticket: Ticket, current: (u64, u64)) -> Result<(), &'static str> {
        let Some(active) = self.active.as_mut() else {
            return Err("backupStale");
        };
        if active.ticket != ticket || active.phase != Phase::Dialog {
            return Err("backupStale");
        }
        if ticket.session != current {
            self.active = None;
            return Err("backupStale");
        }
        active.phase = Phase::Saving;
        active.cancel = None;
        Ok(())
    }
    fn complete(&mut self, ticket: Ticket) {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.ticket == ticket && active.phase == Phase::Saving)
        {
            self.active = None;
        }
    }
    fn invalidate(&mut self) {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.phase == Phase::Dialog)
        {
            if let Some(active) = self.active.take() {
                if let Some(cancel) = active.cancel {
                    let _ = cancel.send(Err(ExportError::new("backupStale")));
                }
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportError {
    code: &'static str,
}
impl ExportError {
    fn new(code: &'static str) -> Self {
        Self { code }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportOutcome {
    Saved,
    Cancelled,
}

fn write_complete(writer: &mut impl Write, bytes: &[u8]) -> Result<(), &'static str> {
    writer.write_all(bytes).map_err(|_| "backupWriteFailed")
}
fn write_and_sync<W: Write>(
    writer: &mut W,
    bytes: &[u8],
    sync: impl FnOnce(&mut W) -> io::Result<()>,
) -> Result<(), &'static str> {
    write_complete(writer, bytes)?;
    sync(writer).map_err(|_| "backupWriteFailed")
}
fn save_new(path: &Path, bytes: &[u8]) -> Result<(), &'static str> {
    let mut file = match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err("backupAlreadyExists")
        }
        Err(_) => return Err("backupWriteFailed"),
    };
    // Leave an incomplete new file on failure. Removing by path after the
    // handle closes could delete another process's replacement of that path.
    // The UI explicitly tells the user to remove any incomplete file.
    write_and_sync(&mut file, bytes, |file| file.sync_all())
}

pub(crate) fn invalidate<R: Runtime>(app: &AppHandle<R>) {
    if let Some(state) = app.try_state::<Mutex<ExportGate>>() {
        state.lock().unwrap().invalidate();
    }
}
fn current_session<R: Runtime>(
    app: &AppHandle<R>,
    window: &WebviewWindow<R>,
) -> Result<(u64, u64), ExportError> {
    if window.label() != "settings" || !window.is_visible().unwrap_or(false) {
        return Err(ExportError::new("backupStale"));
    }
    reminders::ui::settings_export_session(app).ok_or(ExportError::new("backupStale"))
}
fn prepare<R: Runtime>(
    app: &AppHandle<R>,
    window: &WebviewWindow<R>,
    cancel: mpsc::Sender<DialogReply>,
) -> Result<(Ticket, Vec<u8>), ExportError> {
    let session = current_session(app, window)?;
    if app.state::<DataDirectory>().root().is_none() {
        return Err(ExportError::new("backupUnavailable"));
    }
    let character =
        characters::native::snapshot(app).ok_or(ExportError::new("backupUnavailable"))?;
    let reminder =
        reminders::native::snapshot(app).map_err(|_| ExportError::new("backupUnavailable"))?;
    let position =
        desktop::export_current(app).map_err(|_| ExportError::new("backupUnavailable"))?;
    let files = export_snapshot::capture(Some(&character), &reminder, position)
        .map_err(|_| ExportError::new("backupUnavailable"))?;
    let bytes = backup_codec::encode(files, chrono::Utc::now().timestamp_millis())
        .map_err(|_| ExportError::new("backupUnavailable"))?;
    let ticket = app
        .state::<Mutex<ExportGate>>()
        .lock()
        .unwrap()
        .begin(session, cancel)
        .map_err(ExportError::new)?;
    Ok((ticket, bytes))
}

/// UI action: freeze qualified bytes before opening the non-blocking native picker.
#[tauri::command]
pub async fn export_local_backup<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
) -> Result<ExportOutcome, ExportError> {
    if window.label() != "settings" {
        return Err(ExportError::new("invalidWindow"));
    }
    let (send, receive) = std::sync::mpsc::channel();
    let handle = app.clone();
    let dialog_window = window.clone();
    app.run_on_main_thread(
        move || match prepare(&handle, &dialog_window, send.clone()) {
            Err(error) => {
                let _ = send.send(Err(error));
            }
            Ok((ticket, bytes)) => {
                let name = format!(
                    "march7-backup-{}.json",
                    chrono::Utc::now().format("%Y-%m-%d")
                );
                handle
                    .dialog()
                    .file()
                    .set_parent(&dialog_window)
                    .set_title("导出本地备份")
                    .set_file_name(name)
                    .add_filter("JSON 备份", &["json"])
                    .save_file(move |path| {
                        let _ = send.send(Ok((ticket, bytes, path)));
                    });
            }
        },
    )
    .map_err(|_| ExportError::new("backupUnavailable"))?;
    let selected = tauri::async_runtime::spawn_blocking(move || receive.recv())
        .await
        .map_err(|_| ExportError::new("backupUnavailable"))?
        .map_err(|_| ExportError::new("backupUnavailable"))??;
    let (ticket, bytes, path) = selected;
    let (send, receive) = std::sync::mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let current = current_session(&handle, &window).unwrap_or((u64::MAX, u64::MAX));
        let result = handle
            .state::<Mutex<ExportGate>>()
            .lock()
            .unwrap()
            .authorize(ticket, current)
            .map_err(ExportError::new);
        let _ = send.send(result);
    })
    .map_err(|_| ExportError::new("backupUnavailable"))?;
    tauri::async_runtime::spawn_blocking(move || receive.recv())
        .await
        .map_err(|_| ExportError::new("backupUnavailable"))?
        .map_err(|_| ExportError::new("backupUnavailable"))??;
    let result = if let Some(path) = path {
        match path.into_path() {
            Ok(path) => {
                match tauri::async_runtime::spawn_blocking(move || save_new(&path, &bytes)).await {
                    Ok(write) => write
                        .map_err(ExportError::new)
                        .map(|_| ExportOutcome::Saved),
                    Err(_) => Err(ExportError::new("backupWriteFailed")),
                }
            }
            Err(_) => Err(ExportError::new("backupWriteFailed")),
        }
    } else {
        Ok(ExportOutcome::Cancelled)
    };
    app.state::<Mutex<ExportGate>>()
        .lock()
        .unwrap()
        .complete(ticket);
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs, io,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Temp(std::path::PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "march7-export-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn file(&self) -> std::path::PathBuf {
            self.0.join("backup.json")
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn saver_never_overwrites_an_existing_path() {
        let temp = Temp::new();
        fs::write(temp.file(), b"original").unwrap();
        assert_eq!(
            save_new(&temp.file(), b"replacement"),
            Err("backupAlreadyExists")
        );
        assert_eq!(fs::read(temp.file()).unwrap(), b"original");
    }
    #[test]
    fn saver_commits_complete_bytes_only_after_sync() {
        let temp = Temp::new();
        assert_eq!(save_new(&temp.file(), b"complete"), Ok(()));
        assert_eq!(fs::read(temp.file()).unwrap(), b"complete");
    }
    #[test]
    fn saver_reports_short_write_and_sync_failures() {
        struct Short(bool);
        impl io::Write for Short {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                if self.0 {
                    Ok(0)
                } else {
                    self.0 = true;
                    Ok(b.len().min(1))
                }
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert_eq!(
            write_complete(&mut Short(false), b"many"),
            Err("backupWriteFailed")
        );
        struct FailingSync;
        impl io::Write for FailingSync {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                Ok(b.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert_eq!(
            write_and_sync(&mut FailingSync, b"bytes", |_| Err(io::Error::other(
                "sync"
            ))),
            Err("backupWriteFailed")
        );
    }
    #[test]
    fn gate_rejects_repeated_or_old_operations_and_cancel_unblocks_waiter() {
        let mut gate = ExportGate::default();
        let (sender, receiver) = mpsc::channel();
        let first = gate.begin((2, 4), sender.clone()).unwrap();
        assert_eq!(gate.begin((2, 4), sender.clone()), Err("backupBusy"));
        gate.invalidate();
        assert_eq!(receiver.recv().unwrap().err().unwrap().code, "backupStale");
        let second = gate.begin((2, 5), sender).unwrap();
        assert_eq!(gate.authorize(first, (2, 5)), Err("backupStale"));
        assert_eq!(gate.authorize(second, (2, 5)), Ok(()));
        gate.complete(first);
        assert!(gate.active.is_some());
        gate.complete(second);
        assert!(gate.active.is_none());
    }
    #[test]
    fn saving_phase_blocks_another_export_until_a_blocked_writer_finishes() {
        let gate = std::sync::Arc::new(Mutex::new(ExportGate::default()));
        let (sender, _) = mpsc::channel();
        let ticket = gate.lock().unwrap().begin((4, 8), sender.clone()).unwrap();
        gate.lock().unwrap().authorize(ticket, (4, 8)).unwrap();
        let (started_send, started_receive) = mpsc::channel();
        let (continue_send, continue_receive) = mpsc::channel();
        struct Blocked(mpsc::Sender<()>, mpsc::Receiver<()>);
        impl io::Write for Blocked {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.0.send(()).unwrap();
                self.1.recv().unwrap();
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let writing = std::thread::spawn(move || {
            write_and_sync(
                &mut Blocked(started_send, continue_receive),
                b"backup",
                |_| Ok(()),
            )
        });
        started_receive.recv().unwrap();
        gate.lock().unwrap().invalidate(); // closing while already saving cannot permit a second write
        assert_eq!(
            gate.lock().unwrap().begin((4, 9), sender.clone()),
            Err("backupBusy")
        );
        continue_send.send(()).unwrap();
        assert_eq!(writing.join().unwrap(), Ok(()));
        gate.lock().unwrap().complete(ticket);
        assert!(gate.lock().unwrap().begin((4, 9), sender).is_ok());
    }
}
