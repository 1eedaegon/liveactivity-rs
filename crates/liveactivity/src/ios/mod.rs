//! The iOS backend: the commands of the contract on activitykit.
//!
//! It is generic over [`Kit`], which `activitykit::ActivityKit` implements on iOS (`crate::ios`) and a fake implements
//! in the tests, so that what the backend adds to activitykit runs on the host too: the records, the separation of the
//! two contract versions, and the progress and timer templates of contract v1, which become layout activities
//! ([`template`]). Revisions, repeated starts and lookups by id are activitykit's; the backend keeps the handles of the
//! activities it ended, so that ending one again still reaches activitykit after the system stops listing it.

mod backend;
mod kit;
pub(crate) mod template;

#[cfg(test)]
mod fake;
#[cfg(test)]
mod tests;

use crate::{
    Error, Result,
    event::{StartedEvent, StateEvent},
    layout::{
        ActionEvent, Alert, LayoutActivityRecord, LayoutEndInput, LayoutPush, LayoutSnapshot,
        LayoutStartInput, LayoutUpdateInput,
    },
    models::*,
    pushes::{PushTokenEvent, UpdatedEvent},
    validation::Validate,
};
use activitykit::{State, Values};
use std::{
    collections::VecDeque,
    future::Future,
    sync::{Mutex, PoisonError},
};

/// What holds a tap's intent, and with it an app that the system launched for the tap, until it is acknowledged.
pub(crate) trait Acknowledge: Send + 'static {
    fn acknowledge(&self);
}

/// What the backend takes from a kit's events: the taps with what holds their intent, and what the pushes bring.
pub(crate) enum KitEvent<H, A> {
    Tap(ActionEvent, A),
    /// A push token of the activity, new or changed, in lowercase hex.
    PushToken {
        activity: H,
        token: String,
    },
    /// The app's push-to-start token, new or changed, in lowercase hex.
    PushToStartToken {
        token: String,
    },
    /// A push delivered content to the activity, which its handle holds now.
    Updated(H),
    /// An activity began while the app ran: one the app started, or one the system or a push started.
    Started(H),
    /// ActivityKit reports a new lifecycle state of the activity.
    State(H, State),
    /// The whole authorization after the person changed it in Settings.
    Authorization(Authorization),
}

/// What the backend needs of activitykit's `ActivityKit`.
pub(crate) trait Kit: Send + Sync + 'static {
    type Handle: Handle;
    type Ack: Acknowledge;
    /// Every event of the kit from now on, until the kit ends: each tap of a button with what holds its intent, and
    /// what the pushes bring.
    fn events(&self) -> Box<dyn Iterator<Item = KitEvent<Self::Handle, Self::Ack>> + Send>;
    fn authorization(&self) -> Authorization;
    fn start(
        &self,
        start: StartSpec,
    ) -> impl Future<Output = activitykit::Result<Self::Handle>> + Send;
    /// The activity `id`, or `None` when ActivityKit does not know it.
    fn activity(
        &self,
        id: &str,
    ) -> impl Future<Output = activitykit::Result<Option<Self::Handle>>> + Send;
    fn activities(&self) -> impl Future<Output = activitykit::Result<Vec<Self::Handle>>> + Send;
    fn write_image(&self, name: &str, png: &[u8]) -> activitykit::Result<()>;
}

/// What the backend needs of activitykit's `Activity`. Times are epoch milliseconds.
pub(crate) trait Handle: Clone + Send + Sync + 'static {
    fn id(&self) -> &str;
    fn key(&self) -> &str;
    fn state(&self) -> State;
    fn values(&self) -> Values;
    fn revision(&self) -> Option<u64>;
    fn stale_at(&self) -> Option<u64>;
    fn timestamp(&self) -> Option<u64>;
    /// The relevance score of the last accepted content: 0 unless one was given.
    fn relevance(&self) -> f64;
    fn degraded_features(&self) -> Vec<String>;
    fn update(&self, update: UpdateSpec) -> impl Future<Output = activitykit::Result<()>> + Send;
    fn end(&self, end: EndSpec) -> impl Future<Output = activitykit::Result<()>> + Send;
}

/// activitykit's `Authorization`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Authorization {
    pub(crate) supported: bool,
    pub(crate) enabled: bool,
    pub(crate) buttons: bool,
    /// Supported, and frequent pushes are allowed for the app in Settings.
    pub(crate) frequent_pushes: bool,
}

