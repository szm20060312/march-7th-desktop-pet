mod geometry;
mod state;
mod store;

use geometry::{Monitor, Placement};
use state::{Availability, Lifecycle, Mode, Save, Session};
use std::{
    error::Error,
    io::{Error as IoError, ErrorKind},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    time::{Duration, Instant},
};
use store::Store;
use tauri::{
    menu::{CheckMenuItem, MenuBuilder, MenuEvent, MenuItem},
    tray::TrayIconBuilder,
    App, AppHandle, Builder, Manager, PhysicalPosition, RunEvent, Runtime, WebviewWindow, Window,
    WindowEvent,
};

const MAIN_WINDOW_LABEL: &str = "main";
const MONITOR_INTERVAL: Duration = Duration::from_secs(2);

struct Shared {
    session: Mutex<Session>,
    wake: Condvar,
    monitor_queued: AtomicBool,
}
struct Desktop<R: Runtime> {
    shared: Arc<Shared>,
    interaction: CheckMenuItem<R>,
    click_through: CheckMenuItem<R>,
    status: MenuItem<R>,
}

pub fn configure<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder.setup(setup).on_window_event(handle_window_event)
}

fn setup<R: Runtime>(app: &mut App<R>) -> Result<(), Box<dyn Error>> {
    let window = main_window(app.handle())?;
    // Config starts hidden: no default-position flash before restoration.
    window.set_ignore_cursor_events(false)?;
    let (store, placement, load_error) = match app.path().app_config_dir() {
        Ok(path) => {
            let (store, placement, error) = Store::load(path.join("desktop-state.json"));
            (Some(store), placement, error)
        }
        Err(error) => (
            None,
            None,
            Some(format!("locate configuration directory: {error}")),
        ),
    };
    let writable = store.as_ref().is_some_and(|s| s.writable);
    let availability = if load_error.is_some() {
        Availability::Unavailable
    } else if placement.is_some() {
        Availability::Saved
    } else {
        Availability::Pending
    };
    if let Some(error) = load_error {
        report_failure("load-placement", error);
    }
    let topology = monitors(&window)?;
    apply_placement(&window, placement.as_ref(), &topology)?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| IoError::new(ErrorKind::NotFound, "configured tray icon is unavailable"))?;
    let interaction =
        CheckMenuItem::with_id(app, "interaction", "交互模式", true, true, None::<&str>)?;
    let click_through =
        CheckMenuItem::with_id(app, "click-through", "点击穿透", true, false, None::<&str>)?;
    let status = MenuItem::with_id(
        app,
        "persistence-status",
        availability.label(),
        false,
        None::<&str>,
    )?;
    let menu = MenuBuilder::new(app)
        .text("show", "显示")
        .text("hide", "隐藏")
        .text("reset-position", "重置位置")
        .separator()
        .item(&interaction)
        .item(&click_through)
        .item(&status)
        .separator()
        .text("quit", "退出")
        .build()?;
    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .on_menu_event(handle_menu_event)
        .build(app)?;
    let shared = Arc::new(Shared {
        session: Mutex::new(Session::new(placement, topology, availability, writable)),
        wake: Condvar::new(),
        monitor_queued: AtomicBool::new(false),
    });
    app.manage(Desktop {
        shared: shared.clone(),
        interaction,
        click_through,
        status,
    });
    let handle = app.handle().clone();
    std::thread::Builder::new()
        .name("desktop-state".into())
        .spawn(move || worker(handle, shared, store))?;
    show_non_focusable_window(&window)?;
    // Store the effective, clamped position, including first launch defaults.
    if let Err(error) = schedule_current(app.handle()) {
        report_failure("startup-placement-capture", error);
    }
    Ok(())
}

