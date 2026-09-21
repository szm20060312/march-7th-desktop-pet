use super::{
    model::{Command, Error, Response, Time},
    service::{Change, Service, Snapshot},
    store::Store,
};
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
    let path = app
        .path()
        .app_config_dir()
        .ok()
        .map(|p| p.join("reminders.json"));
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
    if native.service.snapshot().stopped {
        return;
    }
    let revision = change.snapshot.revision;
    if app.emit("reminders-changed", change.snapshot).is_err() {
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
pub fn get_reminders<R: Runtime>(app: AppHandle<R>) -> Result<Snapshot, Error> {
    app.try_state::<NativeReminders>()
        .map(|n| n.service.snapshot())
        .ok_or(Error::new("workerUnavailable"))
}
#[tauri::command]
pub async fn reminder_command<R: Runtime>(
    app: AppHandle<R>,
    command: CommandInput,
) -> Result<Snapshot, Error> {
    let command = command.0?;
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
