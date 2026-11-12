use crate::{EventSink, LiveActivity, Result};

/// Connects `sink` to the default native backend of the target platform:
/// [`android::LiveUpdateBackend`](crate::android::LiveUpdateBackend) on the JNI transport of
/// liveupdate's own AAR on Android, and
/// [`ios::ActivityKitBackend`](crate::ios::ActivityKitBackend) elsewhere, which off iOS answers
/// `unsupportedPlatform` and reports no capability.
///
/// Another backend, such as the Android engine over a framework's own bridge, connects through
/// [`LiveActivity::connect`] instead.
///
/// # Errors
///
/// Fails when the backend cannot start, or with `unsupportedPlatform` when the feature of the
/// target's backend is disabled.
pub fn connect_native<S: EventSink>(sink: S) -> Result<LiveActivity> {
    #[cfg(all(feature = "liveupdate", target_os = "android"))]
    return LiveActivity::connect(sink, crate::android::LiveUpdateBackend::android);
    #[cfg(all(feature = "activitykit", not(target_os = "android")))]
    return LiveActivity::connect(sink, crate::ios::ActivityKitBackend::new);
    #[allow(unreachable_code)]
    {
        drop(sink);
        Err(crate::Error::new(
            crate::ErrorCode::UnsupportedPlatform,
            "no native backend is compiled in for this target",
        ))
    }
}