fn active<R: Runtime>(desktop: &Desktop<R>) -> bool {
    desktop.shared.session.lock().unwrap().lifecycle == Lifecycle::Running
}
fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    let Some(desktop) = app.try_state::<Desktop<R>>() else {
        return;
    };
    if !active(&desktop) {
        return;
    }
    let operation = event.id().as_ref();
    let result = match operation {
        "show" => revalidate(app)
            .and_then(|_| main_window(app))
            .and_then(|w| show_non_focusable_window(&w)),
        "hide" => main_window(app).and_then(|w| w.hide().map_err(|e| e.to_string())),
        "reset-position" => reset_position(app),
        "interaction" => change_mode(app, Mode::Interaction),
        "click-through" => change_mode(app, Mode::ClickThrough),
        "quit" => {
            app.exit(0);
            Ok(())
        }
        _ => Ok(()),
    };
    if let Err(error) = result {
        report_failure(operation, error);
    }
}
fn change_mode<R: Runtime>(app: &AppHandle<R>, next: Mode) -> Result<(), String> {
    let desktop = app.state::<Desktop<R>>();
    let mut mode = desktop.shared.session.lock().unwrap().mode;
    let result = mode.change(next, |ignore| {
        main_window(app)?
            .set_ignore_cursor_events(ignore)
            .map_err(|e| e.to_string())
    });
    desktop.shared.session.lock().unwrap().mode = mode;
    // Native check items auto-toggle. Reconcile both after success AND failure.
    for (item, checked) in [
        (&desktop.interaction, mode == Mode::Interaction),
        (&desktop.click_through, mode == Mode::ClickThrough),
    ] {
        if let Err(error) = item.set_checked(checked) {
            report_failure("mode-menu-indicator", error);
        }
    }
    result
}
fn handle_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if window.label() != MAIN_WINDOW_LABEL {
        return;
    }
    let app = window.app_handle();
    let Some(desktop) = app.try_state::<Desktop<R>>() else {
        return;
    };
    if !active(&desktop) {
        return;
    }
    let result = match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            window.hide().map_err(|e| e.to_string())
        }
        WindowEvent::Moved(_) => schedule_current(app),
        WindowEvent::ScaleFactorChanged { .. } => {
            // UI run_on_main_thread is inline; the worker queues after this event.
            {
                let mut session = desktop.shared.session.lock().unwrap();
                session.scale_pending = Some(session.latest.clone());
            }
            desktop.shared.wake.notify_one();
            Ok(())
        }
        _ => Ok(()),
    };
    if let Err(error) = result {
        report_failure("window-event", error);
    }
}
fn reapply_scale<R: Runtime>(
    app: &AppHandle<R>,
    previous: Option<Placement>,
) -> Result<(), String> {
    let desktop = app.state::<Desktop<R>>();
    if !active(&desktop) {
        return Ok(());
    }

    let window = main_window(app)?;
    let topology = monitors(&window)?;
    let current = window
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("current monitor unavailable")?;
    let current = monitor_geometry(&current, true);
    let position = window.outer_position().map_err(|e| e.to_string())?;
    let placement = geometry::after_scale(
        previous.as_ref(),
        &current,
        (position.x, position.y),
        &topology,
    );
    // Limit correction to the current display, including ambiguous names.
    apply_placement(&window, placement.as_ref(), &[current])?;
    schedule_current(app)
}
fn monitor_geometry(m: &tauri::Monitor, primary: bool) -> Monitor {
    let area = m.work_area();
    Monitor {
        name: m.name().cloned(),
        origin: (area.position.x, area.position.y),
        size: (area.size.width, area.size.height),
        scale: m.scale_factor(),
        primary,
    }
}
fn monitors<R: Runtime>(window: &WebviewWindow<R>) -> Result<Vec<Monitor>, String> {
    let primary = window.primary_monitor().unwrap_or_else(|error| {
        report_failure("primary-monitor", error);
        None
    });
    let result: Vec<_> = window
        .available_monitors()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|m| {
            monitor_geometry(
                m,
                primary
                    .as_ref()
                    .is_some_and(|p| p.position() == m.position() && p.name() == m.name()),
            )
        })
        .collect();
    Ok(result)
}
fn apply_placement<R: Runtime>(
    window: &WebviewWindow<R>,
    placement: Option<&Placement>,
    monitors: &[Monitor],
) -> Result<(), String> {
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let (x, y) = geometry::restore(placement, monitors, (size.width, size.height))
        .ok_or("no usable monitor work area")?;
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())
}
fn capture_current<R: Runtime>(app: &AppHandle<R>) -> Result<Placement, String> {
    let window = main_window(app)?;
    let monitor = window
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("current monitor unavailable")?;
    let position = window.outer_position().map_err(|e| e.to_string())?;
    geometry::capture((position.x, position.y), &monitor_geometry(&monitor, false))
        .ok_or("invalid monitor geometry".into())
}
fn schedule_current<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let desktop = app.state::<Desktop<R>>();
    if !active(&desktop) {
        return Ok(());
    }
    let placement = match capture_current(app) {
        Ok(placement) => placement,
        Err(error) => {
            desktop.shared.session.lock().unwrap().capture_failed();
            update_status(&desktop);
            return Err(error);
        }
    };
    desktop
        .shared
        .session
        .lock()
        .unwrap()
        .schedule(Instant::now(), placement);
    desktop.shared.wake.notify_one();
    update_status(&desktop);
    Ok(())
}
fn update_status<R: Runtime>(desktop: &Desktop<R>) {
    let status = {
        let session = desktop.shared.session.lock().unwrap();
        if session.lifecycle != Lifecycle::Running {
            return;
        }
        session.availability
    };
    if let Err(error) = desktop.status.set_text(status.label()) {
        report_failure("persistence-status", error);
    }
}
fn revalidate<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = main_window(app)?;
    let topology = monitors(&window)?;
    let position = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;
    if !geometry::reachable(
        (position.x, position.y),
        (size.width, size.height),
        &topology,
    ) {
        let placement = app
            .state::<Desktop<R>>()
            .shared
            .session
            .lock()
            .unwrap()
            .latest
            .clone();
        apply_placement(&window, placement.as_ref(), &topology)?;
        schedule_current(app)?;
    }
    Ok(())
}
fn reset_position<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = main_window(app)?;
    let mut topology = monitors(&window)?;
    if !topology.iter().any(|m| m.primary) {
        if let Some(current) = window.current_monitor().map_err(|e| e.to_string())? {
            topology.insert(0, monitor_geometry(&current, true));
        }
    }
    apply_placement(&window, None, &topology)?;
    show_non_focusable_window(&window)?;
    schedule_current(app)
}
fn check_topology<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let desktop = app.state::<Desktop<R>>();
    if !active(&desktop) {
        return Ok(());
    }
    let topology = monitors(&main_window(app)?)?;
    let changed = desktop.shared.session.lock().unwrap().topology != topology;
    if changed {
        revalidate(app)?;
        desktop.shared.session.lock().unwrap().topology = topology;
    }
    Ok(())
}

