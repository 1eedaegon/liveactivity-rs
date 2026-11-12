use serde::{Deserialize, Deserializer, Serialize};

fn non_null<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> std::result::Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Timer {
    pub started_at: u64,
    pub ends_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub revision: u64,
    pub title: String,
    pub status: String,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub progress: Option<f64>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub timer: Option<Timer>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub stale_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Template {
    Progress,
    Timer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Fallback {
    None,
    Notification,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartInput {
    pub activity_key: String,
    pub schema_version: u8,
    pub template: Template,
    pub fallback: Fallback,
    pub snapshot: Snapshot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateInput {
    pub id: String,
    pub snapshot: Snapshot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum EndPolicy {
    Immediate {},
    Default {},
    After { at: u64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EndInput {
    pub id: String,
    pub policy: EndPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionKind {
    Notifications,
    LiveActivities,
    Promotion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeatureSupport {
    Supported,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Backend {
    ActivityKit,
    AndroidNotification,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub platform: String,
    pub backend: Backend,
    pub start: FeatureSupport,
    pub update: FeatureSupport,
    pub end: FeatureSupport,
    pub progress: FeatureSupport,
    pub timer: FeatureSupport,
    pub actions: FeatureSupport,
    pub state_observation: FeatureSupport,
    pub silent_update: FeatureSupport,
    pub native_live_activity: FeatureSupport,
    pub max_payload_bytes: u64,
    /// Layout activities of request contract v2; a native side that predates them omits it.
    #[serde(default = "unsupported")]
    pub layout: FeatureSupport,
}

fn unsupported() -> FeatureSupport {
    FeatureSupport::Unsupported
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionState {
    Granted,
    Denied,
    Prompt,
    SettingsRequired,
    NotApplicable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permissions {
    pub notifications: PermissionState,
    pub live_activities: PermissionState,
    pub promotion: PermissionState,
}

/// The lifecycle of an activity. More states may come, so a match needs a `_` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum ActivityState {
    /// Scheduled and not started yet (iOS 26 reports it for a scheduled start).
    Pending,
    Active,
    Stale,
    Ended,
    Dismissed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Visibility {
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PromotionState {
    Promoted,
    NotPromoted,
    Unknown,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRecord {
    pub id: String,
    pub activity_key: String,
    pub schema_version: u8,
    pub template: Template,
    pub snapshot: Snapshot,
    pub backend: Backend,
    pub state: ActivityState,
    pub visibility: Visibility,
    pub degraded_features: Vec<String>,
    pub promotion_requested: bool,
    pub promotion_state: PromotionState,
}

impl Snapshot {
    /// The same snapshot, shown as out of date from `at`, in epoch milliseconds.
    #[must_use]
    pub fn stale_at(self, at: u64) -> Self {
        Self {
            stale_at: Some(at),
            ..self
        }
    }

    fn progress(revision: u64, title: &str, status: &str, progress: f64) -> Self {
        Self {
            revision,
            title: title.into(),
            status: status.into(),
            progress: Some(progress),
            timer: None,
            stale_at: None,
        }
    }

    fn timer(revision: u64, title: &str, status: &str, started_at: u64, ends_at: u64) -> Self {
        Self {
            revision,
            title: title.into(),
            status: status.into(),
            progress: None,
            timer: Some(Timer {
                started_at,
                ends_at,
            }),
            stale_at: None,
        }
    }
}

impl StartInput {
    /// The same start with content that is shown as out of date from `at`, in epoch milliseconds.
    #[must_use]
    pub fn stale(self, at: u64) -> Self {
        Self {
            snapshot: self.snapshot.stale_at(at),
            ..self
        }
    }

    fn of(activity_key: &str, template: Template, snapshot: Snapshot) -> Self {
        Self {
            activity_key: activity_key.into(),
            schema_version: 1,
            template,
            fallback: Fallback::Notification,
            snapshot,
        }
    }

    /// A progress activity under the application's own `activity_key`, with `progress` from 0 to 1.
    pub fn progress(activity_key: &str, title: &str, status: &str, progress: f64) -> Self {
        Self::of(
            activity_key,
            Template::Progress,
            Snapshot::progress(0, title, status, progress),
        )
    }

    /// A countdown from `started_at` to `ends_at`, in epoch milliseconds.
    pub fn timer(
        activity_key: &str,
        title: &str,
        status: &str,
        started_at: u64,
        ends_at: u64,
    ) -> Self {
        Self::of(
            activity_key,
            Template::Timer,
            Snapshot::timer(0, title, status, started_at, ends_at),
        )
    }

    /// The same start, failing with `UNSUPPORTED_FEATURE` where the platform cannot give the activity its native
    /// presentation, instead of showing an ordinary notification.
    #[must_use]
    pub fn without_fallback(self) -> Self {
        Self {
            fallback: Fallback::None,
            ..self
        }
    }
}

impl UpdateInput {
    /// New content of the progress activity `id` under `revision`, which is higher than the last.
    pub fn progress(id: &str, revision: u64, title: &str, status: &str, progress: f64) -> Self {
        Self {
            id: id.into(),
            snapshot: Snapshot::progress(revision, title, status, progress),
        }
    }

    /// New content of the timer activity `id` under `revision`.
    pub fn timer(
        id: &str,
        revision: u64,
        title: &str,
        status: &str,
        started_at: u64,
        ends_at: u64,
    ) -> Self {
        Self {
            id: id.into(),
            snapshot: Snapshot::timer(revision, title, status, started_at, ends_at),
        }
    }
}

impl EndInput {
    /// Ends the activity `id` the platform's default way.
    pub fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            policy: EndPolicy::Default {},
        }
    }

    /// Ends the activity `id` and removes it from the screen at once.
    pub fn immediate(id: &str) -> Self {
        Self {
            id: id.into(),
            policy: EndPolicy::Immediate {},
        }
    }
}
