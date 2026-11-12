//! An in-memory Android driver and a string bridge over it. The driver behaves like the Android side: `post` makes
//! a notification observable, `cancel` removes it, and storage keeps exactly what `write` received.
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

pub struct MemoryState {
    pub capabilities: Capabilities,
    pub records: Vec<DriverRecord>,
    pub observed: BTreeMap<String, Observation>,
    pub events: EventBatch,
    pub posts: usize,
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
                posts: 0,
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
    /// Queues the dismissal of a notification by the person, as the Android side does.
    pub fn dismiss(&self, id: &str, sequence: u64) {
        let event: ActivityEvent = serde_json::from_value(json!({
            "sequence": sequence,
            "activityId": id,
            "occurredAt": 1_800_000_000_000_u64,
            "kind": {"kind": "dismissed"},
        }))
        .unwrap();
        self.lock().events.events.push(event);
    }

    /// The snapshot liveupdate last posted for `id`.
    pub fn posted(&self, id: &str) -> liveupdate::Snapshot {
        self.lock().observed[id].snapshot.clone()
    }
}

impl Driver for MemoryDriver {
    fn capabilities(&self) -> liveupdate::Result<Capabilities> {
        Ok(self.lock().capabilities)
    }

    fn read(&self) -> liveupdate::Result<Vec<DriverRecord>> {
        Ok(self.lock().records.clone())
    }

    fn write(&self, records: &[DriverRecord]) -> liveupdate::Result<()> {
        self.lock().records = records.to_vec();
        Ok(())
    }

    fn observe(&self) -> liveupdate::Result<Vec<Observation>> {
        Ok(self.lock().observed.values().cloned().collect())
    }

    fn post(&self, record: &DriverRecord) -> liveupdate::Result<()> {
        let mut state = self.lock();
        state.posts += 1;
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
