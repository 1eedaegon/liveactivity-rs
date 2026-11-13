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
}

/// Where a backend's events go: implemented by a framework plugin, or any closure.
///
/// On iOS the backend calls the sink from a thread of its own. A tap keeps its intent, and an app the system
/// launched for it, until `emit` has returned, for at most 4 seconds; the other events wait for nothing. On Android
/// the sink is called by a thread that asked: the one that runs `sync_events`, or a call of the application
/// that made a state due, after the call's own work. It is called for one event at a time, in the order the
/// events were taken, and with no lock held, so a sink may call the backend. When two threads ask at once, one
/// of them hands over what both took, and the other's call returns before its events arrived.
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
