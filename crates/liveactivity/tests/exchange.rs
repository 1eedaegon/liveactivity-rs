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
