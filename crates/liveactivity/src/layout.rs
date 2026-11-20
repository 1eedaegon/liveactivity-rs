//! Request contract v2: a layout tree, a style table and a map of values.
//!
//! The requests and records are this crate's own, and so is what they say beside the layout: the options of each
//! platform, the time of a scheduled start, the relevance, an alert and the Android content of a snapshot. The
//! vocabulary of each
//! presentation belongs to the engine that draws it: the node and style types and their validator are the
//! `activitykit` crate's, and the notification styles of an Android content are the `liveupdate` crate's, both
//! re-exported here. A request is read in this crate's types, and the part activitykit knows is converted into its
//! request to be checked.

use crate::{
    Error, Result,
    models::{ActivityState, Backend, EndPolicy, Fallback, PromotionState, Visibility},
    validation::{MAX_TIMESTAMP, Validate, file_name, require, text},
};
use activitykit::contract as wire;
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use std::collections::BTreeMap;

pub use activitykit::Value;
/// What a layout activity asks ActivityKit for on iOS: a push token of its own (`"token"`), reported as a
/// `live-activity://push-token` event once ActivityKit has it, or a broadcast channel the server made at APNs
/// (`{"channel": "<id>"}`, iOS 18). Android ignores it.
pub use activitykit::contract::PushWire as LayoutPush;
pub use activitykit::contract::{Island, IslandExpanded, weight};
pub use activitykit::layout::{
    Accessibility, Alignment, Bindable, Bound, ButtonStyle, Color, ColorName, ContentMode,
    ContentTransition, Dimension, Element, ExpandedRegion, Font, FontDesign, FontWeight, Frame,
    HorizontalAlignment, ImageSource, IndicatorKind, IslandMargins, Node, Padding, Privacy,
    RegionPriorities, Style, SymbolRendering, Transition, VerticalAlignment,
};

/// Android's notification styles, for the [`AndroidContent`] of a snapshot. A [`TimerStyle`] takes liveupdate's
/// `Timer`, which is `liveactivity::liveupdate::Timer`.
pub use liveupdate::{
    ActionTarget, AndroidPresentation, BigTextStyle, IconSource, Metric, MetricStyle, MetricValue,
    NotificationAction, PresentationCapabilities, PresentationStyle, ProgressPoint,
    ProgressSegment, ProgressStyle, PublicVersion, SamsungNowBar, SemanticStyle, TargetKind,
    TimerStyle,
};

/// Lock screen visibility of the Android notification.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AndroidVisibility {
    /// The whole notification shows on a locked screen.
    Public,
    /// The content is hidden on a locked screen (the default).
    Private,
    /// Nothing shows on a locked screen.
    Secret,
}

/// How Android keys the notification of an activity.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AndroidPlacement {
    /// A notification of its own, beside the application's others (the default).
    Tagged,
    /// The notification that a foreground service of the application holds, so that the service shows no second
    /// one. One running activity has this placement at a time. The service is the application's: it attaches
    /// itself with liveupdate's `LiveUpdateForeground` on the Android side.
    Foreground,
}

/// Options of a start that only Android reads. iOS ignores them. More may come: build it with
/// [`AndroidOptions::new`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AndroidOptions {
    /// Lock screen visibility of the notification.
    pub visibility: AndroidVisibility,
    /// What a secure lock screen shows in place of the content, for a private activity. It goes with every
    /// snapshot that has no public version of its own. Where the device offers none, the record names
    /// `publicVersion` in `degradedFeatures`.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub public_version: Option<PublicVersion>,
    /// How Android keys the notification; tagged when absent. A device that does not offer the foreground
    /// placement answers `UNSUPPORTED_FEATURE`.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub placement: Option<AndroidPlacement>,
    /// Whether the activity carries a watch's Ongoing Activity where the device offers one
    /// (`wear_ongoing_activity` in the capabilities): its icon on the watch face of a watch below Wear OS 7 whose
    /// application has `androidx.wear:wear-ongoing`. Carried when absent. `false` goes with every snapshot of
    /// the activity, the projected ones and an Android content, whatever that says of it.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub wear_ongoing_activity: Option<bool>,
    /// What runs the tap of a button or a toggle of the lock screen tree, by its action id: a service, a foreground
    /// service or a receiver of the application, at once and with no screen. The projection puts each target on
    /// its action; a device without `action_targets` answers `UNSUPPORTED_FEATURE`. An Android content names the
    /// targets of its own actions.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub targets: BTreeMap<String, ActionTarget>,
}

