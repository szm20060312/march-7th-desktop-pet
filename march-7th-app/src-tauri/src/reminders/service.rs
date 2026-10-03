use super::{
    model::*,
    store::{Persistence, SaveStatus, Storage},
};
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{mpsc, Arc, Condvar, Mutex},
    time::Duration,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: u64,
    pub settings: Settings,
    pub progress: [Progress; 3],
    pub paused: bool,
    pub quiet: Option<Quiet>,
    pub snooze_pending: bool,
    pub presentation: Option<Presentation>,
    pub persistence: Persistence,
    pub runtime_error: Option<&'static str>,
    pub stopped: bool,
}
impl Snapshot {
    fn initial() -> Self {
        Self::from_engine(
            &Engine::new(Data::default(), false).expect("valid built-in reminder defaults"),
            Persistence::new(SaveStatus::Loading, None),
        )
    }
    fn from_engine(engine: &Engine, persistence: Persistence) -> Self {
        Self {
            revision: 0,
            settings: engine.data.settings.clone(),
            progress: engine.data.progress.clone(),
            paused: engine.data.paused,
            quiet: engine.data.quiet.clone(),
            snooze_pending: engine.data.snooze_pending,
            presentation: engine.presentation.clone(),
            persistence,
            runtime_error: None,
            stopped: false,
        }
    }
}
pub struct Change {
    pub snapshot: Snapshot,
    pub response: Option<Response>,
}
// Owned by the one worker; neither Engine nor Storage is behind the shared UI lock.
pub struct Core<S: Storage> {
    engine: Engine,
    store: S,
    snapshot: Snapshot,
}
impl<S: Storage> Core<S> {
    pub fn load(mut store: S) -> Self {
        let (data, mut persistence) = store.load();
        let engine = Engine::new(data, persistence.protected()).unwrap_or_else(|_| {
            persistence = Persistence::new(SaveStatus::ReadOnly, Some("invalidFile"));
            Engine::new(Data::default(), true).expect("valid built-in reminder defaults")
        });
        let mut snapshot = Snapshot::from_engine(&engine, persistence);
        snapshot.revision = 1;
        Self {
            engine,
            store,
            snapshot,
        }
    }
    pub fn pump(
        &mut self,
        command: Option<Command>,
        time: Result<Time, Error>,
    ) -> Result<Option<Change>, Error> {
        if self.snapshot.revision >= MAX_SAFE {
            return Err(Error::new("sequenceExhausted"));
        }
        let time = match time.and_then(|t| {
            t.validate()?;
            Ok(t)
        }) {
            Ok(t) => t,
            Err(error) => return self.clock_failure(error),
        };
        let previous = self.engine.clone();
        let retry = command.is_some() && self.snapshot.persistence.status == SaveStatus::Unsaved;
        let response = match self.engine.step(command, time) {
            Ok(response) => response,
            Err(error) if error.code == "invalidTime" => return self.clock_failure(error),
            Err(error) => return Err(error),
        };
        let mut persistence = self.snapshot.persistence.clone();
        if previous.data != self.engine.data || retry {
            persistence = self.store.save(&self.engine.data);
            if persistence.protected() {
                self.engine.protected = true;
                if self
                    .engine
                    .presentation
                    .as_ref()
                    .is_some_and(|p| p.mode == Mode::Automatic)
                {
                    self.engine.presentation = None;
                }
            }
        }
        let mut snapshot = Snapshot::from_engine(&self.engine, persistence);
        snapshot.revision = self.snapshot.revision;
        if snapshot != self.snapshot {
            snapshot.revision += 1;
            self.snapshot = snapshot.clone();
            Ok(Some(Change { snapshot, response }))
        } else if response.is_some() {
            // Repeating SnoozeAll at the exact same clock instant is still an explicit action.
            self.snapshot.revision += 1;
            Ok(Some(Change {
                snapshot: self.snapshot.clone(),
                response,
            }))
        } else {
            Ok(None)
        }
    }
    fn clock_failure(&mut self, error: Error) -> Result<Option<Change>, Error> {
        if self.snapshot.runtime_error == Some(error.code) {
            return Err(error);
        }
        self.snapshot.runtime_error = Some(error.code);
        if self
            .engine
            .presentation
            .as_ref()
            .is_some_and(|p| p.mode == Mode::Automatic)
        {
            self.engine.presentation = None;
            self.snapshot.presentation = None;
        }
        self.snapshot.revision += 1;
        Ok(Some(Change {
            snapshot: self.snapshot.clone(),
            response: None,
        }))
    }
}

