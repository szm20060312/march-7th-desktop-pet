use super::{
    model::{Command, Error, Response, Time},
    service::{Change, Service, Snapshot},
    store::Store,
};
use crate::data_directory::{DataDirectory, DataFile};
use chrono::{Local, Timelike};
use serde::{Deserialize, Deserializer, Serialize};
use std::time::Instant;
use tauri::{App, AppHandle, Emitter, Manager, Runtime};

struct NativeReminders {
    service: Service,
}
// Keep malformed command payloads inside the typed error contract instead of
// letting Tauri turn serde's rejection into a framework-specific string.
pub struct CommandInput(Result<Command, Error>);
impl<'de> Deserialize<'de> for CommandInput {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(Self(
            serde_json::from_value(value).map_err(|_| Error::new("invalidCommand")),
        ))
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResponseEvent {
    revision: u64,
    #[serde(flatten)]
    response: Response,
}
pub fn setup<R: Runtime>(app: &mut App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let path = app.state::<DataDirectory>().path(DataFile::Reminders);
    let handle = app.handle().clone();
    let started = Instant::now();
    let service = Service::start(
        Store::new(path),
        move || sample(started),
        move |change| {
            let handle = handle.clone();
            let queued = handle.clone();
            if handle
                .run_on_main_thread(move || emit_change(&queued, change))
                .is_err()
            {
                eprintln!("Reminder notification queue unavailable");
            }
        },
    )
    .map_err(|e| std::io::Error::other(e.code))?;
    app.manage(NativeReminders { service });
    Ok(())
}
fn sample(started: Instant) -> Result<Time, Error> {
    let local = Local::now();
    let time = Time {
        utc_ms: local.timestamp_millis(),
        local_minute: (local.hour() * 60 + local.minute()) as i32,
        monotonic_ms: u64::try_from(started.elapsed().as_millis())
            .map_err(|_| Error::new("invalidTime"))?,
    };
    time.validate()?;
    Ok(time)
}
fn emit_change<R: Runtime>(app: &AppHandle<R>, change: Change) {
    let Some(native) = app.try_state::<NativeReminders>() else {
        return;
    };
    // All native emissions and ExitRequested handling execute on the main thread.
    // Recheck here, not just when the worker queues this callback.
    let latest = native.service.snapshot();
    if latest.stopped {
        return;
    }
    let revision = change.snapshot.revision;
    super::ui::reconcile(app);
    if app
        .emit_to("settings", "reminders-changed", latest)
        .is_err()
    {
        eprintln!("Reminder snapshot event unavailable");
    }
    if let Some(response) = change.response {
        if app
            .emit("reminder-response", ResponseEvent { revision, response })
            .is_err()
        {
            eprintln!("Reminder response event unavailable");
        }
    }
}
pub fn stop<R: Runtime>(app: &AppHandle<R>) {
    if let Some(native) = app.try_state::<NativeReminders>() {
        native.service.stop();
    }
}
#[tauri::command]
pub fn get_reminders<R: Runtime>(
    app: AppHandle<R>,
    window: tauri::WebviewWindow<R>,
) -> Result<serde_json::Value, Error> {
    if window.label() == "reminder" {
        super::ui::bubble_snapshot(&app)
    } else {
        serde_json::to_value(snapshot(&app)?).map_err(|_| Error::new("invalidState"))
    }
}
pub(crate) fn snapshot<R: Runtime>(app: &AppHandle<R>) -> Result<Snapshot, Error> {
    app.try_state::<NativeReminders>()
        .map(|n| n.service.snapshot())
        .ok_or(Error::new("workerUnavailable"))
}
#[tauri::command]
pub async fn open_reminder_choices<R: Runtime>(
    app: AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    presentation_id: u64,
) -> Result<(), Error> {
    if window.label() != "main" || presentation_id == 0 || presentation_id > super::model::MAX_SAFE
    {
        return Err(Error::new("invalidWindow"));
    }
    let source = on_ui(&app, move |app| {
        super::ui::choice_source(app, presentation_id)
    })
    .await?;
    dispatch(
        &app,
        Command::ExtendChoices {
            presentation_id: source,
        },
    )
    .await?;
    on_ui(&app, move |app| {
        super::ui::open_choices(app, presentation_id)
    })
    .await
}
#[tauri::command]
pub async fn reminder_command<R: Runtime>(
    app: AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    command: CommandInput,
) -> Result<serde_json::Value, Error> {
    let command = command.0?;
    if !command_allowed(window.label(), &command) {
        return Err(Error::new("invalidWindow"));
    }
    if window.label() == "reminder" {
        let mapped = on_ui(&app, move |app| super::ui::map_bubble_command(app, command)).await?;
        if let Some(command) = mapped {
            dispatch(&app, command).await?;
        }
        on_ui(&app, super::ui::bubble_snapshot).await
    } else {
        serde_json::to_value(dispatch(&app, command).await?).map_err(|_| Error::new("invalidState"))
    }
}
async fn on_ui<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    action: impl FnOnce(&AppHandle<R>) -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    let (send, receive) = std::sync::mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = send.send(action(&handle));
    })
    .map_err(|_| Error::new("workerUnavailable"))?;
    tauri::async_runtime::spawn_blocking(move || {
        receive
            .recv()
            .map_err(|_| Error::new("workerUnavailable"))?
    })
    .await
    .map_err(|_| Error::new("workerUnavailable"))?
}
fn command_allowed(label: &str, command: &Command) -> bool {
    match label {
        "settings" => matches!(
            command,
            Command::UpdateSettings { .. } | Command::SetPaused { .. }
        ),
        "reminder" => matches!(
            command,
            Command::Complete { .. }
                | Command::SnoozeAll {}
                | Command::Dismiss { .. }
                | Command::ShowPending {}
        ),
        _ => false,
    }
}
pub(crate) async fn dispatch<R: Runtime>(
    app: &AppHandle<R>,
    command: Command,
) -> Result<Snapshot, Error> {
    let receiver = app
        .try_state::<NativeReminders>()
        .ok_or(Error::new("workerUnavailable"))?
        .service
        .command(command)?;
    tauri::async_runtime::spawn_blocking(move || {
        receiver
            .recv()
            .map_err(|_| Error::new("workerUnavailable"))?
    })
    .await
    .map_err(|_| Error::new("workerUnavailable"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pages_only_receive_their_own_reminder_operations() {
        assert!(command_allowed(
            "settings",
            &Command::SetPaused { paused: true }
        ));
        assert!(!command_allowed(
            "reminder",
            &Command::SetPaused { paused: true }
        ));
        assert!(command_allowed(
            "reminder",
            &Command::Dismiss { presentation_id: 1 }
        ));
        assert!(!command_allowed(
            "settings",
            &Command::Dismiss { presentation_id: 1 }
        ));
        for label in ["main", "settings", "reminder", "unknown"] {
            assert_eq!(
                command_allowed(label, &Command::ShowPending {}),
                label == "reminder"
            );
        }
        assert!(!command_allowed("main", &Command::SnoozeAll {}));
    }
    #[test]
    fn fixround1_nested_all_day_command_rejects_extra_fields() {
        let mut settings =
            serde_json::to_value(crate::reminders::model::Settings::default()).unwrap();
        settings["activeHours"] =
            serde_json::json!({"kind":"allDay","start":540,"unexpected":"retain-me"});
        let input: CommandInput = serde_json::from_value(
            serde_json::json!({"type":"updateSettings","settings":settings}),
        )
        .unwrap();
        assert_eq!(input.0.unwrap_err().code, "invalidCommand");
    }
    #[test]
    fn malformed_wire_commands_return_structured_errors() {
        for raw in [
            "null",
            r#"{"type":"unknown"}"#,
            r#"{"type":"showPending","utcMs":0}"#,
            r#"{"type":"complete","id":"unknown"}"#,
        ] {
            let input: CommandInput = serde_json::from_str(raw).unwrap();
            assert_eq!(
                serde_json::to_value(input.0.unwrap_err()).unwrap(),
                serde_json::json!({"code":"invalidCommand"})
            );
        }
        let input: CommandInput = serde_json::from_str(r#"{"type":"showPending"}"#).unwrap();
        assert!(matches!(input.0, Ok(Command::ShowPending {})));
    }
}
