//! Focus IPC publishes committed events to the shared presentation coordinator.
use super::{
    model::{Command, Error, Time},
    service::{Change, Service, Snapshot},
    store::Store,
};
use crate::data_directory::{DataDirectory, DataFile};
use serde::{Deserialize, Deserializer};
use std::time::Instant;
use tauri::{App, AppHandle, Emitter, Manager, Runtime, WebviewWindow};
struct NativeFocus {
    service: Service,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum Operation {
    Start {
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },
    Pause {},
    Resume {},
    EndEarly {},
    Abandon {},
    View {},
    DismissFeedback {},
}
pub struct CommandInput(Result<Operation, Error>);
impl<'de> Deserialize<'de> for CommandInput {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(Self(serde_json::from_value(value).map_err(|_| Error {
            code: "invalidCommand",
        })))
    }
}
impl From<Operation> for Command {
    fn from(operation: Operation) -> Self {
        match operation {
            Operation::Start { duration_ms } => Self::Start { duration_ms },
            Operation::Pause {} => Self::Pause,
            Operation::Resume {} => Self::Resume,
            Operation::EndEarly {} => Self::EndEarly,
            Operation::Abandon {} => Self::Abandon,
            Operation::View {} => Self::Tick,
            Operation::DismissFeedback {} => Self::DismissFeedback,
        }
    }
}
pub fn setup<R: Runtime>(app: &mut App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let path = app.state::<DataDirectory>().path(DataFile::Focus);
    let handle = app.handle().clone();
    let started = Instant::now();
    let service = Service::start(
        Store::new(path),
        move || {
            Ok(Time {
                utc_ms: chrono::Utc::now().timestamp_millis(),
                monotonic_ms: u64::try_from(started.elapsed().as_millis()).map_err(|_| Error {
                    code: "invalidTime",
                })?,
            })
        },
        move |change| {
            let queued = handle.clone();
            let queued_at = Instant::now();
            let queued_utc = chrono::Utc::now().timestamp_millis();
            let _ = handle.run_on_main_thread(move || {
                if let Some(native) = queued.try_state::<NativeFocus>() {
                    let snapshot = native.service.snapshot();
                    if !snapshot.stopped {
                        crate::reminders::ui::focus_changed(
                            &queued,
                            &change,
                            queued_at.elapsed().as_secs() < 10
                                && (0..10_000).contains(
                                    &(chrono::Utc::now().timestamp_millis() - queued_utc),
                                ),
                        );
                        let _ = queued.emit("focus-changed", change);
                    }
                }
            });
        },
    )
    .map_err(|error| std::io::Error::other(error.code))?;
    app.manage(NativeFocus { service });
    Ok(())
}
pub fn stop<R: Runtime>(app: &AppHandle<R>) {
    if let Some(native) = app.try_state::<NativeFocus>() {
        native.service.stop();
    }
}
pub(crate) fn current<R: Runtime>(app: &AppHandle<R>) -> Option<Change> {
    app.try_state::<NativeFocus>().map(|n| n.service.current())
}
pub(crate) fn export_bytes<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<u8>, Error> {
    app.try_state::<NativeFocus>()
        .ok_or(Error {
            code: "workerUnavailable",
        })?
        .service
        .export_bytes()
}
fn allowed(label: &str) -> Result<(), Error> {
    if matches!(label, "main" | "settings") {
        Ok(())
    } else {
        Err(Error {
            code: "invalidWindow",
        })
    }
}
#[tauri::command]
pub fn get_focus<R: Runtime>(app: AppHandle<R>, window: WebviewWindow<R>) -> Result<Change, Error> {
    allowed(window.label())?;
    Ok(app
        .try_state::<NativeFocus>()
        .ok_or(Error {
            code: "workerUnavailable",
        })?
        .service
        .current())
}
#[tauri::command]
pub async fn focus_command<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    command: CommandInput,
) -> Result<Snapshot, Error> {
    allowed(window.label())?;
    let command = command.0?;
    let receiver = app
        .try_state::<NativeFocus>()
        .ok_or(Error {
            code: "workerUnavailable",
        })?
        .service
        .command(command.into())?;
    tauri::async_runtime::spawn_blocking(move || {
        receiver.recv().map_err(|_| Error {
            code: "workerUnavailable",
        })?
    })
    .await
    .map_err(|_| Error {
        code: "workerUnavailable",
    })?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ipc_rejects_unknown_fields_commands_and_unrelated_windows() {
        for json in [
            r#"{"type":"start","durationMs":"60000"}"#,
            r#"{"type":"view","taskName":"private"}"#,
            r#"{"type":"tick"}"#,
        ] {
            assert_eq!(
                serde_json::from_str::<CommandInput>(json)
                    .unwrap()
                    .0
                    .err()
                    .unwrap()
                    .code,
                "invalidCommand"
            );
        }
        assert!(
            serde_json::from_str::<CommandInput>(r#"{"type":"start","durationMs":60000}"#)
                .unwrap()
                .0
                .is_ok()
        );
        assert!(allowed("main").is_ok());
        assert!(allowed("settings").is_ok());
        assert_eq!(allowed("reminder").unwrap_err().code, "invalidWindow");
    }
}
