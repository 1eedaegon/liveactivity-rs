//! A kit that answers as activitykit 0.1 does, in memory. A write at a revision is taken when the revision is higher
//! than the accepted one, answered as done when it equals it with the same content, and refused when lower
//! (`STALE_REVISION`) or equal with other content (`CONFLICT`); an end without a revision ends at the next one with the
//! last values. A repeated keyed start with the same content, whatever its timestamp, returns the running activity,
//! and one with other content is `CONFLICT`. An update of an ended activity is `NOT_FOUND`; ending again is fine.
//!
//! As on iOS, an activity that ends at once leaves the system's list (here at once, the losing side of the race), and
//! `activity(id)` finds an id through a handle that is alive in the process, else through the list.

use super::{Acknowledge, Authorization, EndSpec, Handle, Kit, KitEvent, StartSpec, UpdateSpec};
use crate::layout::ActionEvent;
use activitykit::{Error, State, Values};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, Weak, mpsc},
};

/// What the tests log: calls, handlers, emits and releases, in order.
pub(crate) type Log = Arc<Mutex<Vec<String>>>;

/// An acknowledgement that logs `released` when the tap's intent is let go.
pub(crate) struct FakeAck(pub(crate) Log);

impl Acknowledge for FakeAck {
    fn acknowledge(&self) {
        self.0.lock().unwrap().push("released".into());
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub(crate) id: String,
    pub(crate) start: StartSpec,
    pub(crate) state: State,
    pub(crate) values: Values,
    pub(crate) revision: u64,
    pub(crate) stale_at: Option<u64>,
    pub(crate) timestamp: Option<u64>,
    /// The score of the last content that gave one.
    pub(crate) relevance: Option<f64>,
    /// Whether the system lists it.
    pub(crate) listed: bool,
    /// What the system could not show of it.
    pub(crate) degraded: Vec<String>,
}

#[derive(Default)]
pub(crate) struct Book {
    pub(crate) entries: Vec<Entry>,
    /// Every call that reached the kit: `start key`, `activity id`, `update id`, `end id`, `list`, `image name`.
    pub(crate) calls: Vec<String>,
    pub(crate) starts: Vec<StartSpec>,
    pub(crate) updates: Vec<UpdateSpec>,
    pub(crate) ends: Vec<EndSpec>,
    /// The answer of the next call instead of its result.
    pub(crate) fail: Option<Error>,
    pub(crate) authorization: Option<Authorization>,
    /// The handles alive in this process, by id; they share one token, as activitykit's share their state.
    alive: HashMap<String, Weak<()>>,
    /// Where the kit's events go, once the backend subscribed.
    events: Option<mpsc::Sender<KitEvent<FakeHandle, FakeAck>>>,
}

impl Book {
    fn alive(&self, id: &str) -> bool {
        self.alive
            .get(id)
            .is_some_and(|token| token.strong_count() > 0)
    }
}

#[derive(Clone, Default)]
pub(crate) struct FakeKit(pub(crate) Arc<Mutex<Book>>);

#[derive(Clone)]
pub(crate) struct FakeHandle {
    id: String,
    key: String,
    book: Arc<Mutex<Book>>,
    _alive: Arc<()>,
}

/// The handle of `id`, which shares the token of a handle alive in the process.
fn adopt(book: &Arc<Mutex<Book>>, guard: &mut Book, id: &str, key: &str) -> FakeHandle {
    let token = match guard.alive.get(id).and_then(Weak::upgrade) {
        Some(token) => token,
        None => {
            let token = Arc::new(());
            guard.alive.insert(id.to_owned(), Arc::downgrade(&token));
            token
        }
    };
    FakeHandle {
        id: id.to_owned(),
        key: key.to_owned(),
        book: book.clone(),
        _alive: token,
    }
}

impl FakeKit {
    pub(crate) fn book(&self) -> MutexGuard<'_, Book> {
        self.0.lock().unwrap()
    }

    pub(crate) fn calls(&self) -> Vec<String> {
        self.book().calls.clone()
    }

    pub(crate) fn fail_next(&self, error: Error) {
        self.book().fail = Some(error);
    }

    /// What the system does: the activity moves to `state`, as an event would tell the handle.
    pub(crate) fn set_state(&self, id: &str, state: State) {
        let mut book = self.book();
        book.entries
            .iter_mut()
            .filter(|entry| entry.id == id)
            .for_each(|entry| entry.state = state);
    }

    /// A tap as activitykit's events give it, to the subscriber of [`Kit::events`].
    pub(crate) fn send_tap(&self, event: ActionEvent, acknowledgement: FakeAck) {
        self.send_event(KitEvent::Tap(event, acknowledgement));
    }

    /// An event as activitykit gives it, to the subscriber of [`Kit::events`].
    pub(crate) fn send_event(&self, event: KitEvent<FakeHandle, FakeAck>) {
        let book = self.book();
        let events = book
            .events
            .as_ref()
            .expect("the backend subscribed to the events");
        events.send(event).unwrap();
    }

