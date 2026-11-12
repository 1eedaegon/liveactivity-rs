use crate::{
    ActivityRecord, BackendContext, Capabilities, EndRequest, EventSink, LiveActivityBackend,
    Result, StartRequest, UpdateRequest,
};
use std::sync::Arc;

/// Framework-neutral service forwarding calls to one injected backend.
#[derive(Clone)]
pub struct LiveActivity {
    backend: Arc<dyn LiveActivityBackend>,
}

impl LiveActivity {
    /// Connects a framework event sink to a newly constructed backend.
    pub fn connect<S, B, F>(sink: S, factory: F) -> Result<Self>
    where
        S: EventSink,
        B: LiveActivityBackend,
        F: FnOnce(BackendContext) -> Result<B>,
    {
        let backend = factory(BackendContext::new(sink))?;
        Ok(Self {
            backend: Arc::new(backend),
        })
    }

    /// Returns capabilities of the selected backend.
    pub async fn capabilities(&self) -> Result<Capabilities> {
        let capabilities = self.backend.capabilities().await?;
        capabilities.validate()?;
        Ok(capabilities)
    }

    /// Starts a Live Activity through the selected backend.
    pub async fn start(&self, request: StartRequest) -> Result<ActivityRecord> {
        request.validate()?;
        let activity = self.backend.start(request).await?;
        activity.validate()?;
        Ok(activity)
    }

    /// Updates a Live Activity through the selected backend.
    pub async fn update(&self, request: UpdateRequest) -> Result<ActivityRecord> {
        request.validate()?;
        let activity = self.backend.update(request).await?;
        activity.validate()?;
        Ok(activity)
    }

    /// Ends a Live Activity through the selected backend.
    pub async fn end(&self, request: EndRequest) -> Result<ActivityRecord> {
        request.validate()?;
        let activity = self.backend.end(request).await?;
        activity.validate()?;
        Ok(activity)
    }

    /// Lists activities through the selected backend.
    pub async fn list(&self) -> Result<Vec<ActivityRecord>> {
        let activities = self.backend.list().await?;
        for activity in &activities {
            activity.validate()?;
        }
        Ok(activities)
    }
}
