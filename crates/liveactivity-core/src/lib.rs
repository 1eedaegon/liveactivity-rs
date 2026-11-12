//! Framework-neutral Live Activity contracts and backend interfaces.
//!
//! Framework adapters implement LiveActivityBackend for commands and
//! EventSink for events. LiveActivity::connect joins both directions without
//! importing any framework-specific handle into this crate.

#![forbid(unsafe_code)]

mod backend;
mod error;
mod event;
mod model;
mod service;

pub use backend::{BackendFuture, LiveActivityBackend};
pub use error::{Error, ErrorCode, Result};
pub use event::{BackendContext, EventSink};
pub use model::{
    ActivityRecord, ActivityState, Capabilities, DismissalPolicy, EndRequest, LiveActivityEvent,
    LiveActivityEventPayload, StartRequest, UpdateRequest,
};
pub use service::LiveActivity;

/// Version of the typed and serialized contract in this crate.
pub const CONTRACT_VERSION: u16 = 1;
