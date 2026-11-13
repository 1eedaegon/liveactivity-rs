//! Layout activities on Android, on liveupdate: liveupdate keeps the lifecycle, the receipts and the notification
//! of each schema-v2 request, which is a snapshot's Android content as given or else the projection of the layout
//! by `android_layout`, and a store of the backend keeps what liveupdate does not, the layout request and its
//! latest snapshot, under liveupdate's id. Taps and dismissals come from liveupdate's events. What only some
//! devices do (another style in an update, an update that alerts, a last content that stays) is asked of
//! liveupdate's capabilities when a request needs it.

use super::{
    convert as android_liveupdate,
    layout::{self as android_layout, Environment},
};
use crate::{
    ActionEvent, ActivityState, Backend, EndPolicy, Error, Event, LayoutActivityRecord,
    LayoutEndInput, LayoutSnapshot, LayoutStartInput, LayoutUpdateInput, Result, StateEvent,
    Visibility,
    layout::{AndroidVisibility, Element, Node},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io::Read, path::PathBuf, sync::Mutex};

/// liveupdate draws app files of at most this many pixels a side and bytes.
const MAX_ICON_SIDE: u32 = 1024;
const MAX_ICON_BYTES: u64 = 1_048_576;
/// The store's entry of a start before liveupdate has answered, under the activity key.
const PENDING: &str = "pending:";
/// What a record names when an update asked for an alert on a device that alerts once.
const ALERT: &str = "alert";
/// How many action ids of its Android contents an activity keeps.
const ACTIONS_KEPT: usize = 32;

/// What the backend keeps of a layout activity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Entry {
    pub(crate) origin: LayoutStartInput,
    pub(crate) snapshot: LayoutSnapshot,
    /// The projection's `degradedFeatures`.
    pub(crate) degraded: Vec<String>,
    /// The action ids of the Android contents the activity has shown, the last [`ACTIONS_KEPT`], oldest first:
    /// Android stores a tap, also while the application does not run, so one may come after a later content
    /// replaced its action.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) actions: Vec<String>,
}

/// The entries by liveupdate id, and by `pending:<key>` while a start is under way.
pub(crate) type Entries = BTreeMap<String, Entry>;

/// Durable storage of the entries.
pub(crate) trait Store: Send + Sync {
    fn load(&self) -> Result<Entries>;
    fn save(&self, entries: &Entries) -> Result<()>;
}

/// The entries as one JSON file, replaced whole.
pub(crate) struct FileStore {
    pub(crate) path: PathBuf,
}

impl Store for FileStore {
    fn load(&self) -> Result<Entries> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| Error::native()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Entries::new()),
            Err(_) => Err(Error::native()),
        }
    }

    fn save(&self, entries: &Entries) -> Result<()> {
        let bytes = serde_json::to_vec(entries).map_err(|_| Error::native())?;
        let written = self.path.with_extension("json.new");
        if let Some(folder) = self.path.parent() {
            fs::create_dir_all(folder).map_err(|_| Error::native())?;
        }
        fs::write(&written, bytes)
            .and_then(|()| fs::rename(&written, &self.path))
            .map_err(|_| Error::native())
    }
}

/// The app around the projection: its name, the Now Bar option and the app's files folder.
pub(crate) struct Settings {
    pub(crate) app_name: String,
    pub(crate) samsung_now_bar: bool,
    pub(crate) files: Option<PathBuf>,
    /// Where `write_image` puts pictures, under the app's files.
    pub(crate) image_folder: String,
}

pub(crate) struct Layouts<D: liveupdate::Driver, S: Store> {
    settings: Settings,
    store: S,
    /// The store's entries change only under this lock, around the liveupdate call they follow.
    lock: Mutex<()>,
    _driver: std::marker::PhantomData<D>,
}

impl<D: liveupdate::Driver, S: Store> Layouts<D, S> {
    pub(crate) fn new(settings: Settings, store: S) -> Self {
        Self {
            settings,
            store,
            lock: Mutex::new(()),
            _driver: std::marker::PhantomData,
        }
    }

