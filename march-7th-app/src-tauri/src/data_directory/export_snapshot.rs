//! Assemble a complete import candidate from current authoritative service snapshots.
//! The caller must qualify the desktop position from the live window; no file is read here.
use super::{ImportFiles, SetValidation};
use crate::{characters, desktop, reminders};

#[allow(dead_code)] // B3b will connect this pure boundary to the settings command.
pub(crate) fn capture(
    character: Option<&characters::Snapshot>,
    reminder: &reminders::ExportSnapshot,
    desktop: desktop::ExportPlacement,
    focus: Vec<u8>,
) -> Result<ImportFiles, &'static str> {
    let characters = characters::export_snapshot(character)?;
    let reminders = reminders::export_snapshot(reminder)?;
    let desktop = desktop::export_placement(desktop)?;
    let files = ImportFiles {
        desktop,
        characters,
        reminders,
        focus,
    };
    files.validate(SetValidation::Candidate)?;
    Ok(files)
}
