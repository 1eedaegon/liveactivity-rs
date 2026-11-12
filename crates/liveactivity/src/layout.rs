//! Request contract v2: a layout tree, a style table and a map of values.
//!
//! The requests and records are this crate's own, and so is what they say beside the layout: the options of each
//! platform, the relevance, the alert of an update and the Android content of a snapshot. The vocabulary of each
//! presentation belongs to the engine that draws it: the node and style types and their validator are the
//! `activitykit` crate's, and the notification styles of an Android content are the `liveupdate` crate's, both
//! re-exported here. A request is read in this crate's types, and the part activitykit knows is converted into its
//! request to be checked.

use crate::{
    Error, Result,
    models::{ActivityState, Backend, EndPolicy, Fallback, PromotionState, Visibility},
    validation::{Validate, require, text},
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
    Alignment, Bindable, Bound, ButtonStyle, Color, ColorName, ContentMode, Dimension, Element,
    Font, FontDesign, FontWeight, Frame, HorizontalAlignment, ImageSource, IndicatorKind, Node,
    Padding, Style, SymbolRendering, VerticalAlignment,
};

/// Android's notification styles, for the [`AndroidContent`] of a snapshot. A [`TimerStyle`] takes liveupdate's
/// `Timer`, which is `liveactivity::liveupdate::Timer`.
pub use liveupdate::{
    AndroidPresentation, BigTextStyle, IconSource, Metric, MetricStyle, MetricValue,
    NotificationAction, PresentationCapabilities, PresentationStyle, ProgressPoint,
    ProgressSegment, ProgressStyle, SamsungNowBar, SemanticStyle, TimerStyle,
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

/// Options of a start that only Android reads. iOS ignores them. More may come: build it with
/// [`AndroidOptions::new`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AndroidOptions {
    /// Lock screen visibility of the notification.
    pub visibility: AndroidVisibility,
}

impl AndroidOptions {
    /// The options with this lock screen visibility.
    pub fn new(visibility: AndroidVisibility) -> Self {
        Self { visibility }
    }
}

/// What Android shows for a snapshot, in Android's own terms, in place of the projection of the layout: a
/// progress with segments and points, metrics, the text of the status chip, icons of actions. iOS ignores it.
///
/// Android keeps the notification style of an activity's first snapshot: a later content of another style is
/// `UNSUPPORTED_FEATURE` there, so an activity gives its content with every snapshot or with none. A tap on one
/// of its actions reaches the event sink like a tap on a button node.
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

/// An alert that comes with an update: on iOS the device lights up, the Dynamic Island expands (or, without one,
/// the Lock Screen presentation shows as a banner), and an Apple Watch shows the title and the body. Android
/// updates silently and reports `alert` in the record's `degradedFeatures`. More may come, such as a sound: build
/// it with [`Alert::new`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Alert {
    /// The alert's title, 1 to 120 characters.
    pub title: String,
    /// The alert's body, 1 to 240 characters.
    pub body: String,
}

impl Alert {
    /// An alert with this title and body.
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
        }
    }
}

/// The member `layout` of a start: one tree per region and the two tints. The layout is fixed when the activity
/// starts; only a bound `background` or `keyline` follows the values.
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
    /// One tree per region and the two tints, fixed at start.
    pub layout: Layout,
    /// Styles by token; nodes name them with `style`.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub styles: Option<BTreeMap<String, Style>>,
    /// The values, with their revision.
    pub snapshot: LayoutSnapshot,
    /// Options that only Android reads.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub android: Option<AndroidOptions>,
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

/// A tap on a button node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionEvent {
    /// The id of the tapped button, as its node declares it.
    pub action_id: String,
    /// Handle of the activity that showed the button.
    pub activity_id: String,
    /// Absent when the backend no longer knows that activity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_key: Option<String>,
}

