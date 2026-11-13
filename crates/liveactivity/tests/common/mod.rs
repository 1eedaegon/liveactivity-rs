//! An in-memory Android driver and a string bridge over it. The driver behaves like the Android side: `post` makes
//! a notification observable, `cancel` removes it, and storage keeps exactly what `write` received. The post of
//! an ended record is its last content, which Android shows until its time and no longer observes. A post whose
//! content names a URL of the scheme [`UNOPENED`] is refused, as Android refuses a URL that nothing in the
//! application opens.
#![allow(dead_code)]

use liveactivity::liveupdate::{
    self, ActivityEvent, ActivityState, Bridge, Capabilities, Driver, DriverRecord, EventBatch,
    Observation, PresentationCapabilities, PromotionState,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
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
        state.posts += 1;
        if state.failing_posts > 0 {
            state.failing_posts -= 1;
            return Err(liveupdate::Error::new(
                liveupdate::ErrorCode::NativeError,
                "Posting failed.",
            ));
        }
        let unopened = record.snapshot.presentation.as_ref().is_some_and(|shown| {
            let of_actions = shown.actions.iter().map(|action| &action.open_url);
            std::iter::once(&shown.open_url)
                .chain(of_actions)
                .flatten()
                .any(|url| url.starts_with(UNOPENED))
        });
        if unopened {
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
        if state.delay_listing {
            state.unlisted.insert(record.id.clone(), observation);
        } else {
            state.observed.insert(record.id.clone(), observation);
        }
        Ok(())
    }

    fn cancel(&self, id: &str) -> liveupdate::Result<()> {
        self.lock().observed.remove(id);
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
            other => panic!("unknown bridge command {other}"),
        };
        let envelope = match data {
            Ok(data) => json!({"status": "ok", "data": data}),
            Err(error) => json!({"status": "error", "error": error}),
        };
        Ok(envelope.to_string())
    }
}
