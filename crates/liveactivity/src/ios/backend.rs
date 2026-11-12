use super::{Acknowledge, Apple, Kit, KitEvent, push_token_event, updated_event};
use crate::{
    ActivityRecord, BackendContext, BackendFuture, Capabilities, EndInput, Error, Event,
    LayoutActivityRecord, LayoutEndInput, LayoutStartInput, LayoutUpdateInput, LiveActivityBackend,
    PermissionKind, Permissions, PushToStartTokenEvent, PushTokens, Result, StartInput,
    UpdateInput, pushes::TokenBook,
};
use activitykit::ActivityKit;
use std::{sync::OnceLock, thread};

/// The default iOS backend: the activitykit crate, which links its own Swift bridge.
///
/// Progress and timer templates become layout activities with a fixed tree; layouts are
/// activitykit's own. The Swift layer answers from the main thread, so do not block the main
/// thread on a call. Off iOS the backend compiles, reports no capability and answers
/// `UNSUPPORTED_PLATFORM`.
pub struct ActivityKitBackend {
    apple: Apple<ActivityKit>,
    /// The same kit, for its events.
    kit: ActivityKit,
    /// The newest push tokens, for `push_tokens`.
    tokens: TokenBook,
    /// Set by the one connection: a second would read the same events.
    attached: OnceLock<()>,
}

impl ActivityKitBackend {
    /// The backend on activitykit's default settings.
    pub fn new() -> Self {
        Self::with_kit(ActivityKit::new())
    }

    /// The backend on a kit that the caller built, for example with redacted receipts or the push-to-start watch:
    /// `ActivityKit::builder().receipts(..).push_to_start(true).build()?`.
    pub fn with_kit(kit: ActivityKit) -> Self {
        Self {
            apple: Apple::new(kit.clone()),
            kit,
            tokens: TokenBook::default(),
            attached: OnceLock::new(),
        }
    }
}

impl Default for ActivityKitBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// A backend takes one event sink: a second connection would share, and so split, the platform's events.
pub(crate) fn already_connected() -> Error {
    Error::new(
        "CONFLICT",
        "The backend is already connected to an event sink",
    )
}

/// Starts the thread that hands every event of `kit` to the sink, until the kit's events end. The kit is subscribed
/// to before this returns, so a tap that launched the app waits for the subscriber. A tap keeps its intent until the
/// sink has returned; a token is noted in `tokens` before its event goes out.
fn forward_events<K: Kit>(kit: &K, events: BackendContext, tokens: TokenBook) -> Result<()> {
    let from_kit = kit.events();
    thread::Builder::new()
        .name("liveactivity-events".into())
        .spawn(move || {
            for event in from_kit {
                match event {
                    KitEvent::Tap(tap, acknowledgement) => {
                        events.emit(Event::Action(tap));
                        acknowledgement.acknowledge();
                    }
                    KitEvent::PushToken { activity, token } => {
                        let token = push_token_event(&activity, token);
                        tokens.note_token(&token);
                        events.emit(Event::PushToken(token));
                    }
                    KitEvent::PushToStartToken { token } => {
                        tokens.note_push_to_start(&token);
                        events.emit(Event::PushToStartToken(PushToStartTokenEvent { token }));
                    }
                    KitEvent::Updated(activity) => {
                        if let Some(updated) = updated_event(&activity) {
                            events.emit(Event::Updated(updated));
                        }
                    }
                }
            }
        })
        .map(|_| ())
        .map_err(|_| Error::native())
}

impl LiveActivityBackend for ActivityKitBackend {
    /// Starts the thread that forwards events. Elsewhere than on a device with Live Activities activitykit reports
    /// no event, so no thread waits for one.
    fn attach(&self, events: BackendContext) -> Result<()> {
        if self.attached.set(()).is_err() {
            return Err(already_connected());
        }
        if Kit::authorization(&self.kit).supported {
            forward_events(&self.kit, events, self.tokens.clone())?;
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

    fn push_tokens(&self) -> BackendFuture<'_, PushTokens> {
        Box::pin(async move { Ok(self.tokens.tokens()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActionEvent, LiveActivity, block_on,
        ios::fake::{FakeAck, FakeKit, Log},
    };
    use std::{sync::mpsc, time::Duration};

    fn wait_for(log: &Log, entries: usize) {
        for _ in 0..400 {
            if log.lock().unwrap().len() >= entries {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn a_tap_keeps_its_intent_until_the_sink_returns_and_pushes_wait_for_nothing() {
        // REQ-E-001
        let kit = FakeKit::default();
        let log = Log::default();
        let (seen, events) = mpsc::channel();
        let sink_log = log.clone();
        let context = BackendContext::new(move |event: Event| {
            if matches!(event, Event::Action(_)) {
                // A slow handler: the tap may not be let go before it is done.
                thread::sleep(Duration::from_millis(40));
                sink_log.lock().unwrap().push("handled".into());
            }
            seen.send(event).unwrap();
        });
        let tokens = TokenBook::default();
        forward_events(&kit, context, tokens.clone()).unwrap();

        let tap = ActionEvent {
            action_id: "pet".into(),
            activity_id: "la-1".into(),
            activity_key: Some("demo:1".into()),
        };
        kit.send_tap(tap.clone(), FakeAck(log.clone()));
        assert_eq!(
            events.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::Action(tap)
        );
        wait_for(&log, 2);
        assert_eq!(*log.lock().unwrap(), ["handled", "released"]);

        // The tokens are in the book before their events go out.
        kit.send_event(KitEvent::PushToStartToken {
            token: "abcd".into(),
        });
        kit.send_event(KitEvent::PushToken {
            activity: kit.handle("la-1", "demo:1"),
            token: "0102".into(),
        });
        let first = events.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            first,
            Event::PushToStartToken(PushToStartTokenEvent {
                token: "abcd".into()
            })
        );
        let Event::PushToken(token) = events.recv_timeout(Duration::from_secs(5)).unwrap() else {
            panic!("a push token is its own event");
        };
        assert_eq!(token.activity_key.as_deref(), Some("demo:1"));
        let held = tokens.tokens();
        assert_eq!(held.push_to_start.as_deref(), Some("abcd"));
        assert_eq!(held.activities, [token]);
        // When the kit's events end, so does the thread.
        kit.close_events();
    }

    #[test]
    fn off_ios_the_backend_reports_nothing_and_answers_unsupported_platform() {
        let live = LiveActivity::builder()
            .backend(ActivityKitBackend::new())
            .event_sink(|_: Event| unreachable!("no events off iOS"))
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
        assert_eq!(block_on(live.push_tokens()).unwrap(), PushTokens::default());
        // A backend is connected once.
        let backend = ActivityKitBackend::new();
        backend.attach(BackendContext::new(|_: Event| {})).unwrap();
        let again = backend.attach(BackendContext::new(|_: Event| {}));
        assert_eq!(again.unwrap_err().code, "CONFLICT");

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
