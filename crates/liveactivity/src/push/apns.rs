//! A message as APNs takes it: the body is activitykit's [`Payload`], the path and the headers are APNs's.

use super::{Application, Event, Message, Platform, Priority, Request, Target};
use crate::{
    EndPolicy, Error, LayoutEndInput, LayoutPush, LayoutSnapshot, LayoutUpdateInput, Result,
    Validate, layout::kit_layout,
};
use activitykit::{
    Values,
    push::{ChannelId, PRIORITY_LOW, PRIORITY_NOW, PUSH_TYPE, Payload, topic},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The longest token APNs gives a device, in bytes.
const MAX_TOKEN_BYTES: usize = 100;
/// The longest bundle id that Apple takes.
const MAX_BUNDLE_ID: usize = 155;

pub(super) fn request(
    message: &Message,
    target: &Target,
    application: &Application,
) -> Result<Request> {
    let bundle = application
        .bundle_id
        .as_deref()
        .filter(|bundle| valid_bundle_id(bundle))
        .ok_or_else(|| {
            Error::new(
                "INVALID_INPUT",
                "An APNs request needs the application's bundle id, for its topic.",
            )
        })?;
    let mut headers = vec![("apns-push-type", PUSH_TYPE.to_owned())];
    let path = match (target, &message.event) {
        (Target::ApnsPushToStart { token }, Event::Start { .. })
        | (Target::ApnsActivity { token }, Event::Update(_) | Event::End) => {
            headers.push(("apns-topic", topic(bundle)));
            format!("/3/device/{}", hex_token(token)?)
        }
        (Target::ApnsChannel { channel }, Event::Update(_) | Event::End) => {
            let channel = ChannelId::new(channel.clone())?;
            headers.push(("apns-channel-id", channel.as_str().to_owned()));
            format!("/4/broadcasts/apps/{bundle}")
        }
        _ => {
            return Err(Error::new(
                "INVALID_INPUT",
                "A start goes to the push-to-start token, an update or an end to an activity's token or a channel.",
            ));
        }
    };
    let priority = match message.priority {
        Priority::Low => PRIORITY_LOW,
        Priority::Now => PRIORITY_NOW,
    };
    headers.push(("apns-priority", priority.to_string()));
    let body = payload(message)?.to_json()?;
    Ok(Request {
        platform: Platform::Apns,
        path,
        headers,
        body,
    })
}

/// activitykit's payload of the message, after this crate's checks of the call it stands for.
fn payload(message: &Message) -> Result<Payload> {
    let sent_at = message.sent_at.map(time);
    let payload = match &message.event {
        Event::Start { id, input } => {
            input.validate()?;
            if input.scheduled.is_some() || input.ios.as_ref().is_some_and(|ios| ios.transient) {
                return Err(Error::new(
                    "INVALID_INPUT",
                    "A push starts an activity now and not as a transient one: the application schedules its own.",
                ));
            }
            let snapshot = &input.snapshot;
            let start = Payload::start(
                id.clone(),
                input.activity_key.clone(),
                kit_layout(input.layout.clone(), input.styles.clone()),
                Values::from(snapshot.values.clone()),
            )
            .revision(snapshot.revision);
            let start = match &input.push {
                None => start,
                Some(LayoutPush::Token) => start.input_push_token(),
                Some(_) => {
                    return Err(Error::unsupported_feature(
                        "A push-to-start asks for the activity's token, not for a channel.",
                    ));
                }
            };
            content(start, snapshot)
        }
        Event::Update(snapshot) => {
            let update = LayoutUpdateInput {
                alert: message.alert.clone(),
                ..LayoutUpdateInput::new(message.key.clone(), (**snapshot).clone())
            };
            update.validate()?;
            content(
                Payload::update(Values::from(snapshot.values.clone())).revision(snapshot.revision),
                snapshot,
            )
        }
        Event::End => {
            let policy = message.policy.clone().unwrap_or(EndPolicy::Default {});
            let end = LayoutEndInput {
                id: message.key.clone(),
                policy: policy.clone(),
                snapshot: message.final_content.clone(),
            };
            end.validate()?;
            let ended = match &message.final_content {
                Some(snapshot) => content(
                    Payload::end_with(Values::from(snapshot.values.clone()))
                        .revision(snapshot.revision),
                    snapshot,
                ),
                None => Payload::end(),
            };
            match policy {
                EndPolicy::Immediate {} => {
                    // At once: the time of sending, or now when that is later.
                    let now = SystemTime::now();
                    ended.dismiss_at(sent_at.map_or(now, |at| at.min(now)))
                }
                EndPolicy::After { at } => ended.dismiss_at(time(at)),
                EndPolicy::Default {} => ended,
            }
        }
    };
    let payload = match &message.alert {
        Some(alert) => {
            alert.validate()?;
            payload.alert(alert.kit())
        }
        None => payload,
    };
    Ok(match sent_at {
        Some(at) => payload.sent_at(at),
        None => payload,
    })
}

/// The members of a snapshot beside its values and revision, as the payload carries them.
fn content(payload: Payload, snapshot: &LayoutSnapshot) -> Payload {
    let payload = match snapshot.stale_at {
        Some(at) => payload.stale_at(time(at)),
        None => payload,
    };
    let payload = match snapshot.timestamp {
        Some(at) => payload.timestamp(time(at)),
        None => payload,
    };
    match snapshot.relevance {
        Some(score) => payload.relevance(score),
        None => payload,
    }
}

/// A token as APNs takes it in the path: hex of 1 to 100 bytes, lowercase.
fn hex_token(token: &str) -> Result<String> {
    let bytes = token.len() / 2;
    let hex = !token.is_empty()
        && token.len().is_multiple_of(2)
        && bytes <= MAX_TOKEN_BYTES
        && token.bytes().all(|byte| byte.is_ascii_hexdigit());
    if hex {
        Ok(token.to_ascii_lowercase())
    } else {
        Err(Error::new(
            "INVALID_INPUT",
            "An APNs token is hex of 1 to 100 bytes.",
        ))
    }
}

/// A bundle id: letters, digits, `.` and `-`.
pub(super) fn valid_bundle_id(bundle: &str) -> bool {
    !bundle.is_empty()
        && bundle.len() <= MAX_BUNDLE_ID
        && bundle
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
}

fn time(millis: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(millis)
}