/// A start: activitykit's `Start` with a key, a revision, a stale date, a timestamp, a relevance and what it asks
/// for of the pushes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StartSpec {
    pub(crate) key: String,
    pub(crate) layout: activitykit::Layout,
    pub(crate) values: Values,
    pub(crate) revision: u64,
    pub(crate) stale_at: Option<u64>,
    pub(crate) timestamp: Option<u64>,
    pub(crate) relevance: Option<f64>,
    pub(crate) push: Option<LayoutPush>,
}

/// An update: activitykit's `Update` at the caller's revision, with a stale date, a timestamp, a relevance and
/// an alert.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UpdateSpec {
    pub(crate) values: Values,
    pub(crate) revision: u64,
    pub(crate) stale_at: Option<u64>,
    pub(crate) timestamp: Option<u64>,
    pub(crate) relevance: Option<f64>,
    pub(crate) alert: Option<Alert>,
}

/// An end: activitykit's `End` with its policy and, when given, the final content, its revision and its relevance.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EndSpec {
    pub(crate) policy: EndPolicy,
    pub(crate) values: Option<Values>,
    pub(crate) revision: Option<u64>,
    pub(crate) stale_at: Option<u64>,
    pub(crate) timestamp: Option<u64>,
    pub(crate) relevance: Option<f64>,
}

/// Which contract version an activity belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Layout,
    Template(Template),
}

impl Kind {
    fn is_layout(self) -> bool {
        self == Self::Layout
    }

    fn is_template(self) -> bool {
        matches!(self, Self::Template(_))
    }
}

/// How many ended activities the backend keeps a handle of.
const ENDED_KEPT: usize = 64;

pub(crate) struct Apple<K: Kit> {
    kit: K,
    /// The handles of the last [`ENDED_KEPT`] activities that this launch ended, oldest first. activitykit finds an
    /// id through a handle alive in the process, else through the system's list, which an activity that ended at once
    /// soon leaves: a kept handle lets a repeated end reach activitykit's record of it, and a late update its refusal.
    ended: Mutex<VecDeque<K::Handle>>,
}

fn not_found() -> Error {
    Error::new("NOT_FOUND", "No active activity has this handle.")
}

fn state(state: State) -> ActivityState {
    match state {
        State::Pending => ActivityState::Pending,
        State::Active => ActivityState::Active,
        State::Stale => ActivityState::Stale,
        State::Ended => ActivityState::Ended,
        State::Dismissed => ActivityState::Dismissed,
        _ => ActivityState::Unknown,
    }
}

fn kind<H: Handle>(handle: &H) -> Kind {
    template::of(&handle.values()).map_or(Kind::Layout, Kind::Template)
}

/// The tap of a button of `handle` as the event sink gets it.
pub(crate) fn action_event<H: Handle>(handle: &H, action: String) -> ActionEvent {
    let key = handle.key();
    ActionEvent {
        action_id: action,
        activity_id: handle.id().to_owned(),
        activity_key: (!key.is_empty()).then(|| key.to_owned()),
    }
}

/// A push token of `handle` as the event sink gets it.
pub(crate) fn push_token_event<H: Handle>(handle: &H, token: String) -> PushTokenEvent {
    let key = handle.key();
    PushTokenEvent {
        activity_id: handle.id().to_owned(),
        activity_key: (!key.is_empty()).then(|| key.to_owned()),
        token,
    }
}

/// Content a push delivered to `handle` as the event sink gets it: the record of a layout activity. An activity of
/// contract v1 asks for no push, and nothing is reported of it.
pub(crate) fn updated_event<H: Handle>(handle: &H) -> Option<UpdatedEvent> {
    kind(handle).is_layout().then(|| UpdatedEvent {
        record: layout_record(handle),
    })
}

/// The start of a layout activity as the event sink gets it. Nothing is reported of an activity of contract v1,
/// whose start answered its record.
pub(crate) fn started_event<H: Handle>(handle: &H) -> Option<StartedEvent> {
    kind(handle).is_layout().then(|| StartedEvent {
        record: layout_record(handle),
    })
}

/// A new lifecycle state of `handle` as the event sink gets it.
pub(crate) fn state_event<H: Handle>(handle: &H, new: State) -> StateEvent {
    let key = handle.key();
    StateEvent {
        activity_id: handle.id().to_owned(),
        activity_key: (!key.is_empty()).then(|| key.to_owned()),
        state: state(new),
    }
}

