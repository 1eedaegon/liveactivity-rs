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

    // The conformance run ended its activities, and each of those ends was reported.
    let ends: Vec<Event> = taps.try_iter().collect();
    assert!(!ends.is_empty());
    assert!(ends.iter().all(|event| matches!(
        event,
        Event::State(change) if change.state == ActivityState::Ended
    )));

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
fn req_a_002_taps_of_a_content_and_dismissals_reach_the_sink() {
    // REQ-A-002, REQ-E-002
    let (live, events, driver) = android_with_files("content-taps");
    let record = block_on(live.start_layout(ride("ride:7", Some(driving(20, "6 min"))))).unwrap();

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
    let tap = |action: &str| {
        Event::Action(ActionEvent {
            action_id: action.into(),
            activity_id: record.id.clone(),
            activity_key: Some("ride:7".into()),
        })
    };
    assert_eq!(events.try_recv().unwrap(), tap("call"));
    assert_eq!(events.try_recv().unwrap(), tap("pet"));
    // The person removed the notification: a state event, in its place among the taps.
    assert_eq!(
        events.try_recv().unwrap(),
        Event::State(liveactivity::StateEvent {
            activity_id: record.id.clone(),
            activity_key: Some("ride:7".into()),
            state: ActivityState::Dismissed,
        })
    );
    assert!(events.try_recv().is_err());

    // Without the app's files the backend knows no key, and reports the dismissal all the same.
    let driver = MemoryDriver::default();
    let (live, events) = connect(LiveUpdateBackend::new(driver.clone()));
    driver.dismiss("gone", 1);
    block_on(live.sync_events()).unwrap();
    assert_eq!(
        events.try_recv().unwrap(),
        Event::State(liveactivity::StateEvent {
            activity_id: "gone".into(),
            activity_key: None,
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

    // One that offers them all reports each, except the placement, which no member of the contract asks for.
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
        "alert",
        "publicVersion",
        "branding",
    ] {
        assert_eq!(offered[extension], true, "{extension}");
    }
    assert_eq!(offered["foregroundPlacement"], false);

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
    let (live, events, driver) = android_on(offering, "extensions");
    let record = block_on(live.start_layout(ride("ride:7", Some(extended())))).unwrap();

    // What liveupdate posted is the presentation as it was sent.
    let posted = serde_json::to_value(driver.posted(&record.id).presentation).unwrap();
    assert_eq!(posted, extended()["presentation"]);
    assert_eq!(
        serde_json::to_value(&record.snapshot.android).unwrap(),
        extended()
    );

    // liveupdate reports an activity that was opened. The contract has no event for it yet: the drain goes on.
    driver.open(&record.id, 0);
    driver.tap(&record.id, 1, "pet");
    block_on(live.sync_events()).unwrap();
    assert!(matches!(events.try_recv().unwrap(), Event::Action(tap) if tap.action_id == "pet"));
    assert!(events.try_recv().is_err());

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

    // Without the app's files the backend runs templates only, and its drain leaves the same kind out.
    let driver = MemoryDriver::default();
    let (templates, events) = connect(LiveUpdateBackend::new(driver.clone()));
    driver.open("opened", 0);
    driver.tap("opened", 1, "pet");
    block_on(templates.sync_events()).unwrap();
    assert!(matches!(events.try_recv().unwrap(), Event::Action(tap) if tap.action_id == "pet"));
    assert!(events.try_recv().is_err());
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

    // A refused update leaves the activity as it is shown. liveupdate keeps the revision it tried, so until a
    // later one is shown the record's state is unknown, and the corrected content needs the next revision.
    let update = layout_update(ride_update(&started.id, 1, Some(opening(&unopened))));
    assert_eq!(
        block_on(live.update_layout(update)).unwrap_err().code,
        "INVALID_INPUT"
    );
    assert_eq!(driver.posted(&started.id).revision, 0);
    let listed = block_on(live.list_layout()).unwrap();
    assert_eq!(
        (listed[0].state, listed[0].snapshot.revision),
        (ActivityState::Unknown, 0)
    );
    let same = layout_update(ride_update(
        &started.id,
        1,
        Some(opening("myapp://rides/1/map")),
    ));
    assert_eq!(
        block_on(live.update_layout(same)).unwrap_err().code,
        "CONFLICT"
    );
    let update = layout_update(ride_update(
        &started.id,
        2,
        Some(opening("myapp://rides/1/map")),
    ));
    let updated = block_on(live.update_layout(update)).unwrap();
    assert_eq!(
        (updated.state, updated.snapshot.revision),
        (ActivityState::Active, 2)
    );
    assert_eq!(driver.posted(&started.id).revision, 2);

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
        // A start is the application's own call too, and reports nothing.
        assert!(events.try_recv().is_err());
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
    block_on(live.end_layout(LayoutEndInput::immediate(&record.id))).unwrap();
    assert_eq!(events.try_recv().unwrap(), ended(&record.id, "ride:7"));
    let kept = block_on(live.start_layout(ride("ride:8", None))).unwrap();
    let mut end = LayoutEndInput::new(&kept.id);
    end.policy = EndPolicy::After { at };
    block_on(live.end_layout(end)).unwrap();
    assert_eq!(events.try_recv().unwrap(), ended(&kept.id, "ride:8"));
    // The activity ended then: removing its last content early ends nothing again.
    block_on(live.end_layout(LayoutEndInput::immediate(&kept.id))).unwrap();
    assert!(events.try_recv().is_err());

    // What the person removed is reported as dismissed, on the drain. Its end reports nothing more.
    let gone = block_on(live.start_layout(ride("ride:9", None))).unwrap();
    driver.lock().observed.remove(&gone.id);
    driver.dismiss(&gone.id, 1);
    block_on(live.sync_events()).unwrap();
    assert!(matches!(
        events.try_recv().unwrap(),
        Event::State(change) if change.state == ActivityState::Dismissed
    ));
    let over = block_on(live.end_layout(LayoutEndInput::new(&gone.id))).unwrap();
    assert_eq!(over.state, ActivityState::Ended);
    assert!(events.try_recv().is_err());

    // An end that is refused ends nothing: a delayed one on a device that offers none.
    driver.lock().capabilities.presentation.delayed_end = false;
    let stays = block_on(live.start_layout(ride("ride:10", None))).unwrap();
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
    block_on(live.end(EndInput::new(&order.id))).unwrap();
    assert_eq!(events.try_recv().unwrap(), ended(&order.id, "order:43"));
    assert!(events.try_recv().is_err());
}
