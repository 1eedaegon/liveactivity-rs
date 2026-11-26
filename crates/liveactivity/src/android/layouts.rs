//! Layout activities on Android, on liveupdate: liveupdate keeps the lifecycle, the receipts and the notification
//! of each schema-v2 request, which is a snapshot's Android content as given or else the projection of the layout
//! by `android_layout`, and a store of the backend keeps what liveupdate does not, the layout request and its
//! latest snapshot, under liveupdate's id. Taps come from liveupdate's events; the latest snapshot says which
//! state a tap on a toggle asks for. What only some devices do (another style in an update, an update that
//! alerts, a last content that stays) is asked of liveupdate's capabilities when a request needs it.

use super::{
    convert as android_liveupdate,
    layout::{self as android_layout, Environment},
};
use crate::{
    ActionEvent, Backend, EndPolicy, Error, Event, HandledEvent, LayoutActivityRecord,
    LayoutEndInput, LayoutSnapshot, LayoutStartInput, LayoutUpdateInput, OpenedEvent, Result,
    UnknownEvent, Visibility,
    layout::{Alert, AndroidOptions, AndroidPlacement, AndroidVisibility},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::PathBuf,
    sync::{Mutex, MutexGuard, PoisonError},
};

/// liveupdate draws app files of at most this many pixels a side and bytes.
const MAX_ICON_SIDE: u32 = 1024;
const MAX_ICON_BYTES: u64 = 1_048_576;
/// The store's entry of a start before liveupdate has answered, under the activity key.
const PENDING: &str = "pending:";
/// What a record names when an update asked for an alert on a device that alerts once.
const ALERT: &str = "alert";
/// What a record names when an alert that alerts named a sound: Android plays the sound of its channel.
const ALERT_SOUND: &str = "alert.sound";
/// What a record names when the start gave a public version on a device that shows none.
const PUBLIC_VERSION: &str = "publicVersion";
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

    /// The lock, also after a thread panicked under it (REQ-A-022). It holds no value, and what changes under
    /// it is a file that is replaced whole, which every call but the drain reconciles with liveupdate.
    fn guard(&self) -> MutexGuard<'_, ()> {
        self.lock.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn start(
        &self,
        updates: &liveupdate::LiveUpdate<D>,
        input: LayoutStartInput,
    ) -> Result<LayoutActivityRecord> {
        // Android has no pushes, no relevance and none of the iOS options: what the start says of them is dropped
        // here, so that it is no part of the request the activity keeps and a repeated start is compared without
        // it.
        let input = LayoutStartInput {
            push: None,
            ios: None,
            snapshot: for_android(input.snapshot),
            android: input.android.map(by_default),
            ..input
        };
        let _guard = self.guard();
        let (mut entries, records) = self.reconciled(updates)?;
        // The activity that runs or waits under the key, with the request it keeps.
        let under_the_key = records
            .iter()
            .filter(|record| running(record))
            .find_map(|record| {
                let entry = entries.get(&record.id)?;
                (entry.origin.activity_key == input.activity_key).then_some((record, entry))
            });
        // liveupdate compares the projections; two starts that project the same may still differ.
        if under_the_key.is_some_and(|(_, entry)| !same_start(&entry.origin, &input)) {
            return Err(Error::new(
                "CONFLICT",
                "a running activity of this key started with another request",
            ));
        }
        let input = match under_the_key {
            Some((record, entry)) if record.state == liveupdate::ActivityState::Scheduled => {
                return waiting(updates, record, entry);
            }
            // The same request as a running activity that Android shows is that activity, whatever this device
            // would make of the request now: what a projection uses follows what the device offers, which may
            // have changed.
            Some((record, entry)) if record.state != liveupdate::ActivityState::Unknown => {
                return Ok(layout_record(record, entry));
            }
            // One that Android does not show yet goes on to liveupdate, which posts it again: with the time it
            // was started with, which liveupdate compares.
            Some((_, entry)) => LayoutStartInput {
                scheduled: entry.origin.scheduled.clone(),
                ..input
            },
            None => input,
        };
        let mut offered = Offered::of(updates);
        let begins = input.scheduled.as_ref();
        // A scheduled start alerts when it begins, as an update with an alert does.
        let alert = begins.map(|scheduled| &scheduled.alert);
        // The request is one of the contract, which liveupdate would call invalid: Android cannot serve it.
        if begins.is_some() && placement(&input) == liveupdate::Placement::Foreground {
            return Err(Error::unsupported_feature(
                "A service holds a notification that is there: a scheduled start takes no foreground placement",
            ));
        }
        let pending = format!("{PENDING}{}", input.activity_key);
        // The second turn, when there is one, sends the projection without its links: see `unlinked`.
        let mut links = true;
        let (record, entry) = loop {
            let projection = self.sent(&mut offered, &input, &input.snapshot, alert, links)?;
            let retry = links && unlinked(&input.snapshot, &projection.snapshot);
            // The style of the first snapshot: its content's, or else the one the layout projects to.
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
                placement: placement(&input),
                start_at: begins.map(|scheduled| scheduled.at),
            };
            let entry = Entry {
                origin: input.clone(),
                snapshot: input.snapshot.clone(),
                degraded: projection.degraded,
                actions: with_actions(Vec::new(), &input.snapshot),
            };
            entries.insert(pending.clone(), entry.clone());
            self.store.save(&entries)?;
            // A failure leaves the pending entry: the next reconciliation adopts it when liveupdate posted the
            // activity after all (an answer lost on the way back), and drops it otherwise, as after a content
            // that Android refused to post, of which liveupdate keeps no receipt.
            match updates.start(request) {
                Ok(record) => break (record, entry),
                Err(error) if retry && refuses_links(&error) => links = false,
                Err(error) => return Err(android_liveupdate::error(error)),
            }
        };
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
        let _guard = self.guard();
        let (mut entries, records) = self.reconciled(updates)?;
        let alert = input.alert.as_ref();
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
        let _guard = self.guard();
        let (mut entries, records) = self.reconciled(updates)?;
        let entry = entries.get(&input.id).ok_or_else(not_found)?;
        // An activity that is over takes no content: its end again answers its record, and an end without a time
        // removes a last content that an earlier end kept.
        // Nor does one that waits for its scheduled start: it ends before anything is shown.
        let state = records
            .iter()
            .find(|record| record.id == input.id && running(record))
            .map(|record| record.state);
        let waits = state == Some(liveupdate::ActivityState::Scheduled);
        let live = state.is_some() && !waits;
        let last = input.snapshot.filter(|_| state.is_some()).map(for_android);
        // The final content is checked before the device is asked, so that a request that is wrong everywhere is
        // not answered as one this device cannot serve; also that of an activity that waits, which is not sent.
        if let Some(snapshot) = &last {
            fits(entry, &input.id, snapshot)?;
        }
        let last = last.filter(|_| live);
        // liveupdate refuses a delayed end where the device offers none. Asked here first, the refusal comes
        // before the final snapshot is shown, and the activity stays as it was.
        let delayed = matches!(input.policy, EndPolicy::After { .. });
        if delayed && live && !Offered::of(updates).get()?.delayed_end {
            return Err(Error::unsupported_feature(
                "This device removes an ended activity at once",
            ));
        }
        if let Some(snapshot) = last {
            self.replace(updates, &mut entries, &records, &input.id, snapshot, None)?;
        }
        // An activity that waits has no last content to keep until a time: any policy ends it, on any device.
        let policy = if waits {
            EndPolicy::Default {}
        } else {
            input.policy
        };
        let record = updates
            .end(&input.id, android_liveupdate::end_policy(policy))
            .map_err(android_liveupdate::error)?;
        let entry = entries.get(&input.id).ok_or_else(not_found)?;
        Ok(layout_record(&record, entry))
    }

    pub(crate) fn list(
        &self,
        updates: &liveupdate::LiveUpdate<D>,
    ) -> Result<Vec<LayoutActivityRecord>> {
        let _guard = self.guard();
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
    /// activity's key, with the state a toggle was asked for, and without those on action ids that the activity
    /// never declared, and the openings.
    pub(crate) fn events(&self, updates: &liveupdate::LiveUpdate<D>) -> Result<Vec<Event>> {
        // The entries are read before the queue is drained, under the lock, so that a start settles first and a
        // tap on its activity finds the entry; a store that cannot be read loses no event, only the keys.
        let _guard = self.guard();
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
                        let on = entry.and_then(|entry| asked_for(entry, &action_id));
                        Some(Event::Action(ActionEvent {
                            activity_key,
                            on,
                            tap: stored_tap(event.sequence, event.occurred_at),
                            ..ActionEvent::new(action_id, event.activity_id)
                        }))
                    }
                    // An opening is the host's report of an intent that liveupdate made and checked: its
                    // action is passed on as it is, declared by the activity's present content or not.
                    liveupdate::EventKind::Opened { action_id } => {
                        Some(Event::Opened(OpenedEvent {
                            activity_id: event.activity_id,
                            activity_key,
                            action_id,
                        }))
                    }
                    // A tap that the application's own component took: reported, and no tap to act on.
                    liveupdate::EventKind::Handled { action_id } => {
                        Some(Event::Handled(HandledEvent {
                            activity_id: event.activity_id,
                            activity_key,
                            action_id,
                            tap: stored_tap(event.sequence, event.occurred_at),
                        }))
                    }
                    // A dismissal is reported with the other states, by the backend: its stored event is the
                    // prompt to ask.
                    liveupdate::EventKind::Dismissed => None,
                    // What a push did names its own key, also when it named no live activity.
                    liveupdate::EventKind::Pushed {
                        command,
                        activity_key,
                        revision,
                        code,
                    } => Some(android_liveupdate::pushed(
                        event.activity_id,
                        &command,
                        activity_key,
                        revision,
                        code,
                    )),
                    // A kind that liveupdate does not know, or one that it has and this crate does not.
                    _ => Some(Event::Unknown(UnknownEvent {
                        activity_id: event.activity_id,
                        activity_key,
                    })),
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
        alert: Option<&Alert>,
    ) -> Result<liveupdate::ActivityRecord> {
        let entry = entries.get(id).ok_or_else(not_found)?;
        fits(entry, id, &snapshot)?;
        let current = records
            .iter()
            .find(|record| record.id == id && running(record))
            .ok_or_else(not_found)?;
        if current.state == liveupdate::ActivityState::Scheduled {
            return Err(Error::new(
                "CONFLICT",
                "a scheduled activity takes no update before it began",
            ));
        }
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
        // The second turn, when there is one, sends the projection without its links: see `unlinked`.
        let mut links = true;
        let (record, degraded) = loop {
            let projection = self.sent(&mut offered, &entry.origin, &snapshot, alert, links)?;
            let retry = links && unlinked(&snapshot, &projection.snapshot);
            // Another style than the one the activity shows needs a device that changes styles. The request is
            // valid; a device that keeps the style cannot show it.
            let style = projection.snapshot.template();
            if style.map_err(android_liveupdate::error)? != current.template
                && !offered.get()?.style_change
            {
                return Err(Error::unsupported_feature(
                    "This device keeps the notification style an activity shows",
                ));
            }
            match updates.update(id, projection.snapshot) {
                Ok(record) => break (record, projection.degraded),
                Err(error) if retry && refuses_links(&error) => links = false,
                Err(error) => return Err(android_liveupdate::error(error)),
            }
        };
        let entry = Entry {
            origin: entry.origin.clone(),
            actions: with_actions(entry.actions.clone(), &snapshot),
            snapshot,
            degraded,
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
    /// again; elsewhere the revision is silent and `alert` is named, until the next snapshot. An alert that alerts
    /// plays the sound of its channel, and a sound it named is named as `alert.sound`. The public version of the
    /// start goes with a snapshot that has none of its own, where the device shows one; elsewhere `publicVersion`
    /// is named. `links` says whether a projection may give the notification URLs to open.
    fn sent(
        &self,
        offered: &mut Offered<'_, D>,
        input: &LayoutStartInput,
        snapshot: &LayoutSnapshot,
        alert: Option<&Alert>,
        links: bool,
    ) -> Result<android_layout::Projection> {
        let shown = self.shown(offered, input, snapshot, links)?;
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
        let asked = alert.is_some() || presentation.is_some_and(|presentation| presentation.alert);
        let alerts = asked && offered.get()?.alert;
        let sound = alerts && alert.is_some_and(|alert| alert.sound.is_some());
        // The public version of the start, for a snapshot that has none of its own.
        let of_the_start = input
            .android
            .as_ref()
            .and_then(|android| android.public_version.as_ref())
            .filter(|_| presentation.is_some_and(|shown| shown.public_version.is_none()));
        let public_version = match of_the_start {
            Some(version) if offered.get()?.public_version => Some(version.clone()),
            _ => None,
        };
        let unshown = of_the_start.is_some() && public_version.is_none();
        // The start's word on a watch's Ongoing Activity, for every snapshot of the activity.
        let carried = input
            .android
            .as_ref()
            .and_then(|android| android.wear_ongoing_activity)
            .unwrap_or(true);
        // And on keeping it off a paired watch.
        let local = input
            .android
            .as_ref()
            .and_then(|android| android.local_only)
            .unwrap_or(false);
        Ok(android_layout::Projection {
            snapshot: marked(shown.snapshot, alerts, public_version, carried, local),
            degraded: shown
                .degraded
                .into_iter()
                .chain((asked && !alerts).then(|| ALERT.to_owned()))
                .chain(sound.then(|| ALERT_SOUND.to_owned()))
                .chain(unshown.then(|| PUBLIC_VERSION.to_owned()))
                .collect(),
        })
    }

    /// What liveupdate shows for `snapshot`: its Android content as given, or else the projection of the layout,
    /// in the style the layout projects to with every snapshot.
    fn shown(
        &self,
        offered: &mut Offered<'_, D>,
        input: &LayoutStartInput,
        snapshot: &LayoutSnapshot,
        links: bool,
    ) -> Result<android_layout::Projection> {
        match &snapshot.android {
            Some(content) => Ok(android_layout::Projection {
                snapshot: content.snapshot(snapshot.revision, snapshot.stale_at),
                degraded: Vec::new(),
            }),
            None => {
                let projection = self.project(offered, input, snapshot, links)?;
                Ok(android_layout::Projection {
                    snapshot: targeted(projection.snapshot, input),
                    ..projection
                })
            }
        }
    }

    /// The projection of the layout, with what the device offers of links and of a clock beside a style. The
    /// device is asked only for a layout that could use one of them.
    fn project(
        &self,
        offered: &mut Offered<'_, D>,
        input: &LayoutStartInput,
        snapshot: &LayoutSnapshot,
        links: bool,
    ) -> Result<android_layout::Projection> {
        let (linked, clocked) = android_layout::could_use(&input.layout);
        let device = if (links && linked) || clocked {
            offered.get()?
        } else {
            liveupdate::PresentationCapabilities::default()
        };
        let image = |name: &str| self.image(name);
        let environment = Environment {
            app_name: &self.settings.app_name,
            samsung_now_bar: self.settings.samsung_now_bar,
            image: &image,
            public: visibility(input) == liveupdate::Visibility::Public,
            links: links && device.links,
            chronometer: device.chronometer,
        };
        Ok(android_layout::project(
            &input.layout,
            input.styles.is_some(),
            snapshot,
            android_layout::template(&input.layout),
            &environment,
        ))
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

/// `snapshot` as a revision that alerts or as one that does not, and with `public_version` when it has none of
/// its own.
fn marked(
    snapshot: liveupdate::Snapshot,
    alerts: bool,
    public_version: Option<liveupdate::PublicVersion>,
    carried: bool,
    local: bool,
) -> liveupdate::Snapshot {
    liveupdate::Snapshot {
        presentation: snapshot
            .presentation
            .map(|presentation| liveupdate::AndroidPresentation {
                alert: alerts,
                public_version: presentation.public_version.or(public_version),
                // Carried where neither the start nor the content of the snapshot turns it off.
                wear_ongoing_activity: carried && presentation.wear_ongoing_activity,
                // Kept off a paired watch where the start or the content of the snapshot says so.
                local_only: local || presentation.local_only,
                ..presentation
            }),
        ..snapshot
    }
}

/// Whether `sent`, what a layout snapshot projected to, has URLs that it can be sent without. Only the device
/// knows whether an activity of the application opens a URL, and liveupdate refuses a post whose URL none opens.
/// A layout that iOS takes is not refused for that: it is sent again without its links, which the record then
/// names. An Android content is the application's own word for Android and is never changed.
fn unlinked(snapshot: &LayoutSnapshot, sent: &liveupdate::Snapshot) -> bool {
    snapshot.android.is_none()
        && sent.presentation.as_ref().is_some_and(|shown| {
            shown.open_url.is_some() || shown.actions.iter().any(|action| action.open_url.is_some())
        })
}

/// Whether liveupdate's answer may be about the links of a projection: the device refused the post, or the
/// activity runs already and began without its links, so that the same start with them reads as another request.
fn refuses_links(error: &liveupdate::Error) -> bool {
    matches!(
        error.code,
        liveupdate::ErrorCode::InvalidInput | liveupdate::ErrorCode::Conflict
    )
}

/// The options as they are kept and compared: a member that names the default is the default.
fn by_default(options: AndroidOptions) -> AndroidOptions {
    AndroidOptions {
        placement: options
            .placement
            .filter(|placement| *placement != AndroidPlacement::Tagged),
        wear_ongoing_activity: options.wear_ongoing_activity.filter(|carried| !carried),
        local_only: options.local_only.filter(|local| *local),
        ..options
    }
}

fn placement(input: &LayoutStartInput) -> liveupdate::Placement {
    match input.android.as_ref().and_then(|android| android.placement) {
        Some(AndroidPlacement::Foreground) => liveupdate::Placement::Foreground,
        _ => liveupdate::Placement::Tagged,
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

/// Whether the activity declared `action_id`: a button or a toggle of its layout, or an action of an Android
/// content it has shown.
fn declares(entry: &Entry, action_id: &str) -> bool {
    crate::layout::controls(&entry.origin.layout.lock_screen, action_id)
        || entry.actions.iter().any(|id| id == action_id)
}

/// The state that a tap on the action `action_id` asks for: for a toggle's action, the other one than the value in
/// the latest snapshot, as the notification of that snapshot shows it. A button asks for none, and neither does
/// an action of an Android content, which is what the notification shows in place of the layout.
fn asked_for(entry: &Entry, action_id: &str) -> Option<bool> {
    if entry.snapshot.android.is_some() {
        return None;
    }
    android_layout::asked_for(&entry.origin.layout, &entry.snapshot.values, action_id)
}

/// The projection of the layout with the start's targets on the actions of their buttons and toggles. A link
/// takes none: its tap opens its URL.
fn targeted(snapshot: liveupdate::Snapshot, input: &LayoutStartInput) -> liveupdate::Snapshot {
    let Some(targets) = input
        .android
        .as_ref()
        .map(|android| &android.targets)
        .filter(|targets| !targets.is_empty())
    else {
        return snapshot;
    };
    liveupdate::Snapshot {
        presentation: snapshot
            .presentation
            .map(|presentation| liveupdate::AndroidPresentation {
                actions: presentation
                    .actions
                    .into_iter()
                    .map(|action| liveupdate::NotificationAction {
                        target: action
                            .open_url
                            .is_none()
                            .then(|| targets.get(&action.id).cloned())
                            .flatten(),
                        ..action
                    })
                    .collect(),
                ..presentation
            }),
        ..snapshot
    }
}

/// Whether two starts are the same request. When and how the activity begins is no part of it, as on iOS: the
/// same request under a key answers the activity that waits or runs, whatever its time, and with or without one.
fn same_start(kept: &LayoutStartInput, asked: &LayoutStartInput) -> bool {
    let asked = LayoutStartInput {
        scheduled: kept.scheduled.clone(),
        ..asked.clone()
    };
    *kept == asked
}

/// The answer to the same start as that of an activity that waits for its time. liveupdate is asked with the
/// request it keeps, which its own record holds: it answers the activity, and hands it to its Android side again
/// where that had failed. It asks the device first, so this fails as a start does while notifications are off.
fn waiting<D: liveupdate::Driver>(
    updates: &liveupdate::LiveUpdate<D>,
    record: &liveupdate::ActivityRecord,
    entry: &Entry,
) -> Result<LayoutActivityRecord> {
    let kept = liveupdate::StartRequest {
        activity_key: record.activity_key.clone(),
        schema_version: 2,
        template: record.template,
        fallback: android_liveupdate::fallback(entry.origin.fallback),
        visibility: visibility(&entry.origin),
        snapshot: record.snapshot.clone(),
        placement: placement(&entry.origin),
        start_at: record.start_at,
    };
    let record = updates.start(kept).map_err(android_liveupdate::error)?;
    Ok(layout_record(&record, entry))
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

/// What the adapter says of a stored tap: its sequence, which no other event of the device has, and when it was
/// stored. A tap on Android has no revision and is never late: the adapter keeps it until it is drained.
pub(crate) fn stored_tap(sequence: u64, occurred_at: u64) -> crate::Tap {
    crate::Tap {
        id: Some(sequence.to_string()),
        at: Some(occurred_at),
        ..crate::Tap::default()
    }
}
