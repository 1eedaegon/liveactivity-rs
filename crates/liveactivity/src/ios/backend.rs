use super::{Acknowledge, Apple, Kit};
use crate::{
    ActivityRecord, BackendContext, BackendFuture, Capabilities, EndInput, Error,
    LayoutActivityRecord, LayoutEndInput, LayoutStartInput, LayoutUpdateInput, LiveActivityBackend,
    PermissionKind, Permissions, Result, StartInput, UpdateInput,
};
use activitykit::ActivityKit;
use std::thread;

/// The default iOS backend: the activitykit crate, which links its own Swift bridge.
///
/// Progress and timer templates become layout activities with a fixed tree; layouts are
/// activitykit's own. The Swift layer answers from the main thread, so do not block the main
/// thread on a call. Off iOS the backend compiles, reports no capability and answers
/// `UNSUPPORTED_PLATFORM`.
pub struct ActivityKitBackend {
    apple: Apple<ActivityKit>,
    /// The same kit, for the taps.
    kit: ActivityKit,
}

impl ActivityKitBackend {
    /// The backend on activitykit's default settings.
    pub fn new() -> Self {
        Self::with_kit(ActivityKit::new())
    }

    /// The backend on a kit that the caller built, for example with redacted receipts.
    pub fn with_kit(kit: ActivityKit) -> Self {
        Self {
            apple: Apple::new(kit.clone()),
            kit,
        }
    }
}

impl Default for ActivityKitBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// Starts the thread that hands every tap of `kit` to the sink, until the kit's events end. The kit is subscribed
/// to before this returns, so a tap that launched the app waits for the subscriber.
fn forward_taps<K: Kit>(kit: &K, context: BackendContext) -> Result<()> {
    let taps = kit.taps();
    thread::Builder::new()
        .name("liveactivity-taps".into())
        .spawn(move || {
            for (event, acknowledgement) in taps {
                // The tap keeps its intent until the sink has returned.
                context.emit(event);
                acknowledgement.acknowledge();
            }
        })
        .map(|_| ())
        .map_err(|_| Error::native())
}

impl LiveActivityBackend for ActivityKitBackend {
    /// Starts the thread that forwards taps. Elsewhere than on a device with Live Activities activitykit reports no
    /// event, so no thread waits for one.
    fn attach(&self, events: BackendContext) -> Result<()> {
        if Kit::authorization(&self.kit).supported {
            forward_taps(&self.kit, events)?;
        }
        Ok(())
    }

    fn capabilities(&self) -> BackendFuture<'_, Capabilities> {
        Box::pin(async move { Ok(self.apple.capabilities()) })
    }

    fn permissions(&self) -> BackendFuture<'_, Permissions> {
        Box::pin(async move { Ok(self.apple.permissions(false)) })
    }

    fn request_permissions(&self, kind: PermissionKind) -> BackendFuture<'_, Permissions> {
        Box::pin(async move { self.apple.request_permissions(kind) })
    }

    fn start(&self, input: StartInput) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(self.apple.start_template(input))
    }

    fn update(&self, input: UpdateInput) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(self.apple.update_template(input))
    }

    fn end(&self, input: EndInput) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(self.apple.end_template(input))
    }

    fn list(&self) -> BackendFuture<'_, Vec<ActivityRecord>> {
        Box::pin(self.apple.list_templates())
    }

    fn start_layout(&self, input: LayoutStartInput) -> BackendFuture<'_, LayoutActivityRecord> {
        Box::pin(self.apple.start_layout(input))
    }

    fn update_layout(&self, input: LayoutUpdateInput) -> BackendFuture<'_, LayoutActivityRecord> {
        Box::pin(self.apple.update_layout(input))
    }

    fn end_layout(&self, input: LayoutEndInput) -> BackendFuture<'_, LayoutActivityRecord> {
        Box::pin(self.apple.end_layout(input))
    }

    fn list_layout(&self) -> BackendFuture<'_, Vec<LayoutActivityRecord>> {
        Box::pin(self.apple.list_layout())
    }

    fn write_image(&self, name: String, png: Vec<u8>) -> BackendFuture<'_, ()> {
        Box::pin(async move { self.apple.write_image(&name, &png) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActionEvent, EndInput, LiveActivity, StartInput, UpdateInput, block_on,
        ios::fake::{FakeAck, FakeKit, Log},
    };
    use std::{sync::mpsc, time::Duration};

    #[test]
    fn a_tap_reaches_the_sink_and_keeps_its_intent_until_the_sink_returns() {
        let kit = FakeKit::default();
        let log = Log::default();
        let (seen, taps) = mpsc::channel();
        let sink_log = log.clone();
        let context = BackendContext::new(move |event: ActionEvent| {
            // A slow handler: the tap may not be let go before it is done.
            thread::sleep(Duration::from_millis(40));
            sink_log.lock().unwrap().push("handled".into());
            seen.send(event).unwrap();
        });
        forward_taps(&kit, context).unwrap();

        let tap = ActionEvent {
            action_id: "pet".into(),
            activity_id: "la-1".into(),
            activity_key: Some("demo:1".into()),
        };
        kit.send_tap(tap.clone(), FakeAck(log.clone()));

        assert_eq!(taps.recv_timeout(Duration::from_secs(5)).unwrap(), tap);
        for _ in 0..200 {
            if log.lock().unwrap().len() == 2 {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(*log.lock().unwrap(), ["handled", "released"]);
        // When the kit's events end, so does the thread.
        kit.close_taps();
    }

    #[test]
    fn off_ios_the_backend_reports_nothing_and_answers_unsupported_platform() {
        let live = LiveActivity::builder()
            .backend(ActivityKitBackend::new())
            .event_sink(|_: ActionEvent| unreachable!("no taps off iOS"))
            .build()
            .unwrap();

        let capabilities = block_on(live.capabilities()).unwrap();
        assert_eq!(capabilities.backend, crate::Backend::Unsupported);
        assert_eq!(capabilities.start, crate::FeatureSupport::Unsupported);
        let permissions = block_on(live.permissions()).unwrap();
        assert_eq!(
            permissions.live_activities,
            crate::PermissionState::NotApplicable
        );

        let start = StartInput::progress("order:42", "Order 42", "Preparing", 0.1);
        let refused = block_on(live.start(start)).unwrap_err();
        assert_eq!(refused.code, "UNSUPPORTED_PLATFORM");
        // A request outside the contract stops before the backend.
        let invalid = StartInput::progress("order 42", "Order 42", "Preparing", 0.1);
        assert_eq!(
            block_on(live.start(invalid)).unwrap_err().code,
            "INVALID_INPUT"
        );
        let update = UpdateInput::progress("id", 1, "Order 42", "On the way", 0.6);
        assert_eq!(
            block_on(live.update(update)).unwrap_err().code,
            "UNSUPPORTED_PLATFORM"
        );
        assert_eq!(
            block_on(live.end(EndInput::new("id"))).unwrap_err().code,
            "UNSUPPORTED_PLATFORM"
        );
        assert_eq!(
            block_on(live.list()).unwrap_err().code,
            "UNSUPPORTED_PLATFORM"
        );
        assert!(block_on(live.list_layout()).is_err());
    }
}