impl LayoutSnapshot {
    fn wire(&self) -> wire::LayoutSnapshot {
        wire::LayoutSnapshot {
            revision: self.revision,
            values: self.values.clone().into(),
            stale_at: self.stale_at,
            timestamp: self.timestamp,
        }
    }
}

impl LayoutStartInput {
    /// The same request in activitykit's types.
    pub(crate) fn wire(&self) -> wire::LayoutStartInput {
        wire::LayoutStartInput {
            activity_key: self.activity_key.clone(),
            schema_version: self.schema_version,
            fallback: match self.fallback {
                Fallback::None => wire::Fallback::None,
                Fallback::Notification => wire::Fallback::Notification,
            },
            layout: self.layout.clone(),
            styles: self.styles.clone(),
            snapshot: self.snapshot.wire(),
            // What only Android reads is this crate's: activitykit is not asked about it.
            android: None,
            push: self.push.clone(),
        }
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
        wire::LayoutUpdateInput {
            id: self.id.clone(),
            snapshot: self.snapshot.wire(),
        }
    }
}

impl LayoutEndInput {
    fn wire(&self) -> wire::EndInput {
        wire::EndInput {
            id: self.id.clone(),
            policy: match self.policy {
                EndPolicy::Immediate {} => wire::EndPolicy::Immediate {},
                EndPolicy::Default {} => wire::EndPolicy::Default {},
                EndPolicy::After { at } => wire::EndPolicy::After { at },
            },
            snapshot: self.snapshot.as_ref().map(LayoutSnapshot::wire),
        }
    }
}

