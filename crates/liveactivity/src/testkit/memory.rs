use crate::{
    ActionEvent, ActivityRecord, ActivityState, Backend, BackendContext, BackendFuture,
    Capabilities, EndInput, EndPolicy, Error, Event, Fallback, FeatureSupport, LiveActivityBackend,
    PermissionKind, PermissionState, Permissions, PromotionState, Result, StartInput, StateEvent,
    UpdateInput, Visibility,
};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

/// A backend that keeps its activities in memory and shows nothing.
///
/// It runs the progress and timer templates under the rules that [`conformance`](super::conformance) checks, stores
/// the names of written images, reports the first state of an activity and its end to the event sink, and reports
/// a tap when asked to.
#[derive(Default)]
pub struct MemoryBackend {
    events: OnceLock<BackendContext>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    activities: Vec<Activity>,
    images: Vec<String>,
    started: u64,
}

struct Activity {
    record: ActivityRecord,
    /// The start as it was asked: a repeated start is compared with it, whatever came since.
    origin: StartInput,
}

fn not_found() -> Error {
    Error::new("NOT_FOUND", "Unknown activity.")
}

fn terminal(record: &ActivityRecord) -> bool {
    matches!(
        record.state,
        ActivityState::Ended | ActivityState::Dismissed
    )
}

impl MemoryBackend {
    /// A backend with no activity.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reports a tap on `action_id` of the activity `id` to the event sink, as a platform would.
    ///
    /// # Errors
    ///
    /// Fails when no activity has `id`.
    pub fn tap(&self, id: &str, action_id: &str) -> Result<()> {
        let activity_key = self
            .lock()
            .activities
            .iter()
            .find(|activity| activity.record.id == id)
            .map(|activity| activity.record.activity_key.clone())
            .ok_or_else(not_found)?;
        if let Some(events) = self.events.get() {
            events.emit(Event::Action(ActionEvent {
                action_id: action_id.into(),
                activity_id: id.into(),
                activity_key: Some(activity_key),
            }));
        }
        Ok(())
    }

    /// The names of the images written so far, oldest first.
    pub fn images(&self) -> Vec<String> {
        self.lock().images.clone()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn started(&self, input: StartInput) -> Result<ActivityRecord> {
        // This backend has no native presentation to promote to.
        if input.fallback == Fallback::None {
            return Err(Error::unsupported_feature(
                "The native presentation is unavailable and fallback was not allowed.",
            ));
        }
        let mut state = self.lock();
        let running = state.activities.iter().find(|activity| {
            !terminal(&activity.record) && activity.record.activity_key == input.activity_key
        });
        if let Some(running) = running {
            return if running.origin == input {
                Ok(running.record.clone())
            } else {
                Err(Error::new(
                    "CONFLICT",
                    "An activity with this key already exists.",
                ))
            };
        }
        state.started += 1;
        let record = ActivityRecord {
            id: format!("memory-{}", state.started),
            activity_key: input.activity_key.clone(),
            schema_version: input.schema_version,
            template: input.template,
            snapshot: input.snapshot.clone(),
            backend: Backend::Unsupported,
            state: ActivityState::Active,
            visibility: Visibility::Unknown,
            degraded_features: vec!["nativeLiveActivity".into()],
            promotion_requested: false,
            promotion_state: PromotionState::NotApplicable,
        };
        state.activities.push(Activity {
            record: record.clone(),
            origin: input,
        });
        drop(state);
        // A platform reports the first state of an activity it shows. Outside the lock: a sink may call back.
        self.report(&record, ActivityState::Active);
        Ok(record)
    }

    /// Hands the sink a state of `record`, as a platform reports it.
    fn report(&self, record: &ActivityRecord, state: ActivityState) {
        if let Some(events) = self.events.get() {
            events.emit(Event::State(StateEvent {
                activity_id: record.id.clone(),
                activity_key: Some(record.activity_key.clone()),
                state,
            }));
        }
    }

    fn updated(&self, input: UpdateInput) -> Result<ActivityRecord> {
        let mut state = self.lock();
        let activity = state
            .activities
            .iter_mut()
            .find(|activity| activity.record.id == input.id && !terminal(&activity.record))
            .ok_or_else(not_found)?;
        let current = &activity.record.snapshot;
        // An update keeps the template of the start: its fields are what the presentation shows.
        if input.snapshot.progress.is_some() != current.progress.is_some() {
            return Err(Error::invalid());
        }
        if input.snapshot.revision < current.revision {
            return Err(Error::new("STALE_REVISION", "Activity revision is stale."));
        }
        if input.snapshot.revision == current.revision && input.snapshot != *current {
            return Err(Error::new(
                "CONFLICT",
                "Revision already contains different content.",
            ));
        }
        activity.record.snapshot = input.snapshot;
        Ok(activity.record.clone())
    }

    fn ended(&self, input: EndInput) -> Result<ActivityRecord> {
        if matches!(input.policy, EndPolicy::After { .. }) {
            return Err(Error::unsupported_feature(
                "This backend does not keep an ended activity on screen.",
            ));
        }
        let (record, was_running) = {
            let mut state = self.lock();
            let activity = state
                .activities
                .iter_mut()
                .find(|activity| activity.record.id == input.id)
                .ok_or_else(not_found)?;
            let was_running = !terminal(&activity.record);
            activity.record.state = ActivityState::Ended;
            (activity.record.clone(), was_running)
        };
        // An end is reported as a platform reports it, once, and outside the lock: a sink may call the backend.
        if was_running {
            self.report(&record, ActivityState::Ended);
        }
        Ok(record)
    }
}

impl LiveActivityBackend for MemoryBackend {
    fn attach(&self, events: BackendContext) -> Result<()> {
        let _ = self.events.set(events);
        Ok(())
    }

