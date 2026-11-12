use crate::{EventSink, LiveActivity, LiveActivityBackend, Result};
use std::sync::Arc;

/// The default backend of the target platform.
pub(crate) fn default_backend() -> Result<Arc<dyn LiveActivityBackend>> {
    #[cfg(target_os = "android")]
    let backend = Arc::new(crate::android::LiveUpdateBackend::android()?);
    #[cfg(not(target_os = "android"))]
    let backend = Arc::new(crate::ios::ActivityKitBackend::new());
    Ok(backend)
}

/// The shortest connection: the default backend of the target platform, with `sink` for taps. The same as
/// `LiveActivity::builder().event_sink(sink).build()`.
///
/// # Errors
///
/// Fails when the backend cannot start: on Android when the host has not initialized `ndk-context` or does not
/// package liveupdate's AAR.
pub fn connect_native<S: EventSink>(sink: S) -> Result<LiveActivity> {
    LiveActivity::builder().event_sink(sink).build()
}
