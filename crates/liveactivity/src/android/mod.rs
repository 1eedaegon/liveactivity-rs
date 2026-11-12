//! The default Android backend: the `liveupdate` engine behind the interface.
//!
//! [`LiveUpdateBackend`] changes nothing of the engine's behavior. The adapter supplies only
//! the transport: a `liveupdate::Driver`, a string `liveupdate::Bridge` such as the one a
//! framework plugin builds over its own native call, or liveupdate's own JNI transport.
//!
//! The neutral `content` is liveupdate's own JSON contract, so no model is declared twice:
//!
//! - start: a `liveupdate::StartRequest` without `activityKey` and `schemaVersion`, which the
//!   neutral request carries.
//! - update: a `liveupdate::Snapshot` whose `revision` is the request's.
//! - record: a `liveupdate::ActivityRecord` without `id`, `activityKey` and `state`, which the
//!   neutral record carries.

mod backend;
mod content;
mod events;

pub use backend::LiveUpdateBackend;
pub use events::EventReport;

/// Stable identifier this backend reports in capabilities and records.
pub const BACKEND_NAME: &str = "android-liveupdate";
