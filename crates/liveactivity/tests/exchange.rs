//! One caller on exchanged backends: the default Android backend (over a driver and over a framework's string
//! bridge) and a backend that is neither default engine. The backend is named in one place, the connection, and
//! nothing else of the caller changes.
#![cfg(feature = "testkit")]

mod common;

use common::{MemoryDriver, StringBridge};
use liveactivity::{
    ActionEvent, ActivityRecord, ActivityState, Backend, BackendFuture, Capabilities, EndInput,
    Error, Event, FeatureSupport, LiveActivity, LiveActivityBackend, PermissionKind,
    PermissionState, Permissions, StartInput, UpdateInput,
    android::LiveUpdateBackend,
    block_on,
    testkit::{MemoryBackend, conformance},
};
use std::sync::{
    Arc,
    mpsc::{self, Receiver},
};

/// A connected service and the events its sink received.
type Connected = (LiveActivity, Receiver<Event>);

/// What an adapter does: it names the backend. The sink is any closure.
fn connect(backend: impl LiveActivityBackend) -> Connected {
    let (sender, taps) = mpsc::channel();
    let live = LiveActivity::builder()
        .backend(backend)
        .event_sink(move |event: Event| sender.send(event).unwrap())
        .build()
        .unwrap();
    (live, taps)
}

fn android_default() -> Connected {
    connect(LiveUpdateBackend::new(MemoryDriver::default()))
}

fn android_over_a_framework_bridge() -> Connected {
    connect(LiveUpdateBackend::over_bridge(StringBridge(
        MemoryDriver::default(),
    )))
}

fn substitute() -> Connected {
    connect(MemoryBackend::new())
}

/// Takes the event that follows a start: the first state of the activity `id`, which the platform shows.
fn shown(events: &Receiver<Event>, id: &str) {
    assert!(matches!(
        events.try_recv().unwrap(),
        Event::State(change) if change.activity_id == id && change.state == ActivityState::Active
    ));
}

/// The one line of an application that names a backend.
type Connect = fn() -> Connected;

const BACKENDS: [(&str, Connect); 3] = [
    ("android", android_default),
    ("android", android_over_a_framework_bridge),
    ("memory", substitute),
];

/// An application's use of the interface, written once. It names no backend.
fn deliver(live: &LiveActivity) -> Vec<(u64, String, ActivityState)> {
    let seen = |record: ActivityRecord| {
        (
            record.snapshot.revision,
            record.snapshot.status,
            record.state,
        )
    };
    let order = block_on(live.start(StartInput::progress(
        "order:42",
        "Order 42",
        "Preparing",
        0.1,
    )))
    .unwrap();
    let updated = block_on(live.update(UpdateInput::progress(
        &order.id,
        1,
        "Order 42",
        "On the way",
        0.6,
    )))
    .unwrap();
    assert_eq!(block_on(live.list()).unwrap().len(), 1);
    let ended = block_on(live.end(EndInput::immediate(&order.id))).unwrap();
    vec![seen(order), seen(updated), seen(ended)]
}

#[test]
fn req_x_001_every_backend_keeps_the_rules_of_the_interface() {
    // REQ-X-001
    for (platform, connect) in BACKENDS {
        let (live, _taps) = connect();
        let report = block_on(conformance(&live));
        assert_eq!(report.platform, platform);
        report.assert_passed();
    }
}

#[test]
fn req_x_002_one_caller_gets_the_same_answers_from_exchanged_backends() {
    // REQ-X-002
    let answers: Vec<_> = BACKENDS
        .iter()
        .map(|(_, connect)| deliver(&connect().0))
        .collect();

    assert_eq!(
        answers[0],
        [
            (0, "Preparing".into(), ActivityState::Active),
            (1, "On the way".into(), ActivityState::Active),
            (1, "On the way".into(), ActivityState::Ended),
        ]
    );
    assert!(answers.iter().all(|answer| answer == &answers[0]));
}

#[test]
fn req_x_003_what_a_backend_lacks_is_reported_and_refused() {
    // REQ-X-003
    let (android, _taps) = android_default();
    let capabilities = block_on(android.capabilities()).unwrap();
    assert_eq!(capabilities.platform, "android");
    assert_eq!(capabilities.backend, Backend::AndroidNotification);
    assert_eq!(capabilities.progress, FeatureSupport::Supported);
    // The default Android backend draws templates: layouts, images and the permission dialog are a host's part.
    assert_eq!(capabilities.layout, FeatureSupport::Unsupported);
    assert_eq!(
        capabilities.native_live_activity,
        FeatureSupport::Unsupported
    );
    assert!(block_on(android.list_layout()).unwrap().is_empty());
    let png = one_pixel();
    assert_eq!(
        block_on(android.write_image("cover", &png))
            .unwrap_err()
            .code,
        "UNSUPPORTED_FEATURE"
    );
    assert_eq!(
        block_on(android.request_permissions(PermissionKind::Notifications))
            .unwrap_err()
            .code,
        "UNSUPPORTED_FEATURE"
    );
    assert_eq!(
        block_on(android.permissions()).unwrap(),
        Permissions {
            notifications: PermissionState::Granted,
            live_activities: PermissionState::NotApplicable,
            promotion: PermissionState::Denied,
            frequent_pushes: PermissionState::NotApplicable,
        }
    );
    // The image is checked before any backend sees it.
    assert_eq!(
        block_on(android.write_image("cover", b"not a png"))
            .unwrap_err()
            .code,
        "INVALID_INPUT"
    );

    let (memory, _taps) = substitute();
    block_on(memory.write_image("cover", &png)).unwrap();
}

#[test]
fn req_x_004_taps_of_any_backend_reach_the_same_sink() {
    // REQ-X-004
    // Android queues its taps: the caller asks for them, for example on resume.
    let driver = MemoryDriver::default();
    let (android, taps) = connect(LiveUpdateBackend::new(driver.clone()));
    let order = block_on(android.start(StartInput::progress(
        "order:42",
        "Order 42",
        "Preparing",
        0.1,
    )))
    .unwrap();
    shown(&taps, &order.id);
    driver.tap(&order.id, 1, "track");
    driver.tap("gone", 2, "track");
    block_on(android.sync_events()).unwrap();
    assert_eq!(
        taps.try_recv().unwrap(),
        Event::Action(ActionEvent {
            action_id: "track".into(),
            activity_id: order.id.clone(),
            // Without the app's files the backend keeps no key of its own.
            activity_key: None,
            on: None,
            // REQ-E-009: what the adapter says of the stored tap.
            tap: stored(1),
        })
    );
    assert!(matches!(taps.try_recv().unwrap(), Event::Action(tap) if tap.activity_id == "gone"));
    block_on(android.sync_events()).unwrap();
    assert!(taps.try_recv().is_err());

    // A backend that calls back reaches the same sink. An adapter that needs its backend later shares it.
    let backend = Arc::new(MemoryBackend::new());
    let (live, taps) = connect(backend.clone());
    let order = block_on(live.start(StartInput::progress(
        "order:42",
        "Order 42",
        "Preparing",
        0.1,
    )))
    .unwrap();
    shown(&taps, &order.id);
    backend.tap(&order.id, "track").unwrap();
    assert!(matches!(taps.try_recv().unwrap(), Event::Action(tap) if tap.action_id == "track"));
    assert_eq!(
        backend.tap("unknown", "track").unwrap_err().code,
        "NOT_FOUND"
    );
}

#[test]
fn req_x_006_activities_outside_the_template_contract_do_not_break_the_list() {
    // REQ-X-006
    let driver = MemoryDriver::default();
    let (android, _taps) = connect(LiveUpdateBackend::new(driver.clone()));
    let order = block_on(android.start(StartInput::progress(
        "order:42",
        "Order 42",
        "Preparing",
        0.1,
    )))
    .unwrap();

    // An ended activity that the application started on the engine itself, with a rich presentation.
    let mut rich = serde_json::to_value(&driver.lock().records[0]).unwrap();
    rich["id"] = "rich".into();
    rich["state"] = "ended".into();
    for snapshot in ["/origin/snapshot", "/snapshot"] {
        let snapshot = rich.pointer_mut(snapshot).unwrap().as_object_mut().unwrap();
        snapshot.remove("progress");
        snapshot.insert(
            "presentation".into(),
            serde_json::json!({"style": {"kind": "standard"}}),
        );
    }
    rich["origin"]["activityKey"] = "rich:1".into();
    rich["origin"]["schemaVersion"] = 2.into();
    rich["origin"]["template"] = "standard".into();
    driver
        .lock()
        .records
        .push(serde_json::from_value(rich).unwrap());

    let listed = block_on(android.list()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, order.id);
    assert_eq!(android_engine_records(&driver), 2);
}

fn android_engine_records(driver: &MemoryDriver) -> usize {
    driver.lock().records.len()
}

/// A host's own backend: it hands the templates to a default backend and answers the rest itself, which is how a
/// framework plugin with its own native side fits the interface.
struct HostBackend {
    templates: LiveUpdateBackend<MemoryDriver>,
}

impl LiveActivityBackend for HostBackend {
    fn attach(&self, events: liveactivity::BackendContext) -> liveactivity::Result<()> {
        self.templates.attach(events)
    }

    fn capabilities(&self) -> BackendFuture<'_, Capabilities> {
        Box::pin(async move {
            let capabilities = self.templates.capabilities().await?;
            Ok(Capabilities {
                layout: FeatureSupport::Supported,
                ..capabilities
            })
        })
    }

    fn permissions(&self) -> BackendFuture<'_, Permissions> {
        self.templates.permissions()
    }

    fn request_permissions(&self, _kind: PermissionKind) -> BackendFuture<'_, Permissions> {
        // The host shows the dialog on its own native side.
        self.templates.permissions()
    }

    fn start(&self, input: StartInput) -> BackendFuture<'_, ActivityRecord> {
        self.templates.start(input)
    }

    fn update(&self, input: UpdateInput) -> BackendFuture<'_, ActivityRecord> {
        self.templates.update(input)
    }

    fn end(&self, input: EndInput) -> BackendFuture<'_, ActivityRecord> {
        self.templates.end(input)
    }

    fn list(&self) -> BackendFuture<'_, Vec<ActivityRecord>> {
        self.templates.list()
    }

    fn write_image(&self, name: String, _png: Vec<u8>) -> BackendFuture<'_, ()> {
        Box::pin(async move {
            if name == "cover" {
                Ok(())
            } else {
                Err(Error::native())
            }
        })
    }

    // A backend that wraps another forwards what it does not answer itself.
    fn open_promotion_settings(&self) -> BackendFuture<'_, ()> {
        self.templates.open_promotion_settings()
    }
}

#[test]
fn req_x_005_a_host_backend_composes_a_default_one_and_stays_exchangeable() {
    // REQ-X-005
    let (live, _taps) = connect(HostBackend {
        templates: LiveUpdateBackend::new(MemoryDriver::default()),
    });

    block_on(conformance(&live)).assert_passed();
    assert_eq!(
        block_on(live.capabilities()).unwrap().layout,
        FeatureSupport::Supported
    );
    block_on(live.write_image("cover", &one_pixel())).unwrap();
    block_on(live.request_permissions(PermissionKind::Notifications)).unwrap();
    // REQ-A-033: the settings of promotion reach the default backend that it wraps.
    assert_eq!(block_on(live.open_promotion_settings()), Ok(()));
}

/// The smallest PNG: one transparent pixel.
fn one_pixel() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ]
}

