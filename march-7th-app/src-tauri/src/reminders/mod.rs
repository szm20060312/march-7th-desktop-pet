mod model;
pub(crate) mod native;
mod service;
mod store;
pub(crate) use model::{ActiveHours as ImportActiveHours, Id as ImportId};
pub(crate) use service::Snapshot as ExportSnapshot;
pub(crate) use store::decode as decode_imported_reminders;
pub(crate) use store::validate_import as validate_imported_reminders;
pub(crate) fn export_snapshot(snapshot: &ExportSnapshot) -> Result<Vec<u8>, &'static str> {
    store::export_snapshot(snapshot)
}
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