    pub(crate) fn start(
        &self,
        updates: &liveupdate::LiveUpdate<D>,
        input: LayoutStartInput,
    ) -> Result<LayoutActivityRecord> {
        // Android has no pushes and no relevance: what the start says of them is dropped here, so that it is no
        // part of the request the activity keeps and a repeated start is compared without it.
        let input = LayoutStartInput {
            push: None,
            snapshot: for_android(input.snapshot),
            ..input
        };
        let _guard = self.lock.lock().map_err(|_| Error::native())?;
        let (mut entries, records) = self.reconciled(updates)?;
        // liveupdate compares the projections; two starts that project the same may still differ.
        let conflicting = records
            .iter()
            .filter(|record| running(record))
            .any(|record| {
                entries.get(&record.id).is_some_and(|entry| {
                    entry.origin.activity_key == input.activity_key && entry.origin != input
                })
            });
        if conflicting {
            return Err(Error::new(
                "CONFLICT",
                "a running activity of this key started with another request",
            ));
        }
        // The style of the first snapshot: its content's, or else the one the layout projects to.
        let mut offered = Offered::of(updates);
        let projection = self.sent(&mut offered, &input, &input.snapshot, false)?;
        let template = projection
            .snapshot
            .template()
            .map_err(android_liveupdate::error)?;
        let request = liveupdate::StartRequest {
            activity_key: input.activity_key.clone(),
            schema_version: 2,
            template,
            fallback: android_liveupdate::fallback(input.fallback),
            visibility: visibility(&input),
            snapshot: projection.snapshot,
            placement: liveupdate::Placement::Tagged,
        };
        let pending = format!("{PENDING}{}", input.activity_key);
        let entry = Entry {
            origin: input.clone(),
            snapshot: input.snapshot.clone(),
            degraded: projection.degraded,
            actions: with_actions(Vec::new(), &input.snapshot),
        };
        entries.insert(pending.clone(), entry.clone());
        self.store.save(&entries)?;
        // A failure leaves the pending entry: the next reconciliation adopts it when liveupdate posted the activity
        // after all (an answer lost on the way back), and drops it otherwise, as after a content that Android
        // refused to post, of which liveupdate keeps no receipt.
        let record = updates.start(request).map_err(android_liveupdate::error)?;
        entries.remove(&pending);
        // The same start again answers the running activity, whose entry has the latest values.
        let entry = entries.entry(record.id.clone()).or_insert(entry).clone();
        self.store.save(&entries)?;
        Ok(layout_record(&record, &entry))
    }

    pub(crate) fn update(
        &self,
        updates: &liveupdate::LiveUpdate<D>,
        input: LayoutUpdateInput,
    ) -> Result<LayoutActivityRecord> {
        let _guard = self.lock.lock().map_err(|_| Error::native())?;
        let (mut entries, records) = self.reconciled(updates)?;
        let alert = input.alert.is_some();
        let snapshot = for_android(input.snapshot);
        let record = self.replace(updates, &mut entries, &records, &input.id, snapshot, alert)?;
        let entry = entries.get(&input.id).ok_or_else(not_found)?;
        Ok(layout_record(&record, entry))
    }

