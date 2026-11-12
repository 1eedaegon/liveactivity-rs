use crate::{LiveActivityEvent, Result};
use std::sync::Arc;

/// Reverse bridge implemented by a framework adapter.
pub trait EventSink: Send + Sync + 'static {
    /// Delivers one owned backend event without blocking the native callback.
    fn emit(&self, event: LiveActivityEvent) -> Result<()>;
}

/// Immutable event capability passed to a backend during construction.
#[derive(Clone)]
pub struct BackendContext {
    sink: Arc<dyn EventSink>,
}

impl BackendContext {
    /// Wraps a framework event sink for backend use.
    pub fn new<S>(sink: S) -> Self
    where
        S: EventSink,
    {
        Self {
            sink: Arc::new(sink),
        }
    }

    /// Delivers an event to the framework adapter.
    pub fn emit(&self, event: LiveActivityEvent) -> Result<()> {
        event.validate()?;
        self.sink.emit(event)
    }
}
