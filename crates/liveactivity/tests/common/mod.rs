//! An in-memory Android driver and a string bridge over it. The driver behaves like the Android side: `post` makes
//! a notification observable, `cancel` removes it, and storage keeps exactly what `write` received. The post of
//! an ended record is its last content, which Android shows until its time and no longer observes. A post whose
//! content names a URL of the scheme [`UNOPENED`] is refused, as Android refuses a URL that nothing in the
//! application opens. A scheduled receipt is posted when the device's clock has reached its time: by
//! [`MemoryDriver::fire_alarms`], which is the alarm while the application does not run, or when the lifecycle asks
//! for the starts, which is how a start whose alarm was lost begins.
#![allow(dead_code)]

use liveactivity::liveupdate::{
    self, ActivityEvent, ActivityState, Bridge, Capabilities, Driver, DriverRecord, EventBatch,
    Observation, PresentationCapabilities, PromotionState, StartOutcome, StartReport,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, MutexGuard},
};

/// The scheme of a URL that no activity of the application opens.
pub const UNOPENED: &str = "nobody-opens-this:";

pub struct MemoryState {
    pub capabilities: Capabilities,
    pub records: Vec<DriverRecord>,
    pub observed: BTreeMap<String, Observation>,
    /// The ended records posted as their last content, by id.
    pub finals: BTreeMap<String, DriverRecord>,
    pub events: EventBatch,
    pub posts: usize,
    /// How many of the next posts fail as a platform that could not post, whatever they hold.
    pub failing_posts: usize,
    /// How often the capabilities were asked.
    pub capability_calls: usize,
    /// The device's clock, in epoch milliseconds: a notification whose stale time has come is listed as stale.
    pub now: u64,
    /// While set, a post is not listed yet, as when Android shows a notification a moment after it was posted.
    pub delay_listing: bool,
    /// The posts that are not listed yet, by id.
    pub unlisted: BTreeMap<String, Observation>,
    /// How often Android was asked what it shows.
    pub observations: usize,
    /// How many of the next such questions fail.
    pub failing_observations: usize,
    /// The scheduled receipts that the Android side has begun, by id: each is posted once.
    pub begun: BTreeSet<String>,
    /// How often a receipt was handed over to begin at its time.
    pub schedules: usize,
    /// How many of the next such hand-overs fail, as an alarm that could not be set.
    pub failing_schedules: usize,
    /// The ids the lifecycle asked to take away: a notification, or the alarm of a scheduled receipt.
    pub cancelled: Vec<String>,
}

impl MemoryState {
    /// Counts a post, and fails it while posts are set to fail.
    fn attempt(&mut self) -> liveupdate::Result<()> {
        self.posts += 1;
        if self.failing_posts > 0 {
            self.failing_posts -= 1;
            return Err(liveupdate::Error::new(
                liveupdate::ErrorCode::NativeError,
                "Posting failed.",
            ));
        }
        Ok(())
    }

    /// Shows `record` as Android does: listed at once, or a moment later while `delay_listing` is set.
    fn list(&mut self, record: &DriverRecord) {
        let promotion_state = if record.promotion_requested {
            PromotionState::Promoted
        } else {
            PromotionState::NotApplicable
        };
        let observation = Observation {
            id: record.id.clone(),
            snapshot: record.snapshot.clone(),
            state: ActivityState::Active,
            promotion_requested: record.promotion_requested,
            promotion_state,
        };
        if self.delay_listing {
            self.unlisted.insert(record.id.clone(), observation);
        } else {
            self.observed.insert(record.id.clone(), observation);
        }
    }

    /// Posts a scheduled receipt whose time has come. One that cannot be posted shows nothing.
    fn begin(&mut self, record: &DriverRecord) {
        self.begun.insert(record.id.clone());
        if self.attempt().is_ok() {
            self.list(record);
        }
    }