/// What `authorization` allows. Live Activities are a switch in Settings: there is no prompt, so a request
/// reports `settingsRequired`.
pub(crate) fn permissions_of(authorization: Authorization, request: bool) -> Permissions {
    let live_activities = match (authorization.supported, authorization.enabled, request) {
        (false, _, _) => PermissionState::NotApplicable,
        (true, true, _) => PermissionState::Granted,
        (true, false, true) => PermissionState::SettingsRequired,
        (true, false, false) => PermissionState::Denied,
    };
    let frequent_pushes = match (authorization.supported, authorization.frequent_pushes) {
        (false, _) => PermissionState::NotApplicable,
        (true, true) => PermissionState::Granted,
        (true, false) => PermissionState::Denied,
    };
    Permissions {
        notifications: PermissionState::NotApplicable,
        live_activities,
        promotion: PermissionState::NotApplicable,
        frequent_pushes,
    }
}

fn layout_record<H: Handle>(handle: &H) -> LayoutActivityRecord {
    // The system's score without one is 0, which a record does not repeat.
    let relevance = handle.relevance();
    LayoutActivityRecord {
        id: handle.id().to_owned(),
        activity_key: handle.key().to_owned(),
        schema_version: 2,
        snapshot: LayoutSnapshot {
            revision: handle.revision().unwrap_or(0),
            values: handle.values().into_inner(),
            stale_at: handle.stale_at(),
            timestamp: handle.timestamp(),
            relevance: (relevance != 0.0).then_some(relevance),
            // iOS holds no Android content.
            android: None,
        },
        backend: Backend::ActivityKit,
        state: state(handle.state()),
        visibility: Visibility::Unknown,
        degraded_features: handle.degraded_features(),
        promotion_requested: false,
        promotion_state: PromotionState::NotApplicable,
    }
}

fn template_record<H: Handle>(handle: &H) -> Option<ActivityRecord> {
    let revision = handle.revision().unwrap_or(0);
    let (template, snapshot) = template::snapshot(revision, &handle.values(), handle.stale_at())?;
    Some(ActivityRecord {
        id: handle.id().to_owned(),
        activity_key: handle.key().to_owned(),
        schema_version: 1,
        template,
        snapshot,
        backend: Backend::ActivityKit,
        state: state(handle.state()),
        visibility: Visibility::Unknown,
        degraded_features: handle.degraded_features(),
        promotion_requested: false,
        promotion_state: PromotionState::NotApplicable,
    })
}

/// The start that a layout request asks for.
pub(crate) fn start_spec(request: LayoutStartInput) -> StartSpec {
    let layout = activitykit::Layout {
        lock_screen: request.layout.lock_screen,
        island: request.layout.island,
        background: request.layout.background,
        keyline: request.layout.keyline,
        url: request.layout.url,
        styles: request.styles.unwrap_or_default(),
    };
    StartSpec {
        key: request.activity_key,
        layout,
        values: request.snapshot.values.into(),
        revision: request.snapshot.revision,
        stale_at: request.snapshot.stale_at,
        timestamp: request.snapshot.timestamp,
        relevance: request.snapshot.relevance,
        push: request.push,
    }
}

fn update_spec(snapshot: LayoutSnapshot, alert: Option<Alert>) -> UpdateSpec {
    UpdateSpec {
        values: snapshot.values.into(),
        revision: snapshot.revision,
        stale_at: snapshot.stale_at,
        timestamp: snapshot.timestamp,
        relevance: snapshot.relevance,
        alert,
    }
}

fn end_spec(policy: EndPolicy, snapshot: Option<LayoutSnapshot>) -> EndSpec {
    match snapshot {
        Some(snapshot) => EndSpec {
            policy,
            values: Some(snapshot.values.into()),
            revision: Some(snapshot.revision),
            stale_at: snapshot.stale_at,
            timestamp: snapshot.timestamp,
            relevance: snapshot.relevance,
        },
        None => EndSpec {
            policy,
            values: None,
            revision: None,
            stale_at: None,
            timestamp: None,
            relevance: None,
        },
    }
}

impl<K: Kit> Apple<K> {
    pub(crate) fn new(kit: K) -> Self {
        Self {
            kit,
            ended: Mutex::default(),
        }
    }

    pub(crate) fn capabilities(&self) -> Capabilities {
        let authorization = self.kit.authorization();
        let support = |yes: bool| {
            if yes {
                FeatureSupport::Supported
            } else {
                FeatureSupport::Unsupported
            }
        };
        let supported = support(authorization.supported);
        Capabilities {
            platform: "ios".into(),
            backend: if authorization.supported {
                Backend::ActivityKit
            } else {
                Backend::Unsupported
            },
            start: supported,
            update: supported,
            end: supported,
            progress: supported,
            timer: supported,
            actions: support(authorization.supported && authorization.buttons),
            state_observation: supported,
            silent_update: supported,
            native_live_activity: supported,
            max_payload_bytes: 4096,
            layout: supported,
            push: supported,
            android_presentation: None,
        }
    }