    pub(crate) fn end(
        &self,
        updates: &liveupdate::LiveUpdate<D>,
        input: LayoutEndInput,
    ) -> Result<LayoutActivityRecord> {
        let _guard = self.lock.lock().map_err(|_| Error::native())?;
        let (mut entries, records) = self.reconciled(updates)?;
        let entry = entries.get(&input.id).ok_or_else(not_found)?;
        // An activity that is over takes no content: its end again answers its record, and an end without a time
        // removes a last content that an earlier end kept.
        let live = records
            .iter()
            .any(|record| record.id == input.id && running(record));
        let last = input.snapshot.filter(|_| live).map(for_android);
        // The final content is checked before the device is asked, so that a request that is wrong everywhere is
        // not answered as one this device cannot serve.
        if let Some(snapshot) = &last {
            fits(entry, &input.id, snapshot)?;
        }
        // liveupdate refuses a delayed end where the device offers none. Asked here first, the refusal comes
        // before the final snapshot is shown, and the activity stays as it was.
        let delayed = matches!(input.policy, EndPolicy::After { .. });
        if delayed && live && !Offered::of(updates).get()?.delayed_end {
            return Err(Error::unsupported_feature(
                "This device removes an ended activity at once",
            ));
        }
        if let Some(snapshot) = last {
            self.replace(updates, &mut entries, &records, &input.id, snapshot, false)?;
        }
        let record = updates
            .end(&input.id, android_liveupdate::end_policy(input.policy))
            .map_err(android_liveupdate::error)?;
        let entry = entries.get(&input.id).ok_or_else(not_found)?;
        Ok(layout_record(&record, entry))
    }

    pub(crate) fn list(
        &self,
        updates: &liveupdate::LiveUpdate<D>,
    ) -> Result<Vec<LayoutActivityRecord>> {
        let _guard = self.lock.lock().map_err(|_| Error::native())?;
        let (entries, records) = self.reconciled(updates)?;
        Ok(records
            .iter()
            .filter_map(|record| {
                entries
                    .get(&record.id)
                    .map(|entry| layout_record(record, entry))
            })
            .collect())
    }

    /// The events in liveupdate's queue, as the backend passes them on, oldest first: the taps, with the
    /// activity's key and without those on action ids that the activity never declared, and the dismissals.
    pub(crate) fn events(&self, updates: &liveupdate::LiveUpdate<D>) -> Result<Vec<Event>> {
        // The entries are read before the queue is drained, under the lock, so that a start settles first and a
        // tap on its activity finds the entry; a store that cannot be read loses no event, only the keys.
        let _guard = self.lock.lock().map_err(|_| Error::native())?;
        let entries = self.store.load().unwrap_or_default();
        let batch = updates.events().map_err(android_liveupdate::error)?;
        Ok(batch
            .events
            .into_iter()
            .filter_map(|event| {
                let entry = entries.get(&event.activity_id);
                let activity_key = entry.map(|entry| entry.origin.activity_key.clone());
                match event.kind {
                    liveupdate::EventKind::Action { action_id } => {
                        if entry.is_some_and(|entry| !declares(entry, &action_id)) {
                            return None;
                        }
                        Some(Event::Action(ActionEvent {
                            action_id,
                            activity_id: event.activity_id,
                            activity_key,
                        }))
                    }
                    liveupdate::EventKind::Dismissed => Some(Event::State(StateEvent {
                        activity_id: event.activity_id,
                        activity_key,
                        state: ActivityState::Dismissed,
                    })),
                    // A kind the contract has no event for, such as an activity that was opened.
                    _ => None,
                }
            })
            .collect())
    }

