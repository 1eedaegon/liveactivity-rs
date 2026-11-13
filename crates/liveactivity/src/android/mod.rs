//! The default Android backend: the `liveupdate` engine, used as it is.
//!
//! [`LiveUpdateBackend`] runs the progress and timer templates on the engine, which keeps the lifecycle, the
//! revisions and the receipts. Given the folder of the app's files ([`AndroidFiles`]), it also runs layouts, which
//! it projects onto the engine's notification styles, stores images, and reports taps on buttons.
//!
//! The engine stores what the person does to a notification, and keeps an account of the state of each activity
//! that it has not answered yet. The backend hands both to the event sink: the stored events when it is asked to,
//! and the states then and after each of its own calls. So the sink learns once where an activity stands, as on
//! iOS, one event at a time and in the order the engine answered. The backend has no timer:
//! [`LiveUpdateBackend::next_check`] says when a host that has one should ask again.
//!
//! The caller supplies the transport: a `liveupdate::Driver`, a string `liveupdate::Bridge` over a framework's
//! own native call, or the JNI transport of liveupdate's AAR. Android's permission dialog needs an Activity, so a
//! host that shows it implements `request_permissions` in a backend of its own around this one.

mod convert;
mod layout;
mod layouts;

use crate::{
    ActivityRecord, BackendContext, BackendFuture, Capabilities, EndInput, Error, Event,
    LayoutActivityRecord, LayoutEndInput, LayoutStartInput, LayoutUpdateInput, LiveActivityBackend,
    Permissions, Result, StartInput, UpdateInput, image,
};
use layouts::{FileStore, Layouts, Settings};
use liveupdate::{Bridge, Driver, EventKind, JsonDriver, LiveUpdate};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Mutex, MutexGuard, OnceLock, PoisonError},
};

/// Where the backend keeps what layouts need, under the folder of the app's files (`Context.getFilesDir()`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AndroidFiles {
    /// The title of a layout without a text.
    pub app_name: String,
    /// The folder of the app's files.
    pub files: PathBuf,
    /// The file of the layout activities, relative to `files`.
    pub layout_store: PathBuf,
    /// The folder of `write_image`, relative to `files`. liveupdate's AAR reads pictures from it.
    pub image_folder: String,
    /// Adds Samsung One UI's Now Bar text extras to layout notifications on Android 17 and later.
    pub samsung_now_bar: bool,
}

impl AndroidFiles {
    /// The files of `app_name` under `files`, in a folder of this crate.
    pub fn new(app_name: impl Into<String>, files: impl Into<PathBuf>) -> Self {
        Self {
            app_name: app_name.into(),
            files: files.into(),
            layout_store: PathBuf::from("liveactivity/layouts.json"),
            image_folder: "liveactivity/images".into(),
            samsung_now_bar: false,
        }
    }
}

/// `LiveActivityBackend` over the liveupdate engine.
///
/// The engine calls its driver synchronously: a call completes on its first poll and runs the native call on the
/// polling thread.
pub struct LiveUpdateBackend<D: Driver> {
    engine: LiveUpdate<D>,
    /// The layouts, once the backend has the app's files.
    layouts: Option<(Layouts<D, FileStore>, PathBuf)>,
    /// Where taps go, once the backend is connected.
    events: OnceLock<BackendContext>,
    /// What the sink is still to receive. Held from a question to the engine until its answer is queued, so
    /// that the queue has the order of the engine's answers; never while the sink runs.
    outbox: Mutex<Outbox>,
}

/// The events the sink is still to receive, oldest first.
#[derive(Default)]
struct Outbox {
    queued: VecDeque<Event>,
    /// Whether a thread is handing the queue to the sink. Another thread leaves its events to that one.
    handing_over: bool,
}

/// Ends a hand-over that a panicking sink left, so that the next ask goes on with the rest of the queue.
struct HandOver<'a> {
    outbox: &'a Mutex<Outbox>,
    /// Whether the hand-over has not come to its end. One that has, has said so under the lock, and another
    /// may have begun.
    unfinished: bool,
}

