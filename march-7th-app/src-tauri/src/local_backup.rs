//! Explicit, single-file local export. The dialog never grants WebView file access.
use crate::{
    characters,
    data_directory::{backup_codec, export_snapshot, DataDirectory},
    desktop,
    import_session::{ImportConfirmation, ImportPreview, ImportSession},
    reminders,
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
pub(crate) type ImportDialogReply = Result<(u64, Option<FilePath>), ExportError>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Dialog,
    Saving,
}
struct Active {
    ticket: Ticket,
    phase: Phase,
    dialog_returned: bool,
    stale: bool,
    cancel: Option<mpsc::Sender<DialogReply>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImportPhase {
    Dialog,
    Reading,
    Preparing,
}
struct ImportActive {
    ticket: u64,
    session: (u64, u64),
    phase: ImportPhase,
    dialog_returned: bool,
    stale: bool,
    cancel: Option<mpsc::Sender<ImportDialogReply>>,
}
#[derive(Default)]
pub(crate) struct LocalDataGate {
    next: u64,
    active: Option<Active>,
    import_active: Option<ImportActive>,
    import_session: ImportSession,
}
impl LocalDataGate {
    fn begin(
        &mut self,
        session: (u64, u64),
        cancel: mpsc::Sender<DialogReply>,
    ) -> Result<Ticket, &'static str> {
        if self.active.is_some() || self.import_active.is_some() {
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
            dialog_returned: false,
            stale: false,
            cancel: Some(cancel),
        });
        Ok(ticket)
    }
    fn authorize(&mut self, ticket: Ticket, current: (u64, u64)) -> Result<(), &'static str> {
        let Some(active) = self.active.as_mut() else {
            return Err("backupStale");
        };
        if active.ticket != ticket || active.phase != Phase::Dialog || active.stale {
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
    fn export_dialog_returned(&mut self, ticket: Ticket) {
        if let Some(active) = self.active.as_mut() {
            if active.ticket == ticket && active.phase == Phase::Dialog {
                active.dialog_returned = true;
                if active.stale {
                    self.active = None;
                }
            }
        }
    }
    fn invalidate(&mut self) {
        self.import_session.invalidate();
        if let Some(active) = self.import_active.as_mut() {
            if active.phase == ImportPhase::Dialog {
                active.stale = true;
                if let Some(cancel) = active.cancel.take() {
                    let _ = cancel.send(Err(ExportError::new("backupStale")));
                }
                if active.dialog_returned {
                    self.import_active = None;
                }
            } else if active.phase == ImportPhase::Reading {
                self.import_active = None;
            }
        }
        if let Some(active) = self.active.as_mut() {
            if active.phase == Phase::Dialog {
                active.stale = true;
                if let Some(cancel) = active.cancel.take() {
                    let _ = cancel.send(Err(ExportError::new("backupStale")));
                }
                if active.dialog_returned {
                    self.active = None;
                }
            }
        }
    }
    fn invalidate_destroyed(&mut self) {
        self.invalidate();
        // A destroyed owner cannot receive a new visible dialog. The Dialog
        // plugin exposes no cancellation handle and may never call back after
        // owner destruction; release only its stale dialog lease here.
        if self
            .import_active
            .as_ref()
            .is_some_and(|active| active.phase == ImportPhase::Dialog && active.stale)
        {
            self.import_active = None;
        }
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.phase == Phase::Dialog && active.stale)
        {
            self.active = None;
        }
    }
    pub(crate) fn begin_import(
        &mut self,
        session: (u64, u64),
        cancel: mpsc::Sender<ImportDialogReply>,
    ) -> Result<u64, &'static str> {
        if self.active.is_some() || self.import_active.is_some() {
            return Err("backupBusy");
        }
        let ticket = self.import_session.begin_selection(session)?;
        self.import_active = Some(ImportActive {
            ticket,
            session,
            phase: ImportPhase::Dialog,
            dialog_returned: false,
            stale: false,
            cancel: Some(cancel),
        });
        Ok(ticket)
    }
    pub(crate) fn authorize_import(
        &mut self,
        ticket: u64,
        current: (u64, u64),
    ) -> Result<(), &'static str> {
        let Some(active) = self.import_active.as_ref() else {
            return Err("backupStale");
        };
        if active.ticket != ticket || active.phase != ImportPhase::Dialog || active.stale {
            return Err("backupStale");
        }
        if active.session != current {
            self.import_active = None;
            self.import_session.invalidate();
            return Err("backupStale");
        }
        let active = self.import_active.as_mut().unwrap();
        active.phase = ImportPhase::Reading;
        active.cancel = None;
        Ok(())
    }
    pub(crate) fn resolve_import_dialog<T>(
        &mut self,
        ticket: u64,
        current: Option<(u64, u64)>,
        selected: Option<T>,
    ) -> Result<Option<T>, &'static str> {
        let Some(current) = current else {
            // A temporarily unavailable settings session must still consume
            // this returned dialog's lease. Never touch a newer ticket.
            if self.import_active.as_ref().is_some_and(|active| {
                active.ticket == ticket && active.phase == ImportPhase::Dialog
            }) {
                self.cancel_import()?;
            }
            return Err("backupStale");
        };
        self.authorize_import(ticket, current)?;
        if selected.is_none() {
            self.cancel_import()?;
        }
        Ok(selected)
    }
    pub(crate) fn import_dialog_returned(&mut self, ticket: u64) {
        if let Some(active) = self.import_active.as_mut() {
            if active.ticket == ticket && active.phase == ImportPhase::Dialog {
                active.dialog_returned = true;
                if active.stale {
                    self.import_active = None;
                }
            }
        }
    }
    pub(crate) fn finish_import(
        &mut self,
        ticket: u64,
        current: (u64, u64),
        loaded: Result<crate::data_directory::backup_codec::DecodedBackup, &'static str>,
    ) -> Result<ImportPreview, &'static str> {
        if !self
            .import_active
            .as_ref()
            .is_some_and(|a| a.ticket == ticket && a.phase == ImportPhase::Reading)
        {
            return Err("backupStale");
        }
        self.import_active = None;
        self.import_session
            .finish_selection(ticket, current, loaded)
    }
    pub(crate) fn cancel_import(&mut self) -> Result<(), &'static str> {
        if self
            .import_active
            .as_ref()
            .is_some_and(|a| a.phase == ImportPhase::Preparing)
        {
            return Err("backupBusy");
        }
        if let Some(active) = self.import_active.take() {
            if let Some(cancel) = active.cancel {
                let _ = cancel.send(Err(ExportError::new("backupStale")));
            }
        }
        self.import_session.cancel();
        Ok(())
    }
    pub(crate) fn begin_import_prepare(
        &mut self,
        ticket: u64,
        current: (u64, u64),
    ) -> Result<crate::data_directory::ImportFiles, &'static str> {
        if self.active.is_some() || self.import_active.is_some() {
            return Err("backupBusy");
        }
        let files = self.import_session.begin_confirmation(ticket, current)?;
        self.import_active = Some(ImportActive {
            ticket,
            session: current,
            phase: ImportPhase::Preparing,
            dialog_returned: true,
            stale: false,
            cancel: None,
        });
        Ok(files)
    }
    pub(crate) fn finish_import_prepare(
        &mut self,
        ticket: u64,
        result: Result<String, &'static str>,
    ) -> Result<ImportConfirmation, &'static str> {
        if !self
            .import_active
            .as_ref()
            .is_some_and(|a| a.ticket == ticket && a.phase == ImportPhase::Preparing)
        {
            return Err("backupStale");
        }
        self.import_active = None;
        match self
            .import_session
            .finish_confirmation(ticket, result.clone())
        {
            Err("backupStale") => result.map(|transaction_id| ImportConfirmation {
                transaction_id,
                restart_required: true,
            }),
            other => other,
        }
    }
    pub(crate) fn import_preview(&self, current: (u64, u64)) -> Option<ImportPreview> {
        self.import_session.current_preview(current)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportError {
    code: &'static str,
}
impl ExportError {
    pub(crate) fn new(code: &'static str) -> Self {
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
    if let Some(state) = app.try_state::<Mutex<LocalDataGate>>() {
        state.lock().unwrap().invalidate();
    }
}
pub(crate) fn invalidate_destroyed<R: Runtime>(app: &AppHandle<R>) {
    if let Some(state) = app.try_state::<Mutex<LocalDataGate>>() {
        state.lock().unwrap().invalidate_destroyed();
    }
}
pub(crate) fn current_session<R: Runtime>(
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
        .state::<Mutex<LocalDataGate>>()
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
                let callback_handle = handle.clone();
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
                        callback_handle
                            .state::<Mutex<LocalDataGate>>()
                            .lock()
                            .unwrap()
                            .export_dialog_returned(ticket);
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
            .state::<Mutex<LocalDataGate>>()
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
    app.state::<Mutex<LocalDataGate>>()
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
        let mut gate = LocalDataGate::default();
        let (sender, receiver) = mpsc::channel();
        let first = gate.begin((2, 4), sender.clone()).unwrap();
        assert_eq!(gate.begin((2, 4), sender.clone()), Err("backupBusy"));
        gate.invalidate();
        assert_eq!(receiver.recv().unwrap().err().unwrap().code, "backupStale");
        assert_eq!(gate.begin((2, 5), sender.clone()), Err("backupBusy"));
        gate.export_dialog_returned(first);
        let second = gate.begin((2, 5), sender).unwrap();
        assert_eq!(gate.authorize(first, (2, 5)), Err("backupStale"));
        assert_eq!(gate.authorize(second, (2, 5)), Ok(()));
        gate.complete(first);
        assert!(gate.active.is_some());
        gate.complete(second);
        assert!(gate.active.is_none());
    }
    #[test]
    fn one_gate_serializes_export_and_import_dialogs_and_rejects_late_selection() {
        let mut gate = LocalDataGate::default();
        let (export_send, _) = mpsc::channel();
        let (import_send, import_receive) = mpsc::channel();
        let export = gate.begin((1, 2), export_send.clone()).unwrap();
        assert_eq!(
            gate.begin_import((1, 2), import_send.clone()),
            Err("backupBusy")
        );
        gate.authorize(export, (1, 2)).unwrap();
        assert_eq!(
            gate.begin_import((1, 2), import_send.clone()),
            Err("backupBusy")
        );
        gate.complete(export);
        let ticket = gate.begin_import((1, 2), import_send).unwrap();
        assert_eq!(gate.begin((1, 2), export_send), Err("backupBusy"));
        assert_eq!(
            gate.begin_import((1, 2), mpsc::channel().0),
            Err("backupBusy")
        );
        gate.invalidate();
        assert_eq!(
            import_receive.recv().unwrap().err().unwrap().code,
            "backupStale"
        );
        let (blocked_send, _) = mpsc::channel();
        assert_eq!(gate.begin((1, 3), blocked_send), Err("backupBusy"));
        gate.import_dialog_returned(ticket);
        assert_eq!(gate.authorize_import(ticket, (1, 2)), Err("backupStale"));
        assert_eq!(
            gate.finish_import(ticket, (1, 2), Err("backupInvalid"))
                .err(),
            Some("backupStale")
        );
        let (next_send, _) = mpsc::channel();
        let next = gate.begin_import((1, 3), next_send).unwrap();
        assert_ne!(ticket, next);
        assert_eq!(gate.authorize_import(next, (1, 2)), Err("backupStale"));
        gate.cancel_import().unwrap();
    }
    #[test]
    fn destroyed_owner_releases_stale_dialog_lease_but_old_callback_cannot_touch_new_session() {
        let mut gate = LocalDataGate::default();
        let (old_send, old_receive) = mpsc::channel();
        let old = gate.begin_import((2, 4), old_send).unwrap();
        gate.invalidate_destroyed();
        assert_eq!(
            old_receive.recv().unwrap().err().unwrap().code,
            "backupStale"
        );
        let (new_send, _) = mpsc::channel();
        let new = gate.begin_import((3, 5), new_send).unwrap();
        gate.import_dialog_returned(old);
        assert_eq!(gate.authorize_import(old, (3, 5)), Err("backupStale"));
        gate.import_dialog_returned(new);
        assert_eq!(gate.authorize_import(new, (3, 5)), Ok(()));
        gate.invalidate_destroyed();
        assert_eq!(
            gate.finish_import(new, (3, 5), Err("backupInvalid")).err(),
            Some("backupStale")
        );
        let (export_old_send, export_old_receive) = mpsc::channel();
        let export_old = gate.begin((4, 6), export_old_send).unwrap();
        gate.invalidate_destroyed();
        assert_eq!(
            export_old_receive.recv().unwrap().err().unwrap().code,
            "backupStale"
        );
        let (export_new_send, _) = mpsc::channel();
        let export_new = gate.begin((5, 7), export_new_send).unwrap();
        gate.export_dialog_returned(export_old);
        assert_eq!(gate.authorize(export_old, (5, 7)), Err("backupStale"));
        gate.export_dialog_returned(export_new);
        assert_eq!(gate.authorize(export_new, (5, 7)), Ok(()));
    }
    #[test]
    fn confirmed_prepare_finishes_after_window_close_without_reopening_old_preview() {
        use crate::data_directory::{acquire, backup_codec, ImportFiles};
        let temp = Temp::new();
        let directory = acquire(Some(temp.0.clone())).unwrap();
        assert_eq!(directory.has_pending_import(), Ok(false));
        let files = ImportFiles {
            desktop: None,
            characters: br#"{"version":1,"selectedCharacterId":"march-7th"}"#.to_vec(),
            reminders: br#"{"version":1,"settings":{"items":[{"id":"water","enabled":false,"intervalMinutes":60},{"id":"move","enabled":false,"intervalMinutes":60},{"id":"eyes","enabled":false,"intervalMinutes":30}],"activeHours":{"kind":"daily","start":540,"end":1320},"snoozeMinutes":10},"progress":[{"id":"water","nextDueAt":null,"pending":false,"autoHandled":false},{"id":"move","nextDueAt":null,"pending":false,"autoHandled":false},{"id":"eyes","nextDueAt":null,"pending":false,"autoHandled":false}],"paused":false,"quiet":null,"snoozePending":false}"#.to_vec(),
        };
        let decoded = backup_codec::decode(&backup_codec::encode(files, 100).unwrap()).unwrap();
        let mut gate = LocalDataGate::default();
        let (sender, _) = mpsc::channel();
        let ticket = gate.begin_import((5, 6), sender).unwrap();
        gate.authorize_import(ticket, (5, 6)).unwrap();
        gate.finish_import(ticket, (5, 6), Ok(decoded)).unwrap();
        let candidate = gate.begin_import_prepare(ticket, (5, 6)).unwrap();
        gate.invalidate();
        let (export_send, _) = mpsc::channel();
        assert_eq!(gate.begin((5, 7), export_send), Err("backupBusy"));
        let receipt = gate
            .finish_import_prepare(ticket, directory.prepare_import(candidate))
            .unwrap();
        assert!(receipt.restart_required);
        assert!(gate.import_preview((5, 6)).is_none());
        assert!(temp.0.join("pending-import.json").exists());
        assert_eq!(directory.has_pending_import(), Ok(true));
    }
    #[test]
    fn saving_phase_blocks_another_export_until_a_blocked_writer_finishes() {
        let gate = std::sync::Arc::new(Mutex::new(LocalDataGate::default()));
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
    #[test]
    fn returned_import_picker_with_unavailable_session_releases_shared_gate() {
        // A reflowing settings window can temporarily have no export session,
        // whether the native picker returned a path or the user cancelled it.
        for selected in [None, Some(())] {
            let mut gate = LocalDataGate::default();
            let (send, _) = mpsc::channel();
            let ticket = gate.begin_import((2, 4), send).unwrap();
            gate.import_dialog_returned(ticket);
            assert_eq!(
                gate.resolve_import_dialog(ticket, None, selected),
                Err("backupStale")
            );
            let (export_send, _) = mpsc::channel();
            let export = gate.begin((2, 5), export_send).unwrap();
            gate.authorize(export, (2, 5)).unwrap();
            gate.complete(export);
            let (import_send, _) = mpsc::channel();
            let next = gate.begin_import((2, 5), import_send).unwrap();
            assert_ne!(next, ticket);
            assert_eq!(
                gate.resolve_import_dialog(ticket, None, Option::<()>::None),
                Err("backupStale")
            );
            assert_eq!(gate.authorize_import(next, (2, 5)), Ok(()));
        }
    }
    #[test]
    fn returned_cancelled_import_picker_with_valid_session_releases_shared_gate() {
        let mut gate = LocalDataGate::default();
        let (send, _) = mpsc::channel();
        let ticket = gate.begin_import((3, 7), send).unwrap();
        gate.import_dialog_returned(ticket);
        assert_eq!(
            gate.resolve_import_dialog(ticket, Some((3, 7)), Option::<()>::None),
            Ok(None)
        );
        let (send, _) = mpsc::channel();
        assert!(gate.begin_import((3, 7), send).is_ok());
    }
}