    fn capabilities(&self) -> BackendFuture<'_, Capabilities> {
        let (yes, no) = (FeatureSupport::Supported, FeatureSupport::Unsupported);
        Box::pin(async move {
            Ok(Capabilities {
                platform: "memory".into(),
                backend: Backend::Unsupported,
                start: yes,
                update: yes,
                end: yes,
                progress: yes,
                timer: yes,
                actions: yes,
                state_observation: no,
                silent_update: yes,
                native_live_activity: no,
                max_payload_bytes: 4096,
                layout: no,
                push: no,
                android_presentation: None,
            })
        })
    }

    fn permissions(&self) -> BackendFuture<'_, Permissions> {
        Box::pin(async {
            Ok(Permissions {
                notifications: PermissionState::Granted,
                live_activities: PermissionState::Granted,
                promotion: PermissionState::NotApplicable,
                frequent_pushes: PermissionState::NotApplicable,
            })
        })
    }

    fn request_permissions(&self, _kind: PermissionKind) -> BackendFuture<'_, Permissions> {
        self.permissions()
    }

    fn start(&self, input: StartInput) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move { self.started(input) })
    }

    fn update(&self, input: UpdateInput) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move { self.updated(input) })
    }

    fn end(&self, input: EndInput) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move { self.ended(input) })
    }

    fn list(&self) -> BackendFuture<'_, Vec<ActivityRecord>> {
        Box::pin(async move {
            let state = self.lock();
            Ok(state
                .activities
                .iter()
                .map(|activity| activity.record.clone())
                .collect())
        })
    }

    fn write_image(&self, name: String, _png: Vec<u8>) -> BackendFuture<'_, ()> {
        Box::pin(async move {
            self.lock().images.push(name);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LiveActivity, block_on};
    use std::sync::mpsc;

    #[test]
    fn req_e_003_the_backend_without_a_platform_reports_its_ends() {
        // REQ-E-003
        let (sender, events) = mpsc::channel();
        let live = LiveActivity::builder()
            .backend(MemoryBackend::new())
            .event_sink(move |event: Event| sender.send(event).unwrap())
            .build()
            .unwrap();
        let start = StartInput::progress("order:42", "Order 42", "Preparing", 0.1);
        let order = block_on(live.start(start)).unwrap();
        let state = |state: ActivityState| {
            Event::State(StateEvent {
                activity_id: order.id.clone(),
                activity_key: Some("order:42".into()),
                state,
            })
        };
        // The first state of an activity it shows, as a platform reports it; the same start again shows nothing new.
        assert_eq!(events.try_recv().unwrap(), state(ActivityState::Active));
        let again = StartInput::progress("order:42", "Order 42", "Preparing", 0.1);
        block_on(live.start(again)).unwrap();
        assert!(events.try_recv().is_err());
        block_on(live.end(EndInput::new(&order.id))).unwrap();
        assert_eq!(events.try_recv().unwrap(), state(ActivityState::Ended));
        // An end again answers the record and reports nothing, and neither does an end that fails.
        block_on(live.end(EndInput::new(&order.id))).unwrap();
        assert!(block_on(live.end(EndInput::new("unknown"))).is_err());
        assert!(events.try_recv().is_err());
    }
}
