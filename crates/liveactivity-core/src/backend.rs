use crate::{ActivityRecord, Capabilities, EndRequest, Result, StartRequest, UpdateRequest};
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
}
