use crate::{
    ActivityRecord, Capabilities, EndInput, Error, LayoutActivityRecord, LayoutEndInput,
    LayoutStartInput, LayoutUpdateInput, PermissionKind, Permissions, Result, StartInput,
    UpdateInput,
};
use std::{future::Future, pin::Pin, sync::Arc};

/// Boxed future of the object-safe backend interface.
pub type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

fn unsupported<'a, T: 'a>(message: &'static str) -> BackendFuture<'a, T> {
    Box::pin(async move { Err(Error::unsupported_feature(message)) })
}

/// What a platform implements to show Live Activities: one method per command of the contract.
///
/// The five template commands and the two queries are required. Layouts, permission requests,
/// images and queued events have defaults, so a backend that draws templates only is complete.
/// The requests are validated before they reach a backend.
pub trait LiveActivityBackend: Send + Sync + 'static {
    /// What the backend can do on this device.
    fn capabilities(&self) -> BackendFuture<'_, Capabilities>;

    /// The operating system's permissions.
    fn permissions(&self) -> BackendFuture<'_, Permissions>;

    /// Starts a progress or timer activity.
    fn start(&self, input: StartInput) -> BackendFuture<'_, ActivityRecord>;

    /// Replaces the content of a progress or timer activity.
    fn update(&self, input: UpdateInput) -> BackendFuture<'_, ActivityRecord>;

    /// Ends a progress or timer activity.
    fn end(&self, input: EndInput) -> BackendFuture<'_, ActivityRecord>;

    /// The progress and timer activities the backend knows.
    fn list(&self) -> BackendFuture<'_, Vec<ActivityRecord>>;

    /// Asks the operating system for a permission. The default cannot ask.
    fn request_permissions(&self, kind: PermissionKind) -> BackendFuture<'_, Permissions> {
        let _ = kind;
        unsupported("This backend cannot request permissions.")
    }

    /// Starts a layout activity. The default draws no layouts.
    fn start_layout(&self, input: LayoutStartInput) -> BackendFuture<'_, LayoutActivityRecord> {
        let _ = input;
        unsupported("This backend draws no layouts.")
    }

    /// Replaces the values of a layout activity. The default draws no layouts.
    fn update_layout(&self, input: LayoutUpdateInput) -> BackendFuture<'_, LayoutActivityRecord> {
        let _ = input;
        unsupported("This backend draws no layouts.")
    }

    /// Ends a layout activity. The default draws no layouts.
    fn end_layout(&self, input: LayoutEndInput) -> BackendFuture<'_, LayoutActivityRecord> {
        let _ = input;
        unsupported("This backend draws no layouts.")
    }

    /// The layout activities the backend knows. The default has none.
    fn list_layout(&self) -> BackendFuture<'_, Vec<LayoutActivityRecord>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    /// Stores a checked PNG under `name` for `image` nodes that name it. The default stores none.
    fn write_image(&self, name: String, png: Vec<u8>) -> BackendFuture<'_, ()> {
        let _ = (name, png);
        unsupported("This backend stores no images.")
    }

    /// Sends the taps the platform queued to the event sink. A platform that calls back needs
    /// nothing here, which is the default.
    fn sync_events(&self) -> BackendFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

impl<T: LiveActivityBackend + ?Sized> LiveActivityBackend for Arc<T> {
    fn capabilities(&self) -> BackendFuture<'_, Capabilities> {
        (**self).capabilities()
    }

    fn permissions(&self) -> BackendFuture<'_, Permissions> {
        (**self).permissions()
    }

    fn start(&self, input: StartInput) -> BackendFuture<'_, ActivityRecord> {
        (**self).start(input)
    }

    fn update(&self, input: UpdateInput) -> BackendFuture<'_, ActivityRecord> {
        (**self).update(input)
    }

    fn end(&self, input: EndInput) -> BackendFuture<'_, ActivityRecord> {
        (**self).end(input)
    }

    fn list(&self) -> BackendFuture<'_, Vec<ActivityRecord>> {
        (**self).list()
    }

    fn request_permissions(&self, kind: PermissionKind) -> BackendFuture<'_, Permissions> {
        (**self).request_permissions(kind)
    }

    fn start_layout(&self, input: LayoutStartInput) -> BackendFuture<'_, LayoutActivityRecord> {
        (**self).start_layout(input)
    }

    fn update_layout(&self, input: LayoutUpdateInput) -> BackendFuture<'_, LayoutActivityRecord> {
        (**self).update_layout(input)
    }

    fn end_layout(&self, input: LayoutEndInput) -> BackendFuture<'_, LayoutActivityRecord> {
        (**self).end_layout(input)
    }

    fn list_layout(&self) -> BackendFuture<'_, Vec<LayoutActivityRecord>> {
        (**self).list_layout()
    }

    fn write_image(&self, name: String, png: Vec<u8>) -> BackendFuture<'_, ()> {
        (**self).write_image(name, png)
    }

    fn sync_events(&self) -> BackendFuture<'_, ()> {
        (**self).sync_events()
    }
}
