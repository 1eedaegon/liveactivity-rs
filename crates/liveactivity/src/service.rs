use crate::{
    ActivityRecord, BackendContext, Capabilities, EndInput, EventSink, LayoutActivityRecord,
    LayoutEndInput, LayoutStartInput, LayoutUpdateInput, LiveActivityBackend, PermissionKind,
    Permissions, PushTokens, Result, StartInput, UpdateInput, Validate, check_image,
};
use std::sync::Arc;

/// Connects a [`LiveActivity`]. Both sides of the interface meet here: an application names where its taps go, and
/// a framework plugin or a platform names the backend. Whatever is not named is the default.
///
/// ```no_run
/// # use liveactivity::{Event, LiveActivity};
/// # fn run(my_backend: impl liveactivity::LiveActivityBackend) -> liveactivity::Result<()> {
/// // An application: the backend of the platform, and a closure for taps.
/// let live = LiveActivity::builder()
///     .event_sink(|event: Event| println!("{event:?}"))
///     .build()?;
///
/// // A framework plugin or a platform: its own backend in place of the default.
/// let live = LiveActivity::builder().backend(my_backend).build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Default)]
#[must_use = "a builder connects nothing until `build` is called"]
pub struct Builder {
    backend: Option<Arc<dyn LiveActivityBackend>>,
    sink: Option<BackendContext>,
}

impl Builder {
    /// The backend that runs the commands. Without one, the default backend of the target platform runs them:
    /// Live Updates on Android, ActivityKit on iOS, and elsewhere [`UnsupportedBackend`](crate::UnsupportedBackend).
    pub fn backend<B: LiveActivityBackend>(self, backend: B) -> Self {
        Self {
            backend: Some(Arc::new(backend)),
            ..self
        }
    }

    /// Where taps on an activity's buttons go: any closure, or a type that implements [`EventSink`]. Without one,
    /// taps are dropped.
    pub fn event_sink<S: EventSink>(self, sink: S) -> Self {
        Self {
            sink: Some(BackendContext::new(sink)),
            ..self
        }
    }

    /// Connects the backend to the sink.
    ///
    /// # Errors
    ///
    /// Fails when the default backend cannot start, or the backend cannot take the sink.
    pub fn build(self) -> Result<LiveActivity> {
        let backend = match self.backend {
            Some(backend) => backend,
            None => crate::native::default_backend()?,
        };
        let sink = self
            .sink
            .unwrap_or_else(|| BackendContext::new(|_: crate::Event| {}));
        backend.attach(sink)?;
        Ok(LiveActivity { backend })
    }
}

/// Live Activities of one application, on whichever backend it was connected to.
///
/// Cheap to clone and safe to use from any thread. Every request is checked against the
/// contract before it reaches the backend, so a backend sees valid requests only.
#[derive(Clone)]
pub struct LiveActivity {
    backend: Arc<dyn LiveActivityBackend>,
}

impl LiveActivity {
    /// Starts a connection: name a backend, an event sink, both or neither, then [`build`](Builder::build).
    pub fn builder() -> Builder {
        Builder::default()
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

    /// The push tokens the backend holds since launch: the newest of each activity that got one, and the app's
    /// push-to-start token. Events report each token as it comes; this is for a sink that listened late.
    pub async fn push_tokens(&self) -> Result<PushTokens> {
        self.backend.push_tokens().await
    }

    /// Sends the taps the platform queued to the event sink. Android queues them: call this
    /// when the application resumes.
    pub async fn sync_events(&self) -> Result<()> {
        self.backend.sync_events().await
    }
}
