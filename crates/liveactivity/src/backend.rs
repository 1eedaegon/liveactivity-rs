use crate::{
    ActivityRecord, Capabilities, EndRequest, Error, ErrorCode, PermissionKind, PermissionState,
    Permissions, Result, StartRequest, UpdateRequest,
};
use std::{future::Future, pin::Pin, sync::Arc};

/// Boxed future used by the object-safe backend interface.
pub type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/// Command bridge implemented by a platform or framework adapter.
pub trait LiveActivityBackend: Send + Sync + 'static {
    /// Reports backend and device capabilities.
    fn capabilities(&self) -> BackendFuture<'_, Capabilities>;

    /// Starts a Live Activity.
    fn start(&self, request: StartRequest) -> BackendFuture<'_, ActivityRecord>;

    /// Updates a Live Activity.
    fn update(&self, request: UpdateRequest) -> BackendFuture<'_, ActivityRecord>;

    /// Ends a Live Activity.
    fn end(&self, request: EndRequest) -> BackendFuture<'_, ActivityRecord>;

    /// Lists the activities known to the backend.
    fn list(&self) -> BackendFuture<'_, Vec<ActivityRecord>>;

    /// Sends the events the platform queued since the last call to the event sink.
    ///
    /// A platform that calls back needs nothing here, which is the default. A platform that
    /// queues events, as Android does, drains them; the framework adapter calls this when its
    /// native side signals an event and when the application resumes.
    fn sync_events(&self) -> BackendFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    /// Reads the operating system's permissions. The default knows none.
    fn permissions(&self) -> BackendFuture<'_, Permissions> {
        Box::pin(async { Ok(Permissions::all(PermissionState::Unknown)) })
    }

    /// Asks the operating system for `kind` and answers the permissions after it. The default
    /// cannot ask.
    fn request_permission(&self, kind: PermissionKind) -> BackendFuture<'_, Permissions> {
        let _ = kind;
        Box::pin(async {
            Err(Error::new(
                ErrorCode::Unsupported,
                "this backend cannot request permissions",
            ))
        })
    }

    /// Stores the PNG `png` under `name` for content that names it, replacing an earlier one.
    /// The default stores no images.
    fn write_image(&self, name: String, png: Vec<u8>) -> BackendFuture<'_, ()> {
        let _ = (name, png);
        Box::pin(async {
            Err(Error::new(
                ErrorCode::Unsupported,
                "this backend stores no images",
            ))
        })
    }
}

impl<T> LiveActivityBackend for Arc<T>
where
    T: LiveActivityBackend + ?Sized,
{
    fn capabilities(&self) -> BackendFuture<'_, Capabilities> {
        (**self).capabilities()
    }

    fn start(&self, request: StartRequest) -> BackendFuture<'_, ActivityRecord> {
        (**self).start(request)
    }

    fn update(&self, request: UpdateRequest) -> BackendFuture<'_, ActivityRecord> {
        (**self).update(request)
    }

    fn end(&self, request: EndRequest) -> BackendFuture<'_, ActivityRecord> {
        (**self).end(request)
    }

    fn list(&self) -> BackendFuture<'_, Vec<ActivityRecord>> {
        (**self).list()
    }

    fn sync_events(&self) -> BackendFuture<'_, ()> {
        (**self).sync_events()
    }

    fn permissions(&self) -> BackendFuture<'_, Permissions> {
        (**self).permissions()
    }

    fn request_permission(&self, kind: PermissionKind) -> BackendFuture<'_, Permissions> {
        (**self).request_permission(kind)
    }

    fn write_image(&self, name: String, png: Vec<u8>) -> BackendFuture<'_, ()> {
        (**self).write_image(name, png)
    }
}
