use super::{Catalog, Service, Snapshot};
use crate::data_directory::{DataDirectory, DataFile};
use tauri::{
    menu::{CheckMenuItem, MenuEvent, MenuItem, Submenu},
    App, AppHandle, Emitter, Manager, Runtime,
};
struct NativeCharacters<R: Runtime> {
    service: Service,
    items: Vec<(String, CheckMenuItem<R>)>,
    status: MenuItem<R>,
}
const EVENT: &str = "selected-character-changed";
const PREFIX: &str = "character:";
pub fn setup<R: Runtime>(app: &mut App<R>) -> Result<Submenu<R>, Box<dyn std::error::Error>> {
    let catalog = Catalog::builtin()?;
    let handle = app.handle().clone();
    let path = app.state::<DataDirectory>().path(DataFile::Characters);
    let service = Service::start(path, move |_| {
        let handle = handle.clone();
        let queued = handle.clone();
        if let Err(error) = handle.run_on_main_thread(move || reconcile(&queued, true)) {
            eprintln!("Character notification queue failed: {error}");
        }
    })?;
    let snapshot = service.snapshot();
    let submenu = Submenu::new(app, "角色", true)?;
    let mut items = Vec::new();
    for character in catalog.characters {
        let item = CheckMenuItem::with_id(
            app,
            format!("{PREFIX}{}", character.id),
            character.display_name,
            true,
            character.id == snapshot.selected_character_id,
            None::<&str>,
        )?;
        submenu.append(&item)?;
        items.push((character.id, item));
    }
    let status = MenuItem::with_id(
        app,
        "character-status",
        snapshot.persistence.label(),
        false,
        None::<&str>,
    )?;
    submenu.append(&status)?;
    app.manage(NativeCharacters {
        service,
        items,
        status,
    });
    Ok(submenu)
}
fn reconcile<R: Runtime>(app: &AppHandle<R>, emit: bool) {
    let Some(native) = app.try_state::<NativeCharacters<R>>() else {
        return;
    };
    let Some(snapshot) = native.service.running_snapshot() else {
        return;
    };
    // Called on the main thread, after snapshot releases its state lock.
    for (id, item) in &native.items {
        if let Err(error) = item.set_checked(*id == snapshot.selected_character_id) {
            eprintln!("Character menu update failed: {error}");
        }
    }
    if let Err(error) = native.status.set_text(snapshot.persistence.label()) {
        eprintln!("Character status update failed: {error}");
    }
    if emit {
        if let Err(error) = app.emit(EVENT, snapshot) {
            eprintln!("Character event failed: {error}");
        }
    }
}
pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: &MenuEvent) -> bool {
    let Some(id) = event.id().as_ref().strip_prefix(PREFIX) else {
        return false;
    };
    if let Some(native) = app.try_state::<NativeCharacters<R>>() {
        if let Err(error) = native.service.select(id.into()) {
            eprintln!("Character selection failed: {error}");
        }
        // Undo the native automatic toggle until the worker commits a selection.
        let queued = app.clone();
        if let Err(error) = app.run_on_main_thread(move || reconcile(&queued, false)) {
            eprintln!("Character menu queue failed: {error}");
        }
    }
    true
}
pub fn stop<R: Runtime>(app: &AppHandle<R>) {
    if let Some(native) = app.try_state::<NativeCharacters<R>>() {
        native.service.stop();
    }
}
#[tauri::command]
pub fn get_selected_character<R: Runtime>(app: AppHandle<R>) -> Result<Snapshot, String> {
    app.state::<NativeCharacters<R>>()
        .service
        .running_snapshot()
        .ok_or("character selection is stopping".into())
}
#[tauri::command]
pub async fn select_character<R: Runtime>(
    app: AppHandle<R>,
    character_id: String,
) -> Result<Snapshot, String> {
    let receiver = app
        .state::<NativeCharacters<R>>()
        .service
        .select(character_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        receiver
            .recv()
            .map_err(|_| "character worker unavailable".to_string())?
    })
    .await
    .map_err(|_| "character selection completion unavailable".to_string())?
}