/// A folder of its own for one test, as the app's files.
fn app_files(test: &str) -> std::path::PathBuf {
    let folder = std::env::temp_dir().join(format!("liveactivity-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

#[test]
fn req_x_007_with_the_apps_files_the_android_default_runs_layouts_images_and_keyed_taps() {
    // REQ-X-007
    let files = app_files("layouts");
    let driver = MemoryDriver::default();
    let backend = LiveUpdateBackend::new(driver.clone())
        .with_files(liveactivity::android::AndroidFiles::new("Mochi", &files));
    let (live, taps) = connect(backend);

    // The templates keep their rules, and the capabilities now name layouts and buttons.
    block_on(conformance(&live)).assert_passed();
    let capabilities = block_on(live.capabilities()).unwrap();
    assert_eq!(capabilities.layout, FeatureSupport::Supported);
    assert_eq!(capabilities.actions, FeatureSupport::Supported);

    let start = liveactivity::parse_request(
        "start_layout",
        serde_json::json!({
            "activityKey": "pet:1",
            "schemaVersion": 2,
            "fallback": "notification",
            "layout": { "lockScreen": { "type": "vstack", "children": [
                { "type": "text", "bind": "title" },
                { "type": "progress", "bind": "done" },
                { "type": "button", "actionId": "pet", "child": { "type": "text", "value": "Pet" } },
            ] } },
            "snapshot": { "revision": 0, "values": { "title": "Mochi", "done": 0.2 } },
        }),
    )
    .unwrap();
    let record = block_on(live.start_layout(start)).unwrap();
    assert_eq!(record.activity_key, "pet:1");
    assert_eq!(record.state, ActivityState::Active);
    // A layout activity is listed with the layouts and not with the templates.
    let listed = block_on(live.list_layout()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, record.id);
    assert!(
        block_on(live.list())
            .unwrap()
            .iter()
            .all(|template| template.id != record.id)
    );

    let update = liveactivity::parse_request(
        "update_layout",
        serde_json::json!({
            "id": record.id,
            "snapshot": { "revision": 1, "values": { "title": "Mochi", "done": 0.7 } },
        }),
    )
    .unwrap();
    assert_eq!(
        block_on(live.update_layout(update))
            .unwrap()
            .snapshot
            .revision,
        1
    );

    // The conformance run started and ended activities, and this test started one: each state was reported.
    let states: Vec<Event> = taps.try_iter().collect();
    let count = |state: ActivityState| {
        let of_that_state =
            |event: &&Event| matches!(event, Event::State(change) if change.state == state);
        states.iter().filter(of_that_state).count()
    };
    assert!(count(ActivityState::Ended) > 0);
    assert_eq!(
        count(ActivityState::Active) + count(ActivityState::Ended),
        states.len()
    );

    // A tap on a declared button names its activity's key; one on an undeclared action is dropped.
    driver.tap(&record.id, 1, "pet");
    driver.tap(&record.id, 2, "undeclared");
    block_on(live.sync_events()).unwrap();
    assert_eq!(
        taps.try_recv().unwrap(),
        Event::Action(ActionEvent {
            action_id: "pet".into(),
            activity_id: record.id.clone(),
            activity_key: Some("pet:1".into()),
            on: None,
            tap: stored(1),
        })
    );
    assert!(taps.try_recv().is_err());

    block_on(live.write_image("cover", &one_pixel())).unwrap();
    assert!(files.join("liveactivity/images/cover").exists());
    // A backend called without the service keeps a name inside its folder all the same.
    let direct = LiveUpdateBackend::new(MemoryDriver::default())
        .with_files(liveactivity::android::AndroidFiles::new("Mochi", &files));
    let escaped = block_on(direct.write_image("../escaped".into(), one_pixel()));
    assert_eq!(escaped.unwrap_err().code, "INVALID_INPUT");
    assert!(!files.join("liveactivity/escaped").exists());
    // And it is connected once.
    direct
        .attach(liveactivity::BackendContext::new(|_: Event| {}))
        .unwrap();
    let again = direct.attach(liveactivity::BackendContext::new(|_: Event| {}));
    assert_eq!(again.unwrap_err().code, "CONFLICT");
    assert!(files.join("liveactivity/layouts.json").exists());

    let end = liveactivity::LayoutEndInput {
        id: record.id,
        policy: liveactivity::EndPolicy::Immediate {},
        snapshot: None,
    };
    assert_eq!(
        block_on(live.end_layout(end)).unwrap().state,
        ActivityState::Ended
    );
    std::fs::remove_dir_all(&files).unwrap();
}

/// A ride with a progress, a text and a button, as a layout, and optionally what Android shows for its first
/// snapshot.
fn ride(key: &str, android: Option<serde_json::Value>) -> liveactivity::LayoutStartInput {
    let mut request = serde_json::json!({
        "activityKey": key,
        "schemaVersion": 2,
        "fallback": "notification",
        "layout": { "lockScreen": { "type": "vstack", "children": [
            { "type": "text", "bind": "eta" },
            { "type": "progress", "bind": "done" },
            { "type": "button", "actionId": "pet", "child": { "type": "text", "value": "Pet" } },
        ] } },
        "snapshot": { "revision": 0, "values": { "eta": "6 min", "done": 0.2 } },
    });
    if let Some(android) = android {
        request["snapshot"]["android"] = android;
    }
    liveactivity::parse_request("start_layout", request).unwrap()
}

/// A segmented progress with a point, a chip text and an action: what no layout can say.
fn driving(progress: u32, chip: &str) -> serde_json::Value {
    serde_json::json!({
        "title": "Driver on the way",
        "status": format!("{chip} away"),
        "presentation": {
            "style": { "kind": "progress", "value": {
                "progress": progress, "styledByProgress": true,
                "segments": [{ "length": 60, "color": 4278190335_u32 }, { "length": 40 }],
                "points": [{ "position": 60 }],
            } },
            "shortCriticalText": chip,
            "actions": [{ "id": "call", "title": "Call" }],
        },
    })
}

fn ride_update(id: &str, revision: u64, android: Option<serde_json::Value>) -> serde_json::Value {
    let mut request = serde_json::json!({
        "id": id,
        "snapshot": { "revision": revision, "values": { "eta": "2 min", "done": 0.7 } },
    });
    if let Some(android) = android {
        request["snapshot"]["android"] = android;
    }
    request
}

fn android_with_files(test: &str) -> (LiveActivity, Receiver<Event>, MemoryDriver) {
    android_on(MemoryDriver::default(), test)
}

/// The Android default backend with the app's files, on `driver`.
fn android_on(driver: MemoryDriver, test: &str) -> (LiveActivity, Receiver<Event>, MemoryDriver) {
    let backend = LiveUpdateBackend::new(driver.clone()).with_files(
        liveactivity::android::AndroidFiles::new("Mochi", app_files(test)),
    );
    let (live, events) = connect(backend);
    (live, events, driver)
}

#[test]
fn req_a_001_an_android_content_is_shown_as_given_in_place_of_the_projection() {
    // REQ-A-001, REQ-I-002
    let (live, _events, driver) = android_with_files("content");
    let start = ride("ride:7", Some(driving(20, "6 min")));
    let record = block_on(live.start_layout(start.clone())).unwrap();

    // What liveupdate posted is the content as given.
    let posted = driver.posted(&record.id);
    assert_eq!(posted.title, "Driver on the way");
    assert_eq!(posted.status, "6 min away");
    let presentation = posted.presentation.unwrap();
    let liveactivity::PresentationStyle::Progress(style) = &presentation.style else {
        panic!("the content's style is posted, not a projected one");
    };
    assert_eq!(
        (style.progress, style.segments.len(), style.points.len()),
        (20, 2, 1)
    );
    assert_eq!(style.segments[0].color, Some(4278190335));
    assert_eq!(presentation.short_critical_text.as_deref(), Some("6 min"));
    assert_eq!(presentation.actions[0].id, "call");
    // No projection was made, so nothing of the layout is reported as left out; the record repeats the snapshot.
    assert!(
        record
            .degraded_features
            .iter()
            .all(|feature| !feature.starts_with("layout."))
    );
    assert_eq!(record.snapshot.android, start.snapshot.android);

    // An update with its content and an alert, on a device that alerts once: Android shows the content silently
    // and says so.
    let mut loud = ride_update(&record.id, 1, Some(driving(70, "2 min")));
    loud["alert"] = serde_json::json!({ "title": "Almost there", "body": "2 minutes away" });
    let loud = liveactivity::parse_request("update_layout", loud).unwrap();
    let updated = block_on(live.update_layout(loud)).unwrap();
    assert!(updated.degraded_features.contains(&"alert".to_owned()));
    let liveactivity::PresentationStyle::Progress(style) =
        driver.posted(&record.id).presentation.unwrap().style
    else {
        panic!("the activity keeps its style");
    };
    assert_eq!(style.progress, 70);
    // The next update has no alert, and nothing is reported of one.
    let quiet = ride_update(&record.id, 2, Some(driving(80, "1 min")));
    let quiet = liveactivity::parse_request("update_layout", quiet).unwrap();
    let updated = block_on(live.update_layout(quiet)).unwrap();
    assert!(!updated.degraded_features.contains(&"alert".to_owned()));

    // This device keeps the style an activity shows: a content of another style is a valid request that it
    // cannot show, and the activity keeps what it shows.
    let mut other = driving(90, "now");
    other["presentation"]["style"] = serde_json::json!({ "kind": "standard" });
    let other =
        liveactivity::parse_request("update_layout", ride_update(&record.id, 3, Some(other)));
    let refused = block_on(live.update_layout(other.unwrap())).unwrap_err();
    assert_eq!(refused.code, "UNSUPPORTED_FEATURE");
    assert_eq!(driver.posted(&record.id).revision, 2);

    // A snapshot without a content is projected, as every snapshot of a layout that never gave one.
    let plain = liveactivity::parse_request("update_layout", ride_update(&record.id, 3, None));
    block_on(live.update_layout(plain.unwrap())).unwrap();
    let projected = driver.posted(&record.id);
    assert_eq!(projected.title, "2 min");
    let liveactivity::PresentationStyle::Progress(style) = projected.presentation.unwrap().style
    else {
        panic!("the layout's progress is projected");
    };
    assert_eq!((style.progress, style.segments.len()), (700, 1));
    // REQ-A-023: the projected bar is styled by its progress, so that it is filled up to its value.
    assert!(style.styled_by_progress);
    let other = block_on(live.start_layout(ride("ride:8", None))).unwrap();
    assert_eq!(driver.posted(&other.id).title, "6 min");
    assert_eq!(other.snapshot.android, None);

    // An activity that began with a projection takes a content of the style it was projected to, and here no other.
    let standard = serde_json::json!({ "title": "Arrived", "status": "At the door",
        "presentation": { "style": { "kind": "standard" } } });
    let refused =
        liveactivity::parse_request("update_layout", ride_update(&other.id, 1, Some(standard)))
            .and_then(|update| block_on(live.update_layout(update)));
    assert_eq!(refused.unwrap_err().code, "UNSUPPORTED_FEATURE");
    let same_style = ride_update(&other.id, 1, Some(driving(50, "3 min")));
    let same_style = liveactivity::parse_request("update_layout", same_style).unwrap();
    block_on(live.update_layout(same_style)).unwrap();
    assert_eq!(driver.posted(&other.id).title, "Driver on the way");
    // REQ-A-023: a content says for itself how its bar is styled. One that leaves `styledByProgress` out is
    // posted without it, as given.
    let mut unstyled = driving(60, "2 min");
    unstyled["presentation"]["style"]["value"]
        .as_object_mut()
        .unwrap()
        .remove("styledByProgress");
    let unstyled =
        liveactivity::parse_request("update_layout", ride_update(&other.id, 2, Some(unstyled)));
    block_on(live.update_layout(unstyled.unwrap())).unwrap();
    let liveactivity::PresentationStyle::Progress(style) =
        driver.posted(&other.id).presentation.unwrap().style
    else {
        panic!("the content's style is posted");
    };
    assert_eq!((style.progress, style.styled_by_progress), (60, false));
    let other = block_on(live.start_layout(ride("ride:9", None))).unwrap();

    // REQ-C-007: what only iOS reads is no part of what Android keeps and compares. The same start with a
    // relevance and a push request is the same activity, and the record repeats neither.
    let mut for_ios = ride("ride:9", None);
    for_ios.snapshot.relevance = Some(80.0);
    for_ios.push = Some(liveactivity::LayoutPush::Token);
    let again = block_on(live.start_layout(for_ios)).unwrap();
    assert_eq!(again.id, other.id);
    assert_eq!(again.snapshot.relevance, None);
    let mut relevant = liveactivity::LayoutSnapshot::new(1, again.snapshot.values.clone());
    relevant.relevance = Some(95.0);
    let updated = liveactivity::LayoutUpdateInput::new(&other.id, relevant);
    assert_eq!(
        block_on(live.update_layout(updated))
            .unwrap()
            .snapshot
            .relevance,
        None
    );
}

#[test]
fn req_a_023_a_start_projected_before_the_flag_is_compared_as_liveupdate_keeps_it() {
    // REQ-A-023
    let (live, _events, driver) = android_with_files("bar-before");
    // An activity that Android has not listed yet, kept as the version before this requirement projected its
    // start: without the flag.
    driver.lock().delay_listing = true;
    let record = block_on(live.start_layout(ride("ride:1", None))).unwrap();
    assert_eq!(record.state, ActivityState::Unknown);
    for kept in &mut driver.lock().records {
        let shown = kept.origin.snapshot.presentation.as_mut().unwrap();
        let liveactivity::PresentationStyle::Progress(bar) = &mut shown.style else {
            panic!("the start was projected to the progress style");
        };
        bar.styled_by_progress = false;
    }

    // The same request is projected with the flag now, and liveupdate compares it with the one it keeps.
    let repeated = block_on(live.start_layout(ride("ride:1", None)));
    assert_eq!(repeated.unwrap_err().code, "CONFLICT");

    // Once Android shows the activity, the same request is that activity, whatever it would project to now,
    // and an update replaces the projection that is shown.
    driver.show_unlisted();
    let shown = block_on(live.start_layout(ride("ride:1", None))).unwrap();
    assert_eq!(shown.id, record.id);
    let update = liveactivity::parse_request("update_layout", ride_update(&record.id, 1, None));
    block_on(live.update_layout(update.unwrap())).unwrap();
    let liveactivity::PresentationStyle::Progress(bar) =
        driver.posted(&record.id).presentation.unwrap().style
    else {
        panic!("the layout's progress is projected");
    };
    assert_eq!((bar.progress, bar.styled_by_progress), (700, true));
}

#[test]
fn req_a_002_taps_of_a_content_and_dismissals_reach_the_sink() {
    // REQ-A-002, REQ-E-002
    let (live, events, driver) = android_with_files("content-taps");
    let record = block_on(live.start_layout(ride("ride:7", Some(driving(20, "6 min"))))).unwrap();
    shown(&events, &record.id);

    // A later content has another action: the one it replaced stays declared, since Android stores a tap, also
    // while the application does not run, and the tap may come after the update.
    let mut later = driving(40, "4 min");
    later["presentation"]["actions"] = serde_json::json!([{ "id": "message", "title": "Message" }]);
    let later =
        liveactivity::parse_request("update_layout", ride_update(&record.id, 1, Some(later)));
    block_on(live.update_layout(later.unwrap())).unwrap();
    let latest = ride_update(&record.id, 2, Some(driving(60, "3 min")));
    let latest = liveactivity::parse_request("update_layout", latest).unwrap();
    block_on(live.update_layout(latest)).unwrap();
    driver.tap(&record.id, 0, "message");
    block_on(live.sync_events()).unwrap();
    assert!(matches!(events.try_recv().unwrap(), Event::Action(tap) if tap.action_id == "message"));

    // The actions of the activity are those of its contents and of its layout's buttons.
    driver.tap(&record.id, 1, "call");
    driver.tap(&record.id, 2, "pet");
    driver.tap(&record.id, 3, "undeclared");
    driver.dismiss(&record.id, 4);
    block_on(live.sync_events()).unwrap();
    let tap = |action: &str, sequence: u64| {
        Event::Action(ActionEvent {
            action_id: action.into(),
            activity_id: record.id.clone(),
            activity_key: Some("ride:7".into()),
            on: None,
            tap: stored(sequence),
        })
    };
    assert_eq!(events.try_recv().unwrap(), tap("call", 1));
    assert_eq!(events.try_recv().unwrap(), tap("pet", 2));
    // The person removed the notification: a state event, after the stored events of the drain.
    assert_eq!(
        events.try_recv().unwrap(),
        Event::State(liveactivity::StateEvent {
            activity_id: record.id.clone(),
            activity_key: Some("ride:7".into()),
            state: ActivityState::Dismissed,
        })
    );
    assert!(events.try_recv().is_err());

    // Without the app's files the backend runs templates, and reports their dismissal with the key as well.
    let driver = MemoryDriver::default();
    let (live, events) = connect(LiveUpdateBackend::new(driver.clone()));
    let start = StartInput::progress("order:42", "Order 42", "Preparing", 0.1);
    let order = block_on(live.start(start)).unwrap();
    shown(&events, &order.id);
    driver.dismiss(&order.id, 1);
    block_on(live.sync_events()).unwrap();
    assert_eq!(
        events.try_recv().unwrap(),
        Event::State(liveactivity::StateEvent {
            activity_id: order.id.clone(),
            activity_key: Some("order:42".into()),
            state: ActivityState::Dismissed,
        })
    );
}

#[test]
fn req_a_003_capabilities_report_the_styles_the_device_draws() {
    // REQ-A-003
    let (live, _events, driver) = android_with_files("styles");
    driver.lock().capabilities.presentation.metric_style = true;
    let capabilities = block_on(live.capabilities()).unwrap();
    let styles = capabilities.android_presentation.unwrap();
    assert!(styles.metric_style && styles.actions);
    assert!(!styles.progress_style);
    let wire = serde_json::to_value(&capabilities).unwrap();
    assert_eq!(wire["androidPresentation"]["metricStyle"], true);
    // This device offers none of the extensions.
    assert!(!(styles.style_change || styles.delayed_end || styles.alert || styles.links));

    // One that offers them all reports each.
    {
        let mut state = driver.lock();
        let offered = &mut state.capabilities.presentation;
        offered.chronometer = true;
        offered.links = true;
        offered.style_change = true;
        offered.delayed_end = true;
        offered.foreground_placement = true;
        offered.alert = true;
        offered.public_version = true;
        offered.branding = true;
    }
    let wire = serde_json::to_value(block_on(live.capabilities()).unwrap()).unwrap();
    let offered = &wire["androidPresentation"];
    for extension in [
        "chronometer",
        "links",
        "styleChange",
        "delayedEnd",
        "foregroundPlacement",
        "alert",
        "publicVersion",
        "branding",
    ] {
        assert_eq!(offered[extension], true, "{extension}");
    }

    // A backend that shows no Android content reports no styles, and its JSON has no such member.
    for (_, connect) in BACKENDS {
        let (live, _events) = connect();
        let capabilities = block_on(live.capabilities()).unwrap();
        assert_eq!(capabilities.android_presentation, None);
        let wire = serde_json::to_value(&capabilities).unwrap();
        assert!(wire.get("androidPresentation").is_none());
    }
}

fn style_of(snapshot: &liveactivity::liveupdate::Snapshot) -> liveactivity::liveupdate::Template {
    snapshot.template().unwrap()
}

fn layout_update(request: serde_json::Value) -> liveactivity::LayoutUpdateInput {
    liveactivity::parse_request("update_layout", request).unwrap()
}

/// A content of the standard style, which the layout of [`ride`] does not project to.
fn arrived() -> serde_json::Value {
    serde_json::json!({ "title": "Arrived", "status": "At the door",
        "presentation": { "style": { "kind": "standard" } } })
}

#[test]
fn req_a_005_a_snapshot_of_another_style_is_shown_where_the_device_changes_styles() {
    // REQ-A-005
    use liveactivity::liveupdate::Template;
    let offering = MemoryDriver::offering(|offered| offered.style_change = true);
    let (live, _events, driver) = android_on(offering, "style-change");

    // The layout projects to a progress. A content of another style replaces it, and the next snapshot without
    // a content is the layout's own style again.
    let record = block_on(live.start_layout(ride("ride:7", None))).unwrap();
    assert_eq!(style_of(&driver.posted(&record.id)), Template::Progress);
    let update = layout_update(ride_update(&record.id, 1, Some(arrived())));
    block_on(live.update_layout(update)).unwrap();
    let shown = driver.posted(&record.id);
    assert_eq!(
        (style_of(&shown), shown.title.as_str()),
        (Template::Standard, "Arrived")
    );
    let update = layout_update(ride_update(&record.id, 2, None));
    let updated = block_on(live.update_layout(update)).unwrap();
    assert_eq!(style_of(&driver.posted(&record.id)), Template::Progress);
    assert_eq!(updated.snapshot.revision, 2);

    // An activity that began with a content is projected in the layout's style, not in the content's.
    let other = block_on(live.start_layout(ride("ride:8", Some(arrived())))).unwrap();
    assert_eq!(style_of(&driver.posted(&other.id)), Template::Standard);
    let update = layout_update(ride_update(&other.id, 1, None));
    block_on(live.update_layout(update)).unwrap();
    let projected = driver.posted(&other.id);
    assert_eq!(style_of(&projected), Template::Progress);
    let liveactivity::PresentationStyle::Progress(style) = projected.presentation.unwrap().style
    else {
        panic!("the layout's progress is projected");
    };
    assert_eq!(style.progress, 700);

    // A device that keeps the style an activity shows refuses both, and the activity keeps what it shows.
    let (live, _events, driver) = android_with_files("style-kept");
    let record = block_on(live.start_layout(ride("ride:7", Some(arrived())))).unwrap();
    let update = layout_update(ride_update(&record.id, 1, None));
    let refused = block_on(live.update_layout(update)).unwrap_err();
    assert_eq!(refused.code, "UNSUPPORTED_FEATURE");
    let update = layout_update(ride_update(&record.id, 1, Some(driving(50, "3 min"))));
    let refused = block_on(live.update_layout(update)).unwrap_err();
    assert_eq!(refused.code, "UNSUPPORTED_FEATURE");
    assert_eq!(driver.posted(&record.id).revision, 0);
    assert_eq!(
        block_on(live.list_layout()).unwrap()[0].snapshot.revision,
        0
    );
}

#[test]
fn req_i_002_an_alert_alerts_on_android_where_the_device_offers_it() {
    // REQ-I-002
    let offering = MemoryDriver::offering(|offered| offered.alert = true);
    let (live, _events, driver) = android_on(offering, "alert");
    let alerting = |posted: liveactivity::liveupdate::Snapshot| posted.presentation.unwrap().alert;
    let loud = |id: &str, revision: u64, android: Option<serde_json::Value>| {
        let mut update = ride_update(id, revision, android);
        update["alert"] = serde_json::json!({ "title": "Almost there", "body": "2 minutes away" });
        layout_update(update)
    };

    // A projected snapshot alerts with its revision, and nothing is named as left out.
    let record = block_on(live.start_layout(ride("ride:7", None))).unwrap();
    assert!(!alerting(driver.posted(&record.id)));
    let updated = block_on(live.update_layout(loud(&record.id, 1, None))).unwrap();
    assert!(alerting(driver.posted(&record.id)));
    assert!(!updated.degraded_features.contains(&"alert".to_owned()));
    // The alert is of that revision alone, and the record keeps the snapshot as it was sent.
    let quiet = layout_update(ride_update(&record.id, 2, None));
    block_on(live.update_layout(quiet)).unwrap();
    assert!(!alerting(driver.posted(&record.id)));

    // A content alerts too, without saying so itself. This one has another style than the projection's and a
    // link, so it needs the device three times over: the device is asked once for them.
    {
        let mut state = driver.lock();
        state.capabilities.presentation.style_change = true;
        state.capabilities.presentation.links = true;
    }
    let mut linked = arrived();
    linked["presentation"]["openUrl"] = serde_json::json!("myapp://rides/7");
    let asked = driver.lock().capability_calls;
    block_on(live.update_layout(loud(&record.id, 3, Some(linked)))).unwrap();
    // liveupdate asks once itself, for the permission of an update.
    assert_eq!(driver.lock().capability_calls - asked, 2);
    assert!(alerting(driver.posted(&record.id)));
    let updated =
        block_on(live.update_layout(loud(&record.id, 4, Some(driving(70, "2 min"))))).unwrap();
    assert!(alerting(driver.posted(&record.id)));
    assert!(!updated.degraded_features.contains(&"alert".to_owned()));
    let sent = updated.snapshot.android.unwrap();
    assert!(!sent.presentation.alert);

    // Elsewhere the update is silent and the record says so.
    let (live, _events, driver) = android_with_files("alert-once");
    let record = block_on(live.start_layout(ride("ride:7", None))).unwrap();
    let updated = block_on(live.update_layout(loud(&record.id, 1, None))).unwrap();
    assert!(!alerting(driver.posted(&record.id)));
    assert!(updated.degraded_features.contains(&"alert".to_owned()));
    // A content that says so itself is treated the same: sent without the alert, which the record names.
    let mut content = driving(70, "2 min");
    content["presentation"]["alert"] = serde_json::json!(true);
    let update = layout_update(ride_update(&record.id, 2, Some(content)));
    let updated = block_on(live.update_layout(update)).unwrap();
    assert!(!alerting(driver.posted(&record.id)));
    assert!(updated.degraded_features.contains(&"alert".to_owned()));
}

#[test]
fn req_a_006_a_delayed_end_keeps_the_last_content_where_the_device_offers_it() {
    // REQ-A-006
    use liveactivity::{EndPolicy, LayoutEndInput};
    let at = 1_800_000_600_000;
    let offering = MemoryDriver::offering(|offered| offered.delayed_end = true);
    let (live, _events, driver) = android_on(offering, "delayed-end");

    // A layout activity ends with a final snapshot, which stays until its time and is no longer observed.
    let record = block_on(live.start_layout(ride("ride:7", None))).unwrap();
    let mut end = LayoutEndInput::new(&record.id);
    end.policy = EndPolicy::After { at };
    end.snapshot = Some(layout_update(ride_update(&record.id, 1, Some(arrived()))).snapshot);
    let ended = block_on(live.end_layout(end.clone()));
    // The content is of another style, which this device does not change: nothing ended, nothing changed.
    assert_eq!(ended.unwrap_err().code, "UNSUPPORTED_FEATURE");
    assert_eq!(driver.posted(&record.id).revision, 0);
    end.snapshot = Some(layout_update(ride_update(&record.id, 1, None)).snapshot);
    let ended = block_on(live.end_layout(end)).unwrap();
    assert_eq!(ended.state, ActivityState::Ended);
    assert_eq!(ended.snapshot.revision, 1);
    {
        let state = driver.lock();
        let last = &state.finals[&record.id];
        assert_eq!((last.dismiss_at, last.snapshot.revision), (Some(at), 1));
        assert!(!state.observed.contains_key(&record.id));
    }
    // A second delayed end changes nothing, also once the device no longer offers one: the activity is over.
    driver.lock().capabilities.presentation.delayed_end = false;
    let mut later = LayoutEndInput::new(&record.id);
    later.policy = EndPolicy::After { at: at + 1 };
    assert_eq!(
        block_on(live.end_layout(later)).unwrap().state,
        ActivityState::Ended
    );
    assert_eq!(driver.lock().records[0].dismiss_at, Some(at));
    driver.lock().capabilities.presentation.delayed_end = true;
    // An end without a time removes the last content early, also when it repeats the final snapshot: an activity
    // that is over takes no content, and its end answers its record.
    let mut early = LayoutEndInput::immediate(&record.id);
    early.snapshot = Some(layout_update(ride_update(&record.id, 1, None)).snapshot);
    let removed = block_on(live.end_layout(early)).unwrap();
    assert_eq!(
        (removed.state, removed.snapshot.revision),
        (ActivityState::Ended, 1)
    );
    assert_eq!(driver.lock().records[0].dismiss_at, None);

    // What the person removed is ended without being shown again, whatever the end says.
    let gone = block_on(live.start_layout(ride("ride:8", None))).unwrap();
    driver.lock().observed.remove(&gone.id);
    let mut end = LayoutEndInput::new(&gone.id);
    end.policy = EndPolicy::After { at };
    end.snapshot = Some(layout_update(ride_update(&gone.id, 1, None)).snapshot);
    let ended = block_on(live.end_layout(end)).unwrap();
    assert_eq!(
        (ended.state, ended.snapshot.revision),
        (ActivityState::Ended, 0)
    );
    assert!(!driver.lock().finals.contains_key(&gone.id));

    // A template activity ends the same way.
    let order = block_on(live.start(StartInput::progress(
        "order:42",
        "Order 42",
        "Preparing",
        0.1,
    )));
    let order = order.unwrap();
    let mut end = EndInput::new(&order.id);
    end.policy = EndPolicy::After { at };
    assert_eq!(block_on(live.end(end)).unwrap().state, ActivityState::Ended);
    assert_eq!(driver.lock().finals[&order.id].dismiss_at, Some(at));

    // A device that removes an ended activity at once refuses, and the activities stay as they were.
    let (live, _events, driver) = android_with_files("immediate-end");
    let record = block_on(live.start_layout(ride("ride:7", None))).unwrap();
    let mut end = LayoutEndInput::new(&record.id);
    end.policy = EndPolicy::After { at };
    end.snapshot = Some(layout_update(ride_update(&record.id, 1, None)).snapshot);
    assert_eq!(
        block_on(live.end_layout(end)).unwrap_err().code,
        "UNSUPPORTED_FEATURE"
    );
    let listed = block_on(live.list_layout()).unwrap();
    assert_eq!(
        (listed[0].state, listed[0].snapshot.revision),
        (ActivityState::Active, 0)
    );
    assert_eq!(driver.posted(&record.id).revision, 0);
    let order = block_on(live.start(StartInput::progress(
        "order:42",
        "Order 42",
        "Preparing",
        0.1,
    )));
    let mut end = EndInput::new(&order.unwrap().id);
    end.policy = EndPolicy::After { at };
    assert_eq!(
        block_on(live.end(end)).unwrap_err().code,
        "UNSUPPORTED_FEATURE"
    );
    assert_eq!(
        block_on(live.list()).unwrap()[0].state,
        ActivityState::Active
    );
}

#[test]
fn req_a_004_a_public_activity_names_the_privacy_a_notification_cannot_keep() {
    // REQ-A-004
    let (live, _events, _driver) = android_with_files("privacy");
    let start = |key: &str, visibility: Option<&str>| {
        let mut request = serde_json::json!({
            "activityKey": key, "schemaVersion": 2, "fallback": "notification",
            "layout": { "url": "myapp://rides/7", "lockScreen": { "type": "vstack", "children": [
                { "type": "text", "bind": "eta", "privacy": "sensitive" },
                { "type": "link", "url": "myapp://rides/7/map",
                  "child": { "type": "text", "value": "Map" } },
            ] } },
            "snapshot": { "revision": 0, "values": { "eta": "6 min" } },
        });
        if let Some(visibility) = visibility {
            request["android"] = serde_json::json!({ "visibility": visibility });
        }
        let input = liveactivity::parse_request("start_layout", request).unwrap();
        block_on(live.start_layout(input))
            .unwrap()
            .degraded_features
    };
    let named = |features: Vec<String>, feature: &str| features.iter().any(|left| left == feature);
    // A notification whose content a locked screen shows cannot hide one node of it.
    let public = start("ride:1", Some("public"));
    assert!(named(public.clone(), "layout.privacy"));
    assert!(named(public.clone(), "layout.link") && named(public, "layout.url"));
    // One that a locked screen hides, as a start is unless it says otherwise, hides the node with the rest.
    assert!(!named(start("ride:2", None), "layout.privacy"));
    assert!(!named(start("ride:3", Some("secret")), "layout.privacy"));
}

/// A content with everything liveupdate 0.2 added to a presentation.
fn extended() -> serde_json::Value {
    serde_json::json!({
        "title": "Driver on the way",
        "status": "4 min away",
        "presentation": {
            "style": { "kind": "standard" },
            "openUrl": "myapp://rides/7",
            "actions": [{ "id": "map", "title": "Map", "openUrl": "myapp://rides/7/map" }],
            "chronometer": {
                "timer": { "startedAt": 1_790_000_000_000_u64, "endsAt": 1_790_000_240_000_u64 },
                "countsDown": true,
            },
            "alert": true,
            "publicVersion": { "title": "Ride", "status": "On the way" },
            "smallIcon": { "kind": "resource", "value": "ic_ride" },
            "accentColor": 4278222848_u32,
            "subText": "Ride 7",
        },
    })
}

#[test]
fn req_a_007_the_extensions_of_a_content_reach_android() {
    // REQ-A-007
    let offering = MemoryDriver::offering(|offered| {
        offered.links = true;
        offered.chronometer = true;
        offered.alert = true;
        offered.public_version = true;
        offered.branding = true;
    });
    let (live, _events, driver) = android_on(offering, "extensions");
    let record = block_on(live.start_layout(ride("ride:7", Some(extended())))).unwrap();

    // What liveupdate posted is the presentation as it was sent.
    let posted = serde_json::to_value(driver.posted(&record.id).presentation).unwrap();
    assert_eq!(posted, extended()["presentation"]);
    assert_eq!(
        serde_json::to_value(&record.snapshot.android).unwrap(),
        extended()
    );

    // A public version is of a private activity. A start that says otherwise is refused before any backend, so
    // on every platform.
    let public = |visibility: &str| {
        let mut request = serde_json::to_value(ride("ride:8", Some(extended()))).unwrap();
        request["android"] = serde_json::json!({ "visibility": visibility });
        liveactivity::validate_request("start_layout", request)
    };
    assert_eq!(public("private"), Ok(()));
    assert_eq!(public("public").unwrap_err().code, "INVALID_INPUT");
    assert_eq!(public("secret").unwrap_err().code, "INVALID_INPUT");

    // In an update only Android knows how the activity started, and refuses there.
    let mut open = serde_json::to_value(ride("ride:9", Some(arrived()))).unwrap();
    open["android"] = serde_json::json!({ "visibility": "public" });
    let open = liveactivity::parse_request("start_layout", open).unwrap();
    let open = block_on(live.start_layout(open)).unwrap();
    let update = layout_update(ride_update(&open.id, 1, Some(extended())));
    assert_eq!(
        block_on(live.update_layout(update)).unwrap_err().code,
        "INVALID_INPUT"
    );
    assert_eq!(driver.posted(&open.id).revision, 0);

    // A content is shown as given or not at all. On a device that does not offer what it uses, each member is a
    // valid request that the device cannot show, at a start and in an update.
    let (bare, _events, bare_driver) = android_with_files("no-extensions");
    let started = block_on(bare.start_layout(ride("ride:7", Some(arrived())))).unwrap();
    for (member, value) in [
        ("openUrl", serde_json::json!("myapp://rides/7")),
        (
            "actions",
            serde_json::json!([{ "id": "map", "title": "Map", "openUrl": "myapp://rides/7/map" }]),
        ),
        (
            "chronometer",
            extended()["presentation"]["chronometer"].clone(),
        ),
        (
            "publicVersion",
            extended()["presentation"]["publicVersion"].clone(),
        ),
        ("smallIcon", extended()["presentation"]["smallIcon"].clone()),
        ("accentColor", serde_json::json!(4278222848_u32)),
        ("subText", serde_json::json!("Ride 7")),
    ] {
        let mut content = arrived();
        content["presentation"][member] = value;
        let update = layout_update(ride_update(&started.id, 1, Some(content.clone())));
        let refused = block_on(bare.update_layout(update)).unwrap_err();
        assert_eq!(refused.code, "UNSUPPORTED_FEATURE", "{member}");
        let refused = block_on(bare.start_layout(ride("ride:8", Some(content)))).unwrap_err();
        assert_eq!(refused.code, "UNSUPPORTED_FEATURE", "{member}");
    }
    assert_eq!(bare_driver.posted(&started.id).revision, 0);
    assert_eq!(block_on(bare.list_layout()).unwrap().len(), 1);
    // What is wrong wherever it can be known is answered as wrong, not as something this device lacks: a public
    // version for an activity that started as public.
    let mut open = serde_json::to_value(ride("ride:9", Some(arrived()))).unwrap();
    open["android"] = serde_json::json!({ "visibility": "public" });
    let open = liveactivity::parse_request("start_layout", open).unwrap();
    let open = block_on(bare.start_layout(open)).unwrap();
    let mut content = arrived();
    content["presentation"]["publicVersion"] = extended()["presentation"]["publicVersion"].clone();
    let update = layout_update(ride_update(&open.id, 1, Some(content)));
    assert_eq!(
        block_on(bare.update_layout(update)).unwrap_err().code,
        "INVALID_INPUT"
    );
}

#[test]
fn req_a_007_a_content_that_android_refuses_at_the_post_leaves_nothing_behind() {
    // REQ-A-007
    let offering = MemoryDriver::offering(|offered| offered.links = true);
    let (live, _events, driver) = android_on(offering, "refused-post");
    let opening = |url: &str| {
        let mut content = arrived();
        content["presentation"]["openUrl"] = serde_json::json!(url);
        content
    };
    let unopened = format!("{}//x", common::UNOPENED);

    // Only the device knows that nothing in the application opens the URL: Android refuses to post the start.
    // Nothing runs afterwards, and the key is free for the start that Android takes.
    let refused = block_on(live.start_layout(ride("ride:1", Some(opening(&unopened)))));
    assert_eq!(refused.unwrap_err().code, "INVALID_INPUT");
    assert!(block_on(live.list_layout()).unwrap().is_empty());
    let started = block_on(live.start_layout(ride("ride:1", Some(opening("myapp://rides/1")))));
    let started = started.unwrap();
    assert_eq!(driver.posted(&started.id).title, "Arrived");
    let listed = block_on(live.list_layout()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(
        (listed[0].id.as_str(), listed[0].state),
        (started.id.as_str(), ActivityState::Active)
    );
    // The same refused start again is refused again, and leaves the running activity of another key alone.
    let refused = block_on(live.start_layout(ride("ride:2", Some(opening(&unopened)))));
    assert_eq!(refused.unwrap_err().code, "INVALID_INPUT");
    let refused = block_on(live.start_layout(ride("ride:2", Some(opening(&unopened)))));
    assert_eq!(refused.unwrap_err().code, "INVALID_INPUT");
    assert_eq!(block_on(live.list_layout()).unwrap().len(), 1);

    // A refused update leaves the activity as it is shown, in the record too, and the corrected content goes
    // out under the same revision.
    let update = layout_update(ride_update(&started.id, 1, Some(opening(&unopened))));
    assert_eq!(
        block_on(live.update_layout(update)).unwrap_err().code,
        "INVALID_INPUT"
    );
    assert_eq!(driver.posted(&started.id).revision, 0);
    let listed = block_on(live.list_layout()).unwrap();
    assert_eq!(
        (listed[0].state, listed[0].snapshot.revision),
        (ActivityState::Active, 0)
    );
    let corrected = layout_update(ride_update(
        &started.id,
        1,
        Some(opening("myapp://rides/1/map")),
    ));
    let updated = block_on(live.update_layout(corrected)).unwrap();
    assert_eq!(
        (updated.state, updated.snapshot.revision),
        (ActivityState::Active, 1)
    );
    assert_eq!(driver.posted(&started.id).revision, 1);

    // A start that failed for a reason of the platform stays, for the same start to try again: the answer may
    // have been lost. When that start is then refused for its content, it is forgotten like any refused start.
    driver.lock().failing_posts = 1;
    let failed = block_on(live.start_layout(ride("ride:3", Some(opening(&unopened)))));
    assert_eq!(failed.unwrap_err().code, "NATIVE_ERROR");
    assert_eq!(block_on(live.list_layout()).unwrap().len(), 2);
    let refused = block_on(live.start_layout(ride("ride:3", Some(opening(&unopened)))));
    assert_eq!(refused.unwrap_err().code, "INVALID_INPUT");
    let listed = block_on(live.list_layout()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, started.id);
    let corrected = block_on(live.start_layout(ride("ride:3", Some(opening("myapp://rides/3")))));
    assert_eq!(corrected.unwrap().state, ActivityState::Active);
}

#[test]
fn req_e_003_an_end_of_the_application_is_reported_on_android() {
    // REQ-E-003
    use liveactivity::{EndPolicy, LayoutEndInput, StateEvent};
    let ended = |id: &str, key: &str| {
        Event::State(StateEvent {
            activity_id: id.into(),
            activity_key: Some(key.into()),
            state: ActivityState::Ended,
        })
    };
    let at = 1_800_000_600_000;

    // A template, over a driver and over a framework's string bridge.
    for connect in [android_default as Connect, android_over_a_framework_bridge] {
        let (live, events) = connect();
        let start = StartInput::progress("order:42", "Order 42", "Preparing", 0.1);
        let order = block_on(live.start(start)).unwrap();
        shown(&events, &order.id);
        block_on(live.end(EndInput::immediate(&order.id))).unwrap();
        assert_eq!(events.try_recv().unwrap(), ended(&order.id, "order:42"));
        // An end again answers the record and reports nothing, and neither does an end that fails.
        block_on(live.end(EndInput::new(&order.id))).unwrap();
        assert_eq!(
            block_on(live.end(EndInput::new("unknown")))
                .unwrap_err()
                .code,
            "NOT_FOUND"
        );
        assert!(events.try_recv().is_err());
    }

    // A layout, at once and with a last content that stays.
    let offering = MemoryDriver::offering(|offered| offered.delayed_end = true);
    let (live, events, driver) = android_on(offering, "end-events");
    let record = block_on(live.start_layout(ride("ride:7", None))).unwrap();
    shown(&events, &record.id);
    block_on(live.end_layout(LayoutEndInput::immediate(&record.id))).unwrap();
    assert_eq!(events.try_recv().unwrap(), ended(&record.id, "ride:7"));
    let kept = block_on(live.start_layout(ride("ride:8", None))).unwrap();
    shown(&events, &kept.id);
    let mut end = LayoutEndInput::new(&kept.id);
    end.policy = EndPolicy::After { at };
    block_on(live.end_layout(end)).unwrap();
    assert_eq!(events.try_recv().unwrap(), ended(&kept.id, "ride:8"));
    // The activity ended then: removing its last content early ends nothing again.
    block_on(live.end_layout(LayoutEndInput::immediate(&kept.id))).unwrap();
    assert!(events.try_recv().is_err());

    // What the person removed is reported as dismissed, on the drain. Its end then makes the record ended, and
    // that state is reported like any other, once.
    let gone = block_on(live.start_layout(ride("ride:9", None))).unwrap();
    shown(&events, &gone.id);
    driver.dismiss(&gone.id, 1);
    block_on(live.sync_events()).unwrap();
    assert!(matches!(
        events.try_recv().unwrap(),
        Event::State(change) if change.state == ActivityState::Dismissed
    ));
    let over = block_on(live.end_layout(LayoutEndInput::new(&gone.id))).unwrap();
    assert_eq!(over.state, ActivityState::Ended);
    assert_eq!(events.try_recv().unwrap(), ended(&gone.id, "ride:9"));
    block_on(live.end_layout(LayoutEndInput::new(&gone.id))).unwrap();
    assert!(events.try_recv().is_err());

    // An end that is refused ends nothing: a delayed one on a device that offers none.
    driver.lock().capabilities.presentation.delayed_end = false;
    let stays = block_on(live.start_layout(ride("ride:10", None))).unwrap();
    shown(&events, &stays.id);
    let mut end = LayoutEndInput::new(&stays.id);
    end.policy = EndPolicy::After { at };
    assert_eq!(
        block_on(live.end_layout(end)).unwrap_err().code,
        "UNSUPPORTED_FEATURE"
    );
    assert!(events.try_recv().is_err());
    // A template of the same backend is reported like a layout.
    let start = StartInput::progress("order:43", "Order 43", "Preparing", 0.1);
    let order = block_on(live.start(start)).unwrap();
    shown(&events, &order.id);
    block_on(live.end(EndInput::new(&order.id))).unwrap();
    assert_eq!(events.try_recv().unwrap(), ended(&order.id, "order:43"));
    assert!(events.try_recv().is_err());
}

/// A layout with a URL of its own, a link, a button and a text.
fn linked(key: &str, url: &str, link: &str) -> liveactivity::LayoutStartInput {
    liveactivity::parse_request(
        "start_layout",
        serde_json::json!({
            "activityKey": key, "schemaVersion": 2, "fallback": "notification",
            "layout": { "url": url, "lockScreen": { "type": "vstack", "children": [
                { "type": "text", "bind": "title" },
                { "type": "link", "url": link, "child": { "type": "text", "value": "Map" } },
                { "type": "button", "actionId": "call", "child": { "type": "text", "value": "Call" } },
            ] } },
            "snapshot": { "revision": 0, "values": { "title": "Ride 7" } },
        }),
    )
    .unwrap()
}

#[test]
fn req_a_008_a_url_nothing_opens_is_left_out_and_named() {
    // REQ-A-008
    let offering = MemoryDriver::offering(|offered| offered.links = true);
    let (live, _events, driver) = android_on(offering, "layout-links");
    let named = |record: &liveactivity::LayoutActivityRecord, feature: &str| {
        record.degraded_features.iter().any(|left| left == feature)
    };
    let update = |id: &str, revision: u64| {
        layout_update(serde_json::json!({
            "id": id, "snapshot": { "revision": revision, "values": { "title": "Ride 7" } },
        }))
    };

    // The application opens the layout's URLs: the notification opens them, and nothing is named.
    let record =
        block_on(live.start_layout(linked("ride:7", "myapp://rides/7", "myapp://rides/7/map")))
            .unwrap();
    let shown = driver.posted(&record.id).presentation.unwrap();
    assert_eq!(shown.open_url.as_deref(), Some("myapp://rides/7"));
    let link = shown.actions.iter().find(|action| action.id == "link:1");
    assert_eq!(
        link.and_then(|action| action.open_url.as_deref()),
        Some("myapp://rides/7/map")
    );
    assert!(!named(&record, "layout.url") && !named(&record, "layout.link"));
    // The device is asked what it offers once for such a snapshot, beside liveupdate's own question for an
    // update, and not at all for a layout that could use nothing of it.
    let asked = driver.lock().capability_calls;
    block_on(live.update_layout(update(&record.id, 1))).unwrap();
    assert_eq!(driver.lock().capability_calls - asked, 2);
    let plain = block_on(live.start_layout(ride("ride:6", None))).unwrap();
    let asked = driver.lock().capability_calls;
    block_on(live.update_layout(layout_update(ride_update(&plain.id, 1, None)))).unwrap();
    assert_eq!(driver.lock().capability_calls - asked, 1);
    // The same start again is the same activity, also when the device has stopped offering what the
    // notification was given.
    driver.lock().capabilities.presentation.links = false;
    let again =
        block_on(live.start_layout(linked("ride:7", "myapp://rides/7", "myapp://rides/7/map")))
            .unwrap();
    assert_eq!(again.id, record.id);
    assert_eq!(again.snapshot.revision, 1);
    driver.lock().capabilities.presentation.links = true;

    // Nothing in the application opens these URLs, which only the device knows. The layout that works on iOS
    // is shown all the same, without its links, and the record says so, at the start and with every update.
    let unopened = format!("{}//rides/8", common::UNOPENED);
    let posts = driver.lock().posts;
    let record = block_on(live.start_layout(linked("ride:8", &unopened, &unopened))).unwrap();
    assert_eq!(record.state, ActivityState::Active);
    // The device refused once, and took the snapshot without the links.
    assert_eq!(driver.lock().posts - posts, 2);
    let shown = driver.posted(&record.id).presentation.unwrap();
    assert_eq!(shown.open_url, None);
    let ids: Vec<_> = shown
        .actions
        .iter()
        .map(|action| action.id.as_str())
        .collect();
    assert_eq!(ids, ["call"]);
    assert!(named(&record, "layout.url") && named(&record, "layout.link"));
    let updated = block_on(live.update_layout(update(&record.id, 1))).unwrap();
    assert_eq!(updated.snapshot.revision, 1);
    assert_eq!(driver.posted(&record.id).revision, 1);
    assert!(named(&updated, "layout.url") && named(&updated, "layout.link"));
    // The same start again is the same activity.
    let again = block_on(live.start_layout(linked("ride:8", &unopened, &unopened))).unwrap();
    assert_eq!(again.id, record.id);

    // An Android content is the application's own word for Android: it is shown as given or refused, at once.
    let mut content = arrived();
    content["presentation"]["openUrl"] = serde_json::json!(unopened);
    let posts = driver.lock().posts;
    let refused = block_on(live.start_layout(ride("ride:9", Some(content))));
    assert_eq!(refused.unwrap_err().code, "INVALID_INPUT");
    assert_eq!(driver.lock().posts - posts, 1);

    // A device that opens no URL is asked nothing of the kind: one post, and the links are named.
    let (bare, _events, bare_driver) = android_with_files("layout-links-none");
    let record =
        block_on(bare.start_layout(linked("ride:7", "myapp://rides/7", "myapp://rides/7/map")))
            .unwrap();
    assert_eq!(bare_driver.lock().posts, 1);
    assert!(named(&record, "layout.url") && named(&record, "layout.link"));
}

#[test]
fn req_e_004_an_opening_from_a_notification_is_reported() {
    // REQ-E-004
    use liveactivity::OpenedEvent;
    let (live, events, driver) = android_with_files("opened");
    let record = block_on(live.start_layout(ride("ride:7", None))).unwrap();
    shown(&events, &record.id);
    // The host handed liveupdate the intent that opened it: by the notification itself, then by an action.
    driver.open(&record.id, 0, None);
    driver.tap(&record.id, 1, "pet");
    driver.open(&record.id, 2, Some("link:1"));
    block_on(live.sync_events()).unwrap();
    let opened = |action: Option<&str>| {
        Event::Opened(OpenedEvent {
            activity_id: record.id.clone(),
            activity_key: Some("ride:7".into()),
            action_id: action.map(str::to_owned),
        })
    };
    assert_eq!(events.try_recv().unwrap(), opened(None));
    assert!(matches!(events.try_recv().unwrap(), Event::Action(tap) if tap.action_id == "pet"));
    assert_eq!(events.try_recv().unwrap(), opened(Some("link:1")));
    assert!(events.try_recv().is_err());
    let wire = serde_json::to_value(OpenedEvent {
        activity_id: "id".into(),
        activity_key: None,
        action_id: None,
    });
    assert_eq!(wire.unwrap(), serde_json::json!({ "activityId": "id" }));

    // Without the app's files the backend knows no key, and reports the opening all the same.
    let driver = MemoryDriver::default();
    let (templates, events) = connect(LiveUpdateBackend::new(driver.clone()));
    driver.open("opened", 0, None);
    block_on(templates.sync_events()).unwrap();
    assert_eq!(
        events.try_recv().unwrap(),
        Event::Opened(OpenedEvent {
            activity_id: "opened".into(),
            activity_key: None,
            action_id: None,
        })
    );
}

#[test]
fn req_a_011_the_public_version_of_a_start_goes_with_every_snapshot() {
    // REQ-A-011
    let offering = MemoryDriver::offering(|offered| offered.public_version = true);
    let (live, _events, driver) = android_on(offering, "public-version");
    let private = |key: &str, android: Option<serde_json::Value>| {
        let mut request = serde_json::to_value(ride(key, android)).unwrap();
        request["android"] = serde_json::json!({ "visibility": "private",
            "publicVersion": { "title": "Ride", "status": "On the way" } });
        liveactivity::parse_request::<liveactivity::LayoutStartInput>("start_layout", request)
    };
    let public_version = |id: &str| {
        let shown = driver.posted(id).presentation.unwrap().public_version;
        shown.map(|version| (version.title, version.status))
    };
    let of_the_start = Some(("Ride".to_owned(), "On the way".to_owned()));

    // A projected snapshot shows it, at the start and after an update.
    let record = block_on(live.start_layout(private("ride:7", None).unwrap())).unwrap();
    assert_eq!(public_version(&record.id), of_the_start);
    assert!(
        !record
            .degraded_features
            .contains(&"publicVersion".to_owned())
    );
    block_on(live.update_layout(layout_update(ride_update(&record.id, 1, None)))).unwrap();
    assert_eq!(public_version(&record.id), of_the_start);
    // A content without one gets it; a content with its own keeps its own.
    let update = layout_update(ride_update(&record.id, 2, Some(driving(70, "2 min"))));
    block_on(live.update_layout(update)).unwrap();
    assert_eq!(public_version(&record.id), of_the_start);
    let mut own = driving(80, "1 min");
    own["presentation"]["publicVersion"] =
        serde_json::json!({ "title": "Ride", "status": "Almost there" });
    block_on(live.update_layout(layout_update(ride_update(&record.id, 3, Some(own))))).unwrap();
    assert_eq!(
        public_version(&record.id),
        Some(("Ride".to_owned(), "Almost there".to_owned()))
    );

    // A device that has none shows the snapshot without it, and the record says so.
    let (bare, _events, bare_driver) = android_with_files("public-version-none");
    let record = block_on(bare.start_layout(private("ride:7", None).unwrap())).unwrap();
    assert_eq!(
        bare_driver
            .posted(&record.id)
            .presentation
            .unwrap()
            .public_version,
        None
    );
    assert!(
        record
            .degraded_features
            .contains(&"publicVersion".to_owned())
    );
}

#[test]
fn req_a_012_a_start_may_ask_for_the_foreground_placement() {
    // REQ-A-012
    use liveactivity::liveupdate::Placement;
    let offering = MemoryDriver::offering(|offered| offered.foreground_placement = true);
    let (live, _events, driver) = android_on(offering, "placement");
    let placed = |key: &str, placement: Option<&str>| {
        let mut request = serde_json::to_value(ride(key, None)).unwrap();
        request["android"] = serde_json::json!({ "visibility": "private" });
        if let Some(placement) = placement {
            request["android"]["placement"] = serde_json::json!(placement);
        }
        liveactivity::parse_request::<liveactivity::LayoutStartInput>("start_layout", request)
            .unwrap()
    };
    let capabilities = block_on(live.capabilities()).unwrap();
    assert!(
        capabilities
            .android_presentation
            .unwrap()
            .foreground_placement
    );

    // The placement reaches liveupdate with the start. A start without one, or with the default by name, is
    // tagged, and the two are the same request.
    let held = block_on(live.start_layout(placed("ride:7", Some("foreground")))).unwrap();
    let tagged = block_on(live.start_layout(placed("ride:8", None))).unwrap();
    let by_name = block_on(live.start_layout(placed("ride:8", Some("tagged")))).unwrap();
    assert_eq!(by_name.id, tagged.id);
    let placement = |id: &str| {
        let state = driver.lock();
        state
            .records
            .iter()
            .find(|record| record.id == id)
            .map(|record| record.origin.placement)
    };
    assert_eq!(placement(&held.id), Some(Placement::Foreground));
    assert_eq!(placement(&tagged.id), Some(Placement::Tagged));

    // One activity has the placement at a time, and a running key keeps the placement it started with.
    let second = block_on(live.start_layout(placed("ride:9", Some("foreground"))));
    assert_eq!(second.unwrap_err().code, "SYSTEM_LIMIT");
    let other = block_on(live.start_layout(placed("ride:7", None)));
    assert_eq!(other.unwrap_err().code, "CONFLICT");
    // It is free once its activity ended.
    block_on(live.end_layout(liveactivity::LayoutEndInput::immediate(&held.id))).unwrap();
    block_on(live.start_layout(placed("ride:9", Some("foreground")))).unwrap();

    // A device that does not offer it says so and refuses the start.
    let (bare, _events, _driver) = android_with_files("placement-none");
    let capabilities = block_on(bare.capabilities()).unwrap();
    assert!(
        !capabilities
            .android_presentation
            .unwrap()
            .foreground_placement
    );
    let refused = block_on(bare.start_layout(placed("ride:7", Some("foreground"))));
    assert_eq!(refused.unwrap_err().code, "UNSUPPORTED_FEATURE");
    // Wherever it runs, a placement the contract does not know is no request.
    let mut unknown = serde_json::to_value(ride("ride:7", None)).unwrap();
    unknown["android"] = serde_json::json!({ "visibility": "private", "placement": "floating" });
    assert_eq!(
        liveactivity::validate_request("start_layout", unknown)
            .unwrap_err()
            .code,
        "INVALID_INPUT"
    );
}

/// The state event of the activity `id` under `key`.
fn state_of(id: &str, key: &str, state: ActivityState) -> Event {
    Event::State(liveactivity::StateEvent {
        activity_id: id.into(),
        activity_key: Some(key.into()),
        state,
    })
}

#[test]
fn req_e_005_each_state_of_an_activity_reaches_the_sink_once() {
    // REQ-E-005
    // A template, on a backend without the app's files: the key is the record's.
    let driver = MemoryDriver::default();
    let (live, events) = connect(LiveUpdateBackend::new(driver.clone()));
    let progress = |id: &str, revision: u64, stale_at: u64| {
        let mut update = UpdateInput::progress(id, revision, "Order 42", "On the way", 0.6);
        update.snapshot.stale_at = Some(stale_at);
        update
    };
    let mut start = StartInput::progress("order:42", "Order 42", "Preparing", 0.1);
    start.snapshot.stale_at = Some(1_000);
    let order = block_on(live.start(start)).unwrap();
    // Android shows it at once: the start is followed by its first state.
    assert_eq!(
        events.try_recv().unwrap(),
        state_of(&order.id, "order:42", ActivityState::Active)
    );
    // An update of an active activity changes no state.
    block_on(live.update(progress(&order.id, 1, 1_000))).unwrap();
    block_on(live.sync_events()).unwrap();
    assert!(events.try_recv().is_err());
    // Its stale time passes. Nobody is told until someone asks, and then once.
    driver.lock().now = 1_000;
    assert!(events.try_recv().is_err());
    block_on(live.sync_events()).unwrap();
    assert_eq!(
        events.try_recv().unwrap(),
        state_of(&order.id, "order:42", ActivityState::Stale)
    );
    block_on(live.sync_events()).unwrap();
    assert!(events.try_recv().is_err());
    // An update with a later stale time makes it active again, at the call.
    block_on(live.update(progress(&order.id, 2, 9_000))).unwrap();
    assert_eq!(
        events.try_recv().unwrap(),
        state_of(&order.id, "order:42", ActivityState::Active)
    );
    // The person removes the notification. The stored event prompts the ask; the state is reported once.
    driver.dismiss(&order.id, 1);
    block_on(live.sync_events()).unwrap();
    assert_eq!(
        events.try_recv().unwrap(),
        state_of(&order.id, "order:42", ActivityState::Dismissed)
    );
    block_on(live.sync_events()).unwrap();
    assert!(events.try_recv().is_err());
    // A dismissal of an activity that liveupdate does not know reports nothing.
    driver.dismiss("gone", 2);
    block_on(live.sync_events()).unwrap();
    assert!(events.try_recv().is_err());

    // The sink is told where an activity stands, not every step it took: a state that came and went between two
    // asks is not reported. A stale time that passed, and an update that made the activity fresh before anyone
    // asked: nothing.
    let mut start = StartInput::progress("order:43", "Order 43", "Preparing", 0.1);
    start.snapshot.stale_at = Some(20_000);
    let fresh = block_on(live.start(start)).unwrap();
    shown(&events, &fresh.id);
    driver.lock().now = 20_000;
    block_on(live.update(progress(&fresh.id, 1, 90_000))).unwrap();
    assert!(events.try_recv().is_err());
    // The person removes it and the application ends it before anyone asked: ended alone.
    driver.dismiss(&fresh.id, 3);
    block_on(live.end(EndInput::immediate(&fresh.id))).unwrap();
    assert_eq!(
        events.try_recv().unwrap(),
        state_of(&fresh.id, "order:43", ActivityState::Ended)
    );
    block_on(live.sync_events()).unwrap();
    assert!(events.try_recv().is_err());

    // A list looks at what Android shows as well: what it finds is in its records and reaches the sink with it.
    let mut start = StartInput::progress("order:44", "Order 44", "Preparing", 0.1);
    start.snapshot.stale_at = Some(30_000);
    let listed = block_on(live.start(start)).unwrap();
    shown(&events, &listed.id);
    driver.lock().now = 30_000;
    let records = block_on(live.list()).unwrap();
    let found = records.iter().find(|record| record.id == listed.id);
    assert_eq!(found.unwrap().state, ActivityState::Stale);
    assert_eq!(
        events.try_recv().unwrap(),
        state_of(&listed.id, "order:44", ActivityState::Stale)
    );
    assert!(events.try_recv().is_err());

    // A drain whose question for the states fails: the stored events it took reach the sink and it fails, so
    // that the host drains again. The states stay due.
    let driver = MemoryDriver::default();
    let (live, events) = connect(LiveUpdateBackend::new(driver.clone()));
    let start = StartInput::progress("order:45", "Order 45", "Preparing", 0.1);
    let tapped = block_on(live.start(start)).unwrap();
    shown(&events, &tapped.id);
    driver.tap(&tapped.id, 1, "track");
    driver.dismiss(&tapped.id, 2);
    driver.lock().failing_observations = 1;
    assert_eq!(
        block_on(live.sync_events()).unwrap_err().code,
        "NATIVE_ERROR"
    );
    assert!(matches!(events.try_recv().unwrap(), Event::Action(tap) if tap.action_id == "track"));
    assert!(events.try_recv().is_err());
    block_on(live.sync_events()).unwrap();
    assert_eq!(
        events.try_recv().unwrap(),
        state_of(&tapped.id, "order:45", ActivityState::Dismissed)
    );

    // A call without a state due asks Android nothing more than the call itself does: as much as on a backend
    // that nobody listens to.
    let asked = |listening: bool| {
        let driver = MemoryDriver::default();
        let backend = Arc::new(LiveUpdateBackend::new(driver.clone()));
        let _listener = listening.then(|| connect(backend.clone()));
        let start = StartInput::progress("order:1", "Order 1", "Preparing", 0.1);
        let order = block_on(backend.start(start)).unwrap();
        let before = driver.lock().observations;
        let update = UpdateInput::progress(&order.id, 1, "Order 1", "On the way", 0.6);
        block_on(backend.update(update)).unwrap();
        driver.lock().observations - before
    };
    assert_eq!(asked(true), asked(false));

    // A layout, with the app's files, that Android shows a moment after the post.
    let (live, events, driver) = android_with_files("states");
    driver.lock().delay_listing = true;
    let record = block_on(live.start_layout(ride("ride:7", None))).unwrap();
    assert_eq!(record.state, ActivityState::Unknown);
    assert!(events.try_recv().is_err());
    driver.show_unlisted();
    block_on(live.sync_events()).unwrap();
    assert_eq!(
        events.try_recv().unwrap(),
        state_of(&record.id, "ride:7", ActivityState::Active)
    );
    // Its end is reported at the call, and an end again reports nothing.
    block_on(live.end_layout(liveactivity::LayoutEndInput::new(&record.id))).unwrap();
    assert_eq!(
        events.try_recv().unwrap(),
        state_of(&record.id, "ride:7", ActivityState::Ended)
    );
    block_on(live.end_layout(liveactivity::LayoutEndInput::new(&record.id))).unwrap();
    block_on(live.sync_events()).unwrap();
    assert!(events.try_recv().is_err());
}

#[test]
fn req_e_006_the_backend_says_when_to_ask_again() {
    // REQ-E-006
    use liveactivity::liveupdate::NextCheck;
    let driver = MemoryDriver::default();
    let backend = Arc::new(LiveUpdateBackend::new(driver.clone()));
    // Nothing runs: nothing is due until an operation or an event.
    assert_eq!(backend.next_check().unwrap(), NextCheck::Never);

    // A state that nobody was told: now. Before a sink is connected it stays due, and a drain takes nothing.
    let mut start = StartInput::progress("order:42", "Order 42", "Preparing", 0.1);
    start.snapshot.stale_at = Some(9_000);
    let order = block_on(backend.start(start)).unwrap();
    assert_eq!(backend.next_check().unwrap(), NextCheck::Now);
    assert_eq!(backend.pump_events().unwrap(), 0);
    assert_eq!(backend.next_check().unwrap(), NextCheck::Now);
    // The sink that comes is told, and the next thing ahead is the stale time.
    let (live, events) = connect(backend.clone());
    assert_eq!(backend.pump_events().unwrap(), 1);
    assert_eq!(
        events.try_recv().unwrap(),
        state_of(&order.id, "order:42", ActivityState::Active)
    );
    assert_eq!(backend.next_check().unwrap(), NextCheck::At(9_000));

    // The time passes. It asks Android nothing, so it names the time still: the host compares it with its clock
    // and asks. A stale activity has no stale time ahead.
    driver.lock().now = 9_000;
    assert_eq!(backend.next_check().unwrap(), NextCheck::At(9_000));
    assert_eq!(backend.pump_events().unwrap(), 1);
    assert_eq!(
        events.try_recv().unwrap(),
        state_of(&order.id, "order:42", ActivityState::Stale)
    );
    assert_eq!(backend.next_check().unwrap(), NextCheck::Never);

    // A post that Android has not shown yet: soon.
    driver.lock().delay_listing = true;
    let start = StartInput::progress("order:43", "Order 43", "Preparing", 0.1);
    let later = block_on(live.start(start)).unwrap();
    assert!(events.try_recv().is_err());
    assert_eq!(backend.next_check().unwrap(), NextCheck::Soon);
    driver.show_unlisted();
    assert_eq!(backend.pump_events().unwrap(), 1);
    assert_eq!(
        events.try_recv().unwrap(),
        state_of(&later.id, "order:43", ActivityState::Active)
    );
    assert_eq!(backend.next_check().unwrap(), NextCheck::Never);
}

#[test]
fn req_e_007_events_reach_the_sink_one_at_a_time_in_the_order_they_were_taken() {
    // REQ-E-007
    use std::{
        sync::{
            Mutex,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };
    let order = |key: &str| StartInput::progress(key, "Order", "Preparing", 0.1);

    // A sink that is busy with its first event until the test lets it go, and notes a second caller.
    let backend = Arc::new(LiveUpdateBackend::new(MemoryDriver::default()));
    let (sender, events) = mpsc::channel();
    let (release, released) = mpsc::channel::<()>();
    let released = Mutex::new(released);
    let (inside, overlapped) = (AtomicBool::new(false), Arc::new(AtomicBool::new(false)));
    let (first, noted) = (AtomicBool::new(true), overlapped.clone());
    let sink = move |event: Event| {
        if inside.swap(true, Ordering::SeqCst) {
            noted.store(true, Ordering::SeqCst);
        }
        sender.send(event).unwrap();
        if first.swap(false, Ordering::SeqCst) {
            // Until the test lets it go, or gave up.
            let _ = released.lock().unwrap().recv();
        }
        inside.store(false, Ordering::SeqCst);
    };
    let live = LiveActivity::builder()
        .backend(backend.clone())
        .event_sink(sink)
        .build()
        .unwrap();
    std::thread::scope(|threads| {
        // Dropped with this closure, so that a failed assertion does not leave the sink waiting.
        let release = release;
        let busy = threads.spawn(|| block_on(live.start(order("order:1"))).unwrap());
        let wait = Duration::from_secs(10);
        let Event::State(one) = events.recv_timeout(wait).unwrap() else {
            panic!("the first event is the state of the first start");
        };
        // While the sink is busy another thread starts and ends an activity. Its calls return, and the sink is
        // not entered: what they took waits its turn.
        let two = block_on(live.start(order("order:2"))).unwrap();
        block_on(live.end(EndInput::immediate(&two.id))).unwrap();
        assert!(events.try_recv().is_err());
        release.send(()).unwrap();
        assert_eq!(busy.join().unwrap().id, one.activity_id);
        // The thread that was handing over hands these over too, in the order they were taken.
        assert_eq!(
            events.try_iter().collect::<Vec<_>>(),
            [
                state_of(&two.id, "order:2", ActivityState::Active),
                state_of(&two.id, "order:2", ActivityState::Ended),
            ]
        );
    });
    assert!(!overlapped.load(Ordering::SeqCst));

    // A sink that calls the backend, as no lock is held while it runs: it ends what it is told is shown, and
    // drains. The end's state follows the one in hand.
    let backend = Arc::new(LiveUpdateBackend::new(MemoryDriver::default()));
    let (sender, events) = mpsc::channel();
    let called = backend.clone();
    let sink = move |event: Event| {
        sender.send(event.clone()).unwrap();
        if let Event::State(change) = event
            && change.state == ActivityState::Active
        {
            block_on(called.end(EndInput::immediate(&change.activity_id))).unwrap();
            block_on(called.sync_events()).unwrap();
        }
    };
    let live = LiveActivity::builder()
        .backend(backend)
        .event_sink(sink)
        .build()
        .unwrap();
    let three = block_on(live.start(order("order:3"))).unwrap();
    assert_eq!(
        events.try_iter().collect::<Vec<_>>(),
        [
            state_of(&three.id, "order:3", ActivityState::Active),
            state_of(&three.id, "order:3", ActivityState::Ended),
        ]
    );

    // A sink that panics loses the event it was handed, and no other: what was queued behind it reaches the
    // sink with the next ask. Two activities are shown before anyone listens, so that one drain takes both.
    let backend = Arc::new(LiveUpdateBackend::new(MemoryDriver::default()));
    let four = block_on(backend.start(order("order:4"))).unwrap();
    let five = block_on(backend.start(order("order:5"))).unwrap();
    let (sender, events) = mpsc::channel();
    let first = AtomicBool::new(true);
    let sink = move |event: Event| {
        assert!(!first.swap(false, Ordering::SeqCst), "the sink fails once");
        sender.send(event).unwrap();
    };
    let _live = LiveActivity::builder()
        .backend(backend.clone())
        .event_sink(sink)
        .build()
        .unwrap();
    let drained = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| backend.pump_events()));
    assert!(drained.is_err());
    assert!(events.try_recv().is_err());
    assert_eq!(backend.pump_events().unwrap(), 0);
    let rest: Vec<Event> = events.try_iter().collect();
    assert!(matches!(
        rest.as_slice(),
        [Event::State(change)] if change.state == ActivityState::Active
            && [&four.id, &five.id].contains(&&change.activity_id)
    ));
}

/// A ride whose lock screen has a text for fresh content and one for stale content, a toggle and a button; and
/// what the start says beside its layout.
fn muted_ride(key: &str, members: serde_json::Value) -> liveactivity::LayoutStartInput {
    let mut request = serde_json::json!({
        "activityKey": key,
        "schemaVersion": 2,
        "fallback": "notification",
        "layout": { "lockScreen": { "type": "vstack", "children": [
            { "type": "text", "bind": "eta", "ifStale": false },
            { "type": "text", "value": "No signal", "ifStale": true },
            { "type": "toggle", "actionId": "mute", "bind": "muted",
                "child": { "type": "text", "value": "Mute" } },
            { "type": "button", "actionId": "pet", "child": { "type": "text", "value": "Pet" } },
        ] } },
        "snapshot": { "revision": 0, "values": { "eta": "6 min", "muted": false } },
    });
    request
        .as_object_mut()
        .unwrap()
        .extend(members.as_object().unwrap().clone());
    liveactivity::parse_request("start_layout", request).unwrap()
}

/// An update of a [`muted_ride`], and what it says beside its values.
fn muted_update(
    id: &str,
    revision: u64,
    eta: &str,
    muted: bool,
    members: serde_json::Value,
) -> liveactivity::LayoutUpdateInput {
    let mut request = serde_json::json!({ "id": id,
        "snapshot": { "revision": revision, "values": { "eta": eta, "muted": muted } } });
    let members = members.as_object().unwrap().clone();
    for (name, member) in members {
        match name.as_str() {
            "android" => request["snapshot"]["android"] = member,
            _ => request[name] = member,
        }
    }
    layout_update(request)
}

#[test]
fn req_a_013_a_toggle_is_an_action_on_android_and_its_tap_says_what_was_asked() {
    // REQ-A-013
    let (live, events, driver) = android_with_files("toggle");
    let record = block_on(live.start_layout(muted_ride("ride:7", serde_json::json!({})))).unwrap();
    shown(&events, &record.id);
    let titles = || -> Vec<(String, String)> {
        let posted = driver.posted(&record.id).presentation.unwrap();
        let titled =
            |action: &liveactivity::NotificationAction| (action.id.clone(), action.title.clone());
        posted.actions.iter().map(titled).collect()
    };
    let pair = |id: &str, title: &str| (id.to_owned(), title.to_owned());
    let tap = |action: &str, on: Option<bool>, sequence: u64| {
        Event::Action(ActionEvent {
            action_id: action.into(),
            activity_id: record.id.clone(),
            activity_key: Some("ride:7".into()),
            on,
            tap: stored(sequence),
        })
    };

    // The switch is off: its action is titled by its label, and a tap asks for on. A button's tap asks nothing.
    assert_eq!(titles(), [pair("mute", "Mute"), pair("pet", "Pet")]);
    driver.tap(&record.id, 1, "mute");
    driver.tap(&record.id, 2, "pet");
    block_on(live.sync_events()).unwrap();
    assert_eq!(events.try_recv().unwrap(), tap("mute", Some(true), 1));
    assert_eq!(events.try_recv().unwrap(), tap("pet", None, 2));
    // The tap changed nothing: the application moves the switch with an update, and the next tap asks for off.
    assert_eq!(titles(), [pair("mute", "Mute"), pair("pet", "Pet")]);
    let on = muted_update(&record.id, 1, "5 min", true, serde_json::json!({}));
    block_on(live.update_layout(on)).unwrap();
    assert_eq!(titles(), [pair("mute", "✓ Mute"), pair("pet", "Pet")]);
    driver.tap(&record.id, 3, "mute");
    block_on(live.sync_events()).unwrap();
    assert_eq!(events.try_recv().unwrap(), tap("mute", Some(false), 3));

    // A snapshot with an Android content shows that content's actions: one of them has no state to ask for,
    // whatever its id.
    let content = serde_json::json!({ "android": { "title": "Driver on the way", "status": "4 min away",
        "presentation": { "style": { "kind": "standard" },
            "actions": [{ "id": "mute", "title": "Mute all" }] } } });
    block_on(live.update_layout(muted_update(&record.id, 2, "4 min", true, content))).unwrap();
    assert_eq!(titles(), [pair("mute", "Mute all")]);
    driver.tap(&record.id, 4, "mute");
    block_on(live.sync_events()).unwrap();
    assert_eq!(events.try_recv().unwrap(), tap("mute", None, 4));
    assert!(events.try_recv().is_err());

    // An id is asked a state for when the action it names is a toggle's: here a button shows under the id while
    // the toggle is hidden, and the toggle once it shows.
    let request = serde_json::json!({ "activityKey": "ride:8", "schemaVersion": 2, "fallback": "notification",
        "layout": { "lockScreen": { "type": "vstack", "children": [
            { "type": "toggle", "actionId": "m", "bind": "muted", "if": "switch",
                "child": { "type": "text", "value": "Mute" } },
            { "type": "button", "actionId": "m", "child": { "type": "text", "value": "Menu" } },
        ] } },
        "snapshot": { "revision": 0, "values": { "muted": true, "switch": false } } });
    let start = liveactivity::parse_request("start_layout", request).unwrap();
    let twins = block_on(live.start_layout(start)).unwrap();
    shown(&events, &twins.id);
    let asked = |sequence: u64| {
        driver.tap(&twins.id, sequence, "m");
        block_on(live.sync_events()).unwrap();
        match events.try_recv().unwrap() {
            Event::Action(tap) => tap.on,
            other => panic!("a tap, not {other:?}"),
        }
    };
    assert_eq!(asked(5), None);
    let switch = layout_update(serde_json::json!({ "id": twins.id,
        "snapshot": { "revision": 1, "values": { "muted": true, "switch": true } } }));
    block_on(live.update_layout(switch)).unwrap();
    assert_eq!(asked(6), Some(false));
}

#[test]
fn req_a_014_android_names_what_it_leaves_of_staleness_and_sounds_and_refuses_a_schedule() {
    // REQ-A-014
    let none = || serde_json::json!({});
    let offering = MemoryDriver::offering(|offered| offered.alert = true);
    let (live, events, driver) = android_on(offering, "staleness");

    // A layout whose nodes follow staleness is drawn as it is while fresh, and the record says so.
    let mut start = muted_ride("ride:7", none());
    start.snapshot.stale_at = Some(1_000);
    let record = block_on(live.start_layout(start)).unwrap();
    shown(&events, &record.id);
    let named = |record: &liveactivity::LayoutActivityRecord, feature: &str| {
        record.degraded_features.iter().any(|name| name == feature)
    };
    assert!(named(&record, "layout.ifStale") && !named(&record, "layout.toggle"));
    assert_eq!(driver.posted(&record.id).title, "6 min");
    // Its stale time passes. Nothing redraws the notification: the sink is told, and the application updates.
    driver.lock().now = 1_000;
    block_on(live.sync_events()).unwrap();
    assert_eq!(
        events.try_recv().unwrap(),
        state_of(&record.id, "ride:7", ActivityState::Stale)
    );
    assert_eq!(driver.posted(&record.id).title, "6 min");
    let told = muted_update(&record.id, 1, "No signal", false, none());
    block_on(live.update_layout(told)).unwrap();
    assert_eq!(driver.posted(&record.id).title, "No signal");

    // The sound of an alert is the channel's on Android: the update alerts, and the record names the sound.
    let alert = |sound: Option<&str>| {
        let mut alert = serde_json::json!({ "title": "Almost there", "body": "2 minutes away" });
        if let Some(sound) = sound {
            alert["sound"] = serde_json::json!(sound);
        }
        serde_json::json!({ "alert": alert })
    };
    let loud = muted_update(&record.id, 2, "2 min", false, alert(Some("chime.aiff")));
    let updated = block_on(live.update_layout(loud)).unwrap();
    assert!(driver.posted(&record.id).presentation.unwrap().alert);
    assert!(named(&updated, "alert.sound") && !named(&updated, "alert"));
    // An alert without a sound names nothing, and the next snapshot does not repeat the name.
    let plain = muted_update(&record.id, 3, "1 min", false, alert(None));
    let updated = block_on(live.update_layout(plain)).unwrap();
    assert!(!named(&updated, "alert.sound") && !named(&updated, "alert"));

    // A scheduled start is refused and leaves nothing: nothing is posted, and its key starts an activity.
    let posts = driver.lock().posts;
    let schedule = serde_json::json!({ "scheduled": { "at": 1_900_000_000_000_u64,
        "alert": { "title": "Your ride", "body": "It starts now" } } });
    let refused = block_on(live.start_layout(muted_ride("ride:8", schedule)));
    assert_eq!(refused.unwrap_err().code, "UNSUPPORTED_FEATURE");
    assert_eq!(driver.lock().posts, posts);
    assert_eq!(block_on(live.list_layout()).unwrap().len(), 1);
    // The iOS options are no part of what Android keeps and compares: starts that differ in them are one activity.
    let transient = |transient: bool| serde_json::json!({ "ios": { "transient": transient } });
    let first = block_on(live.start_layout(muted_ride("ride:8", transient(true)))).unwrap();
    let plain = block_on(live.start_layout(muted_ride("ride:8", none()))).unwrap();
    let other = block_on(live.start_layout(muted_ride("ride:8", transient(false)))).unwrap();
    assert_eq!((&plain.id, &other.id), (&first.id, &first.id));
    assert_eq!(block_on(live.list_layout()).unwrap().len(), 2);

    // Where the device alerts once, the whole alert is left out and named, as before.
    let (live, _events, driver) = android_with_files("staleness-quiet");
    let record = block_on(live.start_layout(muted_ride("ride:7", none()))).unwrap();
    let loud = muted_update(&record.id, 1, "2 min", false, alert(Some("chime.aiff")));
    let updated = block_on(live.update_layout(loud)).unwrap();
    assert!(!driver.posted(&record.id).presentation.unwrap().alert);
    assert!(named(&updated, "alert") && !named(&updated, "alert.sound"));
}

/// A [`muted_ride`] that begins at `at`, with the alert the contract asks of a scheduled start, and what else the
/// start says in `more`.
fn scheduled_ride(key: &str, at: u64, more: serde_json::Value) -> liveactivity::LayoutStartInput {
    let mut members = serde_json::json!({ "scheduled": { "at": at,
        "alert": { "title": "Your ride", "body": "It starts now" } } });
    let more = more.as_object().unwrap().clone();
    members.as_object_mut().unwrap().extend(more);
    muted_ride(key, members)
}

/// A device whose liveupdate begins an activity at a time.
fn scheduling() -> MemoryDriver {
    MemoryDriver::offering(|offered| offered.scheduled_start = true)
}

#[test]
fn req_a_015_a_scheduled_start_waits_on_android_and_begins_at_its_time() {
    // REQ-A-015
    let none = || serde_json::json!({});
    let (live, _events, driver) = android_on(scheduling(), "scheduled");
    driver.lock().now = 1_000;
    let record = block_on(live.start_layout(scheduled_ride("ride:7", 60_000, none()))).unwrap();
    assert_eq!(record.state, ActivityState::Pending);
    // The time reached liveupdate, which handed the receipt to its Android side: nothing is posted.
    {
        let state = driver.lock();
        assert_eq!((state.schedules, state.posts), (1, 0));
        assert_eq!(state.records[0].origin.start_at, Some(60_000));
        assert!(state.observed.is_empty());
    }
    let listed = block_on(live.list_layout()).unwrap();
    assert_eq!(listed[0].state, ActivityState::Pending);
    // At its time the Android side posts it, while the application does not run. The next call finds it running.
    driver.lock().now = 60_000;
    driver.fire_alarms();
    let listed = block_on(live.list_layout()).unwrap();
    assert_eq!(
        (listed[0].id.as_str(), listed[0].state),
        (record.id.as_str(), ActivityState::Active)
    );
    assert_eq!(driver.posted(&record.id).title, "6 min");
    // A time that has passed begins the activity at once: its record is never pending.
    let late = block_on(live.start_layout(scheduled_ride("ride:8", 59_000, none()))).unwrap();
    assert_eq!(late.state, ActivityState::Active);
    assert_eq!(driver.posted(&late.id).title, "6 min");

    // Where liveupdate offers no scheduled start, the start is refused and nothing is kept: nothing reaches the
    // Android side, and the key starts an activity.
    let (bare, _events, driver) = android_with_files("scheduled-none");
    let refused = block_on(bare.start_layout(scheduled_ride("ride:7", 60_000, none())));
    assert_eq!(refused.unwrap_err().code, "UNSUPPORTED_FEATURE");
    {
        let state = driver.lock();
        assert_eq!((state.schedules, state.posts), (0, 0));
        assert!(state.records.is_empty());
    }
    assert!(block_on(bare.list_layout()).unwrap().is_empty());
    let now = block_on(bare.start_layout(muted_ride("ride:7", none()))).unwrap();
    assert_eq!(driver.posted(&now.id).title, "6 min");
}

#[test]
fn req_a_016_a_pending_activity_keeps_its_key_takes_no_update_and_can_be_ended() {
    // REQ-A-016
    let none = || serde_json::json!({});
    let (live, _events, driver) = android_on(scheduling(), "pending");
    driver.lock().now = 1_000;
    let start = |key: &str, at: u64| block_on(live.start_layout(scheduled_ride(key, at, none())));
    let kept = |key: &str| {
        let state = driver.lock();
        let of_key =
            |record: &&liveactivity::liveupdate::DriverRecord| record.origin.activity_key == key;
        state
            .records
            .iter()
            .filter(of_key)
            .map(|record| record.origin.start_at)
            .collect::<Vec<_>>()
    };
    let pending = start("ride:7", 60_000).unwrap();

    // The same start again answers it, and so does the same start with another time: the activity keeps its own.
    for at in [60_000, 90_000] {
        let again = start("ride:7", at).unwrap();
        assert_eq!(
            (again.id.as_str(), again.state),
            (pending.id.as_str(), ActivityState::Pending)
        );
    }
    assert_eq!(kept("ride:7"), [Some(60_000)]);
    assert_eq!(driver.lock().schedules, 1);
    // Another request under its key is a conflict.
    let mut other = serde_json::to_value(scheduled_ride("ride:7", 60_000, none())).unwrap();
    other["snapshot"]["values"]["eta"] = serde_json::json!("9 min");
    let other = liveactivity::parse_request("start_layout", other).unwrap();
    assert_eq!(
        block_on(live.start_layout(other)).unwrap_err().code,
        "CONFLICT"
    );
    // It begins with the content of its start: an update is a conflict, also one that repeats that content, and
    // reaches nothing.
    for (revision, eta) in [(1, "5 min"), (0, "6 min")] {
        let early = muted_update(&pending.id, revision, eta, false, none());
        assert_eq!(
            block_on(live.update_layout(early)).unwrap_err().code,
            "CONFLICT"
        );
    }
    assert_eq!(driver.lock().posts, 0);

    // A scheduling that failed is made again by the same start, at the time the activity keeps.
    driver.lock().failing_schedules = 1;
    assert!(start("ride:8", 70_000).is_err());
    let retried = start("ride:8", 75_000).unwrap();
    assert_eq!(retried.state, ActivityState::Pending);
    assert_eq!(kept("ride:8"), [Some(70_000)]);
    assert_eq!(driver.lock().schedules, 3);

    // A start whose time has passed began at once. Until Android lists it, the same start with another time is
    // still that activity: liveupdate is asked with the time the activity keeps.
    driver.lock().delay_listing = true;
    let began = start("ride:9", 500).unwrap();
    assert_eq!(began.state, ActivityState::Unknown);
    assert_eq!(start("ride:9", 800).unwrap().id, began.id);
    assert_eq!(kept("ride:9"), [Some(500)]);
    driver.show_unlisted();
    block_on(live.end_layout(liveactivity::LayoutEndInput::immediate(&began.id))).unwrap();
    driver.lock().posts = 0;

    // When and how an activity begins is no part of what makes two starts the same: the request without a time
    // is the activity that waits.
    let at_once = block_on(live.start_layout(muted_ride("ride:7", none()))).unwrap();
    assert_eq!(
        (at_once.id.as_str(), at_once.state),
        (pending.id.as_str(), ActivityState::Pending)
    );

    // An end ends it: its alarm is taken away, nothing is shown at its time, and its key is free.
    let ended = block_on(live.end_layout(liveactivity::LayoutEndInput::immediate(&pending.id)));
    assert_eq!(ended.unwrap().state, ActivityState::Ended);
    assert!(driver.lock().cancelled.contains(&pending.id));
    assert_ne!(start("ride:7", 60_000).unwrap().id, pending.id);
    // With any policy, also on a device that keeps no last content. Its final content is checked as that of any
    // end, and is not applied.
    let last = |values: serde_json::Value| {
        let end = serde_json::json!({ "id": retried.id, "policy": { "kind": "after", "at": 500_000 },
            "snapshot": { "revision": 1, "values": values } });
        liveactivity::parse_request::<liveactivity::LayoutEndInput>("end_layout", end).unwrap()
    };
    let undeclared =
        last(serde_json::json!({ "eta": "Arrived", "muted": false, "driver": "Mochi" }));
    assert_eq!(
        block_on(live.end_layout(undeclared)).unwrap_err().code,
        "INVALID_INPUT"
    );
    let arrived = last(serde_json::json!({ "eta": "Arrived", "muted": false }));
    let ended = block_on(live.end_layout(arrived)).unwrap();
    assert_eq!(
        (ended.state, ended.snapshot.revision),
        (ActivityState::Ended, 0)
    );
    for id in block_on(live.list_layout())
        .unwrap()
        .into_iter()
        .map(|record| record.id)
    {
        block_on(live.end_layout(liveactivity::LayoutEndInput::immediate(&id))).unwrap();
    }
    driver.lock().now = 90_000;
    driver.fire_alarms();
    block_on(live.list_layout()).unwrap();
    {
        let state = driver.lock();
        assert_eq!(state.posts, 0);
        assert!(state.observed.is_empty() && state.finals.is_empty());
    }

    // A scheduling that failed has no alarm: its time passes and nothing begins it, neither the alarm nor a
    // call of the application. The same start does, at once.
    let (live, _events, driver) = android_on(scheduling(), "pending-unset");
    driver.lock().failing_schedules = 1;
    assert!(block_on(live.start_layout(scheduled_ride("ride:7", 60_000, none()))).is_err());
    driver.lock().now = 60_000;
    driver.fire_alarms();
    let listed = block_on(live.list_layout()).unwrap();
    assert_eq!(listed[0].state, ActivityState::Pending);
    assert_eq!(driver.lock().posts, 0);
    let begun = block_on(live.start_layout(scheduled_ride("ride:7", 60_000, none()))).unwrap();
    assert_eq!(
        (begun.id.as_str(), begun.state),
        (listed[0].id.as_str(), ActivityState::Active)
    );
    assert_eq!(driver.posted(&begun.id).title, "6 min");
}

#[test]
fn req_a_017_a_scheduled_start_alerts_when_it_begins_where_the_device_alerts() {
    // REQ-A-017
    let none = || serde_json::json!({});
    let named = |record: &liveactivity::LayoutActivityRecord, feature: &str| {
        record.degraded_features.iter().any(|name| name == feature)
    };
    let with_a_sound = |key: &str| {
        let scheduled = serde_json::json!({ "scheduled": { "at": 60_000,
            "alert": { "title": "Your ride", "body": "It starts now", "sound": "chime.aiff" } } });
        muted_ride(key, scheduled)
    };
    let alerting = MemoryDriver::offering(|offered| {
        offered.scheduled_start = true;
        offered.alert = true;
    });
    let (live, _events, driver) = android_on(alerting, "scheduled-alert");
    driver.lock().now = 1_000;
    // The receipt that the Android side posts at the time alerts. The sound is the channel's, and is named.
    let loud = block_on(live.start_layout(with_a_sound("ride:7"))).unwrap();
    assert!(named(&loud, "alert.sound") && !named(&loud, "alert"));
    let plain = block_on(live.start_layout(scheduled_ride("ride:8", 60_000, none()))).unwrap();
    assert!(!named(&plain, "alert.sound") && !named(&plain, "alert"));
    driver.lock().now = 60_000;
    driver.fire_alarms();
    for record in [&loud, &plain] {
        let posted = driver.posted(&record.id);
        assert!(posted.presentation.as_ref().unwrap().alert);
        // The notification alerts with its own content: the alert's title and body are iOS's.
        let written = serde_json::to_string(&posted).unwrap();
        assert!(!written.contains("Your ride") && !written.contains("It starts now"));
    }

    // A device that alerts once begins it silently, and the record names the alert.
    let (live, _events, driver) = android_on(scheduling(), "scheduled-quiet");
    let quiet = block_on(live.start_layout(with_a_sound("ride:7"))).unwrap();
    assert!(named(&quiet, "alert") && !named(&quiet, "alert.sound"));
    driver.lock().now = 60_000;
    driver.fire_alarms();
    assert!(!driver.posted(&quiet.id).presentation.unwrap().alert);
}

#[test]
fn req_a_018_the_sink_hears_pending_and_what_became_of_the_start() {
    // REQ-A-018
    use liveactivity::liveupdate::NextCheck;
    let none = || serde_json::json!({});
    let driver = scheduling();
    let files = liveactivity::android::AndroidFiles::new("Mochi", app_files("scheduled-states"));
    let backend = Arc::new(LiveUpdateBackend::new(driver.clone()).with_files(files));
    let (live, events) = connect(backend.clone());
    let told = |id: &str, key: &str, state: ActivityState| {
        assert_eq!(events.try_recv().unwrap(), state_of(id, key, state));
        assert!(events.try_recv().is_err());
    };
    driver.lock().now = 1_000;

    // Pending, once. The backend is next asked at the start time.
    let mut request = scheduled_ride("ride:7", 60_000, none());
    request.snapshot.stale_at = Some(900_000);
    let ride = block_on(live.start_layout(request)).unwrap();
    told(&ride.id, "ride:7", ActivityState::Pending);
    assert_eq!(backend.next_check().unwrap(), NextCheck::At(60_000));
    block_on(live.sync_events()).unwrap();
    assert!(events.try_recv().is_err());
    // It began while the application did not run: active, once, and the stale time is what is ahead.
    driver.lock().now = 60_000;
    driver.fire_alarms();
    block_on(live.sync_events()).unwrap();
    told(&ride.id, "ride:7", ActivityState::Active);
    assert_eq!(backend.next_check().unwrap(), NextCheck::At(900_000));

    // The person removed it before the application ran again: dismissed, with no active before it.
    let removed = block_on(live.start_layout(scheduled_ride("ride:8", 120_000, none()))).unwrap();
    told(&removed.id, "ride:8", ActivityState::Pending);
    driver.lock().now = 120_000;
    driver.fire_alarms();
    driver.dismiss(&removed.id, 1);
    block_on(live.sync_events()).unwrap();
    told(&removed.id, "ride:8", ActivityState::Dismissed);
    // Android could not post it at its time: the same.
    let unposted = block_on(live.start_layout(scheduled_ride("ride:9", 180_000, none()))).unwrap();
    told(&unposted.id, "ride:9", ActivityState::Pending);
    driver.lock().now = 180_000;
    driver.lock().failing_posts = 1;
    driver.fire_alarms();
    block_on(live.sync_events()).unwrap();
    told(&unposted.id, "ride:9", ActivityState::Dismissed);

    // The alarm was lost: the activity begins at the application's next call.
    let lost = block_on(live.start_layout(scheduled_ride("ride:10", 240_000, none()))).unwrap();
    told(&lost.id, "ride:10", ActivityState::Pending);
    driver.lock().now = 240_000;
    let listed = block_on(live.list_layout()).unwrap();
    let state = |id: &str| {
        listed
            .iter()
            .find(|record| record.id == id)
            .map(|record| record.state)
    };
    assert_eq!(state(&lost.id), Some(ActivityState::Active));
    assert_eq!(driver.posted(&lost.id).title, "6 min");
    told(&lost.id, "ride:10", ActivityState::Active);

    // A time that has passed: active, and no pending.
    let late = block_on(live.start_layout(scheduled_ride("ride:11", 239_000, none()))).unwrap();
    told(&late.id, "ride:11", ActivityState::Active);
}

#[test]
fn req_a_019_the_capabilities_say_whether_a_start_can_be_scheduled_and_how_exactly() {
    // REQ-A-019
    let offered = |driver: MemoryDriver, test: &str| {
        let (live, _events, _driver) = android_on(driver, test);
        let capabilities = block_on(live.capabilities()).unwrap();
        let json = serde_json::to_value(&capabilities).unwrap();
        let presentation = capabilities.android_presentation.unwrap();
        let read = (presentation.scheduled_start, presentation.exact_start);
        let written = &json["androidPresentation"];
        assert_eq!(written["scheduledStart"], serde_json::json!(read.0));
        assert_eq!(written["exactStart"], serde_json::json!(read.1));
        read
    };
    let exact = MemoryDriver::offering(|offered| {
        offered.scheduled_start = true;
        offered.exact_start = true;
    });
    assert_eq!(offered(exact, "capabilities-exact"), (true, true));
    assert_eq!(offered(scheduling(), "capabilities-inexact"), (true, false));
    assert_eq!(
        offered(MemoryDriver::default(), "capabilities-unscheduled"),
        (false, false)
    );
}

#[test]
fn req_a_020_a_scheduled_start_takes_no_foreground_placement() {
    // REQ-A-020
    let placed = |placement: &str| serde_json::json!({ "android": { "visibility": "private", "placement": placement } });
    let offering = MemoryDriver::offering(|offered| {
        offered.scheduled_start = true;
        offered.foreground_placement = true;
    });
    let (live, _events, driver) = android_on(offering, "scheduled-foreground");
    let held = block_on(live.start_layout(scheduled_ride("ride:7", 60_000, placed("foreground"))));
    assert_eq!(held.unwrap_err().code, "UNSUPPORTED_FEATURE");
    // Nothing is kept: nothing reached the Android side, and the key starts a scheduled activity that is tagged.
    {
        let state = driver.lock();
        assert_eq!(state.schedules, 0);
        assert!(state.records.is_empty());
    }
    let tagged = block_on(live.start_layout(scheduled_ride("ride:7", 60_000, placed("tagged"))));
    assert_eq!(tagged.unwrap().state, ActivityState::Pending);
}

#[test]
fn req_a_022_a_native_call_that_panics_fails_no_later_call() {
    // REQ-A-022
    use common::PanickingBridge;
    use liveactivity::LayoutEndInput;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    /// A call of the application, with the id of an activity that runs.
    type Call = fn(&LiveActivity, &str) -> liveactivity::Result<()>;
    /// An activity as the list shows it: its key, its revision and its state.
    type Listed = (&'static str, u64, ActivityState);

    let update = |id: &str, revision: u64| {
        liveactivity::parse_request("update_layout", ride_update(id, revision, None)).unwrap()
    };
    let listed = |live: &LiveActivity| -> liveactivity::Result<Vec<(String, u64, ActivityState)>> {
        let record = |record: liveactivity::LayoutActivityRecord| {
            (record.activity_key, record.snapshot.revision, record.state)
        };
        Ok(block_on(live.list_layout())?
            .into_iter()
            .map(record)
            .collect())
    };
    let (active, ended) = (ActivityState::Active, ActivityState::Ended);
    // The native call that panics and the call of the application that meets it; then what the list holds once
    // the same call was made again.
    let met: [(&str, &str, Call, &[Listed]); 5] = [
        (
            "read",
            "list",
            |live, _| block_on(live.list_layout()).map(drop),
            &[("ride:7", 0, active)],
        ),
        (
            "post",
            "start",
            |live, _| block_on(live.start_layout(ride("ride:8", None))).map(drop),
            &[("ride:7", 0, active), ("ride:8", 0, active)],
        ),
        (
            "post",
            "update",
            |live, id| {
                let update = ride_update(id, 1, None);
                let update = liveactivity::parse_request("update_layout", update).unwrap();
                block_on(live.update_layout(update)).map(drop)
            },
            &[("ride:7", 1, active)],
        ),
        (
            "write",
            "end",
            |live, id| block_on(live.end_layout(LayoutEndInput::new(id))).map(drop),
            &[("ride:7", 0, ended)],
        ),
        (
            "events",
            "drain",
            |live, _| block_on(live.sync_events()),
            &[("ride:7", 0, active)],
        ),
    ];
    for (command, name, call, left) in met {
        let bridge = PanickingBridge::new(MemoryDriver::default());
        let files = app_files(&format!("native-panic-{name}"));
        let backend = LiveUpdateBackend::over_bridge(bridge.clone())
            .with_files(liveactivity::android::AndroidFiles::new("Mochi", &files));
        let (live, events) = connect(backend);
        let running = block_on(live.start_layout(ride("ride:7", None))).unwrap();
        shown(&events, &running.id);

        // The panic reaches the caller: the backend catches none.
        bridge.panic_in(Some(command));
        let panicked = catch_unwind(AssertUnwindSafe(|| call(&live, &running.id)));
        assert!(
            panicked.is_err(),
            "{name}: the call panics with its native call"
        );
        bridge.panic_in(None);

        // The same call again is answered, and it leaves what it leaves without a panic before it: each activity
        // once, with the revision and the state the call asked for, and Android shows a notification of that
        // revision for each that runs.
        assert_eq!(
            call(&live, &running.id),
            Ok(()),
            "{name}: the same call again"
        );
        let running_revisions: Vec<u64> = left
            .iter()
            .filter(|(.., state)| *state == active)
            .map(|(_, revision, _)| *revision)
            .collect();
        let left: Vec<_> = left
            .iter()
            .map(|(key, revision, state)| ((*key).to_owned(), *revision, *state))
            .collect();
        assert_eq!(listed(&live), Ok(left), "{name}: the list");
        let mut posted: Vec<u64> = {
            let android = bridge.android.lock();
            let shown = android.observed.values();
            shown.map(|shown| shown.snapshot.revision).collect()
        };
        posted.sort_unstable();
        assert_eq!(posted, running_revisions, "{name}: what Android shows");

        // And so is every call of another activity: its start, an update, a tap in a drain, the list and its end.
        let other = block_on(live.start_layout(ride("ride:9", None)));
        let other = other.unwrap_or_else(|error| panic!("{name}: a start answers {error:?}"));
        let updated = block_on(live.update_layout(update(&other.id, 1)));
        assert_eq!(
            updated.map(|record| record.snapshot.revision),
            Ok(1),
            "{name}"
        );
        bridge.android.tap(&other.id, 0, "pet");
        assert_eq!(block_on(live.sync_events()), Ok(()), "{name}: a drain");
        let tapped =
            |event: Event| matches!(event, Event::Action(tap) if tap.activity_id == other.id);
        assert!(
            events.try_iter().any(tapped),
            "{name}: a tap reaches the sink"
        );
        let of_other = |listed: Vec<(String, u64, ActivityState)>| {
            let other = |(key, ..): &(String, u64, ActivityState)| key == "ride:9";
            listed.into_iter().filter(other).collect::<Vec<_>>()
        };
        assert_eq!(
            listed(&live).map(of_other),
            Ok(vec![("ride:9".to_owned(), 1, active)]),
            "{name}"
        );
        let ended = block_on(live.end_layout(LayoutEndInput::new(&other.id)));
        assert_eq!(
            ended.map(|record| record.state),
            Ok(ActivityState::Ended),
            "{name}"
        );
        std::fs::remove_dir_all(&files).unwrap();
    }

    // A drain that panics after it took a tap: the read of the states panics, which follows the stored events. The
    // tap is not lost and not handed over by that call: it waits in the queue for the next ask.
    let bridge = PanickingBridge::new(MemoryDriver::default());
    let files = app_files("native-panic-taken");
    let backend = LiveUpdateBackend::over_bridge(bridge.clone())
        .with_files(liveactivity::android::AndroidFiles::new("Mochi", &files));
    let (live, events) = connect(backend);
    let running = block_on(live.start_layout(ride("ride:7", None))).unwrap();
    shown(&events, &running.id);
    bridge.android.tap(&running.id, 0, "pet");
    bridge.panic_in(Some("read"));
    let panicked = catch_unwind(AssertUnwindSafe(|| block_on(live.sync_events())));
    assert!(panicked.is_err());
    bridge.panic_in(None);
    assert!(bridge.android.lock().events.events.is_empty());
    assert!(events.try_recv().is_err());
    assert_eq!(block_on(live.sync_events()), Ok(()));
    let tapped = |event: Event| matches!(event, Event::Action(tap) if tap.action_id == "pet");
    assert!(events.try_iter().any(tapped));
    std::fs::remove_dir_all(&files).unwrap();
}

#[test]
fn req_a_024_a_start_says_whether_its_activity_carries_a_watchs_ongoing_activity() {
    // REQ-A-024
    let (live, _events, driver) = android_with_files("wear-ongoing");
    let with_option = |key: &str, carried: Option<bool>, android: Option<serde_json::Value>| {
        let mut request = serde_json::to_value(ride(key, android)).unwrap();
        request["android"] = serde_json::json!({ "visibility": "private" });
        if let Some(carried) = carried {
            request["android"]["wearOngoingActivity"] = serde_json::json!(carried);
        }
        liveactivity::parse_request::<liveactivity::LayoutStartInput>("start_layout", request)
            .unwrap()
    };
    let carried = |id: &str| {
        driver
            .posted(id)
            .presentation
            .unwrap()
            .wear_ongoing_activity
    };

    // A start that says nothing carries it, in what the backend projects and in a content that says nothing.
    let plain = block_on(live.start_layout(with_option("ride:1", None, None))).unwrap();
    assert!(carried(&plain.id));
    let update = layout_update(ride_update(&plain.id, 1, Some(driving(70, "2 min"))));
    block_on(live.update_layout(update)).unwrap();
    assert!(carried(&plain.id));
    // A content turns it off for its snapshot, and the next snapshot carries it again.
    let mut off = driving(80, "1 min");
    off["presentation"]["wearOngoingActivity"] = serde_json::json!(false);
    block_on(live.update_layout(layout_update(ride_update(&plain.id, 2, Some(off))))).unwrap();
    assert!(!carried(&plain.id));
    block_on(live.update_layout(layout_update(ride_update(&plain.id, 3, None)))).unwrap();
    assert!(carried(&plain.id));
    // A start that says `true` is the same activity as one that says nothing.
    let again = block_on(live.start_layout(with_option("ride:1", Some(true), None))).unwrap();
    assert_eq!(again.id, plain.id);

    // A start that turns it off does so for every snapshot of the activity: the projected ones, and a content,
    // whatever it says of it.
    let quiet = block_on(live.start_layout(with_option("ride:2", Some(false), None))).unwrap();
    assert!(!carried(&quiet.id));
    block_on(live.update_layout(layout_update(ride_update(&quiet.id, 1, None)))).unwrap();
    assert!(!carried(&quiet.id));
    let update = layout_update(ride_update(&quiet.id, 2, Some(driving(70, "2 min"))));
    block_on(live.update_layout(update)).unwrap();
    assert!(!carried(&quiet.id));
    let mut on = driving(80, "1 min");
    on["presentation"]["wearOngoingActivity"] = serde_json::json!(true);
    block_on(live.update_layout(layout_update(ride_update(&quiet.id, 3, Some(on))))).unwrap();
    assert!(!carried(&quiet.id));
    // The same start again is the same activity.
    let again = block_on(live.start_layout(with_option("ride:2", Some(false), None))).unwrap();
    assert_eq!(again.id, quiet.id);
    // The other start of the same key, which carries it, is another activity than the one that does not.
    let other = block_on(live.start_layout(with_option("ride:2", None, None)));
    assert_eq!(other.unwrap_err().code, "CONFLICT");
}

#[test]
fn req_a_024_the_capabilities_say_whether_a_watch_shows_an_ongoing_activity() {
    // REQ-A-024
    let watch = MemoryDriver::offering(|offered| offered.wear_ongoing_activity = true);
    let (live, _events, _driver) = android_on(watch, "wear-capability");
    let offered = block_on(live.capabilities()).unwrap();
    assert!(offered.android_presentation.unwrap().wear_ongoing_activity);
    assert_eq!(
        serde_json::to_value(block_on(live.capabilities()).unwrap()).unwrap()["androidPresentation"]
            ["wearOngoingActivity"],
        true
    );
    let (phone, _events, _driver) = android_with_files("wear-capability-phone");
    let offered = block_on(phone.capabilities()).unwrap();
    assert!(!offered.android_presentation.unwrap().wear_ongoing_activity);
}

#[test]
fn req_a_032_a_start_keeps_every_snapshot_of_its_activity_off_a_paired_watch() {
    // REQ-A-032
    // A device that keeps an activity off a paired watch, and the last content of an end.
    let local_device = MemoryDriver::offering(|offered| {
        offered.local_only = true;
        offered.delayed_end = true;
    });
    let (live, _events, driver) = android_on(local_device, "local-only");
    let with_option = |key: &str, local: Option<bool>| {
        let mut request = serde_json::to_value(ride(key, None)).unwrap();
        request["android"] = serde_json::json!({ "visibility": "private" });
        if let Some(local) = local {
            request["android"]["localOnly"] = serde_json::json!(local);
        }
        liveactivity::parse_request::<liveactivity::LayoutStartInput>("start_layout", request)
            .unwrap()
    };
    let local = |id: &str| driver.posted(id).presentation.unwrap().local_only;

    // A start that says nothing sends its activity to a paired watch; a content keeps its own snapshot off it.
    let plain = block_on(live.start_layout(with_option("ride:1", None))).unwrap();
    assert!(!local(&plain.id));
    let mut kept = driving(70, "2 min");
    kept["presentation"]["localOnly"] = serde_json::json!(true);
    block_on(live.update_layout(layout_update(ride_update(&plain.id, 1, Some(kept))))).unwrap();
    assert!(local(&plain.id));
    block_on(live.update_layout(layout_update(ride_update(&plain.id, 2, None)))).unwrap();
    assert!(!local(&plain.id));
    // A start that says `false` is the same activity as one that says nothing.
    let again = block_on(live.start_layout(with_option("ride:1", Some(false)))).unwrap();
    assert_eq!(again.id, plain.id);

    // A start kept local keeps every snapshot local: the projected ones, and a content, whatever it says.
    let off = block_on(live.start_layout(with_option("ride:2", Some(true)))).unwrap();
    assert!(local(&off.id));
    block_on(live.update_layout(layout_update(ride_update(&off.id, 1, None)))).unwrap();
    assert!(local(&off.id));
    let mut sent = driving(80, "1 min");
    sent["presentation"]["localOnly"] = serde_json::json!(false);
    block_on(live.update_layout(layout_update(ride_update(&off.id, 2, Some(sent))))).unwrap();
    assert!(local(&off.id));
    // The same start again is the same activity, and the start of the same key that sends it is another one,
    // which posts nothing.
    let again = block_on(live.start_layout(with_option("ride:2", Some(true)))).unwrap();
    assert_eq!(again.id, off.id);
    let posts = driver.lock().posts;
    let other = block_on(live.start_layout(with_option("ride:2", None)));
    assert_eq!(other.unwrap_err().code, "CONFLICT");
    assert_eq!(driver.lock().posts, posts);
    // The last content of an end stays local too.
    let mut end = serde_json::json!({ "id": off.id, "policy": { "kind": "after", "at": 500_000 },
        "snapshot": { "revision": 3, "values": { "eta": "here", "done": 1.0 } } });
    end["snapshot"]["android"] = driving(100, "here");
    let end =
        liveactivity::parse_request::<liveactivity::LayoutEndInput>("end_layout", end).unwrap();
    block_on(live.end_layout(end)).unwrap();
    let last = driver.lock().finals[&off.id].snapshot.clone();
    assert!(last.presentation.unwrap().local_only);

    // A member that is no Boolean is refused on every platform, in the options and in a content.
    for wrong in [
        serde_json::json!(null),
        serde_json::json!("yes"),
        serde_json::json!(1),
    ] {
        let mut request = serde_json::to_value(ride("ride:5", None)).unwrap();
        request["android"] = serde_json::json!({ "visibility": "private", "localOnly": wrong });
        let read =
            liveactivity::parse_request::<liveactivity::LayoutStartInput>("start_layout", request);
        assert_eq!(read.unwrap_err().code, "INVALID_INPUT", "{wrong}");
        let mut content = driving(70, "2 min");
        content["presentation"]["localOnly"] = wrong.clone();
        let update = ride_update(&plain.id, 9, Some(content));
        let read =
            liveactivity::parse_request::<liveactivity::LayoutUpdateInput>("update_layout", update);
        assert_eq!(read.unwrap_err().code, "INVALID_INPUT", "{wrong}");
    }
}

#[test]
fn req_a_032_an_activity_kept_off_the_watch_needs_the_devices_capability() {
    // REQ-A-032
    let local_device = MemoryDriver::offering(|offered| offered.local_only = true);
    let (live, _events, _driver) = android_on(local_device, "local-capability");
    let offered = block_on(live.capabilities()).unwrap();
    assert!(offered.android_presentation.unwrap().local_only);
    assert_eq!(
        serde_json::to_value(block_on(live.capabilities()).unwrap()).unwrap()["androidPresentation"]
            ["localOnly"],
        true
    );

    // An adapter before liveupdate 0.9 would send it to the watch: such a start or update is refused and posts
    // nothing.
    let (earlier, _events, driver) = android_with_files("local-capability-earlier");
    assert!(
        !block_on(earlier.capabilities())
            .unwrap()
            .android_presentation
            .unwrap()
            .local_only
    );
    let mut request = serde_json::to_value(ride("ride:3", None)).unwrap();
    request["android"] = serde_json::json!({ "visibility": "private", "localOnly": true });
    let start =
        liveactivity::parse_request::<liveactivity::LayoutStartInput>("start_layout", request)
            .unwrap();
    let posts = driver.lock().posts;
    let refused = block_on(earlier.start_layout(start)).unwrap_err();
    assert_eq!(refused.code, "UNSUPPORTED_FEATURE");
    assert_eq!(driver.lock().posts, posts);
    let plain = block_on(earlier.start_layout(ride("ride:4", None))).unwrap();
    let mut kept = driving(70, "2 min");
    kept["presentation"]["localOnly"] = serde_json::json!(true);
    let refused =
        block_on(earlier.update_layout(layout_update(ride_update(&plain.id, 1, Some(kept)))));
    assert_eq!(refused.unwrap_err().code, "UNSUPPORTED_FEATURE");
}

#[test]
fn req_a_033_the_settings_of_promotion_open_through_the_android_backend() {
    // REQ-A-033
    let (live, _events, driver) = android_with_files("promotion-settings");
    assert_eq!(block_on(live.open_promotion_settings()), Ok(()));
    assert_eq!(driver.lock().settings_opened, 1);
    // A device with no such screen, and an adapter before liveupdate 0.9, say so.
    driver.lock().settings_screen = false;
    let refused = block_on(live.open_promotion_settings()).unwrap_err();
    assert_eq!(refused.code, "UNSUPPORTED_FEATURE");
    assert_eq!(driver.lock().settings_opened, 1);

    // A backend of an application's own that does not have the command says that it cannot, and a platform
    // without Live Activities that it has none.
    let (own, _taps) = connect(liveactivity::testkit::MemoryBackend::new());
    assert_eq!(
        block_on(own.open_promotion_settings()).unwrap_err().code,
        "UNSUPPORTED_FEATURE"
    );
    let host = LiveActivity::builder().build().unwrap();
    assert_eq!(
        block_on(host.open_promotion_settings()).unwrap_err().code,
        "UNSUPPORTED_PLATFORM"
    );
}

#[test]
fn req_e_008_an_event_of_an_unknown_kind_reaches_the_sink() {
    // REQ-E-008
    use liveactivity::UnknownEvent;
    let (live, events, driver) = android_with_files("unknown-event");
    let record = block_on(live.start_layout(ride("ride:7", None))).unwrap();
    shown(&events, &record.id);
    // A later AAR stored an event of a kind that this liveupdate does not know, between two it knows.
    driver.tap(&record.id, 0, "pet");
    driver.later(&record.id, 1, "snoozed");
    driver.open(&record.id, 2, None);
    block_on(live.sync_events()).unwrap();
    assert!(matches!(events.try_recv().unwrap(), Event::Action(tap) if tap.action_id == "pet"));
    assert_eq!(
        events.try_recv().unwrap(),
        Event::Unknown(UnknownEvent {
            activity_id: record.id.clone(),
            activity_key: Some("ride:7".into()),
        })
    );
    assert!(matches!(events.try_recv().unwrap(), Event::Opened(_)));
    assert!(events.try_recv().is_err());
    let wire = serde_json::to_value(UnknownEvent {
        activity_id: "id".into(),
        activity_key: None,
    });
    assert_eq!(wire.unwrap(), serde_json::json!({ "activityId": "id" }));

    // Without the app's files the backend knows no key, and passes the event on all the same. A dismissal is
    // no such event: it is a state.
    let driver = MemoryDriver::default();
    let (templates, events) = connect(LiveUpdateBackend::new(driver.clone()));
    driver.later("gone", 0, "snoozed");
    driver.dismiss("gone", 1);
    block_on(templates.sync_events()).unwrap();
    assert_eq!(
        events.try_recv().unwrap(),
        Event::Unknown(UnknownEvent {
            activity_id: "gone".into(),
            activity_key: None,
        })
    );
    assert!(
        events
            .try_iter()
            .all(|event| !matches!(event, Event::Unknown(_)))
    );
}

#[test]
fn req_a_026_a_start_without_room_in_the_store_is_a_system_limit() {
    // REQ-A-026
    let (live, _events, driver) = android_with_files("store-room");
    // The longest text an Android content takes, in characters of four bytes: liveupdate keeps it twice a receipt.
    let long = |text: &str| {
        serde_json::json!({ "title": "Ride", "status": "On the way",
            "presentation": { "style": { "kind": "bigText", "value": { "text": text } } } })
    };
    let text = "\u{1F431}".repeat(4000);
    let mut live_ones = Vec::new();
    let refused = loop {
        let key = format!("ride:{}", live_ones.len());
        match block_on(live.start_layout(ride(&key, Some(long(&text))))) {
            Ok(record) => live_ones.push(record),
            Err(error) => break error,
        }
        assert!(live_ones.len() < 32, "no start was refused for the room");
    };
    assert_eq!(refused.code, "SYSTEM_LIMIT");
    assert!(
        refused.message.contains("too large to store"),
        "{refused:?}"
    );

    // Nothing of the refused start is kept or posted: the same start is refused again, and no activity is new.
    let key = format!("ride:{}", live_ones.len());
    let posts = driver.lock().posts;
    let again = block_on(live.start_layout(ride(&key, Some(long(&text)))));
    assert_eq!(again.unwrap_err().code, "SYSTEM_LIMIT");
    assert_eq!(driver.lock().posts, posts);
    let listed = block_on(live.list_layout()).unwrap();
    assert_eq!(listed.len(), live_ones.len());
    assert!(listed.iter().all(|record| record.activity_key != key));

    // A small activity still has room, and so may its update to the long text, which a receipt then holds once.
    // The first such update that has none is refused, and leaves what was shown.
    let grow = |id: &str| layout_update(ride_update(id, 1, Some(long(&text))));
    let mut small = None;
    for place in 0..2 {
        let key = format!("ride:small:{place}");
        let record = block_on(live.start_layout(ride(&key, Some(long("Short"))))).unwrap();
        if let Err(error) = block_on(live.update_layout(grow(&record.id))) {
            assert_eq!(error.code, "SYSTEM_LIMIT");
            small = Some(record);
            break;
        }
    }
    let small = small.expect("an update was refused for the room");
    let shown = |id: &str| {
        let listed = block_on(live.list_layout()).unwrap();
        let record = listed.into_iter().find(|record| record.id == id);
        record.unwrap().snapshot.revision
    };
    let text_of = |id: &str| match driver.posted(id).presentation.unwrap().style {
        liveactivity::PresentationStyle::BigText(style) => style.text.chars().count(),
        _ => 0,
    };
    assert_eq!((shown(&small.id), text_of(&small.id)), (0, 5));

    // An end makes room: the ended activity's receipt goes for it, and the update is taken.
    let ended = liveactivity::LayoutEndInput::immediate(live_ones[0].id.clone());
    block_on(live.end_layout(ended)).unwrap();
    block_on(live.update_layout(grow(&small.id))).unwrap();
    assert_eq!((shown(&small.id), text_of(&small.id)), (1, 4000));
}

/// What the adapter says of the tap that `MemoryDriver::tap` stored under `sequence`.
fn stored(sequence: u64) -> liveactivity::Tap {
    liveactivity::Tap {
        id: Some(sequence.to_string()),
        at: Some(1_800_000_000_000),
        ..liveactivity::Tap::default()
    }
}

#[test]
fn req_a_029_a_layouts_button_and_a_contents_action_carry_their_targets() {
    // REQ-A-029
    let record_target = serde_json::json!({ "kind": "foregroundService", "component": "dev.example.app.RecorderService",
        "action": "dev.example.app.RECORD" });
    let start = |key: &str| {
        let mut request = serde_json::to_value(ride(key, None)).unwrap();
        request["android"] =
            serde_json::json!({ "visibility": "public", "targets": { "pet": record_target } });
        liveactivity::parse_request::<liveactivity::LayoutStartInput>("start_layout", request)
            .unwrap()
    };
    let target_of = |driver: &MemoryDriver, id: &str, action: &str| {
        driver
            .posted(id)
            .presentation
            .unwrap()
            .actions
            .into_iter()
            .find(|posted| posted.id == action)
            .and_then(|posted| posted.target)
            .map(|target| serde_json::to_value(target).unwrap())
    };

    // A device whose adapter delivers targets: the projected button carries the start's target, in every snapshot.
    let offering = MemoryDriver::offering(|offered| offered.action_targets = true);
    let (live, _events, driver) = android_on(offering, "targets");
    let record = block_on(live.start_layout(start("rec:1"))).unwrap();
    assert_eq!(
        target_of(&driver, &record.id, "pet"),
        Some(record_target.clone())
    );
    block_on(live.update_layout(layout_update(ride_update(&record.id, 1, None)))).unwrap();
    assert_eq!(
        target_of(&driver, &record.id, "pet"),
        Some(record_target.clone())
    );
    // An Android content names its own: the start's target is the projection's alone.
    let mut content = driving(60, "3 min");
    content["presentation"]["actions"] = serde_json::json!([
        { "id": "stop", "title": "Stop", "target": { "kind": "broadcast", "component": "dev.example.app.StopReceiver" } },
        { "id": "pet", "title": "Pet" },
    ]);
    block_on(live.update_layout(layout_update(ride_update(
        &record.id,
        2,
        Some(content.clone()),
    ))))
    .unwrap();
    assert_eq!(
        target_of(&driver, &record.id, "stop"),
        Some(
            serde_json::json!({ "kind": "broadcast", "component": "dev.example.app.StopReceiver" })
        )
    );
    assert_eq!(target_of(&driver, &record.id, "pet"), None);

    // A device that does not deliver them: the start is refused and nothing is posted.
    let (live, _events, driver) = android_with_files("no-targets");
    let refused = block_on(live.start_layout(start("rec:2"))).unwrap_err();
    assert_eq!(refused.code, "UNSUPPORTED_FEATURE");
    assert!(block_on(live.list_layout()).unwrap().is_empty());
    let _ = driver;
}

#[test]
fn req_e_010_a_handled_tap_is_reported_and_is_no_action() {
    // REQ-E-010
    let (live, events, driver) = android_with_files("handled");
    let record = block_on(live.start_layout(ride("ride:7", None))).unwrap();
    driver.handled(&record.id, 5, "pet");
    block_on(live.sync_events()).unwrap();
    let handled = events
        .try_iter()
        .find(|event| !matches!(event, Event::State(_) | Event::Started(_)))
        .unwrap();
    assert_eq!(
        handled,
        Event::Handled(liveactivity::HandledEvent {
            activity_id: record.id.clone(),
            activity_key: Some("ride:7".into()),
            action_id: "pet".into(),
            tap: stored(5),
        })
    );
    assert!(
        !events
            .try_iter()
            .any(|event| matches!(event, Event::Action(_)))
    );
    let Event::Handled(payload) = handled else {
        panic!("a handled tap is `Event::Handled`");
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({ "activityId": record.id, "activityKey": "ride:7", "actionId": "pet",
            "tap": { "id": "5", "at": 1_800_000_000_000_u64 } })
    );
}
