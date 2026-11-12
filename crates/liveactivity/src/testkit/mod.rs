//! What an implementer of the interface checks a backend with.
//!
//! [`conformance`] runs the template lifecycle on any connected [`LiveActivity`](crate::LiveActivity) and reports each
//! rule a backend keeps to be exchangeable with another. [`MemoryBackend`] is a backend without a platform, for an
//! application's own tests and as the smallest example of an implementation.

mod conformance;
mod memory;

pub use conformance::{Check, Report, conformance};
pub use memory::MemoryBackend;
