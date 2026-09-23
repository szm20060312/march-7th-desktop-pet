//! Concrete native reminder/settings coordination. Service remains the authority;
//! this state only guards window lifetimes and observed placement/readiness.
use super::{
    model::{Command, Error},
    native,
    store::SaveStatus,
    ui_geometry::{self, Observation, Placement},
    ui_policy::{PresentationUi, SettingsUi, Ticket},
};
use crate::desktop::{coordinates::NATIVE, monitor_geometry};
#[path = "ui_bubble.rs"]
mod bubble_host;
pub(crate) use bubble_host::{bubble_snapshot, focus_changed, map_bubble_command};
use bubble_host::{character_visible, dismiss_bubble, refresh, respond_focus};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{
    menu::{CheckMenuItem, MenuEvent, MenuItem, Submenu},
    webview::PageLoadEvent,
    App, AppHandle, Emitter, Manager, PhysicalSize, Runtime, WebviewWindow, WebviewWindowBuilder,
    Window, WindowEvent,
};

#[derive(Default)]
struct State {
    reminder: PresentationUi,
    bubble: super::bubble::Bubble,
    reminder_settle: Option<(Ticket, Observation, Placement)>,
    reminder_visible: bool,
    reminder_attempts: u16,
    settings: SettingsUi,
    settings_focus_target: bool,
    settings_was_visible: bool,
    settings_settle: Option<(u64, Observation, Placement)>,
    settings_attempts: u16,
    settings_reflow: bool,
    error: Option<&'static str>,
    menu_state: Option<(bool, bool, String)>,
}
struct Ui<R: Runtime> {
    state: Mutex<State>,
    stopped: Arc<AtomicBool>,
    started: std::time::Instant,
    pause: CheckMenuItem<R>,
    pending: MenuItem<R>,
    status: MenuItem<R>,
}