impl AndroidOptions {
    /// The options with this lock screen visibility, and nothing else.
    pub fn new(visibility: AndroidVisibility) -> Self {
        Self {
            visibility,
            public_version: None,
            placement: None,
            wear_ongoing_activity: None,
            targets: BTreeMap::new(),
        }
    }

    /// The options with `target` running the tap of the button or toggle `action_id`.
    #[must_use]
    pub fn with_target(mut self, action_id: impl Into<String>, target: ActionTarget) -> Self {
        self.targets.insert(action_id.into(), target);
        self
    }

    /// The options of an activity that carries a watch's Ongoing Activity, or does not.
    #[must_use]
    pub fn with_wear_ongoing_activity(self, carried: bool) -> Self {
        Self {
            wear_ongoing_activity: Some(carried),
            ..self
        }
    }
}

/// Options of a start that only iOS reads. Android ignores them: two starts that differ only in them are the
/// same activity there. More may come: build it with [`IosOptions::new`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IosOptions {
    /// A transient activity (iOS 18 and later; `UNSUPPORTED_FEATURE` before). By Apple's description it shows in
    /// the expanded Dynamic Island, and the system ends it when the person locks the device, collapses that
    /// presentation or leaves the application.
    pub transient: bool,
}

impl IosOptions {
    /// The options of an activity that is transient, or is not, and nothing else.
    pub fn new(transient: bool) -> Self {
        Self { transient }
    }
}

/// A start at a later time. Until then the activity is `pending`: it holds its key, it can be ended, and it takes
/// no update (`CONFLICT`), since it begins with the content of its start. The sink receives `State` with `active`
/// when it begins. iOS 26 and later, and Android where liveupdate's library begins it
/// (`android_presentation.scheduled_start`); elsewhere the start is `UNSUPPORTED_FEATURE`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduledStart {
    /// When the activity begins, in epoch milliseconds. A time that has passed starts it at once.
    pub at: u64,
    /// The alert the system shows when it starts the activity. The system requires one.
    pub alert: Alert,
}

impl ScheduledStart {
    /// A start at `at`, in epoch milliseconds, with the alert the system shows then.
    pub fn new(at: u64, alert: Alert) -> Self {
        Self { at, alert }
    }
}

/// What Android shows for a snapshot, in Android's own terms, in place of the projection of the layout: a
/// progress with segments and points, metrics, the text of the status chip, icons of actions, the URL a tap
/// opens, a clock in the header. iOS ignores it.
///
/// A content of another notification style than the one the activity shows needs a device that changes styles
/// (`styleChange` in the capabilities); on another device it is `UNSUPPORTED_FEATURE`, and the activity keeps what
/// it shows. A tap on one of its actions reaches the event sink like a tap on a button node, unless the action
/// opens a URL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AndroidContent {
    /// The notification's title, 1 to 120 characters.
    pub title: String,
    /// The notification's text, 1 to 240 characters.
    pub status: String,
    /// The notification style and what it shows.
    pub presentation: AndroidPresentation,
}

/// An alert that comes with an update, or with a scheduled start: on iOS the device lights up, the Dynamic Island
/// expands (or, without one, the Lock Screen presentation shows as a banner), and an Apple Watch shows the title
/// and the body. On Android the notification alerts again with that revision, with its own title and text, where
/// the device offers it (`alert` in the capabilities); elsewhere the update is silent and the record reports
/// `alert` in `degradedFeatures`. More may come: build it with [`Alert::new`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Alert {
    /// The alert's title, 1 to 120 characters.
    pub title: String,
    /// The alert's body, 1 to 240 characters.
    pub body: String,
    /// The sound of the alert on iOS: the file name of a sound in the application's bundle or in its
    /// `Library/Sounds`, such as `chime.aiff`, 1 to 64 of `A-Za-z0-9._-` beginning with a letter or a digit.
    /// Without it the system plays its default sound. Android alerts with the sound of its channel, which is the
    /// person's choice, and the record names `alert.sound`.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub sound: Option<String>,
}

