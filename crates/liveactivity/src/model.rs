use crate::{CONTRACT_VERSION, Content, Error, ErrorCode, Fallback, Result, Template};
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

/// The largest integer that crosses JavaScript and both default engines exactly.
pub(crate) const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
/// The longest activity key both default engines take.
const MAX_KEY_CHARACTERS: usize = 128;

fn validate_revision(revision: u64) -> Result<()> {
    if revision <= MAX_SAFE_INTEGER {
        Ok(())
    } else {
        Err(Error::new(
            ErrorCode::InvalidRequest,
            "revision is out of range",
        ))
    }
}

/// A key every backend takes: ASCII letters and digits, then also `.`, `_`, `:` and `-`.
fn validate_key(key: &str) -> Result<()> {
    let starts = key
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric());
    let fits = key.chars().count() <= MAX_KEY_CHARACTERS
        && key.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '-')
        });
    if starts && fits {
        Ok(())
    } else {
        Err(Error::new(
            ErrorCode::InvalidRequest,
            "activityKey must be 1 to 128 letters, digits, '.', '_', ':' or '-', starting with a letter or digit",
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
    /// Schema version of the content: 1 for a template, the backend's own for native content.
    pub schema_version: u32,
    /// First revision of portable content. Native content carries its own.
    #[serde(default)]
    pub revision: u64,
    /// What to do when the platform's native presentation is unavailable, for portable
    /// content. Native content carries its own.
    #[serde(default)]
    pub fallback: Fallback,
    /// What the activity shows.
    pub content: Content,
}

impl StartRequest {
    /// The schema version of a template start.
    pub const TEMPLATE_SCHEMA_VERSION: u32 = 1;

    /// Creates a request under the current contract version. A JSON value is native content.
    pub fn new(
        activity_key: impl Into<String>,
        schema_version: u32,
        content: impl Into<Content>,
    ) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            activity_key: activity_key.into(),
            schema_version,
            revision: 0,
            fallback: Fallback::default(),
            content: content.into(),
        }
    }

    /// Creates a portable start, which runs on every backend with `templates`.
    pub fn template(activity_key: impl Into<String>, template: Template) -> Self {
        Self::new(activity_key, Self::TEMPLATE_SCHEMA_VERSION, template)
    }

    /// The same request with `revision` as the first revision of portable content.
    #[must_use]
    pub fn revision(self, revision: u64) -> Self {
        Self { revision, ..self }
    }

    /// The same request with `fallback` for portable content.
    #[must_use]
    pub fn fallback(self, fallback: Fallback) -> Self {
        Self { fallback, ..self }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate_contract_version(self.contract_version)?;
        validate_required(&self.activity_key, "activityKey")?;
        validate_key(&self.activity_key)?;
        validate_revision(self.revision)?;
        let portable = self.content.template().is_some();
        if portable && self.schema_version != Self::TEMPLATE_SCHEMA_VERSION {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "schemaVersion must be 1 for a template",
            ));
        }
        // Native content carries its own: a value here would be dropped without a word.
        if !portable && (self.revision != 0 || self.fallback != Fallback::default()) {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "revision and fallback belong to portable content; native content carries its own",
            ));
        }
        self.content.validate()
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
    /// New content, of the kind the activity started with.
    pub content: Content,
}

impl UpdateRequest {
    /// Creates a request under the current contract version.
    pub fn new(id: impl Into<String>, revision: u64, content: impl Into<Content>) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            id: id.into(),
            revision,
            content: content.into(),
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate_contract_version(self.contract_version)?;
        validate_required(&self.id, "id")?;
        validate_revision(self.revision)?;
        self.content.validate()
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
        /// Unix timestamp in milliseconds, the unit of both default engines.
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
    /// Optional final content.
    pub content: Option<Content>,
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
            content: content.map(Content::from),
            dismissal,
        }
    }

    /// The same request with `content` as the final content.
    #[must_use]
    pub fn with_content(self, content: impl Into<Content>) -> Self {
        Self {
            content: Some(content.into()),
            ..self
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate_contract_version(self.contract_version)?;
        validate_required(&self.id, "id")?;
        self.content.as_ref().map_or(Ok(()), Content::validate)
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
    /// The activity is scheduled and not started yet.
    Pending,
    /// The platform has not confirmed a state, or reports one this contract does not name.
    Unknown,
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
    /// The backend's own record of the activity, in its contract.
    pub content: Value,
    /// The last accepted content as a portable template, when the activity is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<Template>,
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

/// Whether a backend provides a feature. `Unknown` never counts as supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeatureSupport {
    /// The backend provides it on this device.
    Supported,
    /// The backend or the device does not provide it.
    Unsupported,
    /// The backend cannot tell.
    Unknown,
}

impl FeatureSupport {
    /// Whether the feature is known to be provided.
    pub fn is_supported(self) -> bool {
        self == Self::Supported
    }
}

impl From<bool> for FeatureSupport {
    fn from(supported: bool) -> Self {
        if supported {
            Self::Supported
        } else {
            Self::Unsupported
        }
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
    /// Creating activities.
    pub start: FeatureSupport,
    /// Updating activities.
    pub update: FeatureSupport,
    /// Ending activities.
    pub end: FeatureSupport,
    /// Enumerating activities.
    pub list: FeatureSupport,
    /// Delivering native actions as events.
    pub actions: FeatureSupport,
    /// Drawing portable template content.
    pub templates: FeatureSupport,
    /// Drawing layouts from the backend's own content.
    pub layouts: FeatureSupport,
    /// Storing images that content names.
    pub images: FeatureSupport,
    /// The platform's native presentation, such as a Live Activity or a promoted Live
    /// Update, as opposed to an ordinary notification.
    pub native_presentation: FeatureSupport,
    /// Ending with final content.
    pub final_content: FeatureSupport,
    /// Keeping an ended activity on screen until a time (`DismissalPolicy::After`).
    pub delayed_dismissal: FeatureSupport,
    /// Largest content the backend accepts, in bytes, when it has a limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_content_bytes: Option<u64>,
}

impl Capabilities {
    /// Capabilities of `backend` with every feature in `support`, to set the others on.
    pub fn all(backend: impl Into<String>, support: FeatureSupport) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            backend: backend.into(),
            start: support,
            update: support,
            end: support,
            list: support,
            actions: support,
            templates: support,
            layouts: support,
            images: support,
            native_presentation: support,
            final_content: support,
            delayed_dismissal: support,
            max_content_bytes: None,
        }
    }

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
                activity_key: None,
            },
        }
    }

    /// Creates an action event that also names the caller-owned key of its activity.
    pub fn keyed_action(
        activity_id: impl Into<String>,
        activity_key: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            payload: LiveActivityEventPayload::Action {
                activity_id: activity_id.into(),
                action: action.into(),
                activity_key: Some(activity_key.into()),
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
                ..
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
        /// Caller-owned key of the activity, when the backend still knows it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        activity_key: Option<String>,
    },
    /// The backend observed a lifecycle or content change.
    StateChanged {
        /// Latest backend record.
        activity: ActivityRecord,
    },
}