impl AndroidContent {
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
        require(text(&self.title, 120) && text(&self.body, 240))
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

impl Validate for LayoutStartInput {
    fn validate(&self) -> Result<()> {
        self.validate_contract()?;
        self.snapshot.beside_the_layout()?;
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

/// Whether `sent` is `encoded` but for members at liveupdate's defaults, which liveupdate leaves out when it
/// writes a presentation: `false` and an empty array. A `null` is no default, and an array is no object.
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
                None => matches!(sent, Bool(false)) || sent.as_array().is_some_and(Vec::is_empty),
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
mod tests {
    use super::*;
    use serde_json::json;

    fn start() -> serde_json::Value {
        json!({"activityKey":"demo:1","schemaVersion":2,"fallback":"notification",
            "layout":{"lockScreen":{"type":"text","bind":"status","style":"s"},"keyline":{"bind":"tone"}},
            "styles":{"s":{"color":{"bind":"tone"},"font":"headline"}},
            "snapshot":{"revision":0,"values":{"status":"On time","tone":"green"},"staleAt":5,"timestamp":7},
            "android":{"visibility":"private"}})
    }
    #[test]
    fn a_request_reads_into_the_contracts_types_and_writes_back_the_same_json() {
        let input: LayoutStartInput = parse("start_layout", start()).unwrap();
        assert_eq!(serde_json::to_value(&input).unwrap(), start());
        assert_eq!(input.snapshot.values["tone"], Value::Text("green".into()));
        assert_eq!(input.snapshot.timestamp, Some(7));
        // activitykit is shown the request without what only Android reads.
        let mut shown = start();
        shown.as_object_mut().unwrap().remove("android");
        assert_eq!(serde_json::to_value(input.wire()).unwrap(), shown);
        let end = json!({"id":"a","policy":{"kind":"after","at":3},"snapshot":{"revision":1,"values":{}}});
        let typed: LayoutEndInput = parse("end_layout", end.clone()).unwrap();
        assert_eq!(serde_json::to_value(&typed).unwrap(), end);
        assert_eq!(serde_json::to_value(typed.wire()).unwrap(), end);
        let plain: LayoutEndInput =
            parse("end_layout", json!({"id":"a","policy":{"kind":"default"}})).unwrap();
        assert_eq!(plain.snapshot, None);
        assert_eq!(
            serde_json::to_value(plain.wire()).unwrap(),
            json!({"id":"a","policy":{"kind":"default"}})
        );
        let immediate = LayoutEndInput {
            id: "a".into(),
            policy: EndPolicy::Immediate {},
            snapshot: None,
        };
        assert_eq!(
            serde_json::to_value(immediate.wire()).unwrap()["policy"]["kind"],
            "immediate"
        );
    }
    #[test]
    fn parse_is_activitykits_strict_check_and_the_types_validate_the_same() {
        let mut nulled = start();
        nulled["snapshot"]["timestamp"] = json!(null);
        assert_eq!(
            parse::<LayoutStartInput>("start_layout", nulled),
            Err(Error::invalid())
        );
        let mut unknown = start();
        unknown["extra"] = json!(1);
        assert_eq!(
            parse::<LayoutStartInput>("start_layout", unknown),
            Err(Error::invalid())
        );
        // Valid for activitykit, but not this type.
        assert_eq!(
            parse::<LayoutStartInput>("end_layout", json!({"id":"a","policy":{"kind":"default"}})),
            Err(Error::invalid())
        );
        let input: LayoutStartInput = serde_json::from_value(start()).unwrap();
        assert_eq!(input.validate(), Ok(()));
        assert_eq!(input.snapshot.validate(), Ok(()));
        let mut late = input.snapshot.clone();
        late.timestamp = Some(8_640_000_000_000_001);
        assert_eq!(late.validate(), Err(Error::invalid()));
        let update = LayoutUpdateInput::new("a", input.snapshot.clone());
        assert_eq!(
            (update.validate(), input.validate_update(&update)),
            (Ok(()), Ok(()))
        );
        let mut crimson = update.clone();
        crimson
            .snapshot
            .values
            .insert("tone".into(), Value::Text("crimson".into()));
        assert_eq!(input.validate_update(&crimson), Err(Error::invalid()));
        let end = LayoutEndInput {
            id: "a".into(),
            policy: EndPolicy::Default {},
            snapshot: Some(crimson.snapshot),
        };
        assert_eq!(
            (end.validate(), input.validate_end(&end)),
            (Ok(()), Err(Error::invalid()))
        );
        let bare = LayoutEndInput {
            snapshot: None,
            ..end
        };
        assert_eq!(input.validate_end(&bare), Ok(()));
    }
    fn code<T>(result: Result<T>) -> Option<String> {
        result.err().map(|error| error.code)
    }

    #[test]
    fn req_c_005_a_request_is_read_strictly_in_this_crates_types() {
        // REQ-C-005
        // What activitykit's contract does not carry is taken, and reads back as the same JSON.
        let mut rich = start();
        rich["snapshot"]["relevance"] = json!(80);
        rich["snapshot"]["android"] = json!({"title":"On time","status":"Gate 4","presentation":{
            "style":{"kind":"progress","value":{"progress":40,"styledByProgress":true,
                "segments":[{"length":60},{"length":40}],"points":[{"position":60}]}},
            "shortCriticalText":"4 min"}});
        let input: LayoutStartInput = parse("start_layout", rich.clone()).unwrap();
        assert_eq!(input.snapshot.relevance, Some(80.0));
        let content = input.snapshot.android.as_ref().unwrap();
        assert_eq!(
            content.presentation.short_critical_text.as_deref(),
            Some("4 min")
        );
        assert!(same(&serde_json::to_value(&input).unwrap(), &rich));
        // The presentation is read as liveupdate reads it: a default may be written out.
        let mut spelled = rich.clone();
        spelled["snapshot"]["android"]["presentation"]["style"]["value"]["indeterminate"] =
            json!(false);
        spelled["snapshot"]["android"]["presentation"]["actions"] = json!([]);
        let same_input: LayoutStartInput = parse("start_layout", spelled).unwrap();
        assert_eq!(same_input, input);
        let update = json!({"id":"a","snapshot":{"revision":1,"values":{"status":"Late","tone":"red"}},
            "alert":{"title":"Gate changed","body":"Now gate 12"}});
        let typed: LayoutUpdateInput = parse("update_layout", update.clone()).unwrap();
        assert_eq!(typed.alert.as_ref().unwrap().title, "Gate changed");
        assert_eq!(serde_json::to_value(&typed).unwrap(), update);

        // Strictly: no unknown member at any level, no `null`, nothing that serde would correct.
        let refused = |edit: fn(&mut serde_json::Value)| {
            let mut request = start();
            edit(&mut request);
            code(parse::<LayoutStartInput>("start_layout", request))
        };
        let invalid = Some("INVALID_INPUT".to_owned());
        assert_eq!(refused(|request| request["ios"] = json!({})), invalid);
        assert_eq!(
            refused(|request| request["snapshot"]["extra"] = json!(1)),
            invalid
        );
        assert_eq!(
            refused(|request| request["android"]["presentation"] = json!({})),
            invalid
        );
        assert_eq!(
            refused(|request| {
                request["snapshot"]["android"] = json!({"title":"On time","status":"Gate 4",
                    "presentation":{"style":{"kind":"standard"},"sound":"loud"}});
            }),
            invalid
        );
        assert_eq!(
            refused(|request| request["snapshot"]["relevance"] = json!(null)),
            invalid
        );
        assert_eq!(
            refused(|request| request["snapshot"]["android"] = json!(null)),
            invalid
        );
        assert_eq!(
            refused(|request| request["layout"]["lockScreen"]["surprise"] = json!(true)),
            invalid
        );
        // The presentation is liveupdate's, and as strict: a default may be written out, but a `null` is no
        // default and an array is no object.
        assert_eq!(
            refused(|request| {
                request["snapshot"]["android"] = json!({"title":"On time","status":"Gate 4",
                    "presentation":{"style":{"kind":"standard"},"largeIcon":null}});
            }),
            invalid
        );
        assert_eq!(
            refused(|request| {
                request["snapshot"]["android"] = json!({"title":"On time","status":"Gate 4",
                    "presentation":[{"kind":"standard"},"4 min",null,[],null]});
            }),
            invalid
        );
        assert_eq!(
            refused(|request| {
                request["snapshot"]["android"] = json!({"title":"On time","status":"Gate 4",
                    "presentation":{"style":{"kind":"standard"},"actions":[],
                        "shortCriticalText":"4 min"}});
            }),
            None
        );
        // A struct that serde also reads from an array does not encode back to the array.
        assert_eq!(
            refused(|request| request["android"] = json!(["private"])),
            invalid
        );
        // JSON nested deeper than a request can be is refused unread.
        let deep = (0..40).fold(json!(1), |inner, _| json!([inner]));
        assert_eq!(
            refused(|request| request["snapshot"]["values"]["status"] = json!("x")),
            None
        );
        let mut nested = start();
        nested["snapshot"]["values"]["status"] = deep;
        assert_eq!(
            code(parse::<LayoutStartInput>("start_layout", nested)),
            invalid
        );

        // A request is read as the request of its command: a start is no update, whatever the caller's type.
        assert_eq!(
            code(parse::<LayoutStartInput>("update_layout", start())),
            invalid
        );
        assert_eq!(code(parse::<LayoutStartInput>("start", start())), invalid);
    }

    #[test]
    fn req_c_006_what_a_request_says_beside_the_layout_is_checked() {
        // REQ-C-006
        let invalid = Some("INVALID_INPUT".to_owned());
        let input: LayoutStartInput = serde_json::from_value(start()).unwrap();
        let with = |edit: fn(&mut LayoutSnapshot)| {
            let mut snapshot = input.snapshot.clone();
            edit(&mut snapshot);
            snapshot
        };
        let content = |title: &str, chip: &str| AndroidContent {
            title: title.into(),
            status: "Gate 4".into(),
            presentation: AndroidPresentation {
                style: PresentationStyle::Standard,
                short_critical_text: Some(chip.into()),
                large_icon: None,
                actions: Vec::new(),
                samsung_now_bar: None,
            },
        };

        // A relevance is a number.
        assert_eq!(code(with(|s| s.relevance = Some(-3.5)).validate()), None);
        assert_eq!(
            code(with(|s| s.relevance = Some(f64::NAN)).validate()),
            invalid
        );
        assert_eq!(
            code(with(|s| s.relevance = Some(f64::NEG_INFINITY)).validate()),
            invalid
        );
        // An Android content is one liveupdate takes: here a title, and a chip text of at most 7 characters.
        let good = LayoutSnapshot {
            android: Some(content("On time", "4 min")),
            ..input.snapshot.clone()
        };
        assert_eq!(code(good.validate()), None);
        for wrong in [content(" ", "4 min"), content("On time", "four minutes")] {
            let snapshot = LayoutSnapshot {
                android: Some(wrong),
                ..input.snapshot.clone()
            };
            assert_eq!(code(snapshot.validate()), invalid);
            // In a start, an update and an end alike.
            let start = LayoutStartInput {
                snapshot: snapshot.clone(),
                ..input.clone()
            };
            assert_eq!(code(start.validate()), invalid);
            assert_eq!(
                code(LayoutUpdateInput::new("a", snapshot.clone()).validate()),
                invalid
            );
            let end = LayoutEndInput {
                id: "a".into(),
                policy: EndPolicy::Default {},
                snapshot: Some(snapshot),
            };
            assert_eq!(code(end.validate()), invalid);
        }
        // An alert has a title and a body within their lengths.
        let alert = |title: &str, body: &str| LayoutUpdateInput {
            alert: Some(Alert {
                title: title.into(),
                body: body.into(),
            }),
            ..LayoutUpdateInput::new("a", input.snapshot.clone())
        };
        assert_eq!(code(alert("Gate changed", "Now gate 12").validate()), None);
        assert_eq!(code(alert("", "Now gate 12").validate()), invalid);
        assert_eq!(code(alert("Gate changed", " ").validate()), invalid);
        assert_eq!(code(alert(&"a".repeat(121), "b").validate()), invalid);
        assert_eq!(code(alert("a", &"b".repeat(241)).validate()), invalid);
    }

    #[test]
    fn the_template_marker_is_no_value_key_of_a_layout_request() {
        let code = |result: Result<()>| result.map_err(|error| error.code);
        let invalid = Err("INVALID_INPUT".to_owned());
        let mut marked = start();
        marked["snapshot"]["values"]["liveActivityTemplate"] = json!("progress");
        // The commands' check, and the Rust API's.
        assert_eq!(
            code(crate::validate_request("start_layout", marked.clone())),
            invalid
        );
        let typed: LayoutStartInput = serde_json::from_value(marked).unwrap();
        assert_eq!(code(typed.validate()), invalid);
        let update =
            json!({"id":"a","snapshot":{"revision":1,"values":{"liveActivityTemplate":"timer"}}});
        assert_eq!(
            code(crate::validate_request("update_layout", update.clone())),
            invalid
        );
        let typed: LayoutUpdateInput = serde_json::from_value(update).unwrap();
        assert_eq!(code(typed.validate()), invalid);
        let end = json!({"id":"a","policy":{"kind":"immediate"},
            "snapshot":{"revision":1,"values":{"liveActivityTemplate":"timer"}}});
        assert_eq!(
            code(crate::validate_request("end_layout", end.clone())),
            invalid
        );
        let typed: LayoutEndInput = serde_json::from_value(end).unwrap();
        assert_eq!(code(typed.validate()), invalid);
        // Only that key: a longer one is a value like any other, and an end without content has none.
        let mut similar = start();
        similar["snapshot"]["values"]["liveActivityTemplates"] = json!("progress");
        assert_eq!(
            code(crate::validate_request("start_layout", similar)),
            Ok(())
        );
        let bare = json!({"id":"a","policy":{"kind":"immediate"}});
        assert_eq!(code(crate::validate_request("end_layout", bare)), Ok(()));
    }
}