impl Alert {
    /// An alert with this title and body, and the system's sound.
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            sound: None,
        }
    }

    /// The alert with the sound of the file `name`.
    pub fn with_sound(self, name: impl Into<String>) -> Self {
        Self {
            sound: Some(name.into()),
            ..self
        }
    }
}

/// The member `layout` of a start: one tree per region, the two tints and the URL a tap opens. The layout is fixed
/// when the activity starts; only a bound `background`, `keyline` or `url` follows the values.
pub type Layout = wire::LayoutRegions;

/// Reads a member that may be absent but never `null`.
fn non_null<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> std::result::Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

fn none<T>(member: &Option<T>) -> bool {
    member.is_none()
}

/// The values a layout shows. An update replaces the whole map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutSnapshot {
    /// The revision of this content: higher than the last accepted one.
    pub revision: u64,
    /// Every value the layout binds, by key. An update sends them all again.
    pub values: BTreeMap<String, Value>,
    /// When the system may show the activity as out of date, in epoch milliseconds.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub stale_at: Option<u64>,
    /// When the content was made, in epoch milliseconds. ActivityKit (iOS 17.2) drops content older than what it
    /// shows. Without it, iOS takes the time the content is sent.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub timestamp: Option<u64>,
    /// Which of the application's activities the Dynamic Island shows first: the highest score wins. Without it
    /// the activity keeps its score, which is 0 at first. Android ignores it.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub relevance: Option<f64>,
    /// What Android shows for this snapshot instead of the projection of the layout. iOS ignores it.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub android: Option<AndroidContent>,
}

impl LayoutSnapshot {
    /// The values at `revision`, with nothing else.
    pub fn new(revision: u64, values: BTreeMap<String, Value>) -> Self {
        Self {
            revision,
            values,
            stale_at: None,
            timestamp: None,
            relevance: None,
            android: None,
        }
    }
}

/// The input of `start_layout`: a layout tree, its styles and its first values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutStartInput {
    /// The application's own name of the activity: a running activity with the same key and request is the same activity.
    pub activity_key: String,
    /// The version of the request contract: 1 for a template, 2 for a layout.
    pub schema_version: u8,
    /// What to do when the platform cannot give the activity its native presentation.
    pub fallback: Fallback,
    /// One tree per region, the two tints and the URL a tap opens, fixed at start.
    pub layout: Layout,
    /// Styles by token; nodes name them with `style`.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub styles: Option<BTreeMap<String, Style>>,
    /// The values, with their revision.
    pub snapshot: LayoutSnapshot,
    /// Options that only Android reads.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub android: Option<AndroidOptions>,
    /// Options that only iOS reads.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub ios: Option<IosOptions>,
    /// Starts the activity at a later time instead of now. A platform that cannot answers `UNSUPPORTED_FEATURE`.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub scheduled: Option<ScheduledStart>,
    /// Ignored on Android.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub push: Option<LayoutPush>,
}

/// The input of `update_layout`: every value again, under a higher revision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutUpdateInput {
    /// The handle of the activity, new for every start.
    pub id: String,
    /// The values, with their revision.
    pub snapshot: LayoutSnapshot,
    /// An alert that comes with the update. Without it the update is silent.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub alert: Option<Alert>,
}

impl LayoutStartInput {
    /// A start of `layout` under the application's own `activity_key`, with its first values: without styles,
    /// options or a push request, and shown as a notification where the platform has no native presentation.
    pub fn new(activity_key: impl Into<String>, layout: Layout, snapshot: LayoutSnapshot) -> Self {
        Self {
            activity_key: activity_key.into(),
            schema_version: 2,
            fallback: Fallback::Notification,
            layout,
            styles: None,
            snapshot,
            android: None,
            ios: None,
            scheduled: None,
            push: None,
        }
    }
}

impl LayoutEndInput {
    /// Ends the activity `id` the platform's default way, with its last accepted values.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            policy: EndPolicy::Default {},
            snapshot: None,
        }
    }

    /// Ends the activity `id` and removes it from the screen at once.
    pub fn immediate(id: impl Into<String>) -> Self {
        Self {
            policy: EndPolicy::Immediate {},
            ..Self::new(id)
        }
    }
}