    /// Checks `snapshot` as an update of the activity `id` and sends it, keeping it in the store.
    fn replace(
        &self,
        updates: &liveupdate::LiveUpdate<D>,
        entries: &mut Entries,
        records: &[liveupdate::ActivityRecord],
        id: &str,
        snapshot: LayoutSnapshot,
        alert: bool,
    ) -> Result<liveupdate::ActivityRecord> {
        let entry = entries.get(id).ok_or_else(not_found)?;
        fits(entry, id, &snapshot)?;
        let current = records
            .iter()
            .find(|record| record.id == id && running(record))
            .ok_or_else(not_found)?;
        if snapshot.revision < entry.snapshot.revision {
            return Err(Error::new(
                "STALE_REVISION",
                "the revision is below the accepted one",
            ));
        }
        if snapshot.revision == entry.snapshot.revision {
            return if snapshot == entry.snapshot {
                Ok(current.clone())
            } else {
                Err(Error::new(
                    "CONFLICT",
                    "the accepted revision has other values",
                ))
            };
        }
        let mut offered = Offered::of(updates);
        let projection = self.sent(&mut offered, &entry.origin, &snapshot, alert)?;
        // Another style than the one the activity shows needs a device that changes styles. The request is valid;
        // a device that keeps the style cannot show it.
        let style = projection.snapshot.template();
        if style.map_err(android_liveupdate::error)? != current.template
            && !offered.get()?.style_change
        {
            return Err(Error::unsupported_feature(
                "This device keeps the notification style an activity shows",
            ));
        }
        let record = updates
            .update(id, projection.snapshot)
            .map_err(android_liveupdate::error)?;
        let entry = Entry {
            origin: entry.origin.clone(),
            actions: with_actions(entry.actions.clone(), &snapshot),
            snapshot,
            degraded: projection.degraded,
        };
        entries.insert(id.to_owned(), entry);
        self.store.save(entries)?;
        Ok(record)
    }

    /// liveupdate's schema-v2 records and the entries of the activities it keeps; the store forgets the others. A
    /// pending entry whose start liveupdate took before the backend could note it is adopted under its id; any other
    /// pending entry is from a start that never got an answer.
    fn reconciled(
        &self,
        updates: &liveupdate::LiveUpdate<D>,
    ) -> Result<(Entries, Vec<liveupdate::ActivityRecord>)> {
        let records: Vec<_> = updates
            .list()
            .map_err(android_liveupdate::error)?
            .into_iter()
            .filter(|record| record.schema_version == 2)
            .collect();
        let stored = self.store.load()?;
        let entries: Entries = records
            .iter()
            .filter_map(|record| {
                let pending = || {
                    running(record)
                        .then(|| stored.get(&format!("{PENDING}{}", record.activity_key)))
                        .flatten()
                };
                stored
                    .get(&record.id)
                    .or_else(pending)
                    .map(|entry| (record.id.clone(), entry.clone()))
            })
            .collect();
        if entries != stored {
            self.store.save(&entries)?;
        }
        Ok((entries, records))
    }

    /// What liveupdate is sent for `snapshot` on this device, and what the record names as left out of it. A
    /// content that uses what the device does not offer cannot be shown as given, and is refused. An alert, asked
    /// for by the update or by the content itself, marks the revision as one that alerts where the device alerts
    /// again; elsewhere the revision is silent and `alert` is named, until the next snapshot.
    fn sent(
        &self,
        offered: &mut Offered<'_, D>,
        input: &LayoutStartInput,
        snapshot: &LayoutSnapshot,
        alert: bool,
    ) -> Result<android_layout::Projection> {
        let shown = self.shown(input, snapshot);
        let presentation = shown.snapshot.presentation.as_ref();
        let needed = presentation.map(extensions).unwrap_or_default();
        if !needed.is_empty() {
            let device = offered.get()?;
            if let Some((name, _)) = needed.iter().find(|(_, offers)| !offers(&device)) {
                return Err(Error::unsupported_feature(&format!(
                    "This device does not offer `{name}` of an Android content"
                )));
            }
        }
        let asked = alert || presentation.is_some_and(|presentation| presentation.alert);
        let alerts = asked && offered.get()?.alert;
        Ok(android_layout::Projection {
            snapshot: alerting(shown.snapshot, alerts),
            degraded: shown
                .degraded
                .into_iter()
                .chain((asked && !alerts).then(|| ALERT.to_owned()))
                .collect(),
        })
    }

    /// What liveupdate shows for `snapshot`: its Android content as given, or else the projection of the layout,
    /// in the style the layout projects to with every snapshot.
    fn shown(
        &self,
        input: &LayoutStartInput,
        snapshot: &LayoutSnapshot,
    ) -> android_layout::Projection {
        match &snapshot.android {
            Some(content) => android_layout::Projection {
                snapshot: content.snapshot(snapshot.revision, snapshot.stale_at),
                degraded: Vec::new(),
            },
            None => self.project(input, snapshot),
        }
    }

