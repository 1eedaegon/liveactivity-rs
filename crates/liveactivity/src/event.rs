use crate::{ActionEvent, PushToStartTokenEvent, PushTokenEvent, UpdatedEvent};
use std::sync::Arc;

/// What a platform reports of the application's activities. More kinds may come: match with a `_` arm.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A tap on a button of an activity.
    Action(ActionEvent),
    /// A push token of an activity, new or changed (iOS).
    PushToken(PushTokenEvent),
    /// The app's push-to-start token, new or changed (iOS).
    PushToStartToken(PushToStartTokenEvent),
    /// Content that a push delivered to a layout activity (iOS).
    Updated(UpdatedEvent),
}

/// Where a backend's events go: implemented by a framework plugin, or any closure.
///
/// On iOS the backend calls the sink from a thread of its own. A tap keeps its intent, and an app the system
/// launched for it, until `emit` has returned, for at most 4 seconds; the other events wait for nothing. On Android
/// the sink is called on the thread that runs `sync_events`, while the backend holds its drain: a sink must not
/// call `sync_events` itself.
pub trait EventSink: Send + Sync + 'static {
    /// Takes one event.
    fn emit(&self, event: Event);
}

impl<F> EventSink for F
where
    F: Fn(Event) + Send + Sync + 'static,
{
    fn emit(&self, event: Event) {
        self(event);
    }
}

/// The event sink as a backend holds it, handed over in `attach`.
#[derive(Clone)]
pub struct BackendContext {
    sink: Arc<dyn EventSink>,
}

impl BackendContext {
    /// Wraps a sink for a backend.
    pub fn new<S: EventSink>(sink: S) -> Self {
        Self {
            sink: Arc::new(sink),
        }
    }

    /// Hands an event to the sink.
    pub fn emit(&self, event: Event) {
        self.sink.emit(event);
    }
}