    /// The end of the kit's events.
    pub(crate) fn close_events(&self) {
        self.book().events = None;
    }

    /// What a push does to the activity `id`: it shows `values` at `revision`, above what it had.
    #[allow(dead_code)]
    pub(crate) fn pushed(&self, id: &str, revision: u64, values: Values) {
        let mut book = self.book();
        for entry in book.entries.iter_mut().filter(|entry| entry.id == id) {
            entry.values = values.clone();
            entry.revision = revision;
        }
    }

    /// What activitykit reports the system could not show of the activity `id`.
    pub(crate) fn degrade(&self, id: &str, features: &[&str]) {
        let mut book = self.book();
        for entry in book.entries.iter_mut().filter(|entry| entry.id == id) {
            entry.degraded = features.iter().map(|&feature| feature.to_owned()).collect();
        }
    }

    /// Whether the system lists the activity `id`.
    pub(crate) fn listed(&self, id: &str) -> bool {
        self.book()
            .entries
            .iter()
            .any(|entry| entry.id == id && entry.listed)
    }

    /// A kit of a later launch: the same activities, no handles of this launch.
    pub(crate) fn relaunched(&self) -> Self {
        let entries = self.book().entries.clone();
        Self(Arc::new(Mutex::new(Book {
            entries,
            ..Book::default()
        })))
    }

    /// The handle that an event would give: activitykit's tap names the activity's key.
    pub(crate) fn handle(&self, id: &str, key: &str) -> FakeHandle {
        adopt(&self.0, &mut self.book(), id, key)
    }
}

fn take(book: &mut Book, call: String) -> Result<(), Error> {
    book.calls.push(call);
    book.fail.take().map_or(Ok(()), Err)
}

fn live(state: State) -> bool {
    matches!(state, State::Active | State::Stale | State::Pending)
}

/// A start's content, as a repeated keyed start compares it: everything but the timestamp, the relevance, the
/// push and when and how the activity begins, which are no part of the request that ActivityKit's side compares.
fn content(start: &StartSpec) -> StartSpec {
    StartSpec {
        timestamp: None,
        relevance: None,
        push: None,
        transient: false,
        scheduled: None,
        ..start.clone()
    }
}

/// Swift's rules for content at `revision` against `entry`: `Ok(true)` to take it, `Ok(false)` when it is done.
fn takes(
    entry: &Entry,
    revision: u64,
    values: &Values,
    stale_at: Option<u64>,
) -> Result<bool, Error> {
    if revision < entry.revision {
        return Err(Error::StaleRevision(
            "The revision is older than the current activity state.".into(),
        ));
    }
    if revision > entry.revision {
        return Ok(true);
    }
    if *values == entry.values && stale_at == entry.stale_at {
        Ok(false)
    } else {
        Err(Error::Conflict(
            "This revision already contains different state.".into(),
        ))
    }
}

impl Kit for FakeKit {
    type Handle = FakeHandle;
    type Ack = FakeAck;

    fn events(&self) -> Box<dyn Iterator<Item = KitEvent<FakeHandle, FakeAck>> + Send> {
        let (sender, receiver) = mpsc::channel();
        let mut book = self.book();
        book.calls.push("events".into());
        book.events = Some(sender);
        Box::new(receiver.into_iter())
    }

    fn authorization(&self) -> Authorization {
        self.book().authorization.unwrap_or(Authorization {
            supported: true,
            enabled: true,
            buttons: true,
            frequent_pushes: true,
        })
    }

    fn start(
        &self,
        start: StartSpec,
    ) -> impl Future<Output = activitykit::Result<FakeHandle>> + Send {
        let book = self.0.clone();
        async move {
            let mut guard = book.lock().unwrap();
            take(&mut guard, format!("start {}", start.key))?;
            let key = start.key.clone();
            let running = guard
                .entries
                .iter()
                .find(|entry| entry.start.key == key && live(entry.state));
            if let Some(running) = running {
                if content(&running.start) != content(&start) {
                    return Err(Error::Conflict(
                        "An activity with this key already exists with different input.".into(),
                    ));
                }
                let id = running.id.clone();
                return Ok(adopt(&book, &mut guard, &id, &key));
            }
            let id = format!("la-ios-layout-{}", guard.entries.len() + 1);
            guard.entries.push(Entry {
                id: id.clone(),
                start: start.clone(),
                // A scheduled activity waits for its time.
                state: match start.scheduled {
                    Some(_) => State::Pending,
                    None => State::Active,
                },
                values: start.values.clone(),
                revision: start.revision,
                stale_at: start.stale_at,
                timestamp: Some(start.timestamp.unwrap_or(1_000)),
                relevance: start.relevance,
                listed: true,
                degraded: Vec::new(),
            });
            guard.starts.push(start);
            Ok(adopt(&book, &mut guard, &id, &key))
        }
    }

