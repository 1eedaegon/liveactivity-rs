//! The answer of a call for code that cannot await.

use crate::{Error, Result};
use std::{
    future::Future,
    pin::pin,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    thread,
};

/// Whether the calling thread is the process's main thread, where activitykit answers.
#[cfg(target_os = "ios")]
#[allow(unsafe_code)]
fn on_main_thread() -> bool {
    unsafe extern "C" {
        fn pthread_main_np() -> std::ffi::c_int;
    }
    // SAFETY: pthread_main_np takes no argument and only reads which thread calls it.
    unsafe { pthread_main_np() != 0 }
}

#[cfg(not(target_os = "ios"))]
fn on_main_thread() -> bool {
    false
}

/// The answer of a call, for code that cannot await: [`block_on`] with a guard. On the main thread of an iOS app
/// the answer would never come, because activitykit answers there, so the call answers `NATIVE_ERROR` at once
/// instead of hanging.
///
/// # Errors
///
/// The error of the call, or `NATIVE_ERROR` on the main thread of an iOS app.
pub fn wait<T>(future: impl Future<Output = Result<T>>) -> Result<T> {
    wait_on(on_main_thread(), future)
}

fn wait_on<T>(main: bool, future: impl Future<Output = Result<T>>) -> Result<T> {
    if main {
        return Err(Error::new(
            "NATIVE_ERROR",
            "A blocking call on the main thread, where activitykit answers, would never end: call it on another \
             thread, or await it.",
        ));
    }
    block_on(future)
}

/// Drives `future` to its end on this thread.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blocking_call_on_the_main_thread_answers_at_once() {
        // REQ-C-004
        let call = || async { Ok::<_, Error>(7) };
        assert_eq!(wait_on(false, call()), Ok(7));
        assert_eq!(wait_on(true, call()).unwrap_err().code, "NATIVE_ERROR");
        // A test runs off the main thread.
        assert_eq!(wait(call()), Ok(7));
    }

    #[test]
    fn block_on_drives_a_future_that_another_thread_completes() {
        let (sender, receiver) = activitykit::async_channel::bounded(1);
        let answer = thread::spawn(move || {
            thread::sleep(std::time::Duration::from_millis(20));
            sender.send_blocking(42).unwrap();
        });
        assert_eq!(block_on(async { receiver.recv().await.unwrap() }), 42);
        answer.join().unwrap();
    }
}