impl LayoutUpdateInput {
    /// A silent update of the activity `id`.
    pub fn new(id: impl Into<String>, snapshot: LayoutSnapshot) -> Self {
        Self {
            id: id.into(),
            snapshot,
            alert: None,
        }
    }
}

/// The input of `end_layout`: the policy, and optionally the final content, which is checked as an update. Without
/// it the activity ends with its last accepted values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutEndInput {
    /// The handle of the activity, new for every start.
    pub id: String,
    /// When the ended activity leaves the screen.
    pub policy: EndPolicy,
    /// The final content, checked as an update. Without it the activity ends with its last accepted values.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub snapshot: Option<LayoutSnapshot>,
}

/// A layout activity as the native side reports it. It does not repeat the layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutActivityRecord {
    /// The handle of the activity, new for every start.
    pub id: String,
    /// The application's own name of the activity: a running activity with the same key and request is the same activity.
    pub activity_key: String,
    /// The version of the request contract: 1 for a template, 2 for a layout.
    pub schema_version: u8,
    /// The values, with their revision.
    pub snapshot: LayoutSnapshot,
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

/// A tap on a button or a toggle node, or on an action of an Android content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionEvent {
    /// The id of the tapped button or toggle, as its node declares it.
    pub action_id: String,
    /// Handle of the activity that showed it.
    pub activity_id: String,
    /// Absent when the backend no longer knows that activity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_key: Option<String>,
    /// The state the person asked a toggle for: the other one than the value it showed. The tap changes nothing:
    /// update the value to move the switch. Absent for a button and for an action of an Android content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<bool>,
    /// Which tap it is and when it was made; absent when nothing of it is known.
    #[serde(default, skip_serializing_if = "Tap::is_unknown")]
    pub tap: Tap,
}

impl ActionEvent {
    /// A tap on the action `action_id` of the activity `activity_id`, of no known key, state or tap.
    pub fn new(action_id: impl Into<String>, activity_id: impl Into<String>) -> Self {
        Self {
            action_id: action_id.into(),
            activity_id: activity_id.into(),
            activity_key: None,
            on: None,
            tap: Tap::default(),
        }
    }
}

/// What a tap says of itself, beside the activity and the action of an [`ActionEvent`]. Each member is absent when it
/// is not known.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tap {
    /// The same for a tap however often it is handed on, also in a later launch: an application that keeps what it
    /// handled can tell a tap it saw before. On Android the adapter's sequence of the stored tap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// When the person tapped, in epoch milliseconds. On Android when the tap was stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<u64>,
    /// The revision of the content the activity showed (iOS). A tap on content older than the activity's current
    /// revision acts on what the person saw.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    /// iOS: the application did not take the tap while its intent posted it, so it was kept in the application's
    /// container and comes to a later kit, in this launch or the next, at most 12 hours later. The activity may have
    /// ended since. Never on Android.
    #[serde(default, skip_serializing_if = "is_false")]
    pub late: bool,
}

impl Tap {
    pub(crate) fn is_unknown(&self) -> bool {
        self == &Self::default()
    }
}

fn is_false(value: &bool) -> bool {
    !value
}

impl LayoutSnapshot {
    fn wire(&self) -> wire::LayoutSnapshot {
        let mut snapshot = wire::LayoutSnapshot::new(self.revision, self.values.clone().into());
        snapshot.stale_at = self.stale_at;
        snapshot.timestamp = self.timestamp;
        snapshot
    }
}

impl LayoutStartInput {
    /// The same request in activitykit's types.
    pub(crate) fn wire(&self) -> wire::LayoutStartInput {
        let fallback = match self.fallback {
            Fallback::None => wire::Fallback::None,
            Fallback::Notification => wire::Fallback::Notification,
        };
        let mut start = wire::LayoutStartInput::new(
            self.activity_key.clone(),
            self.layout.clone(),
            self.snapshot.wire(),
        )
        .fallback(fallback);
        // The request's own schema version, which the checks refuse when it is not 2. What only Android reads is
        // this crate's: activitykit is not asked about it.
        start.schema_version = self.schema_version;
        start.styles = self.styles.clone();
        start.push = self.push.clone();
        start
    }