    fn activity(
        &self,
        id: &str,
    ) -> impl Future<Output = activitykit::Result<Option<FakeHandle>>> + Send {
        let (book, id) = (self.0.clone(), id.to_owned());
        async move {
            let mut guard = book.lock().unwrap();
            take(&mut guard, format!("activity {id}"))?;
            let alive = guard.alive(&id);
            let key = guard
                .entries
                .iter()
                .find(|entry| entry.id == id && (alive || entry.listed))
                .map(|entry| entry.start.key.clone());
            Ok(key.map(|key| adopt(&book, &mut guard, &id, &key)))
        }
    }

    fn activities(&self) -> impl Future<Output = activitykit::Result<Vec<FakeHandle>>> + Send {
        let book = self.0.clone();
        async move {
            let mut guard = book.lock().unwrap();
            take(&mut guard, "list".into())?;
            let named: Vec<(String, String)> = guard
                .entries
                .iter()
                .filter(|entry| entry.listed)
                .map(|entry| (entry.id.clone(), entry.start.key.clone()))
                .collect();
            Ok(named
                .iter()
                .map(|(id, key)| adopt(&book, &mut guard, id, key))
                .collect())
        }
    }

    fn write_image(&self, name: &str, _png: &[u8]) -> activitykit::Result<()> {
        take(&mut self.book(), format!("image {name}"))
    }
}

impl FakeHandle {
    fn entry(&self) -> Option<Entry> {
        self.book
            .lock()
            .unwrap()
            .entries
            .iter()
            .find(|entry| entry.id == self.id)
            .cloned()
    }
}

impl Handle for FakeHandle {
    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> &str {
        &self.key
    }

    fn state(&self) -> State {
        self.entry().map_or(State::Unknown, |entry| entry.state)
    }

    fn values(&self) -> Values {
        self.entry().map(|entry| entry.values).unwrap_or_default()
    }

    fn revision(&self) -> Option<u64> {
        self.entry().map(|entry| entry.revision)
    }

    fn stale_at(&self) -> Option<u64> {
        self.entry().and_then(|entry| entry.stale_at)
    }

    fn timestamp(&self) -> Option<u64> {
        self.entry().and_then(|entry| entry.timestamp)
    }

    fn relevance(&self) -> f64 {
        self.entry()
            .and_then(|entry| entry.relevance)
            .unwrap_or(0.0)
    }

    fn degraded_features(&self) -> Vec<String> {
        self.entry().map(|entry| entry.degraded).unwrap_or_default()
    }

    fn update(&self, update: UpdateSpec) -> impl Future<Output = activitykit::Result<()>> + Send {
        let (book, id) = (self.book.clone(), self.id.clone());
        async move {
            let mut guard = book.lock().unwrap();
            take(&mut guard, format!("update {id}"))?;
            // A pending activity begins with the content of its start, whatever is sent meanwhile.
            let pending = |entry: &Entry| entry.id == id && entry.state == State::Pending;
            if guard.entries.iter().any(pending) {
                return Err(Error::Conflict(
                    "A scheduled activity takes no update before it began.".into(),
                ));
            }
            let entry = guard.entries.iter_mut().find(|entry| {
                entry.id == id && matches!(entry.state, State::Active | State::Stale)
            });
            let Some(entry) = entry else {
                return Err(Error::NotFound(
                    "The activity is no longer available.".into(),
                ));
            };
            if takes(entry, update.revision, &update.values, update.stale_at)? {
                entry.revision = update.revision;
                entry.values = update.values.clone();
                entry.stale_at = update.stale_at;
                entry.timestamp = Some(update.timestamp.unwrap_or(2_000));
                // Without a score the activity keeps its own.
                entry.relevance = update.relevance.or(entry.relevance);
            }
            guard.updates.push(update);
            Ok(())
        }
    }

    fn end(&self, end: EndSpec) -> impl Future<Output = activitykit::Result<()>> + Send {
        let (book, id) = (self.book.clone(), self.id.clone());
        async move {
            let mut guard = book.lock().unwrap();
            take(&mut guard, format!("end {id}"))?;
            let Some(entry) = guard.entries.iter_mut().find(|entry| entry.id == id) else {
                return Err(Error::NotFound(
                    "The activity is no longer available.".into(),
                ));
            };
            if !matches!(entry.state, State::Ended | State::Dismissed) {
                let values = end.values.clone().unwrap_or_else(|| entry.values.clone());
                let revision = end.revision.unwrap_or(entry.revision + 1);
                takes(entry, revision, &values, end.stale_at).map(|_| ())?;
                entry.revision = revision;
                entry.values = values;
                entry.stale_at = end.stale_at;
                entry.timestamp = Some(end.timestamp.unwrap_or(3_000));
                entry.relevance = end.relevance.or(entry.relevance);
                entry.state = State::Ended;
                entry.listed = !matches!(end.policy, crate::models::EndPolicy::Immediate {});
            }
            guard.ends.push(end);
            Ok(())
        }
    }
}