    pub(crate) fn permissions(&self, request: bool) -> Permissions {
        permissions_of(self.kit.authorization(), request)
    }

    pub(crate) fn request_permissions(&self, kind: PermissionKind) -> Result<Permissions> {
        match kind {
            PermissionKind::LiveActivities => Ok(self.permissions(true)),
            _ => Err(Error::new(
                "UNSUPPORTED_FEATURE",
                "This backend only exposes Live Activity settings.",
            )),
        }
    }

    pub(crate) fn write_image(&self, name: &str, png: &[u8]) -> Result<()> {
        crate::image::check_image(name, png)?;
        Ok(self.kit.write_image(name, png)?)
    }

    pub(crate) async fn start_layout(
        &self,
        input: LayoutStartInput,
    ) -> Result<LayoutActivityRecord> {
        input.validate()?;
        Ok(layout_record(&self.start(input).await?))
    }

    pub(crate) async fn update_layout(
        &self,
        input: LayoutUpdateInput,
    ) -> Result<LayoutActivityRecord> {
        input.validate()?;
        let handle = self.find(&input.id, Kind::is_layout).await?;
        handle
            .update(update_spec(input.snapshot, input.alert))
            .await?;
        Ok(layout_record(&handle))
    }

    pub(crate) async fn end_layout(&self, input: LayoutEndInput) -> Result<LayoutActivityRecord> {
        input.validate()?;
        let handle = self.find(&input.id, Kind::is_layout).await?;
        handle.end(end_spec(input.policy, input.snapshot)).await?;
        self.keep_ended(&handle);
        Ok(layout_record(&handle))
    }

    pub(crate) async fn list_layout(&self) -> Result<Vec<LayoutActivityRecord>> {
        let handles = self.kit.activities().await?;
        Ok(handles
            .iter()
            .filter(|handle| kind(*handle).is_layout())
            .map(layout_record)
            .collect())
    }

    pub(crate) async fn start_template(&self, input: StartInput) -> Result<ActivityRecord> {
        input.validate()?;
        let request = template::start(&input);
        request.validate_contract()?;
        let handle = self.start(request).await?;
        template_record(&handle).ok_or_else(Error::native)
    }

    pub(crate) async fn update_template(&self, input: UpdateInput) -> Result<ActivityRecord> {
        input.validate()?;
        let (template, snapshot) = template::layout_snapshot(&input.snapshot);
        let handle = self.find(&input.id, Kind::is_template).await?;
        // The other template's fields would change the keys of the start.
        if kind(&handle) != Kind::Template(template) {
            return Err(Error::invalid());
        }
        handle.update(update_spec(snapshot, None)).await?;
        template_record(&handle).ok_or_else(Error::native)
    }

    pub(crate) async fn end_template(&self, input: EndInput) -> Result<ActivityRecord> {
        input.validate()?;
        let handle = self.find(&input.id, Kind::is_template).await?;
        handle.end(end_spec(input.policy, None)).await?;
        self.keep_ended(&handle);
        template_record(&handle).ok_or_else(Error::native)
    }

    pub(crate) async fn list_templates(&self) -> Result<Vec<ActivityRecord>> {
        let handles = self.kit.activities().await?;
        Ok(handles.iter().filter_map(template_record).collect())
    }

    async fn start(&self, request: LayoutStartInput) -> Result<K::Handle> {
        Ok(self.kit.start(start_spec(request)).await?)
    }

    fn keep_ended(&self, handle: &K::Handle) {
        let mut ended = self.ended.lock().unwrap_or_else(PoisonError::into_inner);
        if ended.iter().any(|kept| kept.id() == handle.id()) {
            return;
        }
        if ended.len() == ENDED_KEPT {
            ended.pop_front();
        }
        ended.push_back(handle.clone());
    }

    /// The activity `id`, when activitykit knows it and it belongs to the contract version that `wanted` takes.
    async fn find(&self, id: &str, wanted: fn(Kind) -> bool) -> Result<K::Handle> {
        match self.kit.activity(id).await? {
            Some(handle) if wanted(kind(&handle)) => Ok(handle),
            _ => Err(not_found()),
        }
    }
}

pub use backend::ActivityKitBackend;
