use serde::{Deserialize, Deserializer, Serialize};

fn non_null<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> std::result::Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

/// A countdown, in epoch milliseconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Timer {
    /// Start of the countdown.
    pub started_at: u64,
    /// End of the countdown, after `started_at`.
    pub ends_at: u64,
}

/// The content of a template activity: a title, a status and either a progress or a timer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    /// The revision of this content: higher than the last accepted one.
    pub revision: u64,
    /// The headline, 1 to 120 characters.
    pub title: String,
    /// The status line, 1 to 240 characters.
    pub status: String,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    /// The progress from 0 to 1, for the progress template.
    pub progress: Option<f64>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    /// The countdown, for the timer template.
    pub timer: Option<Timer>,
    #[serde(
        default,
        deserialize_with = "non_null",
        skip_serializing_if = "Option::is_none"
    )]
    /// When the platform may show the content as out of date, in epoch milliseconds.
    pub stale_at: Option<u64>,
}

/// The two templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Template {
    /// A title, a status and a progress bar.
    Progress,
    /// A title, a status and a countdown.
    Timer,
}

/// What a start does when the platform cannot give the activity its native presentation.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Fallback {
    /// Fail with `UNSUPPORTED_FEATURE` and show nothing.
    None,
    /// Show the activity in the platform's ordinary presentation.
    Notification,
}

/// The input of `start`: a progress or timer activity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartInput {
    /// The application's own name of the activity: a running activity with the same key and request is the same activity.
    pub activity_key: String,
    /// The version of the request contract: 1 for a template, 2 for a layout.
    pub schema_version: u8,
    /// The template, fixed at start.
    pub template: Template,
    /// What to do when the platform cannot give the activity its native presentation.
    pub fallback: Fallback,
    /// The content, with its revision.
    pub snapshot: Snapshot,
}

/// The input of `update`: the whole content again, under a higher revision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateInput {
    /// The handle of the activity, new for every start.
    pub id: String,
    /// The content, with its revision.
    pub snapshot: Snapshot,
}

/// When an ended activity leaves the screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum EndPolicy {
    /// Remove the activity from the screen at once.
    Immediate {},
    /// Leave it to the platform.
    Default {},
    /// Keep the ended activity on screen for a while, where the platform can.
    After {
        /// Until when, in epoch milliseconds.
        at: u64,
    },
}

/// The input of `end`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EndInput {
    /// The handle of the activity, new for every start.
    pub id: String,
    /// When the ended activity leaves the screen.
    pub policy: EndPolicy,
}

/// A permission that a request can ask for.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionKind {
    /// Posting notifications (Android).
    Notifications,
    /// Showing Live Activities (iOS).
    LiveActivities,
    /// Promoting a notification to a Live Update (Android).
    Promotion,
}

/// Whether a backend provides a feature.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeatureSupport {
    /// Provided on this device.
    Supported,
    /// Not provided on this platform or device.
    Unsupported,
    /// Not known, or a value this version does not name.
    Unknown,
}

/// Which backend shows an activity.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Backend {
    /// ActivityKit, on iOS.
    ActivityKit,
    /// An Android notification, promoted to a Live Update where Android allows.
    AndroidNotification,
    /// Not provided on this platform or device.
    Unsupported,
}

/// What the backend can do on this device. A feature is supported, unsupported or unknown, and unknown never counts as supported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// The platform the backend runs on, such as `ios` or `android`.
    pub platform: String,
    /// Which backend shows the activity.
    pub backend: Backend,
    /// Starting activities.
    pub start: FeatureSupport,
    /// Updating activities.
    pub update: FeatureSupport,
    /// Ending activities.
    pub end: FeatureSupport,
    /// The progress template.
    pub progress: FeatureSupport,
    /// The timer template.
    pub timer: FeatureSupport,
    /// Buttons whose taps reach the event sink.
    pub actions: FeatureSupport,
    /// Reading an activity's state back from the platform.
    pub state_observation: FeatureSupport,
    /// Updating without alerting the person.
    pub silent_update: FeatureSupport,
    /// The platform's native presentation: a Live Activity, or a promoted Live Update.
    pub native_live_activity: FeatureSupport,
    /// The largest content the backend takes, in bytes.
    pub max_payload_bytes: u64,
    /// Layout activities of request contract v2; a native side that predates them omits it.
    #[serde(default = "unsupported")]
    pub layout: FeatureSupport,
    /// Pushes: tokens, channels and push-to-start (iOS); a native side that predates them omits it.
    #[serde(default = "unsupported")]
    pub push: FeatureSupport,
    /// Which notification styles the device draws, for the Android content of a snapshot. Present when the
    /// Android backend runs layouts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub android_presentation: Option<crate::layout::PresentationCapabilities>,
}

fn unsupported() -> FeatureSupport {
    FeatureSupport::Unsupported
}

fn not_applicable() -> PermissionState {
    PermissionState::NotApplicable
}

/// The state of one permission.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionState {
    /// Allowed.
    Granted,
    /// Refused.
    Denied,
    /// Not asked yet: a request shows the system's prompt.
    Prompt,
    /// Only the person can change it, in the system settings.
    SettingsRequired,
    /// The platform has no such thing.
    NotApplicable,
    /// Not known, or a value this version does not name.
    Unknown,
}

/// What the operating system and the person allow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permissions {
    /// Posting notifications (Android).
    pub notifications: PermissionState,
    /// Showing Live Activities (iOS).
    pub live_activities: PermissionState,
    /// Promoting a notification to a Live Update (Android).
    pub promotion: PermissionState,
    /// Frequent pushes, a switch in Settings on iOS (`granted` or `denied`); `notApplicable` where there are no
    /// pushes. A native side that predates it omits it.
    #[serde(default = "not_applicable")]
    pub frequent_pushes: PermissionState,
}

/// The lifecycle of an activity. More states may come, so a match needs a `_` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum ActivityState {
    /// Scheduled and not started yet (iOS 26 reports it for a scheduled start).
    Pending,
    /// On screen and taking updates.
    Active,
    /// Past its stale date; it may still be on screen.
    Stale,
    /// Ended by the application; the platform may still show it.
    Ended,
    /// Gone from the screen.
    Dismissed,
    /// Not known, or a value this version does not name.
    Unknown,
}

/// What is known of an activity being on screen.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Visibility {
    /// Not known, or a value this version does not name.
    Unknown,
}

/// What Android did with a request to promote a notification to a Live Update.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PromotionState {
    /// Android shows the notification as a Live Update.
    Promoted,
    /// Android took the notification without promoting it.
    NotPromoted,
    /// Not known, or a value this version does not name.
    Unknown,
    /// The platform has no such thing.
    NotApplicable,
}

/// A template activity as the backend reports it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRecord {
    /// The handle of the activity, new for every start.
    pub id: String,
    /// The application's own name of the activity: a running activity with the same key and request is the same activity.
    pub activity_key: String,
    /// The version of the request contract: 1 for a template, 2 for a layout.
    pub schema_version: u8,
    /// The template, fixed at start.
    pub template: Template,
    /// The content, with its revision.
    pub snapshot: Snapshot,
    /// Which backend shows the activity.
    pub backend: Backend,
    /// The lifecycle state the platform reports.
    pub state: ActivityState,
    /// What is known of the activity being on screen.
    pub visibility: Visibility,
    /// What the platform left out of the request, such as `nativeLiveActivity` or `layout.<node>`.
    pub degraded_features: Vec<String>,
    /// Whether promotion to a Live Update was requested (Android).
    pub promotion_requested: bool,
    /// What Android did with a request to promote the notification.
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
