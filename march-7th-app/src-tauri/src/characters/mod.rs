pub(crate) mod native;
mod store;
pub use native::{handle_menu_event, setup, stop};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
};
pub(crate) use store::validate_import as validate_imported_characters;
use store::Store;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    pub id: String,
    pub display_name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub default_id: String,
    pub characters: Vec<Character>,
}
impl Catalog {
    pub fn builtin() -> Result<Self, String> {
        let catalog: Self =
            serde_json::from_str(include_str!("../../../src/characters/catalog.json"))
                .map_err(|_| "invalid built-in character catalog")?;
        let mut ids = std::collections::HashSet::new();
        if !catalog.contains(&catalog.default_id)
            || catalog
                .characters
                .iter()
                .any(|c| c.id.is_empty() || c.display_name.is_empty() || !ids.insert(&c.id))
        {
            return Err("invalid built-in character identities".into());
        }
        Ok(catalog)
    }
    fn contains(&self, id: &str) -> bool {
        self.characters.iter().any(|c| c.id == id)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Persistence {
    Default,
    Saved,
    Fallback,
    SessionOnly,
    SaveFailed,
}
impl Persistence {
    fn label(self) -> &'static str {
        match self {
            Self::Default => "角色：默认（尚未保存）",
            Self::Saved => "角色：已保存",
            Self::Fallback => "角色：未知记录，使用默认（未覆盖）",
            Self::SessionOnly => "角色：仅本次有效",
            Self::SaveFailed => "角色：保存失败，保留原选择",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub selected_character_id: String,
    pub revision: u64,
    pub persistence: Persistence,
}
struct State {
    snapshot: Snapshot,
    stopped: bool,
}
type Reply = mpsc::Sender<Result<Snapshot, String>>;
enum Request {
    Select(String, Reply),
    Stop,
}
pub struct Service {
    catalog: Catalog,
    state: Arc<Mutex<State>>,
    sender: mpsc::Sender<Request>,
}
impl Service {
    pub fn start(
        path: Option<PathBuf>,
        notify: impl Fn(Snapshot) + Send + 'static,
    ) -> Result<Self, String> {
        let catalog = Catalog::builtin()?;
        let (mut store, snapshot) = Store::load(path, &catalog);
        let state = Arc::new(Mutex::new(State {
            snapshot,
            stopped: false,
        }));
        let (sender, receiver) = mpsc::channel();
        let shared = state.clone();
        std::thread::Builder::new()
            .name("character-preferences".into())
            .spawn(move || {
                run_worker(shared, receiver, |id| store.save(id), notify);
            })
            .map_err(|_| "cannot start character worker")?;
        Ok(Self {
            catalog,
            state,
            sender,
        })
    }
    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().unwrap().snapshot.clone()
    }
    pub fn running_snapshot(&self) -> Option<Snapshot> {
        let state = self.state.lock().unwrap();
        (!state.stopped).then(|| state.snapshot.clone())
    }
    pub fn select(&self, id: String) -> Result<mpsc::Receiver<Result<Snapshot, String>>, String> {
        if !self.catalog.contains(&id) {
            return Err("unknown character id".into());
        }
        let state = self.state.lock().unwrap();
        if state.stopped {
            return Err("character selection is stopping".into());
        }
        let (reply, receiver) = mpsc::channel();
        self.sender
            .send(Request::Select(id, reply))
            .map_err(|_| "character worker unavailable")?;
        Ok(receiver)
    }
    pub fn stop(&self) {
        let mut state = self.state.lock().unwrap();
        state.stopped = true;
        let _ = self.sender.send(Request::Stop);
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.stop();
    }
}
fn run_worker(
    state: Arc<Mutex<State>>,
    receiver: mpsc::Receiver<Request>,
    mut save: impl FnMut(&str) -> Result<Persistence, String>,
    notify: impl Fn(Snapshot),
) {
    while let Ok(Request::Select(id, reply)) = receiver.recv() {
        if state.lock().unwrap().stopped {
            let _ = reply.send(Err("character selection is stopping".into()));
            continue;
        }
        let result = save(&id); // No shared lock during any file IO.
        if let Err(error) = &result {
            eprintln!("Character selection failed: {error}");
        }
        let (snapshot, result) = {
            let mut state = state.lock().unwrap();
            if state.stopped {
                let _ = reply.send(Err("character selection stopped".into()));
                continue;
            }
            state.snapshot.revision += 1;
            let result = match result {
                Ok(persistence) => {
                    state.snapshot.selected_character_id = id;
                    state.snapshot.persistence = persistence;
                    Ok(state.snapshot.clone())
                }
                Err(error) => {
                    state.snapshot.persistence = Persistence::SaveFailed;
                    Err(error)
                }
            };
            (state.snapshot.clone(), result)
        };
        notify(snapshot);
        let _ = reply.send(result);
    }
}
#[cfg(test)]
mod tests;