    /// Checks `update` against this start as the native layer does: the same keys and value types, the layout's
    /// references (bound style members included) and the weight of the layout, the styles and the new values.
    pub fn validate_update(&self, update: &LayoutUpdateInput) -> Result<()> {
        Ok(self.wire().validate_update(&update.wire())?)
    }

    /// Checks `end` against this start: its final content, when it has one, as an update.
    pub fn validate_end(&self, end: &LayoutEndInput) -> Result<()> {
        Ok(self.wire().validate_end(&end.wire())?)
    }
}

impl LayoutUpdateInput {
    fn wire(&self) -> wire::LayoutUpdateInput {
        wire::LayoutUpdateInput::new(self.id.clone(), self.snapshot.wire())
    }
}

impl LayoutEndInput {
    fn wire(&self) -> wire::EndInput {
        let policy = match self.policy {
            EndPolicy::Immediate {} => wire::EndPolicy::immediate(),
            EndPolicy::Default {} => wire::EndPolicy::default(),
            EndPolicy::After { at } => wire::EndPolicy::after(at),
        };
        let mut end = wire::EndInput::new(self.id.clone(), policy);
        end.snapshot = self.snapshot.as_ref().map(LayoutSnapshot::wire);
        end
    }
}

impl AndroidContent {
    /// What Android shows: a title, a text and the notification's style with what it shows.
    #[must_use]
    pub fn new(
        title: impl Into<String>,
        status: impl Into<String>,
        presentation: AndroidPresentation,
    ) -> Self {
        Self {
            title: title.into(),
            status: status.into(),
            presentation,
        }
    }

    /// The content as liveupdate takes it, at the snapshot's revision and stale date.
    pub(crate) fn snapshot(&self, revision: u64, stale_at: Option<u64>) -> liveupdate::Snapshot {
        liveupdate::Snapshot {
            revision,
            title: self.title.clone(),
            status: self.status.clone(),
            progress: None,
            timer: None,
            stale_at,
            presentation: Some(self.presentation.clone()),
        }
    }
}

impl LayoutSnapshot {
    /// This crate's rules for what activitykit's contract does not carry: a finite relevance, and an Android
    /// content that liveupdate takes. They hold on every platform, so that a request is valid everywhere or nowhere.
    fn beside_the_layout(&self) -> Result<()> {
        require(self.relevance.is_none_or(f64::is_finite))?;
        match &self.android {
            Some(content) => content
                .snapshot(self.revision, self.stale_at)
                .validate()
                .map_err(|_| Error::invalid()),
            None => Ok(()),
        }
    }
}

impl Validate for Alert {
    fn validate(&self) -> Result<()> {
        require(text(&self.title, 120) && text(&self.body, 240))?;
        require(self.sound.as_deref().is_none_or(file_name))
    }
}

impl Validate for ScheduledStart {
    fn validate(&self) -> Result<()> {
        require(self.at <= MAX_TIMESTAMP)?;
        self.alert.validate()
    }
}

impl Validate for LayoutSnapshot {
    fn validate(&self) -> Result<()> {
        wire::Validate::validate(&self.wire())?;
        self.beside_the_layout()
    }
}

/// The value key that marks contract v1's template activities on iOS: a layout request may not use it, or its activity
/// would pass for a template and leave the layout commands.
pub(crate) const TEMPLATE_MARKER: &str = "liveActivityTemplate";

/// This crate's rule beyond activitykit's contract: no value under [`TEMPLATE_MARKER`].
fn unmarked(snapshot: &LayoutSnapshot) -> Result<()> {
    if snapshot.values.contains_key(TEMPLATE_MARKER) {
        Err(Error::invalid())
    } else {
        Ok(())
    }
}

impl LayoutStartInput {
    /// activitykit's rules alone, for the starts that the backend makes of contract v1's templates.
    pub(crate) fn validate_contract(&self) -> Result<()> {
        Ok(wire::Validate::validate(&self.wire())?)
    }
}

