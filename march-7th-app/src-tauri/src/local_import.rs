//! Explicit native picker and one-use frozen preview. No external path or bytes reach the WebView.
use crate::{
    data_directory::DataDirectory,
    import_session::{read_selected, ImportConfirmation, ImportPreview},
    local_backup::{current_session, ExportError, ImportDialogReply, LocalDataGate},
};
use std::sync::{mpsc, Mutex};
use tauri::{AppHandle, Manager, Runtime, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

fn gate<R: Runtime>(app: &AppHandle<R>) -> tauri::State<'_, Mutex<LocalDataGate>> {
    app.state::<Mutex<LocalDataGate>>()
}

/// Schedule a native dialog without blocking the WebView or exposing a file capability.
#[tauri::command]
pub async fn select_local_backup<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
) -> Result<Option<ImportPreview>, ExportError> {
    if window.label() != "settings" {
        return Err(ExportError::new("invalidWindow"));
    }
    let pending_app = app.clone();
    let pending = tauri::async_runtime::spawn_blocking(move || {
        pending_app.state::<DataDirectory>().has_pending_import()
    })
    .await
    .map_err(|_| ExportError::new("backupUnavailable"))?
    .map_err(ExportError::new)?;
    if pending {
        return Err(ExportError::new("pendingImportExists"));
    }
    let (send, receive) = mpsc::channel::<ImportDialogReply>();
    let handle = app.clone();
    let dialog_window = window.clone();
    app.run_on_main_thread(move || {
        let selected = current_session(&handle, &dialog_window)
            .map_err(|_| ExportError::new("backupStale"))
            .and_then(|session| {
                gate(&handle)
                    .lock()
                    .unwrap()
                    .begin_import(session, send.clone())
                    .map_err(ExportError::new)
            });
        match selected {
            Ok(ticket) => {
                let callback_handle = handle.clone();
                handle
                    .dialog()
                    .file()
                    .set_parent(&dialog_window)
                    .set_title("导入本地备份")
                    .add_filter("JSON 备份", &["json"])
                    .pick_file(move |path| {
                        let _ = send.send(Ok((ticket, path)));
                        callback_handle
                            .state::<Mutex<LocalDataGate>>()
                            .lock()
                            .unwrap()
                            .import_dialog_returned(ticket);
                    });
            }
            Err(error) => {
                let _ = send.send(Err(error));
            }
        }
    })
    .map_err(|_| ExportError::new("backupUnavailable"))?;
    let (ticket, selected) = tauri::async_runtime::spawn_blocking(move || receive.recv())
        .await
        .map_err(|_| ExportError::new("backupUnavailable"))?
        .map_err(|_| ExportError::new("backupUnavailable"))??;
    let current = current_session_on_main(&app, &window).await.ok();
    let selected = gate(&app)
        .lock()
        .unwrap()
        .resolve_import_dialog(ticket, current, selected)
        .map_err(ExportError::new)?;
    let Some(selected) = selected else {
        return Ok(None);
    };
    let decoded = match selected.into_path() {
        Ok(path) => tauri::async_runtime::spawn_blocking(move || read_selected(&path))
            .await
            .unwrap_or(Err("backupReadFailed")),
        Err(_) => Err("backupReadFailed"),
    };
    let now = current_session_on_main(&app, &window)
        .await
        .unwrap_or((u64::MAX, u64::MAX));
    // Both the native generation and the frozen selection ticket must still match.
    gate(&app)
        .lock()
        .unwrap()
        .finish_import(ticket, now, decoded)
        .map(Some)
        .map_err(ExportError::new)
}

async fn current_session_on_main<R: Runtime>(
    app: &AppHandle<R>,
    window: &WebviewWindow<R>,
) -> Result<(u64, u64), ExportError> {
    let (send, receive) = mpsc::channel();
    let handle = app.clone();
    let window = window.clone();
    app.run_on_main_thread(move || {
        let _ = send.send(current_session(&handle, &window));
    })
    .map_err(|_| ExportError::new("backupUnavailable"))?;
    tauri::async_runtime::spawn_blocking(move || receive.recv())
        .await
        .map_err(|_| ExportError::new("backupUnavailable"))?
        .map_err(|_| ExportError::new("backupUnavailable"))?
}

/// Confirm only a still-visible, frozen preview. The B2 write runs off the UI thread.
#[tauri::command]
pub async fn confirm_local_backup<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    ticket: u64,
) -> Result<ImportConfirmation, ExportError> {
    if window.label() != "settings" {
        return Err(ExportError::new("invalidWindow"));
    }
    let session = current_session_on_main(&app, &window).await?;
    let files = gate(&app)
        .lock()
        .unwrap()
        .begin_import_prepare(ticket, session)
        .map_err(ExportError::new)?;
    let handle = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        handle.state::<DataDirectory>().prepare_import(files)
    })
    .await
    .unwrap_or(Err("backupUnavailable"));
    gate(&app)
        .lock()
        .unwrap()
        .finish_import_prepare(ticket, result)
        .map_err(ExportError::new)
}

#[tauri::command]
pub async fn cancel_local_backup<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    ticket: u64,
) -> Result<(), ExportError> {
    if window.label() != "settings" {
        return Err(ExportError::new("invalidWindow"));
    }
    let session = current_session_on_main(&app, &window).await?;
    let bound = gate(&app);
    let mut state = bound.lock().unwrap();
    if state
        .import_preview(session)
        .is_none_or(|p| p.ticket != ticket)
    {
        return Err(ExportError::new("backupStale"));
    }
    state.cancel_import().map_err(ExportError::new)
}
