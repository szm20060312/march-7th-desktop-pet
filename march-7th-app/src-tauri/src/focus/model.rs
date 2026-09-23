//! Pure single-session focus rules. The caller supplies clocks and owns persistence
//! and presentation; this module never reads the system clock or emits UI events.

use serde::{Deserialize, Serialize};

pub const VERSION: u8 = 1;
pub const MIN_DURATION_MS: u64 = 60_000;
pub const MAX_DURATION_MS: u64 = 4 * 60 * 60_000;
pub const MAX_TRUSTED_GAP_MS: u64 = 24 * 60 * 60_000;
pub const MAX_SAFE: u64 = 9_007_199_254_740_991;
pub const MAX_UTC: i64 = 253_402_300_799_999;
// Allow sampling jitter, but treat a meaningful wall/monotonic split as
// uncertain sleep or clock correction rather than silently extending a timer.
const MAX_CLOCK_DRIFT_MS: u64 = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Time {
    pub utc_ms: i64,
    pub monotonic_ms: u64,
}

impl Time {
    fn validate(self) -> Result<(), Error> {
        if !(0..=MAX_UTC).contains(&self.utc_ms) || self.monotonic_ms > MAX_SAFE {
            return Err(Error::new("invalidTime"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Data {
    pub version: u8,
    pub session: Session,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            version: VERSION,
            session: Session::Idle {},
        }
    }
}

impl Data {
    pub fn validate(&self) -> Result<(), Error> {
        if self.version != VERSION {
            return Err(Error::new("unsupportedVersion"));
        }
        let duration = match self.session {
            Session::Idle {} => return Ok(()),
            Session::Running {
                duration_ms,
                remaining_ms,
                anchor_utc_ms,
            } => {
                if remaining_ms == 0
                    || remaining_ms > duration_ms
                    || !valid_deadline(anchor_utc_ms, remaining_ms)
                {
                    return Err(Error::new("invalidState"));
                }
                duration_ms
            }
            Session::Paused {
                duration_ms,
                remaining_ms,
            }
            | Session::Interrupted {
                duration_ms,
                remaining_ms,
            } => {
                if remaining_ms == 0 || remaining_ms > duration_ms {
                    return Err(Error::new("invalidState"));
                }
                duration_ms
            }
            Session::Finished {
                duration_ms,
                outcome,
                feedback,
            } => {
                if (outcome == Outcome::Natural && feedback == Feedback::None)
                    || (outcome != Outcome::Natural && feedback != Feedback::None)
                {
                    return Err(Error::new("invalidState"));
                }
                duration_ms
            }
        };
        if !(MIN_DURATION_MS..=MAX_DURATION_MS).contains(&duration) {
            return Err(Error::new("invalidState"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
pub enum Session {
    Idle {},
    Running {
        duration_ms: u64,
        remaining_ms: u64,
        anchor_utc_ms: i64,
    },
    Paused {
        duration_ms: u64,
        remaining_ms: u64,
    },
    /// The clocks did not provide a trustworthy elapsed duration. Resume is
    /// an explicit user decision; the last known remaining time is retained.
    Interrupted {
        duration_ms: u64,
        remaining_ms: u64,
    },
    Finished {
        duration_ms: u64,
        outcome: Outcome,
        feedback: Feedback,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Natural,
    EndedEarly,
    Abandoned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Feedback {
    None,
    Pending,
    Dismissed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Tick,
    Start { duration_ms: u64 },
    Pause,
    Resume,
    EndEarly,
    Abandon,
    DismissFeedback,
}

/// Only a live transition returns `completed_now = true`. Restoring an expired
/// snapshot preserves pending review but deliberately produces no event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Transition {
    pub completed_now: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub code: &'static str,
}

impl Error {
    fn new(code: &'static str) -> Self {
        Self { code }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Engine {
    pub data: Data,
    monotonic_anchor_ms: Option<u64>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            data: Data::default(),
            monotonic_anchor_ms: None,
        }
    }

    /// Rebuild the volatile monotonic anchor from a bounded UTC gap. A past
    /// deadline becomes reviewable state, never a newly emitted completion.
    pub fn restore(mut data: Data, now: Time) -> Result<Self, Error> {
        data.validate()?;
        now.validate()?;
        let mut monotonic_anchor_ms = None;
        if let Session::Running {
            duration_ms,
            remaining_ms,
            anchor_utc_ms,
        } = data.session
        {
            let elapsed = now
                .utc_ms
                .checked_sub(anchor_utc_ms)
                .and_then(|value| u64::try_from(value).ok());
            data.session = match elapsed {
                Some(elapsed) if elapsed <= MAX_TRUSTED_GAP_MS && elapsed >= remaining_ms => {
                    Session::Finished {
                        duration_ms,
                        outcome: Outcome::Natural,
                        feedback: Feedback::Pending,
                    }
                }
                Some(elapsed) if elapsed <= MAX_TRUSTED_GAP_MS => {
                    monotonic_anchor_ms = Some(now.monotonic_ms);
                    Session::Running {
                        duration_ms,
                        remaining_ms: remaining_ms - elapsed,
                        anchor_utc_ms: now.utc_ms,
                    }
                }
                _ => Session::Interrupted {
                    duration_ms,
                    remaining_ms,
                },
            };
        }
        Ok(Self {
            data,
            monotonic_anchor_ms,
        })
    }

    /// Compute the next complete state first, then publish it atomically to
    /// this in-memory engine. A future persistence service must write before
    /// publishing its own externally visible snapshot.
    pub fn step(&mut self, command: Command, now: Time) -> Result<Transition, Error> {
        now.validate()?;
        self.data.validate()?;
        if let Command::Start { duration_ms } = command {
            if !(MIN_DURATION_MS..=MAX_DURATION_MS).contains(&duration_ms) {
                return Err(Error::new("invalidDuration"));
            }
            if !matches!(
                self.data.session,
                Session::Idle {} | Session::Finished { .. }
            ) {
                return Err(Error::new("alreadyActive"));
            }
        }
        let mut next = self.clone();
        let mut result = Transition::default();
        if !matches!(command, Command::Start { .. }) {
            result.completed_now = next.advance_clock(now)?;
        }
        next.apply(command, now)?;
        next.data.validate()?;
        *self = next;
        Ok(result)
    }

    fn advance_clock(&mut self, now: Time) -> Result<bool, Error> {
        let Session::Running {
            duration_ms,
            remaining_ms,
            anchor_utc_ms,
        } = self.data.session
        else {
            return Ok(false);
        };
        let anchor = self.monotonic_anchor_ms.ok_or(Error::new("invalidState"))?;
        let mono_elapsed = now.monotonic_ms.checked_sub(anchor);
        let wall_elapsed = now
            .utc_ms
            .checked_sub(anchor_utc_ms)
            .and_then(|value| u64::try_from(value).ok());
        let trustworthy = match (mono_elapsed, wall_elapsed) {
            (Some(mono), Some(wall)) => {
                mono <= MAX_TRUSTED_GAP_MS
                    && wall <= MAX_TRUSTED_GAP_MS
                    && mono.abs_diff(wall) <= MAX_CLOCK_DRIFT_MS
            }
            _ => false,
        };
        if !trustworthy {
            self.data.session = Session::Interrupted {
                duration_ms,
                remaining_ms,
            };
            self.monotonic_anchor_ms = None;
            return Ok(false);
        }
        let elapsed = mono_elapsed.expect("trustworthy elapsed exists");
        if elapsed >= remaining_ms {
            self.data.session = Session::Finished {
                duration_ms,
                outcome: Outcome::Natural,
                feedback: Feedback::Pending,
            };
            self.monotonic_anchor_ms = None;
            return Ok(true);
        }
        self.data.session = Session::Running {
            duration_ms,
            remaining_ms: remaining_ms - elapsed,
            // Preserve any tolerated wall-clock skew across ticks. Rebasing to
            // `now.utc_ms` would discard the gap and extend the session.
            anchor_utc_ms: anchor_utc_ms + elapsed as i64,
        };
        self.monotonic_anchor_ms = Some(now.monotonic_ms);
        Ok(false)
    }

    fn apply(&mut self, command: Command, now: Time) -> Result<(), Error> {
        self.data.session = match (command, self.data.session) {
            (Command::Tick, state) => state,
            (Command::Start { duration_ms }, Session::Idle {} | Session::Finished { .. }) => {
                if !valid_deadline(now.utc_ms, duration_ms) {
                    return Err(Error::new("invalidTime"));
                }
                self.monotonic_anchor_ms = Some(now.monotonic_ms);
                Session::Running {
                    duration_ms,
                    remaining_ms: duration_ms,
                    anchor_utc_ms: now.utc_ms,
                }
            }
            (
                Command::Pause,
                Session::Running {
                    duration_ms,
                    remaining_ms,
                    ..
                },
            ) => {
                self.monotonic_anchor_ms = None;
                Session::Paused {
                    duration_ms,
                    remaining_ms,
                }
            }
            (Command::Pause, state @ Session::Paused { .. }) => state,
            (
                Command::Resume,
                Session::Paused {
                    duration_ms,
                    remaining_ms,
                }
                | Session::Interrupted {
                    duration_ms,
                    remaining_ms,
                },
            ) => {
                if !valid_deadline(now.utc_ms, remaining_ms) {
                    return Err(Error::new("invalidTime"));
                }
                self.monotonic_anchor_ms = Some(now.monotonic_ms);
                Session::Running {
                    duration_ms,
                    remaining_ms,
                    anchor_utc_ms: now.utc_ms,
                }
            }
            (Command::Resume, state @ Session::Running { .. }) => state,
            (
                Command::EndEarly,
                Session::Running { duration_ms, .. }
                | Session::Paused { duration_ms, .. }
                | Session::Interrupted { duration_ms, .. },
            ) => {
                self.monotonic_anchor_ms = None;
                Session::Finished {
                    duration_ms,
                    outcome: Outcome::EndedEarly,
                    feedback: Feedback::None,
                }
            }
            (
                Command::EndEarly,
                state @ Session::Finished {
                    outcome: Outcome::EndedEarly,
                    ..
                },
            ) => state,
            (
                Command::Abandon,
                Session::Running { duration_ms, .. }
                | Session::Paused { duration_ms, .. }
                | Session::Interrupted { duration_ms, .. },
            ) => {
                self.monotonic_anchor_ms = None;
                Session::Finished {
                    duration_ms,
                    outcome: Outcome::Abandoned,
                    feedback: Feedback::None,
                }
            }
            (
                Command::Abandon,
                state @ Session::Finished {
                    outcome: Outcome::Abandoned,
                    ..
                },
            ) => state,
            (
                Command::DismissFeedback,
                Session::Finished {
                    duration_ms,
                    outcome: Outcome::Natural,
                    ..
                },
            ) => Session::Finished {
                duration_ms,
                outcome: Outcome::Natural,
                feedback: Feedback::Dismissed,
            },
            (
                Command::Pause | Command::Resume | Command::EndEarly | Command::Abandon,
                state @ Session::Finished {
                    outcome: Outcome::Natural,
                    ..
                },
            ) => state,
            _ => return Err(Error::new("invalidTransition")),
        };
        Ok(())
    }
}

fn valid_deadline(anchor_utc_ms: i64, remaining_ms: u64) -> bool {
    (0..=MAX_UTC).contains(&anchor_utc_ms)
        && i64::try_from(remaining_ms)
            .ok()
            .and_then(|remaining| anchor_utc_ms.checked_add(remaining))
            .is_some_and(|deadline| deadline <= MAX_UTC)
}
