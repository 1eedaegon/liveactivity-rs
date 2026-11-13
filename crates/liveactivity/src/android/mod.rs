//! The default Android backend: the `liveupdate` engine, used as it is.
//!
//! [`LiveUpdateBackend`] runs the progress and timer templates on the engine, which keeps the lifecycle, the
//! revisions and the receipts. Given the folder of the app's files ([`AndroidFiles`]), it also runs layouts, which
//! it projects onto the engine's notification styles, stores images, and reports taps on buttons.
//!
//! The engine stores what the person does to a notification, and the backend hands that to the event sink when it
//! is asked to. An end is the application's own call, which the engine stores nothing for: the backend reports it
//! itself, so that the sink learns of an end as it does on iOS.
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
    path::PathBuf,
    sync::{Mutex, OnceLock, PoisonError},
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
    /// Held for one whole drain, so that taps reach the sink in order.
    pump: Mutex<()>,
    /// Held while an end asks whether its activity runs and then ends it, so that one end reports it.
    ending: Mutex<()>,
}

impl<D: Driver + 'static> LiveUpdateBackend<D> {
    /// The backend on `driver`, which is the Android side. It runs templates; see [`with_files`](Self::with_files)
    /// for layouts.
    pub fn new(driver: D) -> Self {
        Self {
            engine: LiveUpdate::new(driver),
            layouts: None,
            events: OnceLock::new(),
            pump: Mutex::new(()),
            ending: Mutex::new(()),
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

    /// Drains the taps, dismissals and openings Android queued and hands each to the event sink, oldest first. Answers how
    /// many it handed over. A host calls this, or `sync_events`, when its Android side signals a stored event and when the
    /// application starts or resumes: liveupdate's receiver stores taps also while the app does not run.
    ///
    /// # Errors
    ///
    /// Fails when the drain fails.
    pub fn pump_events(&self) -> Result<usize> {
        // Not connected yet: the taps stay queued for the sink that comes.
        let Some(events) = self.events.get() else {
            return Ok(0);
        };
        let _pump = self.pump.lock().unwrap_or_else(PoisonError::into_inner);
        let queued = match &self.layouts {
            // With the layouts' store an event names its activity's key, and a tap on an undeclared action is
            // dropped.
            Some((layouts, _)) => layouts.events(&self.engine)?,
            None => self
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
                    })),
                    EventKind::Dismissed => Some(Event::State(crate::StateEvent {
                        activity_id: event.activity_id,
                        activity_key: None,
                        state: crate::ActivityState::Dismissed,
                    })),
                    EventKind::Opened { action_id } => Some(Event::Opened(crate::OpenedEvent {
                        activity_id: event.activity_id,
                        activity_key: None,
                        action_id,
                    })),
                    // A kind that a later liveupdate adds, and the contract has no event for.
                    _ => None,
                })
                .collect(),
        };
        let count = queued.len();
        queued.into_iter().for_each(|event| events.emit(event));
        Ok(count)
    }

    /// Runs `end`, an end of the activity `id`, and tells the sink that the activity ended when it was running
    /// before: an end of an activity that is over answers its record and ends nothing. The engine stores no event
    /// for an end, so the backend reports it. The sink is called after the end, outside the backend's locks.
    fn ending<T>(
        &self,
        id: &str,
        end: impl FnOnce() -> Result<T>,
        key: impl FnOnce(&T) -> String,
    ) -> Result<T> {
        let (ended, was_running) = {
            let _one_end = self.ending.lock().unwrap_or_else(PoisonError::into_inner);
            let was_running = self.runs(id);
            (end()?, was_running)
        };
        if was_running && let Some(events) = self.events.get() {
            events.emit(Event::State(crate::StateEvent {
                activity_id: id.to_owned(),
                activity_key: Some(key(&ended)),
                state: crate::ActivityState::Ended,
            }));
        }
        Ok(ended)
    }

    /// Whether the engine lists the activity `id` as one that is not over. A list that fails says nothing of the
    /// activity, and then no end is reported.
    fn runs(&self, id: &str) -> bool {
        self.engine.list().is_ok_and(|records| {
            records.iter().any(|record| {
                record.id == id
                    && !matches!(
                        record.state,
                        liveupdate::ActivityState::Ended | liveupdate::ActivityState::Dismissed
                    )
            })
        })
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
            self.engine
                .start(convert::start_request(input))
                .map_err(convert::error)
                .and_then(convert::activity_record)
        })
    }

    fn update(&self, input: UpdateInput) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move {
            self.engine
                .update(&input.id, convert::live_snapshot(input.snapshot))
                .map_err(convert::error)
                .and_then(convert::activity_record)
        })
    }

    fn end(&self, input: EndInput) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move {
            let end = || {
                self.engine
                    .end(&input.id, convert::end_policy(input.policy))
                    .map_err(convert::error)
                    .and_then(convert::activity_record)
            };
            self.ending(&input.id, end, |record| record.activity_key.clone())
        })
    }

    /// The progress and timer activities; layout activities share the engine's list and are left out.
    fn list(&self) -> BackendFuture<'_, Vec<ActivityRecord>> {
        Box::pin(async move {
            let records = self.engine.list().map_err(convert::error)?;
            convert::v1_records(records)
        })
    }

    fn start_layout(&self, input: LayoutStartInput) -> BackendFuture<'_, LayoutActivityRecord> {
        Box::pin(async move { self.layouts()?.start(&self.engine, input) })
    }

    fn update_layout(&self, input: LayoutUpdateInput) -> BackendFuture<'_, LayoutActivityRecord> {
        Box::pin(async move { self.layouts()?.update(&self.engine, input) })
    }

    fn end_layout(&self, input: LayoutEndInput) -> BackendFuture<'_, LayoutActivityRecord> {
        Box::pin(async move {
            let layouts = self.layouts()?;
            let id = input.id.clone();
            let end = || layouts.end(&self.engine, input);
            self.ending(&id, end, |record| record.activity_key.clone())
        })
    }

    fn list_layout(&self) -> BackendFuture<'_, Vec<LayoutActivityRecord>> {
        Box::pin(async move {
            match &self.layouts {
                Some((layouts, _)) => layouts.list(&self.engine),
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
