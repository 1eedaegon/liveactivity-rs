use crate::{
    ActionEvent, ActivityState, LayoutActivityRecord, Permissions, PushToStartTokenEvent,
    PushTokenEvent, UpdatedEvent,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A layout activity that began while the application ran: one it started, or one a push started.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartedEvent {
    /// The activity as it began.
    pub record: LayoutActivityRecord,
}

/// A new lifecycle state of an activity, of a template or a layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateEvent {
    /// The handle of the activity.
    pub activity_id: String,
    /// Absent when the backend no longer knows that activity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_key: Option<String>,
    /// The state the platform reports now.
    pub state: ActivityState,
}

/// The person opened the application from the notification of an activity (Android), as the host reported it to
/// liveupdate with the opening intent. The application receives the URL itself, from that intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedEvent {
    /// The handle of the activity.
    pub activity_id: String,
    /// Absent when the backend no longer knows that activity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_key: Option<String>,
    /// The action that opened the application, when the person tapped one and not the notification itself: an
    /// action of an Android content, or `link:<n>` for the nth link of a layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
}

/// A tap that the application's own component took (Android): the action's target ran it, and the component
/// recorded it with liveupdate's `LiveUpdateActions.take`. It is no [`Event::Action`]: the component acted on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandledEvent {
    /// The handle of the activity.
    pub activity_id: String,
    /// Absent when the backend no longer knows that activity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_key: Option<String>,
    /// The id of the action whose target took the tap.
    pub action_id: String,
    /// What is known of the tap: the adapter's sequence and when it was stored.
    #[serde(default, skip_serializing_if = "crate::layout::Tap::is_unknown")]
    pub tap: crate::layout::Tap,
}

/// An event of a kind that this version does not know (Android): liveupdate's library of a later version stored
/// it, and liveupdate read what it knows of it, which is the activity. The application's crate is older than its
/// AAR, and what happened is in that version's documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnknownEvent {
    /// The handle of the activity.
    pub activity_id: String,
    /// Absent when the backend no longer knows that activity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_key: Option<String>,
}

/// What a push from the application's server did (Android, liveupdate 0.10): liveupdate's push path ran it while
/// the application may not have run, and stored this. A push is taken or refused as the call it stands for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushedEvent {
    /// The handle of the activity the push acted on; absent when it named no live activity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_id: Option<String>,
    /// The key the push named.
    pub activity_key: String,
    /// What the push asked for.
    pub command: PushCommand,
    /// The revision of the push's snapshot; absent for an end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    /// Why the push was refused, one of [`ERROR_CODES`](crate::ERROR_CODES); absent when it did what it asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// What a push asks of an activity.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PushCommand {
    /// A start.
    Start,
    /// An update.
    Update,
    /// An end.
    End,
}

/// What a platform reports of the application's activities. More kinds may come: match with a `_` arm.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A tap on a button of an activity.
    Action(ActionEvent),
    /// A push token of an activity, new or changed (iOS).
    PushToken(PushTokenEvent),
    /// The app's push-to-start token, new or changed (iOS).
    PushToStartToken(PushToStartTokenEvent),
    /// Content that a push delivered to a layout activity (iOS).
    Updated(UpdatedEvent),
    /// A layout activity began: one the application started, or one a push started (iOS).
    Started(StartedEvent),
    /// A new lifecycle state of an activity. iOS reports every change while the application runs. Android reports
    /// the state an activity has when the backend is asked, by `sync_events` or by a call of the application, if
    /// it is another than the one it reported last: `Active` once Android shows the activity, `Stale` after its
    /// stale time, `Ended`, and `Dismissed` when the person removed the notification. An activity is over at
    /// `Ended` and at `Dismissed`.
    State(StateEvent),
    /// What the person allows, after a change in Settings (iOS).
    Permissions(Permissions),
    /// The person opened the application from a notification (Android).
    Opened(OpenedEvent),
    /// Something that this version has no event for (Android).
    Unknown(UnknownEvent),
    /// A tap that the application's own component took (Android).
    Handled(HandledEvent),
    /// What a push from the application's server did, taken or refused (Android).
    Pushed(PushedEvent),
}

/// Where a backend's events go: implemented by a framework plugin, or any closure.
///
/// On iOS the backend calls the sink from a thread of its own. A tap keeps its intent, and an app the system
/// launched for it, until `emit` has returned, for at most 4 seconds; the other events wait for nothing. On Android
/// the sink is called by a thread that asked: the one that runs `sync_events`, or a call of the application
/// that made a state due, after the call's own work. It is called for one event at a time, in the order the
/// events were taken, and with no lock held, so a sink may call the backend. When two threads ask at once, one
/// of them hands over what both took, and the other's call returns before its events arrived. A sink that panics
/// there loses the event it was handed, and the call that was handing over panics with it; what was queued behind
/// reaches the sink with the next call.
pub trait EventSink: Send + Sync + 'static {
    /// Takes one event.
    fn emit(&self, event: Event);
}

impl<F> EventSink for F
where
    F: Fn(Event) + Send + Sync + 'static,
{
    fn emit(&self, event: Event) {
        self(event);
    }
}

/// The event sink as a backend holds it, handed over in `attach`.
#[derive(Clone)]
pub struct BackendContext {
    sink: Arc<dyn EventSink>,
}

impl BackendContext {
    /// Wraps a sink for a backend.
    pub fn new<S: EventSink>(sink: S) -> Self {
        Self {
            sink: Arc::new(sink),
        }
    }

    /// Hands an event to the sink.
    pub fn emit(&self, event: Event) {
        self.sink.emit(event);
    }
}
