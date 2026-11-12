use crate::{EventSink, LiveActivity, Result};

/// Connects `sink` to the default backend of the target platform: Live Updates on Android,
/// ActivityKit elsewhere, which off iOS reports no capability and answers
/// `UNSUPPORTED_PLATFORM`.
///
/// # Errors
///
/// Fails when the backend cannot start: on Android when the host has not initialized
/// `ndk-context` or does not package liveupdate's AAR.
pub fn connect_native<S: EventSink>(sink: S) -> Result<LiveActivity> {
    #[cfg(target_os = "android")]
    let live = LiveActivity::connect(sink, crate::android::LiveUpdateBackend::android);
    #[cfg(not(target_os = "android"))]
    let live = LiveActivity::connect(sink, crate::ios::ActivityKitBackend::new);
    live
}
