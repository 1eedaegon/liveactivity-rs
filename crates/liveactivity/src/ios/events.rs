//! activitykit's events, handed to the framework sink. Only a device with Live Activities
//! reports any, so the simulator runs this file and the host tests do not.

use super::content::{self, Seen};
use crate::{BackendContext, Error, ErrorCode, LiveActivityEvent, Result};
use activitykit::{ActivityKit, Event};
use std::thread;

/// Starts the thread that reads the kit's events until every handle of the kit is gone. It
/// subscribes before it returns, so a tap that launched the app waits for this subscriber.
pub(crate) fn forward(kit: &ActivityKit, context: BackendContext) -> Result<()> {
    let events = kit.events();
    thread::Builder::new()
        .name("liveactivity-activitykit-events".into())
        .spawn(move || {
            while let Ok(event) = events.recv_blocking() {
                // `event` drops after the sink returned: a tap keeps its intent until then.
                if let Some(neutral) = neutral(&event) {
                    // A callback thread has no caller to answer; the sink owns its failures.
                    let _ = context.emit(neutral);
                }
            }
        })
        .map(|_| ())
        .map_err(|_| {
            Error::new(
                ErrorCode::BackendFailure,
                "the activitykit event thread did not start",
            )
        })
}

/// The neutral event, or `None` for a kind the neutral contract does not name, such as an
/// authorization change.
fn neutral(event: &Event) -> Option<LiveActivityEvent> {
    match event {
        Event::Action {
            activity, action, ..
        } => Some(match activity.key() {
            // ActivityKit no longer lists the activity, so only its handle is known.
            "" => LiveActivityEvent::action(activity.id(), action.clone()),
            key => LiveActivityEvent::keyed_action(activity.id(), key, action.clone()),
        }),
        Event::Started { activity, .. } => content::record(content::seen(activity))
            .ok()
            .map(LiveActivityEvent::state_changed),
        Event::State {
            activity, state, ..
        } => content::record(Seen {
            state: *state,
            ..content::seen(activity)
        })
        .ok()
        .map(LiveActivityEvent::state_changed),
        _ => None,
    }
}