    /// Begins the scheduled receipts whose time has come, each once. A receipt whose hand-over was not confirmed
    /// has no alarm: it waits for the start that hands it over again.
    fn begin_due(&mut self) {
        let due = |record: &&DriverRecord| {
            record.state == ActivityState::Scheduled
                && !record.post_failed
                && record.origin.start_at.is_some_and(|at| at <= self.now)
                && !self.begun.contains(&record.id)
        };
        let due: Vec<DriverRecord> = self.records.iter().filter(due).cloned().collect();
        for record in due {
            self.begin(&record);
        }
    }
}

/// Whether `record` names a URL that nothing in the application opens.
fn unopened(record: &DriverRecord) -> bool {
    record.snapshot.presentation.as_ref().is_some_and(|shown| {
        let of_actions = shown.actions.iter().map(|action| &action.open_url);
        std::iter::once(&shown.open_url)
            .chain(of_actions)
            .flatten()
            .any(|url| url.starts_with(UNOPENED))
    })
}

#[derive(Clone)]
pub struct MemoryDriver {
    state: Arc<Mutex<MemoryState>>,
}

impl Default for MemoryDriver {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(MemoryState {
                capabilities: Capabilities {
                    supported: true,
                    notifications_enabled: true,
                    promotion_enabled: false,
                    presentation: PresentationCapabilities {
                        actions: true,
                        events: true,
                        ..PresentationCapabilities::default()
                    },
                },
                records: Vec::new(),
                observed: BTreeMap::new(),
                finals: BTreeMap::new(),
                events: EventBatch::default(),
                posts: 0,
                failing_posts: 0,
                capability_calls: 0,
                now: 0,
                delay_listing: false,
                unlisted: BTreeMap::new(),
                observations: 0,
                failing_observations: 0,
                begun: BTreeSet::new(),
                schedules: 0,
                failing_schedules: 0,
                cancelled: Vec::new(),
            })),
        }
    }
}

impl MemoryDriver {
    pub fn lock(&self) -> MutexGuard<'_, MemoryState> {
        self.state.lock().unwrap()
    }

    /// Queues a tap on a notification action, as the Android side does.
    pub fn tap(&self, id: &str, sequence: u64, action: &str) {
        let event: ActivityEvent = serde_json::from_value(json!({
            "sequence": sequence,
            "activityId": id,
            "occurredAt": 1_800_000_000_000_u64,
            "kind": {"kind": "action", "actionId": action},
        }))
        .unwrap();
        self.lock().events.events.push(event);
    }
}

impl MemoryDriver {
    /// Lists the posts that were held back, as Android does a moment after they were posted.
    pub fn show_unlisted(&self) {
        let mut state = self.lock();
        state.delay_listing = false;
        let shown = std::mem::take(&mut state.unlisted);
        state.observed.extend(shown);
    }

    /// The application's own component took a tap of a target with `LiveUpdateActions.take`: the Android side
    /// stores it as handled.
    pub fn handled(&self, id: &str, sequence: u64, action: &str) {
        let event: ActivityEvent = serde_json::from_value(json!({
            "sequence": sequence,
            "activityId": id,
            "occurredAt": 1_800_000_000_000_u64,
            "kind": {"kind": "handled", "actionId": action},
        }))
        .unwrap();
        self.lock().events.events.push(event);
    }

    /// The person removes a notification: it is gone, and the Android side queues the dismissal.
    pub fn dismiss(&self, id: &str, sequence: u64) {
        self.lock().observed.remove(id);
        let event: ActivityEvent = serde_json::from_value(json!({
            "sequence": sequence,
            "activityId": id,
            "occurredAt": 1_800_000_000_000_u64,
            "kind": {"kind": "dismissed"},
        }))
        .unwrap();
        self.lock().events.events.push(event);
    }