pub fn setup<R: Runtime>(app: &mut App<R>) -> Result<Submenu<R>, Box<dyn std::error::Error>> {
    let settings = MenuItem::with_id(app, "reminder-settings", "提醒设置…", true, None::<&str>)?;
    let focus_settings = MenuItem::with_id(app, "focus-settings", "当前专注…", true, None::<&str>)?;
    let pending = MenuItem::with_id(app, "reminder-pending", "查看待处理", false, None::<&str>)?;
    let pause = CheckMenuItem::with_id(
        app,
        "reminder-pause",
        "暂停提醒",
        false,
        false,
        None::<&str>,
    )?;
    let status = MenuItem::with_id(app, "reminder-status", "提醒：加载中", false, None::<&str>)?;
    let menu = Submenu::with_items(
        app,
        "提醒",
        true,
        &[&focus_settings, &settings, &pending, &pause, &status],
    )?;
    let stopped = Arc::new(AtomicBool::new(false));
    app.manage(Ui {
        state: Mutex::new(State::default()),
        stopped: stopped.clone(),
        started: std::time::Instant::now(),
        pause,
        pending,
        status,
    });
    let handle = app.handle().clone();
    // This is a UI geometry observation cadence, never a business timer. Only
    // one callback may be queued; it reads current service state on execution.
    std::thread::Builder::new()
        .name("reminder-ui".into())
        .spawn(move || {
            let queued = Arc::new(AtomicBool::new(false));
            while !stopped.load(Ordering::Acquire) {
                if !queued.swap(true, Ordering::AcqRel) {
                    let app = handle.clone();
                    let completion = queued.clone();
                    if handle
                        .run_on_main_thread(move || {
                            reconcile(&app);
                            completion.store(false, Ordering::Release);
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        })?;
    Ok(menu)
}

fn running<R: Runtime>(ui: &Ui<R>) -> bool {
    !ui.stopped.load(Ordering::Acquire)
}

fn settings_open_payload(
    intent: u64,
    focus_target: bool,
    already_visible: bool,
) -> serde_json::Value {
    // Keep the wire field name; its value orders UI intents, never backup sessions.
    serde_json::json!({
        "generation": intent,
        "target": if focus_target { "focus" } else { "settings" },
        "alreadyVisible": already_visible,
    })
}

pub fn stop<R: Runtime>(app: &AppHandle<R>) {
    crate::local_backup::invalidate_destroyed(app);
    let Some(ui) = app.try_state::<Ui<R>>() else {
        return;
    };
    ui.stopped.store(true, Ordering::Release);
    {
        let mut s = ui.state.lock().unwrap();
        let revision = s.reminder.revision;
        s.reminder.update(revision, None, true);
        s.settings.stop();
        s.reminder_settle = None;
        s.settings_settle = None;
    }
    for label in ["settings", "reminder"] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.hide();
        }
    }
    let _ = ui.pause.set_enabled(false);
    let _ = ui.pending.set_enabled(false);
    let _ = ui.status.set_text("提醒：已停止");
}

fn fail<R: Runtime>(app: &AppHandle<R>, code: &'static str, detail: impl std::fmt::Display) {
    let Some(ui) = app.try_state::<Ui<R>>() else {
        return;
    };
    if !running(&ui) {
        return;
    }
    let changed = {
        let mut s = ui.state.lock().unwrap();
        let changed = s.error != Some(code);
        s.error = Some(code);
        changed
    };
    if changed {
        eprintln!("Reminder UI {code}: {detail}");
    }
}

pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: &MenuEvent) -> bool {
    let operation = event.id().as_ref();
    if !matches!(
        operation,
        "focus-settings" | "reminder-settings" | "reminder-pending" | "reminder-pause"
    ) {
        return false;
    }
    let Some(ui) = app.try_state::<Ui<R>>() else {
        return true;
    };
    if !running(&ui) {
        return true;
    }
    ui.state.lock().unwrap().error = None;
    match operation {
        "focus-settings" | "reminder-settings" => {
            let existing = app.get_webview_window("settings");
            let already_visible = match existing.as_ref().map(WebviewWindow::is_visible) {
                Some(Ok(visible)) => visible,
                Some(Err(error)) => {
                    fail(app, "settingsFailed", error);
                    return true;
                }
                None => false,
            };
            let exists = existing.is_some();
            let token = {
                let mut s = ui.state.lock().unwrap();
                s.settings_focus_target = operation == "focus-settings";
                s.settings_was_visible = already_visible;
                s.settings_settle = None;
                s.settings_attempts = 0;
                s.settings.request_open(exists, already_visible)
            };
            if let Some(token) = token {
                create_window(app, "settings", token);
            }
            reconcile(app);
        }
        "reminder-pending" => tray_command(app, Command::ShowPending {}),
        "reminder-pause" => {
            if let Ok(snapshot) = native::snapshot(app) {
                // Native check items auto-toggle BEFORE this event. Restore the
                // authoritative value immediately, including rejected commands.
                if let Err(error) = ui.pause.set_checked(snapshot.paused) {
                    fail(app, "menuFailed", error);
                }
                tray_command(
                    app,
                    Command::SetPaused {
                        paused: !snapshot.paused,
                    },
                );
            }
        }
        _ => {}
    }
    true
}

fn tray_command<R: Runtime>(app: &AppHandle<R>, command: Command) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = native::dispatch(&app, command).await;
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let Some(ui) = handle.try_state::<Ui<R>>() else {
                return;
            };
            if !running(&ui) {
                return;
            }
            if let Err(error) = result {
                fail(&handle, "commandFailed", error.code);
            }
            reconcile(&handle);
        });
    });
}

pub fn handle_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) -> bool {
    if !matches!(window.label(), "settings" | "reminder") {
        return false;
    }
    let app = window.app_handle();
    let Some(ui) = app.try_state::<Ui<R>>() else {
        return true;
    };
    if !running(&ui) {
        return true;
    }
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        // Refresh before capturing the ID. An older auto dismissal can never
        // dismiss a newer manual view, even with queued service notifications.
        refresh(app);
        if window.label() == "settings" {
            match window.hide() {
                Ok(()) => {
                    crate::local_backup::invalidate(app);
                    let mut s = ui.state.lock().unwrap();
                    s.settings.close();
                    s.settings_reflow = false;
                    s.settings_settle = None;
                }
                Err(error) => fail(app, "hideFailed", error),
            }
            return true;
        }
        let id = {
            let mut s = ui.state.lock().unwrap();
            s.reminder_settle = None;
            s.reminder.dismiss()
        };
        if let Err(error) = window.hide() {
            fail(app, "hideFailed", error);
        }
        if let Some(presentation_id) = id {
            dismiss_bubble(app, presentation_id);
        }
    }
    true
}

