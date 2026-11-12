//! An in-memory Android driver, a string bridge over it and a channel sink.
//!
//! The driver behaves like the managed adapter: `post` makes a notification observable,
//! `cancel` removes it, and storage keeps exactly what `write` received.
#![allow(dead_code)]

use liveactivity::android::LiveUpdateBackend;
use liveactivity::{
    Error, ErrorCode, EventSink, LiveActivity, LiveActivityEvent, Result, StartRequest,
};
use liveupdate::{
    ActivityEvent, ActivityState, Bridge, Capabilities, Driver, DriverRecord, EventBatch,
    Observation, PresentationCapabilities, PromotionState,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, MutexGuard,
        mpsc::{self, Receiver, Sender},
    },
};

pub struct MemoryState {
    pub capabilities: Capabilities,
    pub records: Vec<DriverRecord>,
    pub observed: BTreeMap<String, Observation>,
    pub events: EventBatch,
    /// Every driver call in order, by method name.
    pub calls: Vec<&'static str>,
    pub fail_next_observe: bool,
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
                events: EventBatch::default(),
                calls: Vec::new(),
                fail_next_observe: false,
            })),
        }
    }
}

impl MemoryDriver {
    pub fn lock(&self) -> MutexGuard<'_, MemoryState> {
        self.state.lock().unwrap()
    }

    /// Removes the notification as a swipe would and queues the dismissal event.
    pub fn dismiss(&self, id: &str, sequence: u64) {
        let mut state = self.lock();
        state.observed.remove(id);
        state
            .events
            .events
            .push(event(id, sequence, json!({"kind": "dismissed"})));
    }

    /// Queues a tap on a notification action.
    pub fn tap(&self, id: &str, sequence: u64, action: &str) {
        self.lock().events.events.push(event(
            id,
            sequence,
            json!({"kind": "action", "actionId": action}),
        ));
    }
}

fn event(id: &str, sequence: u64, kind: Value) -> ActivityEvent {
    serde_json::from_value(json!({
        "sequence": sequence,
        "activityId": id,
        "occurredAt": 1_800_000_000_000_u64,
        "kind": kind,
    }))
    .unwrap()
}

impl Driver for MemoryDriver {
    fn capabilities(&self) -> liveupdate::Result<Capabilities> {
        let mut state = self.lock();
        state.calls.push("capabilities");
        Ok(state.capabilities)
    }

    fn read(&self) -> liveupdate::Result<Vec<DriverRecord>> {
        let mut state = self.lock();
        state.calls.push("read");
        Ok(state.records.clone())
    }

    fn write(&self, records: &[DriverRecord]) -> liveupdate::Result<()> {
        let mut state = self.lock();
        state.calls.push("write");
        state.records = records.to_vec();
        Ok(())
    }

    fn observe(&self) -> liveupdate::Result<Vec<Observation>> {
        let mut state = self.lock();
        state.calls.push("observe");
        if std::mem::take(&mut state.fail_next_observe) {
            return Err(liveupdate::Error::new(
                liveupdate::ErrorCode::NativeError,
                "Observing failed.",
            ));
        }
        Ok(state.observed.values().cloned().collect())
    }

    fn post(&self, record: &DriverRecord) -> liveupdate::Result<()> {
        let mut state = self.lock();
        state.calls.push("post");
        let promotion_state = if record.promotion_requested {
            PromotionState::Promoted
        } else {
            PromotionState::NotApplicable
        };
        state.observed.insert(
            record.id.clone(),
            Observation {
                id: record.id.clone(),
                snapshot: record.snapshot.clone(),
                state: ActivityState::Active,
                promotion_requested: record.promotion_requested,
                promotion_state,
            },
        );
        Ok(())
    }

    fn cancel(&self, id: &str) -> liveupdate::Result<()> {
        let mut state = self.lock();
        state.calls.push("cancel");
        state.observed.remove(id);
        Ok(())
    }

    fn drain_events(&self) -> liveupdate::Result<EventBatch> {
        let mut state = self.lock();
        state.calls.push("events");
        Ok(std::mem::take(&mut state.events))
    }
}

/// The managed adapter's side of liveupdate's JSON bridge protocol, as a framework transport
/// such as Tauri's `run_mobile_plugin` reaches it: a command name and a JSON payload in, a
/// response envelope out.
pub struct StringBridge {
    pub driver: MemoryDriver,
    /// Every command in order.
    pub commands: Arc<Mutex<Vec<String>>>,
}

impl Bridge for StringBridge {
    fn invoke(&self, command: &str, payload: &str) -> liveupdate::Result<String> {
        self.commands.lock().unwrap().push(command.to_owned());
        let payload: Value = serde_json::from_str(payload).unwrap();
        let data = match command {
            "capabilities" => self.driver.capabilities().map(|data| json!(data)),
            "read" => self.driver.read().map(|data| json!(data)),
            "write" => self
                .driver
                .write(&member::<Vec<DriverRecord>>(&payload, "records"))
                .map(|()| Value::Null),
            "observe" => self.driver.observe().map(|data| json!(data)),
            "post" => self
                .driver
                .post(&member::<DriverRecord>(&payload, "record"))
                .map(|()| Value::Null),
            "cancel" => self
                .driver
                .cancel(&member::<String>(&payload, "id"))
                .map(|()| Value::Null),
            "events" => self.driver.drain_events().map(|data| json!(data)),
            other => panic!("unknown bridge command {other}"),
        };
        let envelope = match data {
            Ok(data) => json!({"status": "ok", "data": data}),
            Err(error) => json!({"status": "error", "error": error}),
        };
        Ok(envelope.to_string())
    }
}

fn member<T: serde::de::DeserializeOwned>(payload: &Value, name: &str) -> T {
    serde_json::from_value(payload[name].clone()).unwrap()
}

/// Framework-side reverse bridge backed by a standard channel.
pub struct ChannelSink(pub Sender<LiveActivityEvent>);

impl EventSink for ChannelSink {
    fn emit(&self, event: LiveActivityEvent) -> Result<()> {
        self.0
            .send(event)
            .map_err(|_| Error::new(ErrorCode::EventDelivery, "the event receiver is closed"))
    }
}

/// The neutral service, the backend it runs on and both observation points.
pub struct Harness {
    pub service: LiveActivity,
    pub backend: Arc<LiveUpdateBackend<MemoryDriver>>,
    pub driver: MemoryDriver,
    pub events: Receiver<LiveActivityEvent>,
}

/// Connects the backend the way a framework adapter does: it keeps the backend in an `Arc`
/// for `pump_events` and hands the same one to the neutral service.
pub fn connect() -> Harness {
    let driver = MemoryDriver::default();
    let (sender, events) = mpsc::channel();
    let mut kept = None;
    let service = LiveActivity::connect(ChannelSink(sender), |context| {
        let backend = Arc::new(LiveUpdateBackend::new(context, driver.clone()));
        kept = Some(backend.clone());
        Ok(backend)
    })
    .unwrap();
    Harness {
        service,
        backend: kept.unwrap(),
        driver,
        events,
    }
}

/// Start content of the v1 progress template: a liveupdate start request without its key
/// and schema version.
pub fn progress_content(progress: f64) -> Value {
    json!({
        "template": "progress",
        "fallback": "notification",
        "visibility": "private",
        "snapshot": {
            "revision": 0,
            "title": "Delivery",
            "status": "Preparing",
            "progress": progress
        }
    })
}

pub fn progress_start(key: &str) -> StartRequest {
    StartRequest::new(key, 1, progress_content(0.25))
}
