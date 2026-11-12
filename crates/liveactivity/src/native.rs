use crate::{
    ActivityRecord, Backend, BackendFuture, Capabilities, EndInput, Error, EventSink,
    FeatureSupport, LayoutActivityRecord, LayoutEndInput, LayoutStartInput, LayoutUpdateInput,
    LiveActivity, LiveActivityBackend, PermissionKind, PermissionState, Permissions, Result,
    StartInput, UpdateInput,
};
use std::sync::Arc;

/// The backend of a platform without Live Activities, which is the default everywhere but on iOS and Android: the
/// two queries answer that nothing is supported, and every other call is `UNSUPPORTED_PLATFORM`. An application
/// that also runs on a desktop needs no code of its own for it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnsupportedBackend;

fn unsupported<T: Send + 'static>() -> BackendFuture<'static, T> {
    Box::pin(async { Err(Error::unsupported()) })
}

impl LiveActivityBackend for UnsupportedBackend {
    fn capabilities(&self) -> BackendFuture<'_, Capabilities> {
        use FeatureSupport::Unsupported;
        Box::pin(async {
            Ok(Capabilities {
                platform: std::env::consts::OS.into(),
                backend: Backend::Unsupported,
                start: Unsupported,
                update: Unsupported,
                end: Unsupported,
                progress: Unsupported,
                timer: Unsupported,
                actions: Unsupported,
                state_observation: Unsupported,
                silent_update: Unsupported,
                native_live_activity: Unsupported,
                max_payload_bytes: 0,
                layout: Unsupported,
                push: Unsupported,
                android_presentation: None,
            })
        })
    }

    fn permissions(&self) -> BackendFuture<'_, Permissions> {
        use PermissionState::NotApplicable;
        Box::pin(async {
            Ok(Permissions {
                notifications: NotApplicable,
                live_activities: NotApplicable,
                promotion: NotApplicable,
                frequent_pushes: NotApplicable,
            })
        })
    }

    fn request_permissions(&self, _kind: PermissionKind) -> BackendFuture<'_, Permissions> {
        unsupported()
    }

    fn start(&self, _input: StartInput) -> BackendFuture<'_, ActivityRecord> {
        unsupported()
    }

    fn update(&self, _input: UpdateInput) -> BackendFuture<'_, ActivityRecord> {
        unsupported()
    }

    fn end(&self, _input: EndInput) -> BackendFuture<'_, ActivityRecord> {
        unsupported()
    }

    fn list(&self) -> BackendFuture<'_, Vec<ActivityRecord>> {
        unsupported()
    }

    fn start_layout(&self, _input: LayoutStartInput) -> BackendFuture<'_, LayoutActivityRecord> {
        unsupported()
    }

    fn update_layout(&self, _input: LayoutUpdateInput) -> BackendFuture<'_, LayoutActivityRecord> {
        unsupported()
    }

    fn end_layout(&self, _input: LayoutEndInput) -> BackendFuture<'_, LayoutActivityRecord> {
        unsupported()
    }

    fn list_layout(&self) -> BackendFuture<'_, Vec<LayoutActivityRecord>> {
        unsupported()
    }

    fn write_image(&self, _name: String, _png: Vec<u8>) -> BackendFuture<'_, ()> {
        unsupported()
    }
}

/// The default backend of the target platform.
pub(crate) fn default_backend() -> Result<Arc<dyn LiveActivityBackend>> {
    #[cfg(target_os = "android")]
    let backend = Arc::new(crate::android::LiveUpdateBackend::android()?);
    #[cfg(target_os = "ios")]
    let backend = Arc::new(crate::ios::ActivityKitBackend::new());
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let backend = Arc::new(UnsupportedBackend);
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