fn create_window<R: Runtime>(app: &AppHandle<R>, label: &'static str, token: u64) {
    let Some(config) = app
        .config()
        .app
        .windows
        .iter()
        .find(|c| c.label == label)
        .cloned()
    else {
        fail(app, "createFailed", "window configuration missing");
        return;
    };
    let app = app.clone();
    let stopped = app.state::<Ui<R>>().stopped.clone();
    // Locked Tauri 2.11.5 webview_window.rs:58: Windows builder deadlocks
    // in synchronous event handlers. Build hidden off-thread, never show here.
    tauri::async_runtime::spawn_blocking(move || {
        if stopped.load(Ordering::Acquire) {
            return;
        }
        let result = WebviewWindowBuilder::from_config(&app, &config).and_then(|builder| {
            let builder = if label == "reminder" {
                builder.initialization_script(format!(
                    "Object.defineProperty(window,'__MARCH7_REMINDER_WINDOW_TOKEN__',{{value:{token},writable:false}});"
                )).on_page_load(move |window, payload| {
                    if payload.event() == PageLoadEvent::Started {
                        let handle = window.app_handle().clone();
                        let app = handle.clone();
                        let _ = handle.run_on_main_thread(move || {
                            let Some(ui) = app.try_state::<Ui<R>>() else { return; };
                            if !running(&ui) { return; }
                            let valid = {
                                let mut s = ui.state.lock().unwrap();
                                if s.reminder.token != Some(token) { false } else {
                                    s.reminder.reload(token);
                                    s.reminder_settle = None;
                                    true
                                }
                            };
                            if valid {
                                if let Err(error) = hide_reminder(&app) { fail(&app,"hideFailed",error); }
                            }
                        });
                    }
                })
            } else { builder };
            builder.build()
        });
        let handle = app.clone();
        // Keep a handle for queue failure cleanup; destroy bypasses CloseRequested.
        let cleanup = result.as_ref().ok().cloned();
        if app
            .run_on_main_thread(move || created(&handle, label, token, result))
            .is_err()
        {
            if let Some(window) = cleanup {
                let _ = window.destroy();
            }
        }
    });
}

fn created<R: Runtime>(
    app: &AppHandle<R>,
    label: &'static str,
    token: u64,
    result: tauri::Result<WebviewWindow<R>>,
) {
    let Some(ui) = app.try_state::<Ui<R>>() else {
        if let Ok(window) = result {
            let _ = window.destroy();
        }
        return;
    };
    refresh(app);
    let valid = {
        let mut s = ui.state.lock().unwrap();
        if !running(&ui) {
            false
        } else if label == "settings" {
            s.settings.created(token)
        } else {
            s.reminder.created(token)
        }
    };
    if !valid {
        if let Ok(window) = result {
            let _ = window.destroy();
        }
        return;
    }
    match result {
        Err(error) => {
            let mut s = ui.state.lock().unwrap();
            if label == "reminder" {
                s.reminder.destroyed(token);
            } else {
                s.settings.open = false;
            }
            drop(s);
            fail(app, "createFailed", error);
        }
        Ok(window) => {
            let handle = app.clone();
            window.on_window_event(move |event| {
                if matches!(event, WindowEvent::Destroyed) {
                    let app = handle.clone();
                    let _ = handle.run_on_main_thread(move || {
                        let Some(ui) = app.try_state::<Ui<R>>() else {
                            return;
                        };
                        if !running(&ui) {
                            return;
                        }
                        let mut s = ui.state.lock().unwrap();
                        if label == "reminder" {
                            let current = s.reminder.token == Some(token);
                            s.reminder.destroyed(token);
                            if current {
                                s.reminder_settle = None;
                                s.reminder_visible = false;
                                drop(s);
                                fail(
                                    &app,
                                    "windowDestroyed",
                                    "reminder window destroyed; pending remains recoverable",
                                );
                            }
                        } else if s.settings.destroyed(token) {
                            crate::local_backup::invalidate_destroyed(&app);
                            s.settings_settle = None;
                        }
                    });
                }
            });
            if label == "reminder" {
                if let Err(error) = window.set_ignore_cursor_events(false) {
                    let mut s = ui.state.lock().unwrap();
                    if let Some(ticket) = s.reminder.ticket() {
                        s.reminder.finish_show(ticket, false);
                    }
                    drop(s);
                    fail(app, "interactionFailed", error);
                    return;
                }
            }
            reconcile(app);
        }
    }
}

fn hide_reminder<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let ui = app.state::<Ui<R>>();
    if !ui.state.lock().unwrap().reminder_visible {
        return Ok(());
    }
    if let Some(window) = app.get_webview_window("reminder") {
        window.hide().map_err(|e| e.to_string())?;
    }
    ui.state.lock().unwrap().reminder_visible = false;
    Ok(())
}

