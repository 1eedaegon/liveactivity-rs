use crate::ActionEvent;
use std::sync::Arc;

/// Where a backend's events go: implemented by a framework plugin, or any closure.
///
/// The event is a tap on a button of an activity. A backend calls the sink without holding a
/// lock, from a thread of its own. On iOS the tap keeps its intent, and an app the system
/// launched for it, until `emit` has returned, for at most 4 seconds.
pub trait EventSink: Send + Sync + 'static {
    /// Takes one tap.
    fn emit(&self, event: ActionEvent);
}

impl<F> EventSink for F
where
    F: Fn(ActionEvent) + Send + Sync + 'static,
{
    fn emit(&self, event: ActionEvent) {
        self(event);
    }
}

/// The event sink as a backend holds it, given to the backend when it is made.
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

    /// Hands a tap to the sink.
    pub fn emit(&self, event: ActionEvent) {
        self.sink.emit(event);
    }
}
