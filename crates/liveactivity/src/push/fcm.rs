//! A message as FCM HTTP v1 takes it: the data member that liveupdate's push entry hands to this crate's layer on the
//! device.

use super::{
    Application, Event, Message, Platform, Priority, Request,
    layer::{LAYER_KEY, Wire},
};
use crate::{EndPolicy, Error, LayoutEndInput, LayoutUpdateInput, Result, Validate};
use serde_json::json;

/// The bytes of UTF-8 that FCM carries in a message's data, its keys and values together.
const MAX_DATA_BYTES: usize = 4096;
/// The longest registration token taken.
const MAX_TOKEN: usize = 4096;

pub(super) fn request(
    message: &Message,
    token: &str,
    application: &Application,
) -> Result<Request> {
    let project = application
        .fcm_project
        .as_deref()
        .filter(|project| valid_project(project))
        .ok_or_else(|| {
            Error::new(
                "INVALID_INPUT",
                "An FCM request needs the Firebase project's id: lowercase letters, digits and `-`.",
            )
        })?;
    if token.is_empty()
        || token.len() > MAX_TOKEN
        || token
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(Error::new(
            "INVALID_INPUT",
            "An FCM token is 1 to 4096 characters without a space.",
        ));
    }
    check(message)?;
    let member = Wire::of(message).text()?;
    if LAYER_KEY.len() + member.len() > MAX_DATA_BYTES {
        return Err(Error::new(
            "PAYLOAD_TOO_LARGE",
            "A data message carries 4096 bytes, its key and value together.",
        ));
    }
    let priority = match message.priority {
        Priority::Now => "high",
        Priority::Low => "normal",
    };
    let body = json!({
        "message": {
            "token": token,
            "android": { "priority": priority },
            "data": { LAYER_KEY: member },
        }
    });
    Ok(Request {
        platform: Platform::Fcm,
        path: format!("/v1/projects/{project}/messages:send"),
        headers: vec![("content-type", "application/json".to_owned())],
        body: body.to_string(),
    })
}

/// The message checked as the Android backend checks the call it stands for.
fn check(message: &Message) -> Result<()> {
    match &message.event {
        Event::Start { input, .. } => {
            input.validate()?;
            if input.scheduled.is_some() || input.ios.as_ref().is_some_and(|ios| ios.transient) {
                return Err(Error::new(
                    "INVALID_INPUT",
                    "A push starts an activity now and not as a transient one: the application schedules its own.",
                ));
            }
            Ok(())
        }
        Event::Update(snapshot) => LayoutUpdateInput {
            alert: message.alert.clone(),
            ..LayoutUpdateInput::new(message.key.clone(), (**snapshot).clone())
        }
        .validate(),
        Event::End => {
            if message.alert.is_some() {
                return Err(Error::new(
                    "INVALID_INPUT",
                    "An end on Android comes without an alert.",
                ));
            }
            LayoutEndInput {
                id: message.key.clone(),
                policy: message.policy.clone().unwrap_or(EndPolicy::Default {}),
                snapshot: message.final_content.clone(),
            }
            .validate()
        }
    }
}

/// A Firebase project id: 6 to 30 of lowercase letters, digits and `-`.
fn valid_project(project: &str) -> bool {
    (6..=30).contains(&project.len())
        && project
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}