type Reply = mpsc::Sender<Result<Snapshot, Error>>;
struct Request {
    command: Command,
    reply: Reply,
}
struct SharedState {
    snapshot: Snapshot,
    queue: VecDeque<Request>,
}
struct Shared {
    state: Mutex<SharedState>,
    wake: Condvar,
}
pub struct Service {
    shared: Arc<Shared>,
}
impl Service {
    pub fn start<S: Storage + 'static>(
        store: S,
        clock: impl FnMut() -> Result<Time, Error> + Send + 'static,
        notify: impl Fn(Change) + Send + 'static,
    ) -> Result<Self, Error> {
        let shared = Arc::new(Shared {
            state: Mutex::new(SharedState {
                snapshot: Snapshot::initial(),
                queue: VecDeque::new(),
            }),
            wake: Condvar::new(),
        });
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("reminders".into())
            .spawn(move || run_worker(worker, store, clock, notify))
            .map_err(|_| Error::new("workerUnavailable"))?;
        Ok(Self { shared })
    }
    pub fn snapshot(&self) -> Snapshot {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot
            .clone()
    }
    pub fn command(
        &self,
        command: Command,
    ) -> Result<mpsc::Receiver<Result<Snapshot, Error>>, Error> {
        let mut state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.snapshot.stopped {
            return Err(Error::new("stopped"));
        }
        if state.queue.len() >= 128 {
            return Err(Error::new("queueFull"));
        }
        let (reply, receiver) = mpsc::channel();
        state.queue.push_back(Request { command, reply });
        self.shared.wake.notify_one();
        Ok(receiver)
    }
    pub fn stop(&self) {
        let mut state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.snapshot.stopped {
            return;
        }
        state.snapshot.stopped = true;
        state.snapshot.presentation = None;
        state.snapshot.revision = state.snapshot.revision.saturating_add(1).min(MAX_SAFE);
        for request in state.queue.drain(..) {
            let _ = request.reply.send(Err(Error::new("stopped")));
        }
        self.shared.wake.notify_one();
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.stop();
    }
}
fn publish(shared: &Shared, change: Change, notify: &impl Fn(Change)) -> bool {
    {
        let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.snapshot.stopped {
            return false;
        }
        state.snapshot = change.snapshot.clone();
    }
    // This callback only queues a native main-thread reconciliation. That callback
    // rechecks lifecycle, so an exit between unlock and enqueue cannot emit late events.
    notify(change);
    true
}
fn run_worker<S: Storage>(
    shared: Arc<Shared>,
    store: S,
    mut clock: impl FnMut() -> Result<Time, Error>,
    notify: impl Fn(Change),
) {
    let mut core = Core::load(store); // Startup file reads also stay off the UI thread.
                                      // Loading can be in flight during exit. Its completion must not start a new
                                      // clock sample, rule transition or save after the stop boundary.
    if shared
        .state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .snapshot
        .stopped
    {
        return;
    }
    let initial = core
        .pump(None, clock())
        .ok()
        .flatten()
        .unwrap_or_else(|| Change {
            snapshot: core.snapshot.clone(),
            response: None,
        });
    if !publish(&shared, initial, &notify) {
        return;
    }
    loop {
        let request = {
            let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if state.snapshot.stopped {
                    return;
                }
                if let Some(request) = state.queue.pop_front() {
                    break Some(request);
                }
                if core.engine.needs_clock() {
                    let (next, timeout) = shared
                        .wake
                        .wait_timeout(state, Duration::from_secs(1))
                        .unwrap_or_else(|e| e.into_inner());
                    state = next;
                    if state.snapshot.stopped {
                        return;
                    }
                    if timeout.timed_out() {
                        break None;
                    }
                } else {
                    state = shared.wake.wait(state).unwrap_or_else(|e| e.into_inner());
                }
            }
        };
        // Stop can race the start of IO; once begun, atomic replace may finish but
        // there is no success reply/state publication/event after the stop boundary.
        if shared
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot
            .stopped
        {
            if let Some(request) = request {
                let _ = request.reply.send(Err(Error::new("stopped")));
            }
            return;
        }
        let result = core.pump(request.as_ref().map(|r| r.command.clone()), clock());
        let reply = match result {
            Ok(change) => {
                if let Some(change) = change {
                    publish(&shared, change, &notify);
                }
                core.snapshot
                    .runtime_error
                    .map_or_else(|| Ok(core.snapshot.clone()), |code| Err(Error::new(code)))
            }
            Err(error) => Err(error),
        };
        if let Some(request) = request {
            let state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            // Linearize the nonblocking reply with stop. A reply committed before
            // exit may be delivered later; consumers still discard destroyed views.
            let _ = request.reply.send(if state.snapshot.stopped {
                Err(Error::new("stopped"))
            } else {
                reply
            });
        }
    }
}

#[cfg(test)]
mod tests;