impl Drop for HandOver<'_> {
    fn drop(&mut self) {
        if self.unfinished {
            let mut outbox = self.outbox.lock().unwrap_or_else(PoisonError::into_inner);
            outbox.handing_over = false;
        }
    }
}

impl<D: Driver + 'static> LiveUpdateBackend<D> {
    /// The backend on `driver`, which is the Android side. It runs templates; see [`with_files`](Self::with_files)
    /// for layouts.
    pub fn new(driver: D) -> Self {
        Self {
            engine: LiveUpdate::new(driver),
            layouts: None,
            events: OnceLock::new(),
            outbox: Mutex::default(),
        }
    }

    /// The same backend with the app's files, which lets it run layouts, store images and report taps on buttons.
    #[must_use]
    pub fn with_files(self, files: AndroidFiles) -> Self {
        let images = files.files.join(&files.image_folder);
        let store = FileStore {
            path: files.files.join(&files.layout_store),
        };
        let settings = Settings {
            app_name: files.app_name,
            samsung_now_bar: files.samsung_now_bar,
            files: Some(files.files),
            image_folder: files.image_folder,
        };
        Self {
            layouts: Some((Layouts::new(settings, store), images)),
            ..self
        }
    }

    /// The wrapped engine, for what liveupdate offers beyond the contract.
    pub fn engine(&self) -> &LiveUpdate<D> {
        &self.engine
    }

    /// Hands the event sink what Android stored, the taps and the openings, oldest first, and then the state of
    /// each activity that the sink was not told yet. Answers how many events it took for the sink. A host calls
    /// this, or `sync_events`, when its Android side signals a stored event, when the application starts or
    /// resumes, and at the time [`next_check`](Self::next_check) names: liveupdate's receiver stores taps also
    /// while the app does not run, and nobody tells Android's side that a stale time has come.
    ///
    /// The sink receives one event at a time, in the order they were taken. When another thread is handing
    /// events over at that moment, that thread hands these over as well, and this call returns before they
    /// arrived.
    ///
    /// # Errors
    ///
    /// Fails when the drain or the question for the states fails. The stored events that it took before the
    /// states failed reach the sink all the same; the states stay due for the next call.
    pub fn pump_events(&self) -> Result<usize> {
        // Not connected yet: the taps stay queued, and the states due, for the sink that comes.
        let Some(sink) = self.events.get() else {
            return Ok(0);
        };
        let taken = {
            let mut outbox = self.outbox();
            let stored = self.stored()?;
            let count = stored.len();
            outbox.queued.extend(stored);
            self.states().map(|states| {
                let count = count + states.len();
                outbox.queued.extend(states);
                count
            })
        };
        self.hand_over(sink);
        taken
    }

    /// When it is worth calling [`pump_events`](Self::pump_events) again for a state that no call of the
    /// application brings. The backend runs no timer: a host that has a thread waits as this says.
    ///
    /// - `Now`: a state is due.
    /// - `Soon`: Android has not shown a post yet. Ask shortly, a little later each time, and give up after a few.
    /// - `At`: the earliest stale time ahead, in epoch milliseconds. One that has passed means now.
    /// - `Never`: nothing until the next call of the application or the next stored event.
    ///
    /// It reads liveupdate's receipts and asks Android nothing, so it does not know that the person removed a
    /// notification: the signal of the stored event is what prompts that ask.
    ///
    /// # Errors
    ///
    /// Fails when the receipts cannot be read.
    pub fn next_check(&self) -> Result<liveupdate::NextCheck> {
        self.engine.next_check().map_err(convert::error)
    }

    /// The events Android stored, as the sink receives them, oldest first. Taking them removes them there.
    fn stored(&self) -> Result<Vec<Event>> {
        match &self.layouts {
            // With the layouts' store an event names its activity's key, and a tap on an undeclared action is
            // dropped.
            Some((layouts, _)) => layouts.events(&self.engine),
            None => Ok(self
                .engine
                .events()
                .map_err(convert::error)?
                .events
                .into_iter()
                .filter_map(|event| match event.kind {
                    EventKind::Action { action_id } => Some(Event::Action(crate::ActionEvent {
                        action_id,
                        activity_id: event.activity_id,
                        activity_key: None,
                        on: None,
                    })),
                    EventKind::Opened { action_id } => Some(Event::Opened(crate::OpenedEvent {
                        activity_id: event.activity_id,
                        activity_key: None,
                        action_id,
                    })),
                    // A dismissal is reported with the other states: its stored event is the prompt to ask.
                    // And a kind that a later liveupdate adds, which the contract has no event for.
                    _ => None,
                })
                .collect()),
        }
    }

    /// The state of each activity that liveupdate has not answered yet, as the sink receives it. liveupdate
    /// answers a state once, whoever asks, and marks it answered before this returns: what is taken here is
    /// queued for the sink, or lost.
    fn states(&self) -> Result<Vec<Event>> {
        let changed = self.engine.state_changes().map_err(convert::error)?;
        let reported = |record: liveupdate::ActivityRecord| {
            Event::State(crate::StateEvent {
                activity_id: record.id,
                activity_key: Some(record.activity_key),
                state: convert::activity_state(record.state),
            })
        };
        Ok(changed.into_iter().map(reported).collect())
    }

    /// After a call of the application that looked at what Android shows: reports the states that are due, such
    /// as the end of an activity or its first showing, whatever the call answered. Without a sink nothing is
    /// asked. liveupdate is asked for the states only when one is due, which costs one read of the receipts; a
    /// failure here leaves them due for the next ask and changes nothing of the call's answer.
    fn after<T>(&self, answer: Result<T>) -> Result<T> {
        let Some(sink) = self.events.get() else {
            return answer;
        };
        if matches!(self.engine.next_check(), Ok(liveupdate::NextCheck::Now)) {
            let mut outbox = self.outbox();
            if let Ok(states) = self.states() {
                outbox.queued.extend(states);
            }
        }
        self.hand_over(sink);
        answer
    }

    /// Hands the sink what is queued, one event at a time and with no lock held, so that a sink may call the
    /// backend. When another thread is doing so, that thread hands over what this one queued as well.
    fn hand_over(&self, sink: &BackendContext) {
        {
            let mut outbox = self.outbox();
            if outbox.handing_over {
                return;
            }
            outbox.handing_over = true;
        }
        let mut hand_over = HandOver {
            outbox: &self.outbox,
            unfinished: true,
        };
        loop {
            let next = {
                let mut outbox = self.outbox();
                let next = outbox.queued.pop_front();
                // Said under the lock that the queue was found empty under: whoever queues after this hands over.
                outbox.handing_over = next.is_some();
                next
            };
            let Some(event) = next else {
                hand_over.unfinished = false;
                return;
            };
            sink.emit(event);
        }
    }

    fn outbox(&self) -> MutexGuard<'_, Outbox> {
        self.outbox.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn layouts(&self) -> Result<&Layouts<D, FileStore>> {
        self.layouts.as_ref().map(|(layouts, _)| layouts).ok_or_else(|| {
            Error::unsupported_feature(
                "Layouts need the folder of the app's files: connect the backend with `with_files`.",
            )
        })
    }
}

