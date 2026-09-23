use super::{Catalog, Persistence, Snapshot};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Config {
    version: u32,
    selected_character_id: String,
}
pub(crate) fn validate_import(bytes: &[u8]) -> Result<(), String> {
    let config = parse(bytes)?;
    let catalog = Catalog::builtin()?;
    if !catalog.contains(&config.selected_character_id) {
        return Err("unknown imported character".into());
    }
    Ok(())
}
pub(crate) fn imported_selected_id(bytes: &[u8]) -> Result<String, String> {
    validate_import(bytes)?;
    Ok(parse(bytes)?.selected_character_id)
}
pub(crate) fn validate_stored(bytes: &[u8]) -> Result<(), String> {
    parse(bytes).map(|_| ())
}
pub(crate) fn export_snapshot(snapshot: Option<&Snapshot>) -> Result<Vec<u8>, &'static str> {
    let snapshot = snapshot.ok_or("exportCharacterUnavailable")?;
    if !matches!(
        snapshot.persistence,
        Persistence::Default | Persistence::Saved
    ) {
        return Err("exportCharacterUnavailable");
    }
    let catalog = Catalog::builtin().map_err(|_| "exportCharacterUnavailable")?;
    if !catalog.contains(&snapshot.selected_character_id) {
        return Err("exportCharacterUnavailable");
    }
    let bytes = serde_json::to_vec_pretty(&Config {
        version: 1,
        selected_character_id: snapshot.selected_character_id.clone(),
    })
    .map_err(|_| "exportCharacterUnavailable")?;
    validate_import(&bytes).map_err(|_| "exportCharacterUnavailable")?;
    Ok(bytes)
}
fn parse(bytes: &[u8]) -> Result<Config, String> {
    let config: Config =
        serde_json::from_slice(bytes).map_err(|_| "invalid character configuration".to_string())?;
    if config.version != 1 {
        return Err("unsupported character configuration version".into());
    }
    Ok(config)
}
pub(super) struct Store {
    path: Option<PathBuf>,
    writable: bool,
}
impl Store {
    pub fn load(path: Option<PathBuf>, catalog: &Catalog) -> (Self, Snapshot) {
        let mut snapshot = Snapshot {
            selected_character_id: catalog.default_id.clone(),
            revision: 0,
            persistence: Persistence::Default,
        };
        let mut store = Self {
            writable: path.is_some(),
            path,
        };
        if let Some(path) = &store.path {
            match fs::read(path) {
                Ok(bytes) => match parse(&bytes) {
                    Ok(config) if catalog.contains(&config.selected_character_id) => {
                        snapshot.selected_character_id = config.selected_character_id;
                        snapshot.persistence = Persistence::Saved;
                    }
                    Ok(_) => snapshot.persistence = Persistence::Fallback,
                    Err(_) => store.writable = false,
                },
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(_) => store.writable = false,
            }
        }
        if !store.writable {
            snapshot.persistence = Persistence::SessionOnly;
        }
        (store, snapshot)
    }
    pub fn save(&mut self, id: &str) -> Result<Persistence, String> {
        if !self.writable {
            return Ok(Persistence::SessionOnly);
        }
        let path = self
            .path
            .as_ref()
            .ok_or("character configuration unavailable")?;
        let previous = match fs::read(path) {
            Ok(bytes) => {
                if parse(&bytes).is_err() {
                    self.writable = false;
                    // Newly invalid external contents are preserved, same as at load.
                    return Ok(Persistence::SessionOnly);
                }
                Some(bytes)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(_) => return Err("cannot read previous character configuration".into()),
        };
        let bytes = serde_json::to_vec_pretty(&Config {
            version: 1,
            selected_character_id: id.into(),
        })
        .map_err(|_| "cannot serialize character configuration")?;
        crate::atomic_file::replace(path, &bytes, previous.as_deref())
            .map_err(|_| "cannot save character configuration".to_string())?;
        Ok(Persistence::Saved)
    }
}