    fn project(
        &self,
        input: &LayoutStartInput,
        snapshot: &LayoutSnapshot,
    ) -> android_layout::Projection {
        let image = |name: &str| self.image(name);
        let environment = Environment {
            app_name: &self.settings.app_name,
            samsung_now_bar: self.settings.samsung_now_bar,
            image: &image,
            public: visibility(input) == liveupdate::Visibility::Public,
        };
        android_layout::project(
            &input.layout,
            input.styles.is_some(),
            snapshot,
            android_layout::template(&input.layout),
            &environment,
        )
    }

    /// The app-file path of a picture `write_image` wrote, when liveupdate can draw it.
    fn image(&self, name: &str) -> Option<String> {
        let folder = &self.settings.image_folder;
        let path = self.settings.files.as_ref()?.join(folder).join(name);
        let metadata = fs::metadata(&path).ok()?;
        if !metadata.is_file() || metadata.len() > MAX_ICON_BYTES {
            return None;
        }
        let (width, height) = png_size(&path)?;
        (1..=MAX_ICON_SIDE).contains(&width).then_some(())?;
        (1..=MAX_ICON_SIDE).contains(&height).then_some(())?;
        Some(format!("{folder}/{name}"))
    }
}

/// The width and height of a PNG, from its IHDR chunk.
fn png_size(path: &std::path::Path) -> Option<(u32, u32)> {
    let mut header = [0_u8; 24];
    fs::File::open(path).ok()?.read_exact(&mut header).ok()?;
    if header[..8] != *b"\x89PNG\r\n\x1a\n" || header[12..16] != *b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(header[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(header[20..24].try_into().ok()?);
    Some((width, height))
}

/// Whether a device offers one of liveupdate's capabilities beyond the styles.
type Offer = fn(&liveupdate::PresentationCapabilities) -> bool;

/// What the device offers beyond the styles, for one request: asked of the driver when the request first needs
/// it, and once.
struct Offered<'a, D: liveupdate::Driver> {
    updates: &'a liveupdate::LiveUpdate<D>,
    known: Option<liveupdate::PresentationCapabilities>,
}

impl<'a, D: liveupdate::Driver> Offered<'a, D> {
    fn of(updates: &'a liveupdate::LiveUpdate<D>) -> Self {
        Self {
            updates,
            known: None,
        }
    }

    fn get(&mut self) -> Result<liveupdate::PresentationCapabilities> {
        if let Some(known) = self.known {
            return Ok(known);
        }
        let asked = self
            .updates
            .capabilities()
            .map_err(android_liveupdate::error)?
            .presentation;
        self.known = Some(asked);
        Ok(asked)
    }
}

/// The capabilities that `presentation` needs for what it uses beyond a style, each by its name in the contract.
/// The alert is not among them: a revision can be shown without alerting.
fn extensions(presentation: &liveupdate::AndroidPresentation) -> Vec<(&'static str, Offer)> {
    let links = presentation.open_url.is_some()
        || presentation
            .actions
            .iter()
            .any(|action| action.open_url.is_some());
    let branding = presentation.small_icon.is_some()
        || presentation.accent_color.is_some()
        || presentation.sub_text.is_some();
    let used: [(bool, &'static str, Offer); 4] = [
        (links, "links", |offered| offered.links),
        (
            presentation.chronometer.is_some(),
            "chronometer",
            |offered| offered.chronometer,
        ),
        (
            presentation.public_version.is_some(),
            "publicVersion",
            |offered| offered.public_version,
        ),
        (branding, "branding", |offered| offered.branding),
    ];
    used.into_iter()
        .filter(|(used, ..)| *used)
        .map(|(_, name, offer)| (name, offer))
        .collect()
}

/// `snapshot` as a revision that alerts, or as one that does not.
fn alerting(snapshot: liveupdate::Snapshot, alerts: bool) -> liveupdate::Snapshot {
    liveupdate::Snapshot {
        presentation: snapshot
            .presentation
            .map(|presentation| liveupdate::AndroidPresentation {
                alert: alerts,
                ..presentation
            }),
        ..snapshot
    }
}

/// Checks `snapshot` as content of the activity that `entry` began: the keys and the value types of its start,
/// and a public version only for a private activity.
fn fits(entry: &Entry, id: &str, snapshot: &LayoutSnapshot) -> Result<()> {
    entry
        .origin
        .validate_update(&LayoutUpdateInput::new(id, snapshot.clone()))?;
    if entry.origin.takes_public_version_of(snapshot) {
        Ok(())
    } else {
        Err(Error::invalid())
    }
}

/// A snapshot as Android keeps and compares it: without the relevance, which only iOS reads.
fn for_android(snapshot: LayoutSnapshot) -> LayoutSnapshot {
    LayoutSnapshot {
        relevance: None,
        ..snapshot
    }
}

/// `kept` with the action ids of the Android content of `snapshot`, each once, the newest last, and no more than
/// [`ACTIONS_KEPT`].
fn with_actions(kept: Vec<String>, snapshot: &LayoutSnapshot) -> Vec<String> {
    let Some(content) = &snapshot.android else {
        return kept;
    };
    let shown: Vec<&String> = content
        .presentation
        .actions
        .iter()
        .map(|action| &action.id)
        .collect();
    let earlier = kept.into_iter().filter(|id| !shown.contains(&id));
    let all: Vec<String> = earlier
        .chain(shown.iter().map(|id| (*id).clone()))
        .collect();
    let surplus = all.len().saturating_sub(ACTIONS_KEPT);
    all.into_iter().skip(surplus).collect()
}

/// Whether the activity declared `action_id`: a button of its layout, or an action of an Android content it has
/// shown.
fn declares(entry: &Entry, action_id: &str) -> bool {
    button(&entry.origin.layout.lock_screen, action_id)
        || entry.actions.iter().any(|id| id == action_id)
}

/// Whether a button of the tree has `action_id`.
fn button(node: &Node, action_id: &str) -> bool {
    match &node.element {
        Element::Button {
            action_id: id,
            child,
        } => id == action_id || button(child, action_id),
        Element::Vstack { children, .. }
        | Element::Hstack { children, .. }
        | Element::Zstack { children, .. } => children.iter().any(|child| button(child, action_id)),
        _ => false,
    }
}

fn running(record: &liveupdate::ActivityRecord) -> bool {
    !matches!(
        record.state,
        liveupdate::ActivityState::Ended | liveupdate::ActivityState::Dismissed
    )
}

fn visibility(input: &LayoutStartInput) -> liveupdate::Visibility {
    match input.android.as_ref().map(|android| android.visibility) {
        Some(AndroidVisibility::Public) => liveupdate::Visibility::Public,
        Some(AndroidVisibility::Secret) => liveupdate::Visibility::Secret,
        _ => liveupdate::Visibility::Private,
    }
}

fn layout_record(record: &liveupdate::ActivityRecord, entry: &Entry) -> LayoutActivityRecord {
    LayoutActivityRecord {
        id: record.id.clone(),
        activity_key: record.activity_key.clone(),
        schema_version: 2,
        snapshot: entry.snapshot.clone(),
        backend: Backend::AndroidNotification,
        state: android_liveupdate::activity_state(record.state),
        visibility: Visibility::Unknown,
        degraded_features: record
            .degraded_features
            .iter()
            .chain(&entry.degraded)
            .cloned()
            .collect(),
        promotion_requested: record.promotion_requested,
        promotion_state: android_liveupdate::promotion_state(record.promotion_state),
    }
}

fn not_found() -> Error {
    Error::new("NOT_FOUND", "no layout activity has this id")
}

#[cfg(test)]
#[path = "layouts_tests.rs"]
mod tests;
