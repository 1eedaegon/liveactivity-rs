//! What an implementer of the interface checks a backend with.
//!
//! [`conformance`] runs the portable lifecycle against any connected [`LiveActivity`](crate::LiveActivity)
//! and reports each rule a backend must keep to be exchangeable with another. [`MemoryBackend`]
//! is a reference backend without a platform, for a framework's own tests and as the simplest
//! example of an implementation.

mod conformance;
mod memory;

pub use conformance::{Check, Report, conformance};
pub use memory::MemoryBackend;

use std::{
    future::Future,
    pin::pin,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    thread,
};

/// Drives `future` to its end on this thread, for tests without an async runtime.
pub fn block_on<F: Future>(future: F) -> F::Output {
    struct Unpark(thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Unpark(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        thread::park();
    }
}