impl LayoutStartInput {
    /// Whether `snapshot` may be content of the activity this start begins, as far as its public version goes. A
    /// public version is what a locked screen shows in place of a private notification's content, so only an
    /// activity that started as private, as one does unless it says otherwise, takes a snapshot with one.
    pub(crate) fn takes_public_version_of(&self, snapshot: &LayoutSnapshot) -> bool {
        let private = self
            .android
            .as_ref()
            .is_none_or(|android| android.visibility == AndroidVisibility::Private);
        let public_version = snapshot
            .android
            .as_ref()
            .and_then(|content| content.presentation.public_version.as_ref());
        private || public_version.is_none()
    }
}

impl LayoutStartInput {
    /// The rule of the public version among the Android options: liveupdate's bounds for its two texts, and a
    /// private activity, since a locked screen shows the content of any other or nothing of it.
    fn public_version_of_the_start_fits(&self) -> bool {
        let Some(android) = &self.android else {
            return true;
        };
        let Some(public_version) = &android.public_version else {
            return true;
        };
        let bounded = AndroidPresentation {
            public_version: Some(public_version.clone()),
            ..AndroidPresentation::new(PresentationStyle::Standard)
        };
        android.visibility == AndroidVisibility::Private && bounded.validate().is_ok()
    }
}

impl LayoutStartInput {
    /// The rule of the targets among the Android options: each names a button or a toggle of the lock screen tree,
    /// which is what Android projects, and each is a target that liveupdate takes on an action of that id.
    fn targets_fit(&self) -> bool {
        let Some(android) = &self.android else {
            return true;
        };
        android.targets.iter().all(|(action_id, target)| {
            let action = NotificationAction {
                target: Some(target.clone()),
                ..NotificationAction::new(action_id.clone(), "Action")
            };
            let bounded = AndroidPresentation {
                actions: vec![action],
                ..AndroidPresentation::new(PresentationStyle::Standard)
            };
            controls(&self.layout.lock_screen, action_id) && bounded.validate().is_ok()
        })
    }
}

/// Whether a button or a toggle of the tree has `action_id`, shown or not.
pub(crate) fn controls(node: &Node, action_id: &str) -> bool {
    match &node.element {
        Element::Button { action_id: id, .. } | Element::Toggle { action_id: id, .. }
            if id == action_id =>
        {
            true
        }
        Element::Button { child, .. } | Element::Toggle { child, .. } => controls(child, action_id),
        Element::Vstack { children, .. }
        | Element::Hstack { children, .. }
        | Element::Zstack { children, .. } => {
            children.iter().any(|child| controls(child, action_id))
        }
        _ => false,
    }
}

impl Validate for LayoutStartInput {
    fn validate(&self) -> Result<()> {
        self.validate_contract()?;
        self.snapshot.beside_the_layout()?;
        require(self.takes_public_version_of(&self.snapshot))?;
        require(self.public_version_of_the_start_fits())?;
        require(self.targets_fit())?;
        self.scheduled.as_ref().map_or(Ok(()), Validate::validate)?;
        unmarked(&self.snapshot)
    }
}

impl Validate for LayoutUpdateInput {
    fn validate(&self) -> Result<()> {
        wire::Validate::validate(&self.wire())?;
        self.snapshot.beside_the_layout()?;
        self.alert.as_ref().map_or(Ok(()), Validate::validate)?;
        unmarked(&self.snapshot)
    }
}

impl Validate for LayoutEndInput {
    fn validate(&self) -> Result<()> {
        wire::Validate::validate(&self.wire())?;
        let Some(snapshot) = &self.snapshot else {
            return Ok(());
        };
        snapshot.beside_the_layout()?;
        unmarked(snapshot)
    }
}

/// A valid request nests at most 20 arrays and objects; deeper JSON is refused unread.
const MAX_NESTING: usize = 32;

fn shallow(value: &serde_json::Value, room: usize) -> bool {
    match value {
        serde_json::Value::Array(items) => {
            room > 0 && items.iter().all(|item| shallow(item, room - 1))
        }
        serde_json::Value::Object(members) => {
            room > 0 && members.values().all(|member| shallow(member, room - 1))
        }
        _ => true,
    }
}

