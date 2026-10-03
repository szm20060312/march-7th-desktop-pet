use serde::{Deserialize, Serialize};

pub const MAX_SAFE: u64 = 9_007_199_254_740_991;
pub const MAX_UTC: i64 = 253_402_300_799_999;
pub const MINUTE: i64 = 60_000;
pub const AUTO_PRESENTATION_MS: i64 = 120_000;
const CHOICES_GRACE_MS: i64 = 120_000;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Id {
    Water,
    Move,
    Eyes,
}
pub const IDS: [Id; 3] = [Id::Water, Id::Move, Id::Eyes];
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemSettings {
    pub id: Id,
    pub enabled: bool,
    pub interval_minutes: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ActiveHours {
    AllDay {},
    Daily { start: u16, end: u16 },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub items: [ItemSettings; 3],
    pub active_hours: ActiveHours,
    pub snooze_minutes: u16,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            items: IDS.map(|id| ItemSettings {
                id,
                enabled: false,
                interval_minutes: if id == Id::Eyes { 30 } else { 60 },
            }),
            active_hours: ActiveHours::Daily {
                start: 540,
                end: 1320,
            },
            snooze_minutes: 10,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Progress {
    pub id: Id,
    pub next_due_at: Option<i64>,
    pub pending: bool,
    pub auto_handled: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Quiet {
    pub until: i64,
    pub duration_minutes: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Data {
    pub version: u8,
    pub settings: Settings,
    pub progress: [Progress; 3],
    pub paused: bool,
    pub quiet: Option<Quiet>,
    pub snooze_pending: bool,
}
impl Default for Data {
    fn default() -> Self {
        Self {
            version: 1,
            settings: Settings::default(),
            progress: IDS.map(|id| Progress {
                id,
                next_due_at: None,
                pending: false,
                auto_handled: false,
            }),
            paused: false,
            quiet: None,
            snooze_pending: false,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Time {
    pub utc_ms: i64,
    pub local_minute: i32,
    pub monotonic_ms: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub code: &'static str,
}
impl Error {
    pub fn new(code: &'static str) -> Self {
        Self { code }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Command {
    UpdateSettings {
        settings: Settings,
    },
    Complete {
        id: Id,
    },
    SnoozeAll {},
    SetPaused {
        paused: bool,
    },
    ShowPending {},
    ExtendChoices {
        #[serde(rename = "presentationId")]
        presentation_id: u64,
    },
    Dismiss {
        #[serde(rename = "presentationId")]
        presentation_id: u64,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Automatic,
    Manual,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Presentation {
    pub id: u64,
    pub mode: Mode,
    pub items: Vec<Id>,
    pub closes_at: Option<i64>,
    #[serde(skip)]
    pub monotonic_deadline: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Response {
    Complete { id: Id },
    SnoozeAll,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Engine {
    pub data: Data,
    pub presentation: Option<Presentation>,
    next_id: u64,
    pub protected: bool,
}
impl Engine {
    pub fn new(data: Data, protected: bool) -> Result<Self, Error> {
        data.validate()?;
        Ok(Self {
            data,
            presentation: None,
            next_id: 0,
            protected,
        })
    }
    // Commit a whole transition only after all input and arithmetic validation.
    pub fn step(
        &mut self,
        command: Option<Command>,
        time: Time,
    ) -> Result<Option<Response>, Error> {
        time.validate()?;
        let mut next = self.clone();
        let response = next.advance(command, time)?;
        next.data.validate()?;
        *self = next;
        Ok(response)
    }
    pub fn needs_clock(&self) -> bool {
        self.data.settings.items.iter().any(|i| i.enabled)
            || self.data.quiet.is_some()
            || self
                .presentation
                .as_ref()
                .is_some_and(|p| p.mode == Mode::Automatic)
    }
    fn pending(&self) -> Vec<Id> {
        self.data
            .progress
            .iter()
            .filter(|p| p.pending)
            .map(|p| p.id)
            .collect()
    }
    fn update_manual(&mut self) -> bool {
        let pending = self.pending();
        if let Some(p) = self
            .presentation
            .as_mut()
            .filter(|p| p.mode == Mode::Manual)
        {
            p.items = pending;
            for progress in &mut self.data.progress {
                if progress.pending {
                    progress.auto_handled = true;
                }
            }
            true
        } else {
            false
        }
    }
    fn present(&mut self, mode: Mode, items: Vec<Id>, time: Time) -> Result<(), Error> {
        self.next_id = self
            .next_id
            .checked_add(1)
            .filter(|id| *id <= MAX_SAFE)
            .ok_or(Error::new("sequenceExhausted"))?;
        self.presentation = Some(Presentation {
            id: self.next_id,
            mode,
            items,
            closes_at: if mode == Mode::Automatic {
                Some(add_utc(time.utc_ms, AUTO_PRESENTATION_MS)?)
            } else {
                None
            },
            monotonic_deadline: if mode == Mode::Automatic {
                Some(
                    time.monotonic_ms
                        .checked_add(AUTO_PRESENTATION_MS as u64)
                        .filter(|v| *v <= MAX_SAFE)
                        .ok_or(Error::new("invalidTime"))?,
                )
            } else {
                None
            },
        });
        Ok(())
    }
    fn advance(&mut self, command: Option<Command>, time: Time) -> Result<Option<Response>, Error> {
        if let Some(Command::UpdateSettings { settings }) = &command {
            settings.validate()?;
            if self.protected {
                return Err(Error::new("readOnly"));
            }
        }
        if matches!(command,Some(Command::Dismiss{presentation_id} | Command::ExtendChoices{presentation_id}) if presentation_id==0 || presentation_id>MAX_SAFE)
        {
            return Err(Error::new("invalidPresentationId"));
        }
        // Bound waiting after a backward jump, including the first pump after load.
        for (p, s) in self.data.progress.iter_mut().zip(&self.data.settings.items) {
            if let Some(due) = p.next_due_at.as_mut() {
                let interval = i64::from(s.interval_minutes) * MINUTE;
                if *due - time.utc_ms > interval {
                    *due = add_utc(time.utc_ms, interval)?;
                }
                if *due <= time.utc_ms {
                    p.next_due_at = None;
                    p.pending = true;
                    p.auto_handled = false;
                }
            }
        }
        if let Some(quiet) = self.data.quiet.as_mut() {
            let duration = i64::from(quiet.duration_minutes) * MINUTE;
            if quiet.until - time.utc_ms > duration {
                quiet.until = add_utc(time.utc_ms, duration)?;
            }
            if quiet.until <= time.utc_ms {
                self.data.quiet = None;
            }
        }
        if self.presentation.as_ref().is_some_and(|p| {
            p.mode == Mode::Automatic
                && (p.closes_at.is_some_and(|t| time.utc_ms >= t)
                    || p.monotonic_deadline.is_some_and(|t| time.monotonic_ms >= t))
        }) {
            self.presentation = None;
        }
        // Reconcile arrivals in an already-open manual view before a command can
        // close it, including when dismiss is the first step after quiet expires.
        let was_manual = self.update_manual();
        let mut response = None;
        match command {
            Some(Command::UpdateSettings { settings }) => {
                for ((p, old), new) in self
                    .data
                    .progress
                    .iter_mut()
                    .zip(&self.data.settings.items)
                    .zip(&settings.items)
                {
                    if old.enabled != new.enabled || old.interval_minutes != new.interval_minutes {
                        p.pending = false;
                        p.auto_handled = false;
                        p.next_due_at = if new.enabled {
                            Some(add_utc(
                                time.utc_ms,
                                i64::from(new.interval_minutes) * MINUTE,
                            )?)
                        } else {
                            None
                        };
                    }
                }
                self.data.settings = settings;
            }
            Some(Command::Complete { id }) => {
                let index = IDS
                    .iter()
                    .position(|v| *v == id)
                    .ok_or(Error::new("invalidId"))?;
                let p = &mut self.data.progress[index];
                if p.pending {
                    p.next_due_at = Some(add_utc(
                        time.utc_ms,
                        i64::from(self.data.settings.items[index].interval_minutes) * MINUTE,
                    )?);
                    p.pending = false;
                    p.auto_handled = false;
                    response = Some(Response::Complete { id });
                }
            }
            Some(Command::SnoozeAll {}) => {
                let duration_minutes = self.data.settings.snooze_minutes;
                self.data.quiet = Some(Quiet {
                    until: add_utc(time.utc_ms, i64::from(duration_minutes) * MINUTE)?,
                    duration_minutes,
                });
                self.data.snooze_pending = true;
                self.presentation = None;
                response = Some(Response::SnoozeAll);
            }
            Some(Command::SetPaused { paused }) => self.data.paused = paused,
            Some(Command::ShowPending {}) => self.present(Mode::Manual, self.pending(), time)?,
            Some(Command::ExtendChoices { presentation_id }) => {
                let presentation = self
                    .presentation
                    .as_mut()
                    .filter(|p| p.id == presentation_id && p.mode == Mode::Automatic)
                    .ok_or(Error::new("stalePresentation"))?;
                presentation.closes_at = Some(add_utc(time.utc_ms, CHOICES_GRACE_MS)?);
                presentation.monotonic_deadline = Some(
                    time.monotonic_ms
                        .checked_add(CHOICES_GRACE_MS as u64)
                        .filter(|v| *v <= MAX_SAFE)
                        .ok_or(Error::new("invalidTime"))?,
                );
            }
            Some(Command::Dismiss { presentation_id })
                if self
                    .presentation
                    .as_ref()
                    .is_some_and(|p| p.id == presentation_id) =>
            {
                self.presentation = None;
            }
            Some(Command::Dismiss { .. }) | None => {}
        }
        let is_manual = self.update_manual();
        let pending = self.pending();
        if let Some(p) = self
            .presentation
            .as_mut()
            .filter(|p| p.mode == Mode::Automatic)
        {
            p.items.retain(|id| pending.contains(id));
            if p.items.is_empty() {
                self.presentation = None;
            }
        }
        let allowed = !self.protected
            && !self.data.paused
            && self.data.settings.active_hours.contains(time.local_minute)
            && self.data.quiet.is_none();
        if !allowed {
            if self
                .presentation
                .as_ref()
                .is_some_and(|p| p.mode == Mode::Automatic)
            {
                self.presentation = None;
            }
            return Ok(response);
        }
        // Once due and permitted, the manual view fulfills this snooze request
        // in place; dismiss must not bounce into a new automatic presentation.
        // A new SnoozeAll has a future quiet and returned above, so is preserved.
        if was_manual || is_manual {
            self.data.snooze_pending = false;
        }
        if is_manual {
            return Ok(response);
        }
        let items: Vec<_> = self
            .data
            .progress
            .iter()
            .filter(|p| p.pending && (self.data.snooze_pending || !p.auto_handled))
            .map(|p| p.id)
            .collect();
        self.data.snooze_pending = false;
        if !items.is_empty() {
            if let Some(p) = self.presentation.as_mut() {
                for id in &items {
                    if !p.items.contains(id) {
                        p.items.push(*id);
                    }
                }
            } else {
                self.present(Mode::Automatic, items.clone(), time)?;
            }
            for p in &mut self.data.progress {
                if items.contains(&p.id) {
                    p.auto_handled = true;
                }
            }
        }
        Ok(response)
    }
}

fn add_utc(now: i64, duration: i64) -> Result<i64, Error> {
    now.checked_add(duration)
        .filter(|v| (0..=MAX_UTC).contains(v))
        .ok_or(Error::new("invalidTime"))
}
impl Time {
    pub fn validate(self) -> Result<(), Error> {
        if !(0..=MAX_UTC).contains(&self.utc_ms)
            || !(0..1440).contains(&self.local_minute)
            || self.monotonic_ms > MAX_SAFE
        {
            return Err(Error::new("invalidTime"));
        }
        Ok(())
    }
}
impl ActiveHours {
    pub(super) fn contains(&self, minute: i32) -> bool {
        match *self {
            Self::AllDay {} => true,
            Self::Daily { start, end } => {
                if start < end {
                    minute >= i32::from(start) && minute < i32::from(end)
                } else {
                    minute >= i32::from(start) || minute < i32::from(end)
                }
            }
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), Error> {
        if !(1..=120).contains(&self.snooze_minutes)
            || self
                .items
                .iter()
                .zip(IDS)
                .any(|(s, id)| s.id != id || !(1..=1440).contains(&s.interval_minutes))
        {
            return Err(Error::new("invalidSettings"));
        }
        if matches!(self.active_hours,ActiveHours::Daily{start,end} if start>=1440 || end>=1440 || start==end)
        {
            return Err(Error::new("invalidSettings"));
        }
        Ok(())
    }
}
impl Data {
    pub fn validate(&self) -> Result<(), Error> {
        if self.version != 1 {
            return Err(Error::new("unsupportedVersion"));
        }
        self.settings.validate()?;
        for ((p, s), id) in self.progress.iter().zip(&self.settings.items).zip(IDS) {
            if p.id != id
                || p.auto_handled && !p.pending
                || p.pending && p.next_due_at.is_some()
                || !s.enabled && (p.pending || p.next_due_at.is_some())
                || s.enabled && !p.pending && p.next_due_at.is_none()
                || p.next_due_at.is_some_and(|t| !(0..=MAX_UTC).contains(&t))
            {
                return Err(Error::new("invalidState"));
            }
        }
        if let Some(q) = &self.quiet {
            if !self.snooze_pending
                || !(0..=MAX_UTC).contains(&q.until)
                || !(1..=120).contains(&q.duration_minutes)
            {
                return Err(Error::new("invalidState"));
            }
        }
        Ok(())
    }
}
