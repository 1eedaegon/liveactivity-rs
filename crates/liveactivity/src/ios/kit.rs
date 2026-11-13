//! activitykit's `ActivityKit` and `Activity` as the [`Kit`] and [`Handle`] that the backend's logic runs on. Only
//! a device with Live Activities runs these calls; the logic itself is tested on a fake kit.

use super::{
    Acknowledge, Authorization, EndSpec, Handle, Kit, KitEvent, StartSpec, UpdateSpec, action_event,
};
use crate::{
    layout::{Alert, LayoutPush},
    models::EndPolicy,
};
use activitykit::{
    Acknowledgement, Activity, ActivityKit, ChannelId, End, Event, Push, Start, State, Update,
    Values,
};
use std::{
    future::Future,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// 9999-12-31T23:59:59.999Z in epoch milliseconds: a cap every platform's `SystemTime` holds.
pub(crate) const LATEST_MILLIS: u64 = 253_402_300_799_999;

/// The time of `millis` epoch milliseconds, clamped to [`LATEST_MILLIS`].
pub(crate) fn time(millis: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(millis.min(LATEST_MILLIS))
}

/// `at` in epoch milliseconds, when it is after the epoch and fits.
pub(crate) fn millis(at: SystemTime) -> Option<u64> {
    u64::try_from(at.duration_since(UNIX_EPOCH).ok()?.as_millis()).ok()
}

/// activitykit's start, or `INVALID_INPUT` for a channel that is no id, which the check of the request keeps out.
pub(crate) fn start_of(spec: StartSpec) -> activitykit::Result<Start> {
    let start = Start::new(spec.layout, spec.values)
        .key(spec.key)
        .revision(spec.revision);
    let start = match spec.stale_at {
        Some(at) => start.stale_at(time(at)),
        None => start,
    };
    let start = match spec.timestamp {
        Some(at) => start.timestamp(time(at)),
        None => start,
    };
    let start = match spec.relevance {
        Some(score) => start.relevance(score),
        None => start,
    };
    let start = if spec.transient {
        start.transient()
    } else {
        start
    };
    let start = match spec.scheduled {
        Some(scheduled) => start.scheduled(time(scheduled.at), alert_of(scheduled.alert)),
        None => start,
    };
    Ok(match spec.push {
        Some(LayoutPush::Token) => start.push(Push::Token),
        Some(LayoutPush::Channel(id)) => start.push(Push::Channel(ChannelId::new(id)?)),
        None => start,
    })
}

pub(crate) fn update_of(spec: UpdateSpec) -> Update {
    let update = Update::new(spec.values).revision(spec.revision);
    let update = match spec.stale_at {
        Some(at) => update.stale_at(time(at)),
        None => update,
    };
    let update = match spec.timestamp {
        Some(at) => update.timestamp(time(at)),
        None => update,
    };
    let update = match spec.relevance {
        Some(score) => update.relevance(score),
        None => update,
    };
    match spec.alert {
        Some(alert) => update.alert(alert_of(alert)),
        None => update,
    }
}

/// activitykit's alert, with its sound when it names one.
fn alert_of(alert: Alert) -> activitykit::Alert {
    let shown = activitykit::Alert::new(alert.title, alert.body);
    match alert.sound {
        Some(name) => shown.sound(name),
        None => shown,
    }
}

pub(crate) fn end_of(spec: EndSpec) -> End {
    let end = match spec.policy {
        EndPolicy::Immediate {} => End::immediately(),
        EndPolicy::Default {} => End::default(),
        EndPolicy::After { at } => End::after(time(at)),
    };
    let end = match spec.values {
        Some(values) => end.values(values),
        None => end,
    };
    let end = match spec.revision {
        Some(revision) => end.revision(revision),
        None => end,
    };
    let end = match spec.stale_at {
        Some(at) => end.stale_at(time(at)),
        None => end,
    };
    let end = match spec.timestamp {
        Some(at) => end.timestamp(time(at)),
        None => end,
    };
    match spec.relevance {
        Some(score) => end.relevance(score),
        None => end,
    }
}

impl Acknowledge for Acknowledgement {
    fn acknowledge(&self) {
        Acknowledgement::acknowledge(self);
    }
}

fn authorization_of(authorization: activitykit::Authorization) -> Authorization {
    Authorization {
        supported: authorization.supported,
        enabled: authorization.enabled,
        buttons: authorization.buttons,
        frequent_pushes: authorization.frequent_pushes,
    }
}

/// What the backend takes of one of activitykit's events; the others pass.
fn kit_event(event: Event) -> Option<KitEvent<Activity, Acknowledgement>> {
    match event {
        Event::Action {
            activity,
            action,
            on,
            acknowledgement,
            ..
        } => Some(KitEvent::Tap(
            action_event(&activity, action, on),
            acknowledgement,
        )),
        Event::PushToken {
            activity, token, ..
        } => Some(KitEvent::PushToken {
            activity,
            token: token.to_string(),
        }),
        Event::PushToStartToken { token, .. } => Some(KitEvent::PushToStartToken {
            token: token.to_string(),
        }),
        Event::Updated { activity, .. } => Some(KitEvent::Updated(activity)),
        Event::Started { activity, .. } => Some(KitEvent::Started(activity)),
        Event::State {
            activity, state, ..
        } => Some(KitEvent::State(activity, state)),
        Event::Authorization(authorization) => {
            Some(KitEvent::Authorization(authorization_of(authorization)))
        }
        _ => None,
    }
}

impl Kit for ActivityKit {
    type Handle = Activity;
    type Ack = Acknowledgement;

    fn events(&self) -> Box<dyn Iterator<Item = KitEvent<Activity, Acknowledgement>> + Send> {
        let events = self.events();
        Box::new(std::iter::from_fn(move || {
            loop {
                if let Some(found) = kit_event(events.recv_blocking().ok()?) {
                    return Some(found);
                }
            }
        }))
    }

    fn authorization(&self) -> Authorization {
        authorization_of(ActivityKit::authorization(self))
    }

    async fn start(&self, spec: StartSpec) -> activitykit::Result<Activity> {
        self.start_with(start_of(spec)?).await
    }

    fn activity(
        &self,
        id: &str,
    ) -> impl Future<Output = activitykit::Result<Option<Activity>>> + Send {
        ActivityKit::activity(self, id)
    }

    fn activities(&self) -> impl Future<Output = activitykit::Result<Vec<Activity>>> + Send {
        ActivityKit::activities(self)
    }

    fn write_image(&self, name: &str, png: &[u8]) -> activitykit::Result<()> {
        ActivityKit::write_image(self, name, png)
    }
}

impl Handle for Activity {
    fn id(&self) -> &str {
        Activity::id(self)
    }

    fn key(&self) -> &str {
        Activity::key(self)
    }

    fn state(&self) -> State {
        Activity::state(self)
    }

    fn values(&self) -> Values {
        Activity::values(self)
    }

    fn revision(&self) -> Option<u64> {
        Activity::revision(self)
    }

    fn stale_at(&self) -> Option<u64> {
        Activity::stale_at(self).and_then(millis)
    }

    fn timestamp(&self) -> Option<u64> {
        Activity::timestamp(self).and_then(millis)
    }

    fn relevance(&self) -> f64 {
        Activity::relevance(self)
    }

    fn degraded_features(&self) -> Vec<String> {
        Activity::degraded_features(self)
    }

    fn update(&self, spec: UpdateSpec) -> impl Future<Output = activitykit::Result<()>> + Send {
        self.update_with(update_of(spec))
    }

    fn end(&self, spec: EndSpec) -> impl Future<Output = activitykit::Result<()>> + Send {
        Activity::end(self, end_of(spec))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn debug(value: impl std::fmt::Debug) -> String {
        format!("{value:?}")
    }

    #[test]
    fn specs_reach_activitykit_with_their_revision_times_and_push() {
        let layout = activitykit::Layout::new(activitykit::layout::text("Recording"));
        let values = || Values::from([("level", 0.2)]);
        let spec = |push| StartSpec {
            key: "rec".into(),
            layout: layout.clone(),
            values: values(),
            revision: 5,
            stale_at: Some(1_000),
            timestamp: Some(2_000),
            relevance: Some(80.0),
            push,
            transient: false,
            scheduled: None,
        };
        let expected = Start::new(layout.clone(), values())
            .key("rec")
            .revision(5)
            .stale_at(time(1_000))
            .timestamp(time(2_000))
            .relevance(80.0);
        assert_eq!(
            debug(start_of(spec(None)).unwrap()),
            debug(expected.clone())
        );
        assert_eq!(
            debug(start_of(spec(Some(LayoutPush::Token))).unwrap()),
            debug(expected.push(Push::Token))
        );
        // A channel that is no id never reaches ActivityKit.
        let refused = start_of(spec(Some(LayoutPush::Channel("no id".into())))).unwrap_err();
        assert_eq!(refused.code(), "INVALID_INPUT");
        // REQ-I-006, REQ-I-007: a transient style, and a later time with the alert shown then and its sound.
        let alert = crate::Alert::new("Recording", "It starts now").with_sound("chime.aiff");
        let later = StartSpec {
            transient: true,
            scheduled: Some(crate::ScheduledStart::new(9_000, alert)),
            ..spec(None)
        };
        let shown = activitykit::Alert::new("Recording", "It starts now").sound("chime.aiff");
        let expected = Start::new(layout.clone(), values())
            .key("rec")
            .revision(5)
            .stale_at(time(1_000))
            .timestamp(time(2_000))
            .relevance(80.0)
            .transient()
            .scheduled(time(9_000), shown);
        assert_eq!(debug(start_of(later).unwrap()), debug(expected));

        let update = UpdateSpec {
            values: values(),
            revision: 6,
            stale_at: None,
            timestamp: Some(u64::MAX),
            relevance: Some(20.0),
            alert: Some(crate::Alert::new("Stopped", "Tap to resume")),
        };
        // REQ-I-001, REQ-I-002
        let expected = Update::new(values())
            .revision(6)
            .timestamp(time(LATEST_MILLIS))
            .relevance(20.0)
            .alert(activitykit::Alert::new("Stopped", "Tap to resume"));
        assert_eq!(debug(update_of(update)), debug(expected));
        let silent = UpdateSpec {
            values: values(),
            revision: 7,
            stale_at: None,
            timestamp: None,
            relevance: None,
            alert: None,
        };
        assert_eq!(
            debug(update_of(silent)),
            debug(Update::new(values()).revision(7))
        );
        // REQ-I-007
        let loud = UpdateSpec {
            values: values(),
            revision: 8,
            stale_at: None,
            timestamp: None,
            relevance: None,
            alert: Some(crate::Alert::new("Stopped", "Tap to resume").with_sound("stop.caf")),
        };
        let shown = activitykit::Alert::new("Stopped", "Tap to resume").sound("stop.caf");
        assert_eq!(
            debug(update_of(loud)),
            debug(Update::new(values()).revision(8).alert(shown))
        );

        let end = |policy, values: Option<Values>| EndSpec {
            policy,
            revision: values.as_ref().map(|_| 7),
            values,
            stale_at: None,
            timestamp: None,
            relevance: None,
        };
        assert_eq!(
            end_of(end(EndPolicy::Immediate {}, None)),
            End::immediately()
        );
        assert_eq!(end_of(end(EndPolicy::Default {}, None)), End::default());
        assert_eq!(
            end_of(end(EndPolicy::After { at: 9_000 }, Some(values()))),
            End::after(time(9_000)).values(values()).revision(7)
        );
        let relevant = EndSpec {
            relevance: Some(5.0),
            ..end(EndPolicy::Default {}, None)
        };
        assert_eq!(end_of(relevant), End::default().relevance(5.0));
        assert_eq!(millis(time(1_800_000_000_000)), Some(1_800_000_000_000));
        assert_eq!(millis(UNIX_EPOCH - Duration::from_secs(1)), None);
    }
}
