use super::content;
use crate::{
    ActivityRecord, BackendContext, BackendFuture, Capabilities, EndRequest, LiveActivityBackend,
    Permissions, StartRequest, UpdateRequest,
};
use liveupdate::{Bridge, Driver, JsonDriver, LiveUpdate};
use std::sync::Mutex;

/// `LiveActivityBackend` over the liveupdate lifecycle engine.
///
/// The engine calls its driver synchronously, so every returned future completes on its first
/// poll and blocks the polling thread for the duration of the native call. Poll it where a
/// blocking Android bridge call is allowed.
///
/// The Android engine keeps no revision for an end and removes the notification at once: an
/// end's `revision` is not compared, and an end with final content is `unsupported`.
pub struct LiveUpdateBackend<D> {
    pub(crate) engine: LiveUpdate<D>,
    pub(crate) context: BackendContext,
    /// Held for one whole `pump_events`.
    pub(crate) pump: Mutex<()>,
}

impl<D: Driver + 'static> LiveUpdateBackend<D> {
    /// Creates the backend from the context supplied by `LiveActivity::connect`.
    pub fn new(context: BackendContext, driver: D) -> Self {
        Self {
            engine: LiveUpdate::new(driver),
            context,
            pump: Mutex::new(()),
        }
    }

    /// The wrapped engine, for Android detail outside the neutral contract such as promotion
    /// capabilities. A framework adapter keeps the backend in an `Arc` to reach it.
    pub fn engine(&self) -> &LiveUpdate<D> {
        &self.engine
    }
}

impl<B: Bridge + 'static> LiveUpdateBackend<JsonDriver<B>> {
    /// Creates the backend over a framework's string transport to the managed Android adapter.
    pub fn over_bridge(context: BackendContext, bridge: B) -> Self {
        Self::new(context, JsonDriver::new(bridge))
    }
}

#[cfg(target_os = "android")]
impl LiveUpdateBackend<liveupdate::AndroidDriver> {
    /// Creates the backend on the JNI transport of liveupdate's own AAR, for a host without a
    /// framework bridge. The host must have initialized `ndk-context`.
    ///
    /// # Errors
    ///
    /// Fails when the Android context or the AAR's bridge class is not available.
    pub fn android(context: BackendContext) -> crate::Result<Self> {
        let bridge = liveupdate::AndroidBridge::new().map_err(content::error)?;
        Ok(Self::over_bridge(context, bridge))
    }
}

impl<D: Driver + 'static> LiveActivityBackend for LiveUpdateBackend<D> {
    fn capabilities(&self) -> BackendFuture<'_, Capabilities> {
        Box::pin(async move {
            self.engine
                .capabilities()
                .map(content::capabilities)
                .map_err(content::error)
        })
    }

    fn start(&self, request: StartRequest) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move {
            let request = content::start_request(request)?;
            self.engine
                .start(request)
                .map_err(content::error)
                .and_then(content::activity_record)
        })
    }

    fn update(&self, request: UpdateRequest) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move {
            let id = request.id.clone();
            let snapshot = content::snapshot(request)?;
            self.engine
                .update(&id, snapshot)
                .map_err(content::error)
                .and_then(content::activity_record)
        })
    }

    fn end(&self, request: EndRequest) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move {
            let policy = content::end_policy(&request)?;
            self.engine
                .end(&request.id, policy)
                .map_err(content::error)
                .and_then(content::activity_record)
        })
    }

    fn list(&self) -> BackendFuture<'_, Vec<ActivityRecord>> {
        Box::pin(async move { self.records() })
    }

    fn sync_events(&self) -> BackendFuture<'_, ()> {
        Box::pin(async move { self.pump_events().map(|_| ()) })
    }

    /// The engine reads permissions and cannot prompt, so `request_permission` keeps its
    /// default: the host shows Android's dialog and reads the result here.
    fn permissions(&self) -> BackendFuture<'_, Permissions> {
        Box::pin(async move {
            self.engine
                .capabilities()
                .map(content::permissions)
                .map_err(content::error)
        })
    }
}
