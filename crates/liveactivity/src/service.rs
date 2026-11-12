use crate::{
    ActivityRecord, BackendContext, Capabilities, EndInput, EventSink, LayoutActivityRecord,
    LayoutEndInput, LayoutStartInput, LayoutUpdateInput, LiveActivityBackend, PermissionKind,
    Permissions, Result, StartInput, UpdateInput, Validate, check_image,
};
use std::sync::Arc;

/// Live Activities of one application, on whichever backend it was connected to.
///
/// Cheap to clone and safe to use from any thread. Every request is checked against the
/// contract before it reaches the backend, so a backend sees valid requests only.
#[derive(Clone)]
pub struct LiveActivity {
    backend: Arc<dyn LiveActivityBackend>,
}

impl LiveActivity {
    /// Connects `sink` to the backend that `backend` makes. This is the one place that names a
    /// backend: everything after it is the same for all of them.
    ///
    /// # Errors
    ///
    /// Fails when the backend cannot be made.
    pub fn connect<S, B, F>(sink: S, backend: F) -> Result<Self>
    where
        S: EventSink,
        B: LiveActivityBackend,
        F: FnOnce(BackendContext) -> Result<B>,
    {
        let backend = backend(BackendContext::new(sink))?;
        Ok(Self {
            backend: Arc::new(backend),
        })
    }

    /// What the backend can do on this device.
    pub async fn capabilities(&self) -> Result<Capabilities> {
        self.backend.capabilities().await
    }

    /// The operating system's permissions.
    pub async fn permissions(&self) -> Result<Permissions> {
        self.backend.permissions().await
    }

    /// Asks the operating system for a permission.
    pub async fn request_permissions(&self, kind: PermissionKind) -> Result<Permissions> {
        self.backend.request_permissions(kind).await
    }

    /// Starts a progress or timer activity. A repeated start with the same key and request
    /// answers the activity that already runs.
    pub async fn start(&self, input: StartInput) -> Result<ActivityRecord> {
        input.validate()?;
        self.backend.start(input).await
    }

    /// Replaces the content of a progress or timer activity under a higher revision.
    pub async fn update(&self, input: UpdateInput) -> Result<ActivityRecord> {
        input.validate()?;
        self.backend.update(input).await
    }

    /// Ends a progress or timer activity. Ending it again answers its record.
    pub async fn end(&self, input: EndInput) -> Result<ActivityRecord> {
        input.validate()?;
        self.backend.end(input).await
    }

    /// The progress and timer activities the backend knows.
    pub async fn list(&self) -> Result<Vec<ActivityRecord>> {
        self.backend.list().await
    }

    /// Starts a layout activity.
    pub async fn start_layout(&self, input: LayoutStartInput) -> Result<LayoutActivityRecord> {
        input.validate()?;
        self.backend.start_layout(input).await
    }

    /// Replaces the values of a layout activity.
    pub async fn update_layout(&self, input: LayoutUpdateInput) -> Result<LayoutActivityRecord> {
        input.validate()?;
        self.backend.update_layout(input).await
    }

    /// Ends a layout activity, with final content when the input has it.
    pub async fn end_layout(&self, input: LayoutEndInput) -> Result<LayoutActivityRecord> {
        input.validate()?;
        self.backend.end_layout(input).await
    }

    /// The layout activities the backend knows.
    pub async fn list_layout(&self) -> Result<Vec<LayoutActivityRecord>> {
        self.backend.list_layout().await
    }

    /// Stores a PNG that `image` nodes show under `name`, replacing an earlier one.
    pub async fn write_image(&self, name: &str, png: &[u8]) -> Result<()> {
        check_image(name, png)?;
        self.backend
            .write_image(name.to_owned(), png.to_vec())
            .await
    }

    /// Sends the taps the platform queued to the event sink. Android queues them: call this
    /// when the application resumes.
    pub async fn sync_events(&self) -> Result<()> {
        self.backend.sync_events().await
    }
}
