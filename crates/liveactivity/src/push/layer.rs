//! The member `liveupdate.layer` of a data message: this crate's own message of an activity's key, which liveupdate's
//! push entry hands to this crate's on the device.

use super::{Event, Message};
use crate::{Alert, EndPolicy, Error, LayoutSnapshot, LayoutStartInput, Result};
use serde::{Deserialize, Serialize};

/// The member of a data map that liveupdate hands to the layer above it.
pub(crate) const LAYER_KEY: &str = "liveupdate.layer";

const VERSION: u64 = 1;

/// The member as JSON: the command, the key, and what the command carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Wire {
    pub(crate) v: u64,
    pub(crate) command: String,
    pub(crate) activity_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) start: Option<LayoutStartInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) snapshot: Option<LayoutSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) alert: Option<Alert>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) policy: Option<EndPolicy>,
}

/// What a member asks of the device, read back.
#[cfg(feature = "app")]
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Layer {
    Start(Box<LayoutStartInput>),
    Update {
        snapshot: LayoutSnapshot,
        alert: Option<Alert>,
    },
    End {
        snapshot: Option<LayoutSnapshot>,
        policy: EndPolicy,
    },
}

impl Wire {
    /// The member of `message`, as the server sends it.
    pub(crate) fn of(message: &Message) -> Self {
        let wire = |command: &str| Self {
            v: VERSION,
            command: command.into(),
            activity_key: message.key.clone(),
            start: None,
            snapshot: None,
            alert: message.alert.clone(),
            policy: None,
        };
        match &message.event {
            Event::Start { input, .. } => Self {
                start: Some((**input).clone()),
                ..wire("start")
            },
            Event::Update(snapshot) => Self {
                snapshot: Some((**snapshot).clone()),
                ..wire("update")
            },
            Event::End => Self {
                snapshot: message.final_content.clone(),
                policy: message.policy.clone(),
                ..wire("end")
            },
        }
    }

    /// The JSON text of the member.
    pub(crate) fn text(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|_| Error::invalid())
    }
}

/// The device's side: the member read back.
#[cfg(feature = "app")]
impl Wire {
    /// The member's text read as this version's, or nothing: a later version or another shape is no message of
    /// this crate's.
    pub(crate) fn read(member: &str) -> Option<Self> {
        let wire: Self = serde_json::from_str(member).ok()?;
        (wire.v == VERSION && matches!(wire.command.as_str(), "start" | "update" | "end"))
            .then_some(wire)
    }

    /// The snapshot's revision, as liveupdate's outcome names it; none for an end without content.
    pub(crate) fn revision(&self) -> Option<u64> {
        match self.command.as_str() {
            "start" => self.start.as_ref().map(|start| start.snapshot.revision),
            "update" => self.snapshot.as_ref().map(|snapshot| snapshot.revision),
            _ => None,
        }
    }

    /// What the member asks for, or `INVALID_INPUT` when its members do not belong to its command.
    pub(crate) fn layer(self) -> Result<Layer> {
        let refused = || Error::new("INVALID_INPUT", "The members do not belong to the command.");
        match (
            self.command.as_str(),
            self.start,
            self.snapshot,
            self.policy,
        ) {
            ("start", Some(start), None, None) if start.activity_key == self.activity_key => {
                Ok(Layer::Start(Box::new(start)))
            }
            ("update", None, Some(snapshot), None) => Ok(Layer::Update {
                snapshot,
                alert: self.alert,
            }),
            ("end", None, snapshot, policy) if self.alert.is_none() => Ok(Layer::End {
                snapshot,
                policy: policy.unwrap_or(EndPolicy::Default {}),
            }),
            _ => Err(refused()),
        }
    }
}