impl<B: Bridge + 'static> LiveUpdateBackend<JsonDriver<B>> {
    /// The backend over a framework's string transport to the Android side.
    pub fn over_bridge(bridge: B) -> Self {
        Self::new(JsonDriver::new(bridge))
    }
}

#[cfg(target_os = "android")]
impl LiveUpdateBackend<liveupdate::AndroidDriver> {
    /// The backend on the JNI transport of liveupdate's own AAR. The host must have initialized `ndk-context` and
    /// package the AAR.
    ///
    /// # Errors
    ///
    /// Fails when the Android context or the AAR's bridge class is not available.
    pub fn android() -> Result<Self> {
        let bridge = liveupdate::AndroidBridge::new().map_err(convert::error)?;
        Ok(Self::over_bridge(bridge))
    }
}

impl<D: Driver + 'static> LiveActivityBackend for LiveUpdateBackend<D> {
    fn attach(&self, events: BackendContext) -> Result<()> {
        // A backend is connected once.
        self.events
            .set(events)
            .map_err(|_| Error::already_connected())
    }

    fn capabilities(&self) -> BackendFuture<'_, Capabilities> {
        Box::pin(async move {
            self.engine
                .capabilities()
                .map(|value| convert::capabilities(value, self.layouts.is_some()))
                .map_err(convert::error)
        })
    }

    /// The engine reads permissions and cannot prompt: the host shows Android's dialog.
    fn permissions(&self) -> BackendFuture<'_, Permissions> {
        Box::pin(async move {
            self.engine
                .capabilities()
                .map(convert::permissions)
                .map_err(convert::error)
        })
    }

    fn start(&self, input: StartInput) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move {
            let started = self
                .engine
                .start(convert::start_request(input))
                .map_err(convert::error)
                .and_then(convert::activity_record);
            self.after(started)
        })
    }

    fn update(&self, input: UpdateInput) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move {
            let updated = self
                .engine
                .update(&input.id, convert::live_snapshot(input.snapshot))
                .map_err(convert::error)
                .and_then(convert::activity_record);
            self.after(updated)
        })
    }

    fn end(&self, input: EndInput) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move {
            let ended = self
                .engine
                .end(&input.id, convert::end_policy(input.policy))
                .map_err(convert::error)
                .and_then(convert::activity_record);
            self.after(ended)
        })
    }

    /// The progress and timer activities; layout activities share the engine's list and are left out.
    fn list(&self) -> BackendFuture<'_, Vec<ActivityRecord>> {
        Box::pin(async move {
            let listed = self.engine.list().map_err(convert::error);
            self.after(listed.and_then(convert::v1_records))
        })
    }

    fn start_layout(&self, input: LayoutStartInput) -> BackendFuture<'_, LayoutActivityRecord> {
        Box::pin(async move { self.after(self.layouts()?.start(&self.engine, input)) })
    }

    fn update_layout(&self, input: LayoutUpdateInput) -> BackendFuture<'_, LayoutActivityRecord> {
        Box::pin(async move { self.after(self.layouts()?.update(&self.engine, input)) })
    }

    fn end_layout(&self, input: LayoutEndInput) -> BackendFuture<'_, LayoutActivityRecord> {
        Box::pin(async move { self.after(self.layouts()?.end(&self.engine, input)) })
    }

    fn list_layout(&self) -> BackendFuture<'_, Vec<LayoutActivityRecord>> {
        Box::pin(async move {
            match &self.layouts {
                Some((layouts, _)) => self.after(layouts.list(&self.engine)),
                None => Ok(Vec::new()),
            }
        })
    }

    /// The file goes to the app's files, where the first such image of a layout becomes the notification's large
    /// icon when it is at most 1024 pixels a side and 1 MiB.
    fn write_image(&self, name: String, png: Vec<u8>) -> BackendFuture<'_, ()> {
        Box::pin(async move {
            let Some((_, images)) = &self.layouts else {
                return Err(Error::unsupported_feature(
                    "Images need the folder of the app's files: connect the backend with `with_files`.",
                ));
            };
            std::fs::create_dir_all(images).map_err(|_| Error::native())?;
            image::write(images, &name, &png)
        })
    }

    fn sync_events(&self) -> BackendFuture<'_, ()> {
        Box::pin(async move { self.pump_events().map(|_| ()) })
    }
}
