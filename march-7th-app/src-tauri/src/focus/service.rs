//! One worker owns both the focus engine and its store. Publication is a commit.
use super::{
    model::{Command, Data, Engine, Error, Session, Time, MAX_SAFE},
    store::{decode, Storage},
};
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{mpsc, Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
const CHECKPOINT_MS: u64 = 60_000;
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: u64,
    pub data: Option<Data>,
    pub error: Option<&'static str>,
    pub stopped: bool,
}
impl Snapshot {
    fn initial() -> Self {
        Self {
            revision: 0,
            data: None,
            error: Some("loading"),
            stopped: false,
        }
    }
}
/// Pending feedback is durable reviewable state. This flag is only a live,
/// successfully committed transition; it is never recovered from the file.

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub snapshot: Snapshot,
    pub completed_now: bool,
    /// A transient worker error; never part of the durable snapshot or export.
    pub error: Option<&'static str>,
}
struct Core<S: Storage> {
    completed_now: bool,
    store: S,
    engine: Option<Engine>,
    snapshot: Snapshot,
    bytes: Vec<u8>,
    loaded: Result<Data, Error>,
}
impl<S: Storage> Core<S> {
    fn load(mut store: S) -> Self {
        let result = store.load();
        let loaded = result
            .as_ref()
            .map_err(|e| *e)
            .and_then(|bytes| decode(bytes));
        let snapshot = Snapshot {
            revision: 1,
            data: loaded.as_ref().ok().cloned(),
            error: loaded.as_ref().err().map(|e| e.code),
            stopped: false,
        };
        Self {
            store,
            completed_now: false,
            engine: None,
            snapshot,
            bytes: result.unwrap_or_default(),
            loaded,
        }
    }
    // Background progress is checkpointed once a minute or at an earlier deadline, never
    // intentionally beyond the saved remaining duration. Explicit commands
    // still enter pump immediately and commit their complete next state.
    fn checkpoint_delay(&self) -> Option<Duration> {
        if self.snapshot.error.is_some() {
            return None;
        }
        match self.snapshot.data.as_ref()?.session {
            Session::Running { remaining_ms, .. } => {
                Some(Duration::from_millis(remaining_ms.min(CHECKPOINT_MS)))
            }
            _ => None,
        }
    }
    fn pump(&mut self, command: Command, time: Time) -> Result<bool, Error> {
        let mut next = match &self.engine {
            Some(engine) => engine.clone(),
            None => Engine::restore(self.loaded.clone()?, time)?,
        };
        let transition = next.step(command, time)?;
        let data_changed = self.snapshot.data.as_ref() != Some(&next.data);
        let changed = data_changed || self.snapshot.error.is_some();
        if changed {
            if self.snapshot.revision >= MAX_SAFE {
                return Err(Error {
                    code: "sequenceExhausted",
                });
            }
            if data_changed {
                let bytes = serde_json::to_vec(&next.data).map_err(|_| Error {
                    code: "invalidState",
                })?;
                self.store.save(&bytes)?;
                // No externally visible state changes before the atomic replacement succeeds.
                self.bytes = bytes;
                self.snapshot.data = Some(next.data.clone());
            }
            self.snapshot.error = None;
            self.snapshot.revision += 1;
        }
        self.completed_now = transition.completed_now;
        self.engine = Some(next);
        Ok(changed)
    }
}
type Reply = mpsc::Sender<Result<Snapshot, Error>>;
struct Request {
    command: Command,
    reply: Reply,
}
struct State {
    error: Option<&'static str>,
    snapshot: Snapshot,
    bytes: Vec<u8>,
    queue: VecDeque<Request>,
}
struct Shared {
    state: Mutex<State>,
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
            state: Mutex::new(State {
                error: None,
                snapshot: Snapshot::initial(),
                bytes: Vec::new(),
                queue: VecDeque::new(),
            }),
            wake: Condvar::new(),
        });
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("focus".into())
            .spawn(move || run_worker(worker, store, clock, notify))
            .map_err(|_| Error {
                code: "workerUnavailable",
            })?;
        Ok(Self { shared })
    }
    /// Read diagnostics as well as committed progress for a newly opened view.
    /// Reading never consumes an error or replays a completion notification.
    pub fn current(&self) -> Change {
        let state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        Change {
            snapshot: state.snapshot.clone(),
            completed_now: false,
            error: state.error,
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot
            .clone()
    }
    /// Freeze only the last published commit, including while a newer save is in flight.
    /// This never samples a clock, writes, or waits for disk IO/the worker.
    pub fn export_bytes(&self) -> Result<Vec<u8>, Error> {
        let state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.snapshot.stopped {
            return Err(Error { code: "stopped" });
        }
        if let Some(code) = state.snapshot.error {
            return Err(Error { code });
        }
        Ok(state.bytes.clone())
    }
    pub fn command(
        &self,
        command: Command,
    ) -> Result<mpsc::Receiver<Result<Snapshot, Error>>, Error> {
        let mut state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.snapshot.stopped {
            return Err(Error { code: "stopped" });
        }
        if state.queue.len() >= 128 {
            return Err(Error { code: "queueFull" });
        }
        let (reply, receiver) = mpsc::channel();
        state.queue.push_back(Request { command, reply });
        self.shared.wake.notify_one();
        Ok(receiver)
    }
    pub fn stop(&self) {
        let mut state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        state.snapshot.stopped = true;
        for request in state.queue.drain(..) {
            let _ = request.reply.send(Err(Error { code: "stopped" }));
        }
        self.shared.wake.notify_one();
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.stop();
    }
}
fn publish<S: Storage>(shared: &Shared, core: &Core<S>, notify: &impl Fn(Change)) -> bool {
    {
        let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.snapshot.stopped {
            return false;
        }
        state.error = None;
        state.snapshot = core.snapshot.clone();
        state.bytes = core.bytes.clone();
    }
    // Native callback queues main-thread work and rechecks stop at actual emission.
    notify(Change {
        snapshot: core.snapshot.clone(),
        completed_now: core.completed_now,
        error: None,
    });
    true
}
fn report_error(shared: &Shared, error: Error, notify: &impl Fn(Change)) {
    let snapshot = {
        let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.snapshot.stopped {
            return;
        }
        state.error = Some(error.code);
        state.snapshot.clone()
    };
    notify(Change {
        snapshot,
        completed_now: false,
        error: Some(error.code),
    });
}
fn run_worker<S: Storage>(
    shared: Arc<Shared>,
    store: S,
    mut clock: impl FnMut() -> Result<Time, Error>,
    notify: impl Fn(Change),
) {
    let mut core = Core::load(store);
    if shared
        .state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .snapshot
        .stopped
    {
        return;
    }
    // Restoration may itself change durable progress. Keep old loaded data on failure.
    let sampled_at = Instant::now();
    if let Err(error) = clock().and_then(|time| core.pump(Command::Tick, time)) {
        core.snapshot.error = Some(error.code);
    }
    if !publish(&shared, &core, &notify) {
        return;
    }
    let mut last_tick_error = None;
    let mut wake_at = core.checkpoint_delay().map(|delay| sampled_at + delay);
    loop {
        let request = {
            let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if state.snapshot.stopped {
                    return;
                }
                // Keep one absolute deadline through spurious wakes/invalid commands.
                // A due checkpoint wins over the queue, preventing command floods
                // from indefinitely postponing natural completion.
                if wake_at.is_some_and(|deadline| Instant::now() >= deadline) {
                    break None;
                }
                if let Some(request) = state.queue.pop_front() {
                    break Some(request);
                }
                if let Some(deadline) = wake_at {
                    let (next, _) = shared
                        .wake
                        .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
                        .unwrap_or_else(|e| e.into_inner());
                    state = next;
                } else {
                    state = shared.wake.wait(state).unwrap_or_else(|e| e.into_inner());
                }
            }
        };
        if shared
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot
            .stopped
        {
            if let Some(request) = request {
                let _ = request.reply.send(Err(Error { code: "stopped" }));
            }
            return;
        }
        let sampled_at = Instant::now();
        let result = clock().and_then(|time| {
            core.pump(request.as_ref().map_or(Command::Tick, |r| r.command), time)
        });
        if result.is_ok() {
            // Anchor the wait before sampling/IO, so slow saves do not extend a session.
            wake_at = core.checkpoint_delay().map(|delay| sampled_at + delay);
        } else if request.is_none() {
            // Failed background IO retains committed progress; bounded retry avoids
            // a hot loop when the original deadline is already in the past.
            wake_at = core
                .checkpoint_delay()
                .map(|_| Instant::now() + Duration::from_millis(CHECKPOINT_MS));
        }
        let reply = match result {
            Ok(changed) => {
                let recovered = last_tick_error.take().is_some();
                if changed || recovered {
                    publish(&shared, &core, &notify);
                }
                Ok(core.snapshot.clone())
            }
            Err(error) => {
                if request.is_none() && last_tick_error != Some(error.code) {
                    report_error(&shared, error, &notify);
                    last_tick_error = Some(error.code);
                }
                Err(error)
            }
        };
        if let Some(request) = request {
            let state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            let _ = request.reply.send(if state.snapshot.stopped {
                Err(Error { code: "stopped" })
            } else {
                reply
            });
        }
    }
}
#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