    /// Queues what the Android side queues when the host reports that the person opened the application from
    /// the notification, by its content or by `action`.
    pub fn open(&self, id: &str, sequence: u64, action: Option<&str>) {
        let mut kind = json!({"kind": "opened"});
        if let Some(action) = action {
            kind["actionId"] = json!(action);
        }
        let event: ActivityEvent = serde_json::from_value(json!({
            "sequence": sequence,
            "activityId": id,
            "occurredAt": 1_800_000_000_000_u64,
            "kind": kind,
        }))
        .unwrap();
        self.lock().events.events.push(event);
    }

    /// Queues an event of the kind `kind`, which this liveupdate does not know, as a later AAR stores one.
    pub fn later(&self, id: &str, sequence: u64, kind: &str) {
        let event: ActivityEvent = serde_json::from_value(json!({
            "sequence": sequence,
            "activityId": id,
            "occurredAt": 1_800_000_000_000_u64,
            "kind": {"kind": kind, "until": 1_800_000_060_000_u64},
        }))
        .unwrap();
        assert_eq!(event.kind, liveupdate::EventKind::Unknown);
        self.lock().events.events.push(event);
    }

    /// The alarm of the Android side, which fires while the application does not run: the scheduled receipts
    /// whose time has come are posted.
    pub fn fire_alarms(&self) {
        self.lock().begin_due();
    }

    /// The snapshot liveupdate last posted for `id`.
    pub fn posted(&self, id: &str) -> liveupdate::Snapshot {
        self.lock().observed[id].snapshot.clone()
    }

    /// A device that offers the extensions `offer` names.
    pub fn offering(offer: fn(&mut PresentationCapabilities)) -> Self {
        let driver = Self::default();
        offer(&mut driver.lock().capabilities.presentation);
        driver
    }
}

impl Driver for MemoryDriver {
    fn capabilities(&self) -> liveupdate::Result<Capabilities> {
        let mut state = self.lock();
        state.capability_calls += 1;
        Ok(state.capabilities)
    }

    fn read(&self) -> liveupdate::Result<Vec<DriverRecord>> {
        Ok(self.lock().records.clone())
    }

    fn write(&self, records: &[DriverRecord]) -> liveupdate::Result<()> {
        self.lock().records = records.to_vec();
        Ok(())
    }

    fn observe(&self) -> liveupdate::Result<Vec<Observation>> {
        let mut state = self.lock();
        state.observations += 1;
        if state.failing_observations > 0 {
            state.failing_observations -= 1;
            return Err(liveupdate::Error::new(
                liveupdate::ErrorCode::NativeError,
                "Listing failed.",
            ));
        }
        // The lifecycle has no clock: the Android side says that a stale time has come.
        let listed = |observation: &Observation| Observation {
            state: match observation.snapshot.stale_at {
                Some(stale_at) if stale_at <= state.now => ActivityState::Stale,
                _ => ActivityState::Active,
            },
            ..observation.clone()
        };
        Ok(state.observed.values().map(listed).collect())
    }

    fn post(&self, record: &DriverRecord) -> liveupdate::Result<()> {
        let mut state = self.lock();
        state.attempt()?;
        if unopened(record) {
            return Err(liveupdate::Error::new(
                liveupdate::ErrorCode::InvalidInput,
                "No activity of the application opens the URL.",
            ));
        }
        if record.state == ActivityState::Ended {
            state.observed.remove(&record.id);
            state.finals.insert(record.id.clone(), record.clone());
            return Ok(());
        }
        state.list(record);
        Ok(())
    }

    fn schedule(&self, record: &DriverRecord) -> liveupdate::Result<()> {
        let mut state = self.lock();
        state.schedules += 1;
        if state.failing_schedules > 0 {
            state.failing_schedules -= 1;
            return Err(liveupdate::Error::new(
                liveupdate::ErrorCode::NativeError,
                "The alarm could not be set.",
            ));
        }
        // The receipt is checked as a post is.
        if unopened(record) {
            return Err(liveupdate::Error::new(
                liveupdate::ErrorCode::InvalidInput,
                "No activity of the application opens the URL.",
            ));
        }
        // A time that has passed begins the activity at once.
        if record.origin.start_at.is_some_and(|at| at <= state.now) {
            state.begin(record);
        }
        Ok(())
    }

