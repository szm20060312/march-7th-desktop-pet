mod model;
pub(crate) mod native;
mod service;
mod store;
pub(crate) use store::validate_import as validate_imported_reminders;
pub(crate) mod ui;
mod ui_geometry;
mod ui_policy;
pub use native::stop;
pub fn setup<R: tauri::Runtime>(
    app: &mut tauri::App<R>,
) -> Result<tauri::menu::Submenu<R>, Box<dyn std::error::Error>> {
    native::setup(app)?;
    ui::setup(app)
}
#[cfg(test)]
mod tests;
