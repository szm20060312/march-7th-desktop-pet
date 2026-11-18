//! In-memory ring-buffered diagnostic log and debug-only inspection/trigger commands.
use chrono::Local;
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
};
use tauri::{AppHandle, Emitter, Runtime};

const MAX_LOGS: usize = 200;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub timestamp: String,
    pub category: String,
    pub message: String,
}

fn store() -> &'static Mutex<VecDeque<LogEntry>> {
    static LOGS: OnceLock<Mutex<VecDeque<LogEntry>>> = OnceLock::new();
    LOGS.get_or_init(|| Mutex::new(VecDeque::with_capacity(MAX_LOGS)))
}

pub fn record(category: &str, message: impl Into<String>) -> LogEntry {
    let entry = LogEntry {
        timestamp: Local::now().format("%H:%M:%S%.3f").to_string(),
        category: category.to_string(),
        message: message.into(),
    };
    let mut logs = store().lock().unwrap();
    if logs.len() >= MAX_LOGS {
        logs.pop_front();
    }
    logs.push_back(entry.clone());
    entry
}

pub fn emit<R: Runtime>(app: &AppHandle<R>, category: &str, message: impl Into<String>) {
    let entry = record(category, message);
    let _ = app.emit_to("settings", "debug-log", entry);
}

#[tauri::command]
pub fn get_debug_logs<R: Runtime>(
    window: tauri::WebviewWindow<R>,
) -> Result<Vec<LogEntry>, String> {
    if window.label() != "settings" {
        return Err("invalidWindow".into());
    }
    Ok(store().lock().unwrap().iter().cloned().collect())
}

#[tauri::command]
pub fn clear_debug_logs<R: Runtime>(window: tauri::WebviewWindow<R>) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("invalidWindow".into());
    }
    store().lock().unwrap().clear();
    Ok(())
}

#[tauri::command]
pub fn get_debug_state<R: Runtime>(
    app: AppHandle<R>,
    window: tauri::WebviewWindow<R>,
) -> Result<serde_json::Value, String> {
    if window.label() != "settings" {
        return Err("invalidWindow".into());
    }
    let character = crate::characters::native::snapshot(&app);
    let reminders = crate::reminders::native::snapshot(&app).ok();
    let focus = crate::focus::native::current(&app).map(|c| c.snapshot);
    Ok(serde_json::json!({
        "character": character,
        "reminders": reminders,
        "focus": focus,
    }))
}

#[tauri::command]
pub fn debug_trigger_pet_action<R: Runtime>(
    app: AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    action: String,
) -> Result<(), String> {
    if window.label() != "settings" {
        return Err("invalidWindow".into());
    }
    let allowed = [
        "wave",
        "jump",
        "offerWater",
        "endWater",
        "phrase",
        "reminderPrompt:water",
        "reminderPrompt:move",
        "reminderPrompt:eyes",
    ];
    if !allowed.contains(&action.as_str()) {
        return Err("invalidAction".into());
    }
    emit(&app, "debug", format!("trigger pet action `{action}`"));
    app.emit_to(
        "main",
        "debug-pet-action",
        serde_json::json!({ "action": action }),
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_caps_at_max_logs() {
        store().lock().unwrap().clear();
        for i in 0..(MAX_LOGS + 25) {
            record("test", format!("msg-{i}"));
        }
        let logs = store().lock().unwrap();
        assert_eq!(logs.len(), MAX_LOGS);
        assert_eq!(logs.front().unwrap().message, "msg-25");
        assert_eq!(
            logs.back().unwrap().message,
            format!("msg-{}", MAX_LOGS + 24)
        );
    }
}
