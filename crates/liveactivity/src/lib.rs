//! Live Activities for Rust applications, behind one interface.
//!
//! An application or a framework plugin talks to [`LiveActivity`]; a platform implements
//! [`LiveActivityBackend`]. The default backends are the `activitykit` crate on iOS and the
//! `liveupdate` crate on Android, used as they are: [`connect_native`] picks the one of the
//! target, and [`LiveActivity::connect`] takes any other.
//!
//! ```no_run
//! use liveactivity::{EndInput, StartInput, UpdateInput};
//!
//! # async fn run() -> liveactivity::Result<()> {
//! let live = liveactivity::connect_native(|tap: liveactivity::ActionEvent| {
//!     println!("{} tapped {}", tap.activity_id, tap.action_id);
//! })?;
//!
//! let order = live
//!     .start(StartInput::progress("order:42", "Order 42", "Preparing", 0.1))
//!     .await?;
//! live.update(UpdateInput::progress(&order.id, 1, "Order 42", "On the way", 0.6))
//!     .await?;
//! live.end(EndInput::new(&order.id)).await?;
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]

pub mod android;
mod backend;

/// The contract alone: the requests, records, layout types, errors and checks, without the service. A framework
/// plugin re-exports this as its own types.
pub mod contract {
    pub use crate::error::{ERROR_CODES, Error, Result};
    pub use crate::image::{
        ImageInput, MAX_IMAGE_BYTES, MAX_IMAGE_HEIGHT, MAX_IMAGE_WIDTH, check_image,
    };
    pub use crate::layout::*;
    pub use crate::models::*;
    pub use crate::validation::{Validate, parse_request, validate_request};
}

mod error;
mod event;
mod image;
pub mod ios;
mod layout;
mod models;
mod native;
mod service;
#[cfg(feature = "testkit")]
pub mod testkit;
mod validation;

/// The iOS engine crate, so a caller builds its kit with the same version.
#[doc(no_inline)]
pub use ::activitykit;
/// The Android engine crate, so a caller implements its bridge against the same version.
#[doc(no_inline)]
pub use ::liveupdate;
pub use backend::{BackendFuture, LiveActivityBackend};
pub use error::{ERROR_CODES, Error, Result};
pub use event::{BackendContext, EventSink};
pub use image::{
    ImageInput, MAX_IMAGE_BYTES, MAX_IMAGE_HEIGHT, MAX_IMAGE_WIDTH, check_image, write as write_png,
};
pub use ios::block_on;
pub use layout::*;
pub use models::*;
pub use native::connect_native;
pub use service::LiveActivity;
pub use validation::{Validate, parse_request, validate_request};
