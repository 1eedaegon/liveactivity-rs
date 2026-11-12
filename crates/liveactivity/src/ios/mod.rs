//! The default iOS backend: the `activitykit` crate behind the interface.
//!
//! [`ActivityKitBackend`] changes nothing of activitykit's behavior. activitykit links its own
//! Swift bridge, so the backend needs no framework transport. Off iOS it compiles and answers
//! `unsupportedPlatform`, as activitykit does.
//!
//! Portable template content becomes a layout activity with a fixed tree (`template`). Native
//! `content` is activitykit's request contract v2, so no model is declared twice:
//!
//! - start: an `activitykit::contract::LayoutStartInput` without `activityKey` and
//!   `schemaVersion`, which the neutral request carries.
//! - update, and the final content of an end: a `LayoutSnapshot` whose `revision` is the
//!   request's.
//! - record: a `LayoutActivityRecord` without `id`, `activityKey` and `state`, which the
//!   neutral record carries.

mod backend;
mod content;
mod events;
mod template;

pub use backend::ActivityKitBackend;

/// Stable identifier this backend reports in capabilities and records.
pub const BACKEND_NAME: &str = "ios-activitykit";