pub fn reconcile<R: Runtime>(app: &AppHandle<R>) {
    let Some(ui) = app.try_state::<Ui<R>>() else {
        return;
    };
    if !running(&ui) {
        return;
    }
    refresh(app);
    update_menu(app);
    if let Some(window) = app.get_webview_window("settings") {
        let idle = {
            let s = ui.state.lock().unwrap();
            !s.settings.open && !s.settings.creating && !s.settings_reflow
        };
        if idle && window.is_visible().unwrap_or(false) {
            match reachable(&window) {
                Ok(false) => {
                    let mut s = ui.state.lock().unwrap();
                    s.settings_reflow = true;
                    s.settings.intent += 1;
                    s.settings_attempts = 0;
                    s.settings_settle = None;
                }
                Err(error) => fail(app, "settingsFailed", error),
                _ => {}
            }
        }
    }
    let (token, ticket, settings) = {
        let mut s = ui.state.lock().unwrap();
        (
            s.reminder.begin_create(),
            s.reminder.ticket(),
            (s.settings.open || s.settings_reflow) && !s.settings.creating,
        )
    };
    if let Some(token) = token {
        create_window(app, "reminder", token);
    }
    if let Some(window) = app.get_webview_window("reminder") {
        if let Some(ticket) = ticket {
            if let Err(error) = settle_reminder(app, &window, ticket) {
                let mut s = ui.state.lock().unwrap();
                s.reminder.finish_show(ticket, false);
                drop(s);
                let _ = hide_reminder(app);
                fail(app, "presentationFailed", error);
            }
        } else if let Err(error) = hide_reminder(app) {
            fail(app, "hideFailed", error);
        }
    }
    if settings {
        if let Some(window) = app.get_webview_window("settings") {
            if let Err(error) = settle_settings(app, &window) {
                ui.state.lock().unwrap().settings.open = false;
                ui.state.lock().unwrap().settings_reflow = false;
                fail(app, "settingsFailed", error);
            }
        }
    }
}

fn update_menu<R: Runtime>(app: &AppHandle<R>) {
    let ui = app.state::<Ui<R>>();
    let Ok(snapshot) = native::snapshot(app) else {
        return;
    };
    let enabled = !snapshot.stopped && snapshot.persistence.status != SaveStatus::Loading;
    let mutable = enabled && snapshot.persistence.status != SaveStatus::ReadOnly;
    let status = if snapshot.stopped {
        "已停止"
    } else if snapshot.runtime_error.is_some() {
        "时钟/运行错误"
    } else {
        match snapshot.persistence.status {
            SaveStatus::Loading => "加载中",
            SaveStatus::Default => "默认设置（尚未保存）",
            SaveStatus::Saved => "已保存",
            SaveStatus::Unsaved => "未保存，请重试",
            SaveStatus::ReadOnly => "只读保护",
        }
    };
    let text = {
        let s = ui.state.lock().unwrap();
        format!(
            "提醒：{status}{}",
            if s.error.is_some() {
                " · 操作/界面失败，请从托盘重试"
            } else {
                ""
            }
        )
    };
    let next = (snapshot.paused, mutable, text.clone());
    if ui.state.lock().unwrap().menu_state.as_ref() == Some(&next) {
        return;
    }
    let result = (|| -> tauri::Result<()> {
        ui.pause.set_checked(snapshot.paused)?;
        ui.pause.set_text(if snapshot.paused {
            "恢复提醒"
        } else {
            "暂停提醒"
        })?;
        ui.pause.set_enabled(mutable)?;
        ui.pending.set_enabled(enabled)?;
        ui.status.set_text(text)?;
        Ok(())
    })();
    match result {
        Ok(()) => ui.state.lock().unwrap().menu_state = Some(next),
        Err(error) => fail(app, "menuFailed", error),
    }
}

fn observe<R: Runtime>(window: &WebviewWindow<R>) -> Result<Observation, String> {
    let p = window.outer_position().map_err(|e| e.to_string())?;
    let inner = window.inner_size().map_err(|e| e.to_string())?;
    let outer = window.outer_size().map_err(|e| e.to_string())?;
    Ok(Observation {
        position: (p.x, p.y),
        inner: (inner.width, inner.height),
        outer: (outer.width, outer.height),
        scale: window.scale_factor().map_err(|e| e.to_string())?,
    })
}

