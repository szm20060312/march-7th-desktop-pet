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
        #[serde(default, rename = "taskName")]
        task_name: Option<String>,
    },
    CompleteTask {},
    AbandonTask {},
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
            Operation::Start {
                duration_ms,
                task_name,
            } => Self::StartWithTask {
                duration_ms,
                task_name: task_name.unwrap_or_default(),
            },
            Operation::CompleteTask {} => Self::CompleteTask,
            Operation::AbandonTask {} => Self::AbandonTask,
            Operation::Pause {} => Self::Pause,
            Operation::Resume {} => Self::Resume,
            Operation::EndEarly {} => Self::EndEarly,
            Operation::Abandon {} => Self::Abandon,
            Operation::View {} => Self::Tick,
            Operation::DismissFeedback {} => Self::DismissFeedback,
        }
    }
}
// No task text crosses into the character window, including on error or restoration.
fn task_response(
    change: &Change,
    current: &Snapshot,
    timely: bool,
    visible: bool,
) -> Option<serde_json::Value> {
    if !timely
        || !visible
        || current.stopped
        || change.error.is_some()
        || current.revision != change.snapshot.revision
    {
        return None;
    }
    match change.task_response? {
        super::model::TaskStatus::Active => None,
        status => Some(serde_json::json!({"revision":change.snapshot.revision,"status":status})),
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
                        let timely = queued_at.elapsed().as_secs() < 10
                            && (0..10_000)
                                .contains(&(chrono::Utc::now().timestamp_millis() - queued_utc));
                        crate::reminders::ui::focus_changed(&queued, &change, timely);
                        let visible = queued
                            .get_webview_window("main")
                            .is_some_and(|w| w.is_visible().unwrap_or(false));
                        if let Some(response) = task_response(&change, &snapshot, timely, visible) {
                            let _ = queued.emit_to("main", "task-response", response);
                        }
                        let _ = queued.emit_to("settings", "focus-changed", change);
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
    if label == "settings" {
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
    fn task_projection_is_content_free_and_suppresses_stale_hidden_restored_and_failed_edges() {
        use super::super::model::{Data, TaskStatus};
        let data=super::super::store::decode(br#"{"version":2,"session":{"status":"paused","duration_ms":60000,"remaining_ms":30000},"task":{"name":"PRIVATE_TASK","status":"completed"}}"#).unwrap();
        let snapshot = Snapshot {
            revision: 8,
            data: Some(data),
            error: None,
            stopped: false,
        };
        let mut change = Change {
            snapshot: snapshot.clone(),
            completed_now: false,
            task_response: Some(TaskStatus::Completed),
            error: None,
        };
        let response = task_response(&change, &snapshot, true, true).unwrap();
        assert_eq!(
            response,
            serde_json::json!({"revision":8,"status":"completed"})
        );
        assert!(!response.to_string().contains("PRIVATE_TASK"));
        assert!(task_response(&change, &snapshot, false, true).is_none());
        assert!(task_response(&change, &snapshot, true, false).is_none());
        let newer = Snapshot {
            revision: 9,
            data: Some(Data::default()),
            ..snapshot.clone()
        };
        assert!(task_response(&change, &newer, true, true).is_none());
        change.error = Some("writeFailed");
        assert!(task_response(&change, &snapshot, true, true).is_none());
        change.error = None;
        change.task_response = None;
        assert!(task_response(&change, &snapshot, true, true).is_none());
    }
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
        assert_eq!(allowed("main").unwrap_err().code, "invalidWindow");
        assert!(allowed("settings").is_ok());
        assert_eq!(allowed("reminder").unwrap_err().code, "invalidWindow");
    }
}