// Native getters/effects run on UI. The sole worker performs IO and waits on one
// replaceable deadline. No lock is held across IO or UI calls; UI never joins it.
fn worker<R: Runtime>(app: AppHandle<R>, shared: Arc<Shared>, mut store: Option<Store>) {
    let mut next_monitor = Instant::now() + MONITOR_INTERVAL;
    loop {
        let (save, quit, poll, scale) = {
            let mut session = shared.session.lock().unwrap();
            loop {
                if let Some((save, code)) = session.quit.take() {
                    break (save, Some(code), false, None);
                }
                let now = Instant::now();
                let save = session.pending.take_due(now);
                let poll = now >= next_monitor;
                let scale = std::mem::take(&mut session.scale_pending);
                if save.is_some() || poll || scale.is_some() {
                    break (save, None, poll, scale);
                }
                let deadline = session
                    .pending
                    .deadline()
                    .map_or(next_monitor, |d| d.min(next_monitor));
                session = shared
                    .wake
                    .wait_timeout(session, deadline.saturating_duration_since(now))
                    .unwrap()
                    .0;
            }
        };
        if let Some(save) = save {
            persist(&app, &shared, &mut store, save);
        }
        if let Some(code) = quit {
            shared.session.lock().unwrap().lifecycle = Lifecycle::ExitReady;
            app.exit(code);
            return;
        }
        if let Some(previous) = scale {
            let handle = app.clone();
            if let Err(error) = app.run_on_main_thread(move || {
                if let Err(error) = reapply_scale(&handle, previous) {
                    report_failure("scale-placement", error);
                }
            }) {
                report_failure("queue-scale-placement", error);
            }
        }
        if poll {
            next_monitor = Instant::now() + MONITOR_INTERVAL;
            if !shared.monitor_queued.swap(true, Ordering::AcqRel) {
                let handle = app.clone();
                let completion = shared.clone();
                if let Err(error) = app.run_on_main_thread(move || {
                    if let Err(error) = check_topology(&handle) {
                        report_failure("monitor-topology", error);
                    }
                    completion.monitor_queued.store(false, Ordering::Release);
                }) {
                    shared.monitor_queued.store(false, Ordering::Release);
                    report_failure("queue-monitor-check", error);
                }
            }
        }
    }
}
fn persist<R: Runtime>(app: &AppHandle<R>, shared: &Shared, store: &mut Option<Store>, save: Save) {
    let result = store
        .as_mut()
        .ok_or_else(|| "configuration directory unavailable".to_string())
        .and_then(|store| store.save(&save.placement));
    let writable = store.as_ref().is_some_and(|s| s.writable);
    shared
        .session
        .lock()
        .unwrap()
        .finish_save(save.revision, result.is_ok(), writable);
    if let Err(error) = result {
        report_failure("save-placement", error);
    }
    let handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        update_status(&handle.state::<Desktop<R>>());
    }) {
        report_failure("queue-persistence-status", error);
    }
}
pub fn on_run_event<R: Runtime>(app: &AppHandle<R>, event: RunEvent) {
    if let RunEvent::ExitRequested { api, code, .. } = event {
        let Some(desktop) = app.try_state::<Desktop<R>>() else {
            return;
        };
        let lifecycle = desktop.shared.session.lock().unwrap().lifecycle;
        if lifecycle == Lifecycle::ExitReady {
            return;
        }
        // Delay orderly quit only for final flush, then request exit again.
        api.prevent_exit();
        if lifecycle == Lifecycle::Running {
            let latest = match capture_current(app) {
                Ok(p) => Some(p),
                Err(e) => {
                    report_failure("quit-placement", e);
                    None
                }
            };
            desktop
                .shared
                .session
                .lock()
                .unwrap()
                .stop(latest, code.unwrap_or(0));
            desktop.shared.wake.notify_one();
        }
    }
}
fn show_non_focusable_window<R: Runtime>(window: &WebviewWindow<R>) -> Result<(), String> {
    window
        .set_focusable(false)
        .map_err(|e| format!("keep window non-focusable: {e}"))?;
    window.show().map_err(|e| e.to_string())
}
fn main_window<R: Runtime>(app: &AppHandle<R>) -> Result<WebviewWindow<R>, String> {
    app.get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "main window unavailable".into())
}
fn report_failure(operation: &str, error: impl std::fmt::Display) {
    eprintln!("March 7th desktop operation `{operation}` failed: {error}");
}
#[cfg(test)]
mod tests {
    #[test]
    fn main_window_stays_hidden_until_restored_and_never_takes_focus() {
        let context = crate::app_context();
        let main = context
            .config()
            .app
            .windows
            .iter()
            .find(|w| w.label == super::MAIN_WINDOW_LABEL)
            .unwrap();
        assert!(!main.focus);
        assert!(!main.focusable);
        assert!(!main.visible);
    }
}