    fn starts(&self) -> liveupdate::Result<Vec<StartReport>> {
        let mut state = self.lock();
        state.begin_due();
        // Of the receipts it began, those that Android does not list: posted a moment ago, or not shown.
        let scheduled = |id: &&String| {
            let waits = |record: &DriverRecord| {
                record.id == **id && record.state == ActivityState::Scheduled
            };
            state.records.iter().any(waits) && !state.observed.contains_key(*id)
        };
        let report = |id: &String| StartReport {
            id: id.clone(),
            outcome: if state.unlisted.contains_key(id) {
                StartOutcome::Posted
            } else {
                StartOutcome::Gone
            },
        };
        Ok(state.begun.iter().filter(scheduled).map(report).collect())
    }

    fn cancel(&self, id: &str) -> liveupdate::Result<()> {
        let mut state = self.lock();
        state.observed.remove(id);
        state.cancelled.push(id.to_owned());
        Ok(())
    }

    fn drain_events(&self) -> liveupdate::Result<EventBatch> {
        Ok(std::mem::take(&mut self.lock().events))
    }
}

/// The Android side of liveupdate's JSON bridge protocol, as a framework's native call reaches it: a command name
/// and a JSON payload in, a response envelope out.
pub struct StringBridge(pub MemoryDriver);

fn member<T: DeserializeOwned>(payload: &Value, name: &str) -> T {
    serde_json::from_value(payload[name].clone()).unwrap()
}

impl Bridge for StringBridge {
    fn invoke(&self, command: &str, payload: &str) -> liveupdate::Result<String> {
        let payload: Value = serde_json::from_str(payload).unwrap();
        let driver = &self.0;
        let data = match command {
            "capabilities" => driver.capabilities().map(|data| json!(data)),
            "read" => driver.read().map(|data| json!(data)),
            "write" => driver
                .write(&member::<Vec<DriverRecord>>(&payload, "records"))
                .map(|()| Value::Null),
            "observe" => driver.observe().map(|data| json!(data)),
            "post" => driver
                .post(&member::<DriverRecord>(&payload, "record"))
                .map(|()| Value::Null),
            "cancel" => driver
                .cancel(&member::<String>(&payload, "id"))
                .map(|()| Value::Null),
            "events" => driver.drain_events().map(|data| json!(data)),
            "schedule" => driver
                .schedule(&member::<DriverRecord>(&payload, "record"))
                .map(|()| Value::Null),
            "starts" => driver.starts().map(|data| json!(data)),
            other => panic!("unknown bridge command {other}"),
        };
        let envelope = match data {
            Ok(data) => json!({"status": "ok", "data": data}),
            Err(error) => json!({"status": "error", "error": error}),
        };
        Ok(envelope.to_string())
    }
}

/// The same bridge with a native call that panics while the test says so, as a framework's call may: the call of
/// one command, and every other reaches the Android side.
#[derive(Clone)]
pub struct PanickingBridge {
    pub android: MemoryDriver,
    panics: Arc<Mutex<Option<&'static str>>>,
}

impl PanickingBridge {
    pub fn new(android: MemoryDriver) -> Self {
        Self {
            android,
            panics: Arc::default(),
        }
    }

    /// The command whose call panics from now on, or none.
    pub fn panic_in(&self, command: Option<&'static str>) {
        *self.panics.lock().unwrap() = command;
    }
}

impl Bridge for PanickingBridge {
    fn invoke(&self, command: &str, payload: &str) -> liveupdate::Result<String> {
        // Read before the panic, which then unwinds with no lock of the test held.
        let panics = *self.panics.lock().unwrap() == Some(command);
        assert!(!panics, "the native call of `{command}` panicked");
        StringBridge(self.android.clone()).invoke(command, payload)
    }
}
