use crate::{CONTRACT_VERSION, Error, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

fn validate_contract_version(version: u16) -> Result<()> {
    if version == CONTRACT_VERSION {
        Ok(())
    } else {
        Err(Error::new(
            ErrorCode::InvalidRequest,
            format!("unsupported contract version {version}"),
        ))
    }
}

fn validate_required(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(Error::new(
            ErrorCode::InvalidRequest,
            format!("{field} must not be empty"),
        ))
    } else {
        Ok(())
    }
}

/// Request to create a new Live Activity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRequest {
    /// Contract version understood by the caller.
    pub contract_version: u16,
    /// Caller-owned stable key used to recover the activity.
    pub activity_key: String,
    /// Schema version of the backend-specific content.
    pub schema_version: u32,
    /// Backend content after framework-boundary validation.
    pub content: Value,
}

impl StartRequest {
    /// Creates a request under the current contract version.
    pub fn new(activity_key: impl Into<String>, schema_version: u32, content: Value) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            activity_key: activity_key.into(),
            schema_version,
            content,
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate_contract_version(self.contract_version)?;
        validate_required(&self.activity_key, "activityKey")
    }
}

/// Request to update an existing Live Activity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRequest {
    /// Contract version understood by the caller.
    pub contract_version: u16,
    /// Opaque backend activity identifier.
    pub id: String,
    /// Monotonically increasing content revision.
    pub revision: u64,
    /// New backend content.
    pub content: Value,
}

impl UpdateRequest {
    /// Creates a request under the current contract version.
    pub fn new(id: impl Into<String>, revision: u64, content: Value) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            id: id.into(),
            revision,
            content,
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate_contract_version(self.contract_version)?;
        validate_required(&self.id, "id")
    }
}

/// Policy controlling how an ended activity is dismissed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum DismissalPolicy {
    /// Remove the activity immediately.
    Immediate,
    /// Use the platform default dismissal behavior.
    Default,
    /// Keep the final state until the Unix timestamp.
    After {
        /// Unix timestamp in seconds.
        at: u64,
    },
}

/// Request to end an existing Live Activity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndRequest {
    /// Contract version understood by the caller.
    pub contract_version: u16,
    /// Opaque backend activity identifier.
    pub id: String,
    /// Final content revision.
    pub revision: u64,
    /// Optional final backend content.
    pub content: Option<Value>,
    /// Dismissal behavior.
    pub dismissal: DismissalPolicy,
}

impl EndRequest {
    /// Creates a request under the current contract version.
    pub fn new(
        id: impl Into<String>,
        revision: u64,
        content: Option<Value>,
        dismissal: DismissalPolicy,
    ) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            id: id.into(),
            revision,
            content,
            dismissal,
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate_contract_version(self.contract_version)?;
        validate_required(&self.id, "id")
    }
}

/// Lifecycle state reported by a backend.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivityState {
    /// The activity accepts updates.
    Active,
    /// The activity is stale but may still be visible.
    Stale,
    /// The backend ended the activity.
    Ended,
    /// The user or operating system dismissed the activity.
    Dismissed,
}

/// Backend-owned representation returned after a command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRecord {
    /// Contract version of this response.
    pub contract_version: u16,
    /// Opaque backend activity identifier.
    pub id: String,
    /// Caller-owned recovery key.
    pub activity_key: String,
    /// Last accepted revision.
    pub revision: u64,
    /// Current lifecycle state.
    pub state: ActivityState,
    /// Last accepted content.
    pub content: Value,
    /// Stable backend identifier.
    pub backend: String,
}

impl ActivityRecord {
    pub(crate) fn validate(&self) -> Result<()> {
        validate_contract_version(self.contract_version)?;
        validate_required(&self.id, "id")?;
        validate_required(&self.activity_key, "activityKey")?;
        validate_required(&self.backend, "backend")
    }
}

/// Features reported by the selected backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// Contract version of this response.
    pub contract_version: u16,
    /// Stable backend identifier.
    pub backend: String,
    /// Whether the backend can create activities.
    pub start: bool,
    /// Whether the backend can update activities.
    pub update: bool,
    /// Whether the backend can end activities.
    pub end: bool,
    /// Whether the backend can enumerate activities.
    pub list: bool,
    /// Whether native actions can be delivered as events.
    pub actions: bool,
}

impl Capabilities {
    pub(crate) fn validate(&self) -> Result<()> {
        validate_contract_version(self.contract_version)?;
        validate_required(&self.backend, "backend")
    }
}

/// Event envelope delivered from a backend to a framework plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveActivityEvent {
    /// Contract version of this event.
    pub contract_version: u16,
    /// Event-specific payload.
    #[serde(flatten)]
    pub payload: LiveActivityEventPayload,
}

impl LiveActivityEvent {
    /// Creates an action event under the current contract version.
    pub fn action(activity_id: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            payload: LiveActivityEventPayload::Action {
                activity_id: activity_id.into(),
                action: action.into(),
            },
        }
    }

    /// Creates a state-change event under the current contract version.
    pub fn state_changed(activity: ActivityRecord) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            payload: LiveActivityEventPayload::StateChanged { activity },
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate_contract_version(self.contract_version)?;
        match &self.payload {
            LiveActivityEventPayload::Action {
                activity_id,
                action,
            } => {
                validate_required(activity_id, "activityId")?;
                validate_required(action, "action")
            }
            LiveActivityEventPayload::StateChanged { activity } => activity.validate(),
        }
    }
}

/// Payload variants emitted by Live Activity backends.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
pub enum LiveActivityEventPayload {
    /// The user selected an action exposed by an activity.
    Action {
        /// Opaque backend activity identifier.
        activity_id: String,
        /// Stable application-defined action identifier.
        action: String,
    },
    /// The backend observed a lifecycle or content change.
    StateChanged {
        /// Latest backend record.
        activity: ActivityRecord,
    },
}