/// JSON equality with numbers compared by value, so `1` equals `1.0`.
fn same(left: &serde_json::Value, right: &serde_json::Value) -> bool {
    use serde_json::Value::{Array, Number, Object};
    match (left, right) {
        (Number(left), Number(right)) => left.as_f64() == right.as_f64(),
        (Array(left), Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| same(left, right))
        }
        (Object(left), Object(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .all(|(key, left)| right.get(key).is_some_and(|right| same(left, right)))
        }
        _ => left == right,
    }
}

/// Where the presentation of a snapshot's Android content sits in a request.
const PRESENTATION: &str = "/snapshot/android/presentation";

/// A request without the presentation of its snapshot's Android content.
fn beside_the_presentation(request: &serde_json::Value) -> serde_json::Value {
    let mut request = request.clone();
    if let Some(content) = request
        .pointer_mut("/snapshot/android")
        .and_then(serde_json::Value::as_object_mut)
    {
        content.remove("presentation");
    }
    request
}

/// The members of a presentation that are `true` by default. liveupdate leaves them out when they are.
const TRUE_BY_DEFAULT: [&str; 1] = ["wearOngoingActivity"];

/// Whether `sent` is `encoded` but for members at liveupdate's defaults, which liveupdate leaves out when it
/// writes a presentation: `false`, an empty array, and `true` for a member that is so by default. A `null` is no
/// default, and an array is no object.
fn same_but_defaults(encoded: &serde_json::Value, sent: &serde_json::Value) -> bool {
    use serde_json::Value::{Array, Bool, Object};
    match (encoded, sent) {
        (Array(encoded), Array(sent)) => {
            encoded.len() == sent.len()
                && encoded
                    .iter()
                    .zip(sent)
                    .all(|(encoded, sent)| same_but_defaults(encoded, sent))
        }
        (Object(encoded), Object(sent)) => {
            let written = sent.iter().all(|(key, sent)| match encoded.get(key) {
                Some(encoded) => same_but_defaults(encoded, sent),
                None => match sent {
                    Bool(sent) => *sent == TRUE_BY_DEFAULT.contains(&key.as_str()),
                    sent => sent.as_array().is_some_and(Vec::is_empty),
                },
            });
            written && encoded.keys().all(|key| sent.contains_key(key))
        }
        _ => same(encoded, sent),
    }
}

/// Reads one layout request strictly, in this crate's type: no unknown member, no `null`, and nothing that serde
/// would correct silently, since the request is taken only when it encodes back to itself. The presentation of
/// an Android content is liveupdate's, which leaves a member at its default out: there a default may be written.
/// Then the rules.
fn strict<T: DeserializeOwned + Serialize + Validate>(input: &serde_json::Value) -> Result<T> {
    require(shallow(input, MAX_NESTING))?;
    let typed: T = serde_json::from_value(input.clone()).map_err(|_| Error::invalid())?;
    let encoded = serde_json::to_value(&typed).map_err(|_| Error::invalid())?;
    require(same(
        &beside_the_presentation(&encoded),
        &beside_the_presentation(input),
    ))?;
    require(
        match (encoded.pointer(PRESENTATION), input.pointer(PRESENTATION)) {
            (Some(encoded), Some(sent)) => same_but_defaults(encoded, sent),
            (None, None) => true,
            _ => false,
        },
    )?;
    typed.validate()?;
    Ok(typed)
}

/// A JSON request of `command` (`start_layout`, `update_layout` or `end_layout`), read strictly as that command's
/// request and then in the caller's type. The request is read in this crate's types first, which know what each
/// platform takes beside the layout; activitykit checks the part it knows.
pub(crate) fn parse<T: DeserializeOwned + Validate>(
    command: &str,
    input: serde_json::Value,
) -> Result<T> {
    match command {
        "start_layout" => strict::<LayoutStartInput>(&input).map(|_| ())?,
        "update_layout" => strict::<LayoutUpdateInput>(&input).map(|_| ())?,
        "end_layout" => strict::<LayoutEndInput>(&input).map(|_| ())?,
        _ => return Err(Error::invalid()),
    }
    let typed: T = serde_json::from_value(input).map_err(|_| Error::invalid())?;
    typed.validate()?;
    Ok(typed)
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