fn reachable<R: Runtime>(window: &WebviewWindow<R>) -> Result<bool, String> {
    let o = observe(window)?;
    let monitors = window
        .available_monitors()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|m| monitor_geometry(m, false))
        .collect::<Vec<_>>();
    Ok(ui_geometry::fully_reachable(NATIVE, &o, &monitors))
}

fn placement<R: Runtime>(
    app: &AppHandle<R>,
    window: &WebviewWindow<R>,
    settings: bool,
) -> Result<(Observation, Placement), String> {
    let main = app
        .get_webview_window("main")
        .ok_or("main window unavailable")?;
    let monitor = main
        .current_monitor()
        .map_err(|e| e.to_string())?
        .or(main.primary_monitor().map_err(|e| e.to_string())?)
        .ok_or("no available display")?;
    let observation = observe(window)?;
    let desired = ui_geometry::place(
        NATIVE,
        &monitor_geometry(&monitor, true),
        &observe(&main)?,
        &observation,
        settings,
    )
    .ok_or("unusable work area")?;
    Ok((observation, desired))
}

fn apply_placement<R: Runtime>(
    window: &WebviewWindow<R>,
    observation: &Observation,
    desired: &Placement,
) -> Result<bool, String> {
    let (position, _) = NATIVE
        .rect(
            observation.position,
            observation.outer,
            observation.scale,
            desired.scale,
        )
        .ok_or("invalid coordinates")?;
    let settled = (observation.scale - desired.scale).abs() < 1e-6
        && observation.inner == desired.inner
        && position == desired.position;
    if !settled {
        if observation.inner != desired.inner {
            window
                .set_size(PhysicalSize::new(desired.inner.0, desired.inner.1))
                .map_err(|e| e.to_string())?;
        }
        if position != desired.position || (observation.scale - desired.scale).abs() >= 1e-6 {
            window
                .set_position(
                    NATIVE
                        .position(desired.position, desired.scale)
                        .ok_or("invalid target")?,
                )
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(settled)
}

fn settle_reminder<R: Runtime>(
    app: &AppHandle<R>,
    window: &WebviewWindow<R>,
    ticket: Ticket,
) -> Result<(), String> {
    let ui = app.state::<Ui<R>>();
    {
        let s = ui.state.lock().unwrap();
        if !s.reminder.accepts(ticket) || s.reminder.creating {
            return Ok(());
        }
        if s.reminder.shown {
            drop(s);
            if reachable(window)? {
                return Ok(());
            }
            hide_reminder(app)?;
            ui.state.lock().unwrap().reminder.shown = false;
        }
    }
    {
        let mut s = ui.state.lock().unwrap();
        s.reminder_attempts += 1;
        if s.reminder_attempts > 100 {
            return Err("placement or page readiness did not settle".into());
        }
    }
    let (observation, desired) = placement(app, window, false)?;
    let settled = apply_placement(window, &observation, &desired)?;
    let candidate = (ticket, observation, desired);
    let stable = {
        let mut s = ui.state.lock().unwrap();
        let stable = settled && s.reminder_settle.as_ref() == Some(&candidate);
        s.reminder_settle = Some(candidate);
        stable && s.reminder.can_show(ticket)
    };
    if stable {
        refresh(app); // no stale presentation show after asynchronous placement
        if !running(&ui) || !ui.state.lock().unwrap().reminder.can_show(ticket) {
            return Ok(());
        }
        window.set_focusable(false).map_err(|e| e.to_string())?;
        ui.state.lock().unwrap().reminder_visible = true;
        window.show().map_err(|e| e.to_string())?;
        ui.state.lock().unwrap().reminder.finish_show(ticket, true);
        let response = if character_visible(app) {
            ui.state
                .lock()
                .unwrap()
                .bubble
                .policy
                .presented(ticket.presentation)
        } else {
            None
        };
        respond_focus(app, response);
        ui.state.lock().unwrap().reminder_attempts = 0;
    }
    Ok(())
}

fn settle_settings<R: Runtime>(
    app: &AppHandle<R>,
    window: &WebviewWindow<R>,
) -> Result<(), String> {
    let ui = app.state::<Ui<R>>();
    let intent = ui.state.lock().unwrap().settings.intent;
    {
        let mut s = ui.state.lock().unwrap();
        s.settings_attempts += 1;
        if s.settings_attempts > 100 {
            return Err("settings placement did not settle".into());
        }
    }
    let (observation, desired) = placement(app, window, true)?;
    let settled = apply_placement(window, &observation, &desired)?;
    let candidate = (intent, observation, desired);
    let stable = {
        let mut s = ui.state.lock().unwrap();
        let stable = settled && s.settings_settle.as_ref() == Some(&candidate);
        s.settings_settle = Some(candidate);
        stable
            && (s.settings.may_show(intent) || (s.settings_reflow && s.settings.intent == intent))
    };
    if stable && running(&ui) {
        if ui.state.lock().unwrap().settings.may_show(intent) {
            // The only focus call belongs to an explicit tray open, never reflow.
            let (focus_target, already_visible) = {
                let state = ui.state.lock().unwrap();
                (state.settings_focus_target, state.settings_was_visible)
            };
            window
                .emit(
                    "reminder-settings-opened",
                    settings_open_payload(intent, focus_target, already_visible),
                )
                .map_err(|e| e.to_string())?;
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
            if focus_target && character_visible(app) {
                let revision = crate::focus::native::current(app)
                    .as_ref()
                    .and_then(super::bubble::natural_revision);
                let response =
                    revision.and_then(|r| ui.state.lock().unwrap().bubble.policy.respond(r));
                respond_focus(app, response);
            }
        }
        let mut state = ui.state.lock().unwrap();
        if state.settings.presented(intent) {
            state.settings_reflow = false;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn hide_reminder_settings<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
) -> Result<(), Error> {
    require_window(window.label(), "settings")?;
    let (send, receive) = std::sync::mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let result = (|| {
            let ui = handle.try_state::<Ui<R>>().ok_or(Error::new("stopped"))?;
            if !running(&ui) {
                return Err(Error::new("stopped"));
            }
            window.hide().map_err(|_| Error::new("hideFailed"))?;
            crate::local_backup::invalidate(&handle);
            let mut s = ui.state.lock().unwrap();
            s.settings.close();
            s.settings_reflow = false;
            s.settings_settle = None;
            Ok(())
        })();
        let _ = send.send(result);
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
pub(crate) fn settings_export_session<R: Runtime>(app: &AppHandle<R>) -> Option<(u64, u64)> {
    let ui = app.try_state::<Ui<R>>()?;
    if !running(&ui) {
        return None;
    }
    let state = ui.state.lock().unwrap();
    state.settings.export_session(state.settings_reflow)
}

#[tauri::command]
pub async fn reminder_ui_ready<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    presentation_id: u64,
    window_token: u64,
) -> Result<(), Error> {
    require_window(window.label(), "reminder")?;
    let (send, receive) = std::sync::mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        refresh(&handle);
        let result = handle
            .try_state::<Ui<R>>()
            .filter(|ui| running(ui))
            .ok_or(Error::new("stopped"))
            .and_then(|ui| {
                if ui
                    .state
                    .lock()
                    .unwrap()
                    .reminder
                    .ready(window_token, presentation_id)
                {
                    Ok(())
                } else {
                    Err(Error::new("stalePresentation"))
                }
            });
        let _ = send.send(result);
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

fn require_window(actual: &str, expected: &str) -> Result<(), Error> {
    if actual == expected {
        Ok(())
    } else {
        Err(Error::new("invalidWindow"))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn settings_open_intent_carries_generation_target_and_visibility_only() {
        assert_eq!(
            super::settings_open_payload(7, true, true),
            serde_json::json!({"generation": 7, "target": "focus", "alreadyVisible": true})
        );
        assert_eq!(
            super::settings_open_payload(8, false, false),
            serde_json::json!({"generation": 8, "target": "settings", "alreadyVisible": false})
        );
    }
    #[test]
    fn ui_commands_reject_the_wrong_native_caller() {
        for expected in ["settings", "reminder"] {
            for actual in ["main", "settings", "reminder", "unknown"] {
                assert_eq!(
                    super::require_window(actual, expected).is_ok(),
                    actual == expected
                );
            }
        }
    }
    #[test]
    fn real_configs_are_lazy_hidden_and_only_settings_can_focus() {
        let context = crate::app_context();
        let configs = &context.config().app.windows;
        for label in ["main", "reminder"] {
            let c = configs.iter().find(|c| c.label == label).unwrap();
            assert!(!c.focus && !c.focusable && !c.visible);
            assert!(c.accept_first_mouse);
        }
        for label in ["reminder", "settings"] {
            assert!(!configs.iter().find(|c| c.label == label).unwrap().create);
        }
        let settings = configs.iter().find(|c| c.label == "settings").unwrap();
        assert!(settings.focusable && !settings.focus && !settings.visible);
    }
}
