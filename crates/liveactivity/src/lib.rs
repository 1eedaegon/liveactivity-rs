//! Framework-neutral Live Activities.
//!
//! One interface, [`LiveActivityBackend`] for commands and [`EventSink`] for
//! events, joined by [`LiveActivity::connect`]. A framework plugin implements
//! the sink and exposes [`LiveActivity`] to its framework; a native
//! implementation implements the backend.
//!
//! The default backends are the `activitykit` and `liveupdate` crates, which
//! this crate depends on and does not reimplement: [`ios::ActivityKitBackend`]
//! and [`android::LiveUpdateBackend`] only fit them to the interface, and
//! [`connect_native`] selects the one of the target. With
//! `default-features = false` the crate is the interface alone.
//!
//! No framework-specific handle is imported into this crate.

#![forbid(unsafe_code)]

#[cfg(feature = "liveupdate")]
pub mod android;
mod backend;
mod content;
mod error;
mod event;
#[cfg(feature = "activitykit")]
pub mod ios;
mod model;
mod native;
mod permission;
mod service;
#[cfg(feature = "testkit")]
pub mod testkit;

/// The iOS engine crate, so an adapter builds its kit with the same version.
#[cfg(feature = "activitykit")]
#[doc(no_inline)]
pub use ::activitykit;
/// The Android engine crate, so an adapter implements its bridge against the same version.
#[cfg(feature = "liveupdate")]
#[doc(no_inline)]
pub use ::liveupdate;
pub use backend::{BackendFuture, LiveActivityBackend};
pub use content::{Content, Fallback, Template, TemplateKind, Timer};
pub use error::{Error, ErrorCode, Result};
pub use event::{BackendContext, EventSink};
pub use model::{
    ActivityRecord, ActivityState, Capabilities, DismissalPolicy, EndRequest, FeatureSupport,
    LiveActivityEvent, LiveActivityEventPayload, StartRequest, UpdateRequest,
};
pub use native::connect_native;
pub use permission::{PermissionKind, PermissionState, Permissions};
pub use service::LiveActivity;

/// Version of the typed and serialized contract in this crate.
pub const CONTRACT_VERSION: u16 = 1;
