//! Main-thread bridge for the one native bubble; all arbitration lives in Bubble.
use super::*;
use chrono::Timelike;
pub(super) fn refresh<R: Runtime>(app: &AppHandle<R>) {
    refresh_completion(app, None);
}
pub(crate) fn focus_changed<R: Runtime>(
    app: &AppHandle<R>,
    change: &crate::focus::service::Change,
    timely: bool,
) {
    let completion = (timely && change.completed_now && change.error.is_none())
        .then_some(change.snapshot.revision);
    refresh_completion(app, completion);
    reconcile(app);
}
fn refresh_completion<R: Runtime>(app: &AppHandle<R>, completion: Option<u64>) {
    let Some(ui) = app.try_state::<Ui<R>>() else {
        return;
    };
    if !running(&ui) {
        return;
    }
    if let Ok(snapshot) = native::snapshot(app) {
        let focus = crate::focus::native::current(app);
        let local = chrono::Local::now();
        let mut s = ui.state.lock().unwrap();
        let old = s.reminder.ticket();
        let changed = s.bubble.update(
            &snapshot,
            focus.as_ref(),
            completion,
            crate::reminders::model::Time {
                monotonic_ms: ui.started.elapsed().as_millis().min(u64::MAX as u128) as u64,
                utc_ms: local.timestamp_millis(),
                local_minute: (local.hour() * 60 + local.minute()) as i32,
            },
        );
        let revision = s.bubble.revision;
        let presentation = s.bubble.selected.map(|(id, _)| id);
        let ended = s.announced_auto.filter(|id| {
            !character_visible(app) || s.bubble.automatic_prompt(&snapshot, *id).is_none()
        });
        if ended.is_some() {
            s.announced_auto = None;
        }
        s.reminder.update(revision, presentation, snapshot.stopped);
        let hide = old != s.reminder.ticket();
        if hide {
            s.reminder_settle = None;
            s.reminder_attempts = 0;
        }
        let projection = changed.then(|| s.bubble.snapshot(&snapshot));
        drop(s);
        if hide {
            if let Err(error) = hide_reminder(app) {
                fail(app, "hideFailed", error);
            }
        }
        if let Some(Ok(projection)) = projection {
            if let Err(error) = app.emit_to("reminder", "reminders-changed", projection) {
                fail(app, "presentationFailed", error);
            }
        }
        if let Some(presentation_id) = ended {
            if let Err(error) = app.emit_to(
                "main",
                "reminder-prompt-ended",
                serde_json::json!({"presentationId": presentation_id}),
            ) {
                eprintln!("Reminder character prompt end unavailable: {error}");
            }
        }
    }
}
// Main-thread serialization is shared with ready/close/native-show. IO remains on its worker.
pub(crate) fn bubble_snapshot<R: Runtime>(app: &AppHandle<R>) -> Result<serde_json::Value, Error> {
    refresh(app);
    let raw = native::snapshot(app)?;
    let ui = app.try_state::<Ui<R>>().ok_or(Error::new("stopped"))?;
    let result = ui.state.lock().unwrap().bubble.snapshot(&raw);
    result
}
pub(crate) fn map_bubble_command<R: Runtime>(
    app: &AppHandle<R>,
    command: Command,
) -> Result<Option<Command>, Error> {
    refresh(app);
    let ui = app.try_state::<Ui<R>>().ok_or(Error::new("stopped"))?;
    if !running(&ui) {
        return Err(Error::new("stopped"));
    }
    let result = ui.state.lock().unwrap().bubble.command(command);
    reconcile(app);
    result
}
pub(super) fn dismiss_bubble<R: Runtime>(app: &AppHandle<R>, presentation_id: u64) {
    match map_bubble_command(app, Command::Dismiss { presentation_id }) {
        Ok(Some(command)) => tray_command(app, command),
        Ok(None) => {}
        Err(error) => fail(app, "commandFailed", error.code),
    }
}
pub(super) fn respond_focus<R: Runtime>(app: &AppHandle<R>, revision: Option<u64>) {
    if let Some(revision) = revision {
        if let Err(error) = app.emit_to(
            "main",
            "focus-response",
            serde_json::json!({"revision":revision}),
        ) {
            fail(app, "responseFailed", error);
        }
    }
}

pub(super) fn character_visible<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.get_webview_window("main")
        .is_some_and(|window| window.is_visible().unwrap_or(false))
}
