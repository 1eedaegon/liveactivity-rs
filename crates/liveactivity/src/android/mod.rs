//! The default Android backend: the `liveupdate` engine, used as it is.
//!
//! [`LiveUpdateBackend`] runs the progress and timer templates on the engine, which keeps the
//! lifecycle, the revisions and the receipts. The caller supplies only the transport: a
//! `liveupdate::Driver`, a string `liveupdate::Bridge` over a framework's own native call, or
//! the JNI transport of liveupdate's AAR. The engine draws Android's system templates, so
//! layouts, permission dialogs and images are not its part: a host that has them implements
//! [`LiveActivityBackend`] itself and hands the templates to this backend.

mod convert;

use crate::{
    ActionEvent, ActivityRecord, BackendContext, BackendFuture, Capabilities, EndInput,
    LiveActivityBackend, Permissions, Result, StartInput, UpdateInput,
};
use liveupdate::{Bridge, Driver, EventKind, JsonDriver, LiveUpdate};
use std::sync::{Mutex, OnceLock, PoisonError};

/// `LiveActivityBackend` over the liveupdate engine.
///
/// The engine calls its driver synchronously: a call completes on its first poll and runs the
/// native call on the polling thread.
pub struct LiveUpdateBackend<D> {
    engine: LiveUpdate<D>,
    /// Where taps go, once the backend is connected.
    events: OnceLock<BackendContext>,
    /// Held for one whole drain, so that taps reach the sink in order.
    pump: Mutex<()>,
}

impl<D: Driver + 'static> LiveUpdateBackend<D> {
    /// The backend on `driver`, which is the Android side.
    pub fn new(driver: D) -> Self {
        Self {
            engine: LiveUpdate::new(driver),
            events: OnceLock::new(),
            pump: Mutex::new(()),
        }
    }

    /// The wrapped engine, for what liveupdate offers beyond the contract.
    pub fn engine(&self) -> &LiveUpdate<D> {
        &self.engine
    }

    /// Drains the taps Android queued and hands each to the event sink, oldest first. Answers
    /// how many it handed over.
    ///
    /// # Errors
    ///
    /// Fails when the drain fails.
    pub fn pump_events(&self) -> Result<usize> {
        // Not connected yet: the taps stay queued for the sink that comes.
        let Some(events) = self.events.get() else {
            return Ok(0);
        };
        let _pump = self.pump.lock().unwrap_or_else(PoisonError::into_inner);
        let batch = self.engine.events().map_err(convert::error)?;
        let taps: Vec<_> = batch
            .events
            .into_iter()
            .filter_map(|event| match event.kind {
                EventKind::Action { action_id } => Some((event.activity_id, action_id)),
                // The contract reports taps; `list` shows a dismissal.
                EventKind::Dismissed => None,
            })
            .collect();
        if taps.is_empty() {
            return Ok(0);
        }
        // A tap names its activity's key when the engine still lists the activity.
        let listed = self.engine.list().unwrap_or_default();
        for (activity_id, action_id) in &taps {
            let activity_key = listed
                .iter()
                .find(|record| &record.id == activity_id)
                .map(|record| record.activity_key.clone());
            events.emit(ActionEvent {
                action_id: action_id.clone(),
                activity_id: activity_id.clone(),
                activity_key,
            });
        }
        Ok(taps.len())
    }
}

impl<B: Bridge + 'static> LiveUpdateBackend<JsonDriver<B>> {
    /// The backend over a framework's string transport to the Android side.
    pub fn over_bridge(bridge: B) -> Self {
        Self::new(JsonDriver::new(bridge))
    }
}

#[cfg(target_os = "android")]
impl LiveUpdateBackend<liveupdate::AndroidDriver> {
    /// The backend on the JNI transport of liveupdate's own AAR. The host must have initialized `ndk-context` and
    /// package the AAR.
    ///
    /// # Errors
    ///
    /// Fails when the Android context or the AAR's bridge class is not available.
    pub fn android() -> Result<Self> {
        let bridge = liveupdate::AndroidBridge::new().map_err(convert::error)?;
        Ok(Self::over_bridge(bridge))
    }
}

impl<D: Driver + 'static> LiveActivityBackend for LiveUpdateBackend<D> {
    fn attach(&self, events: BackendContext) -> Result<()> {
        // The first sink stays: a backend is connected once.
        let _ = self.events.set(events);
        Ok(())
    }

    fn capabilities(&self) -> BackendFuture<'_, Capabilities> {
        Box::pin(async move {
            self.engine
                .capabilities()
                .map(convert::capabilities)
                .map_err(convert::error)
        })
    }

    /// The engine reads permissions and cannot prompt: the host shows Android's dialog.
    fn permissions(&self) -> BackendFuture<'_, Permissions> {
        Box::pin(async move {
            self.engine
                .capabilities()
                .map(convert::permissions)
                .map_err(convert::error)
        })
    }

    fn start(&self, input: StartInput) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move {
            self.engine
                .start(convert::start_request(input))
                .map_err(convert::error)
                .and_then(convert::activity_record)
        })
    }

    fn update(&self, input: UpdateInput) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move {
            self.engine
                .update(&input.id, convert::live_snapshot(input.snapshot))
                .map_err(convert::error)
                .and_then(convert::activity_record)
        })
    }

    fn end(&self, input: EndInput) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move {
            self.engine
                .end(&input.id, convert::end_policy(input.policy))
                .map_err(convert::error)
                .and_then(convert::activity_record)
        })
    }

    /// The progress and timer activities. The engine's store may also hold activities of its richer presentations,
    /// which an application started on the engine itself: they are no template activities and are left out.
    fn list(&self) -> BackendFuture<'_, Vec<ActivityRecord>> {
        Box::pin(async move {
            let records = self.engine.list().map_err(convert::error)?;
            Ok(records
                .into_iter()
                .filter_map(|record| convert::activity_record(record).ok())
                .collect())
        })
    }

    fn sync_events(&self) -> BackendFuture<'_, ()> {
        Box::pin(async move { self.pump_events().map(|_| ()) })
    }
}
