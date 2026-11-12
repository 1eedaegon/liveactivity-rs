//! The liveupdate engine behind the neutral SPI, driven through `LiveActivity` only.

#![cfg(feature = "liveupdate")]

mod common;

use common::{
    ChannelSink, Harness, MemoryDriver, StringBridge, connect, progress_content, progress_start,
};
use futures_executor::block_on;
use liveactivity::android::{BACKEND_NAME, LiveUpdateBackend};
use liveactivity::{
    ActivityState, BackendContext, Capabilities, DismissalPolicy, EndRequest, ErrorCode,
    FeatureSupport, LiveActivity, LiveActivityBackend, LiveActivityEvent, PermissionKind,
    PermissionState, Permissions, StartRequest, UpdateRequest,
};
use serde_json::json;
use std::sync::{Arc, Mutex, mpsc};

fn end(id: &str, dismissal: DismissalPolicy) -> EndRequest {
    EndRequest::new(id, 9, None, dismissal)
}

fn code<T>(result: liveactivity::Result<T>) -> ErrorCode {
    result.map(|_| ()).unwrap_err().code
}

fn update(id: &str, revision: u64, status: &str) -> UpdateRequest {
    UpdateRequest::new(
        id,
        revision,
        json!({"title": "Delivery", "status": status, "progress": 0.5}),
    )
}

#[test]
fn req_a_001_the_lifecycle_runs_on_the_liveupdate_engine() {
    // REQ-A-001
    let harness = connect();

    let started = block_on(harness.service.start(progress_start("delivery:1"))).unwrap();
    assert_eq!(started.activity_key, "delivery:1");
    assert_eq!(started.revision, 0);
    assert_eq!(started.state, ActivityState::Active);
    assert_eq!(started.backend, BACKEND_NAME);
    assert_eq!(
        started.content,
        json!({
            "schemaVersion": 1,
            "template": "progress",
            "snapshot": {
                "revision": 0,
                "title": "Delivery",
                "status": "Preparing",
                "progress": 0.25
            },
            "visibility": "unknown",
            "degradedFeatures": ["nativeLiveActivity"],
            "promotionRequested": false,
            "promotionState": "notApplicable"
        })
    );

    let updated = block_on(harness.service.update(update(&started.id, 1, "Arriving"))).unwrap();
    assert_eq!(updated.id, started.id);
    assert_eq!(updated.revision, 1);
    assert_eq!(updated.content["snapshot"]["status"], "Arriving");
    assert_eq!(
        block_on(harness.service.list()).unwrap(),
        std::slice::from_ref(&updated)
    );

    let ended = block_on(
        harness
            .service
            .end(end(&started.id, DismissalPolicy::Immediate)),
    )
    .unwrap();
    assert_eq!(ended.state, ActivityState::Ended);
    assert_eq!(ended.revision, 1);
    assert!(harness.driver.lock().observed.is_empty());
    assert_eq!(block_on(harness.service.list()).unwrap(), [ended]);

    // The engine keeps its behavior: a repeated start with the same request is the same activity.
    let again = block_on(harness.service.start(progress_start("delivery:2"))).unwrap();
    let repeated = block_on(harness.service.start(progress_start("delivery:2"))).unwrap();
    assert_eq!(repeated, again);
}

#[test]
fn req_a_002_engine_errors_keep_their_category() {
    // REQ-A-002
    let harness = connect();
    let service = &harness.service;
    let started = block_on(service.start(progress_start("delivery:1"))).unwrap();
    block_on(service.update(update(&started.id, 2, "Arriving"))).unwrap();

    assert_eq!(
        code(block_on(service.update(update(&started.id, 1, "Late")))),
        ErrorCode::StaleRevision
    );
    assert_eq!(
        code(block_on(service.update(update(&started.id, 2, "Other")))),
        ErrorCode::Conflict
    );
    assert_eq!(
        code(block_on(service.update(update("missing", 3, "Gone")))),
        ErrorCode::NotFound
    );
    assert_eq!(
        code(block_on(service.start(StartRequest::new(
            "delivery:1",
            1,
            progress_content(0.75)
        )))),
        ErrorCode::Conflict
    );
    assert_eq!(
        code(block_on(service.end(end(
            &started.id,
            DismissalPolicy::After { at: 1_800_000_000 }
        )))),
        ErrorCode::Unsupported
    );
    let with_final_content = EndRequest::new(
        &started.id,
        3,
        Some(json!({"title": "Delivery", "status": "Done", "progress": 1.0})),
        DismissalPolicy::Immediate,
    );
    assert_eq!(
        code(block_on(service.end(with_final_content))),
        ErrorCode::Unsupported
    );

    let mut without_fallback = progress_content(0.25);
    without_fallback["fallback"] = json!("none");
    assert_eq!(
        code(block_on(service.start(StartRequest::new(
            "delivery:2",
            1,
            without_fallback
        )))),
        ErrorCode::Unsupported
    );

    harness.driver.lock().capabilities.notifications_enabled = false;
    let denied = block_on(service.start(progress_start("delivery:3"))).unwrap_err();
    assert_eq!(denied.code, ErrorCode::PermissionDenied);
    assert_eq!(
        denied.message,
        "Notifications are disabled for this app or channel."
    );

    harness.driver.lock().fail_next_observe = true;
    assert_eq!(code(block_on(service.list())), ErrorCode::BackendFailure);
}

#[test]
fn req_a_003_content_outside_the_engine_contract_stops_before_the_driver() {
    // REQ-A-003
    let harness = connect();
    let service = &harness.service;

    let mut repeated_key = progress_content(0.25);
    repeated_key["activityKey"] = json!("other");
    let mut unknown_member = progress_content(0.25);
    unknown_member["layout"] = json!({});
    let mut secret = progress_content(0.25);
    secret["snapshot"]["title"] = json!(42);
    for start in [
        StartRequest::new("delivery:1", 1, json!("progress")),
        StartRequest::new("delivery:1", 1, repeated_key),
        StartRequest::new("delivery:1", 1, unknown_member),
        StartRequest::new("delivery:1", 256, progress_content(0.25)),
        StartRequest::new("delivery:1", 1, secret),
    ] {
        let error = block_on(service.start(start)).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidRequest);
        assert!(!error.message.contains("42"), "{}", error.message);
    }

    for update in [
        UpdateRequest::new("id", 1, json!([])),
        UpdateRequest::new("id", 1, json!({"title": "Delivery"})),
        UpdateRequest::new(
            "id",
            1,
            json!({"revision": 2, "title": "Delivery", "status": "Late", "progress": 0.5}),
        ),
    ] {
        let error = block_on(service.update(update)).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidRequest);
    }
    assert!(harness.driver.lock().calls.is_empty());

    // A revision that repeats the request's is the same snapshot.
    let started = block_on(service.start(progress_start("delivery:1"))).unwrap();
    let repeated_revision = UpdateRequest::new(
        &started.id,
        1,
        json!({"revision": 1, "title": "Delivery", "status": "Arriving", "progress": 0.5}),
    );
    assert_eq!(
        block_on(service.update(repeated_revision))
            .unwrap()
            .revision,
        1
    );
}

#[test]
fn req_a_004_android_events_reach_the_framework_sink() {
    // REQ-A-004
    let harness = connect();
    let first = block_on(harness.service.start(progress_start("delivery:1"))).unwrap();
    let second = block_on(harness.service.start(progress_start("delivery:2"))).unwrap();
    assert_eq!(harness.backend.pump_events().unwrap().delivered, 0);

    harness.driver.tap(&first.id, 1, "open");
    harness.driver.dismiss(&second.id, 2);
    harness.driver.dismiss("evicted", 3);
    harness.driver.lock().events.dropped = 4;

    let report = harness.backend.pump_events().unwrap();
    assert_eq!(
        (
            report.delivered,
            report.rejected,
            report.unresolved,
            report.dropped
        ),
        (2, 0, 1, 4)
    );
    assert!(!report.reconciliation_failed);
    assert_eq!(
        harness.events.recv().unwrap(),
        LiveActivityEvent::action(&first.id, "open")
    );
    let LiveActivityEvent {
        payload: liveactivity::LiveActivityEventPayload::StateChanged { activity },
        ..
    } = harness.events.recv().unwrap()
    else {
        panic!("a dismissal is a state change");
    };
    assert_eq!(activity.id, second.id);
    assert_eq!(activity.state, ActivityState::Dismissed);
    assert!(harness.events.try_recv().is_err());

    // The neutral service reaches the same drain, which a framework without the backend's type uses.
    harness.driver.tap(&first.id, 30, "open");
    block_on(harness.service.sync_events()).unwrap();
    assert_eq!(
        harness.events.recv().unwrap(),
        LiveActivityEvent::action(&first.id, "open")
    );

    // A drain removes the events: nothing is delivered twice.
    assert_eq!(harness.backend.pump_events().unwrap().delivered, 0);

    // Actions survive a failed reconciliation; the dismissal stays unresolved.
    harness.driver.tap(&first.id, 40, "open");
    harness.driver.dismiss(&first.id, 50);
    harness.driver.lock().fail_next_observe = true;
    let report = harness.backend.pump_events().unwrap();
    assert_eq!((report.delivered, report.unresolved), (1, 1));
    assert!(report.reconciliation_failed);
    harness.events.recv().unwrap();

    // A sink that went away rejects without stopping the pump.
    let Harness {
        backend,
        driver,
        events,
        ..
    } = harness;
    drop(events);
    driver.tap(&first.id, 60, "open");
    driver.tap(&first.id, 70, "stop");
    let report = backend.pump_events().unwrap();
    assert_eq!((report.delivered, report.rejected), (0, 2));
}

#[test]
fn req_a_005_a_framework_string_bridge_backs_the_same_backend() {
    // REQ-A-005
    let driver = MemoryDriver::default();
    let commands = Arc::new(Mutex::new(Vec::new()));
    let bridge = StringBridge {
        driver: driver.clone(),
        commands: commands.clone(),
    };
    let (sender, _events) = mpsc::channel();
    let service = LiveActivity::connect(ChannelSink(sender), |context| {
        Ok(LiveUpdateBackend::over_bridge(context, bridge))
    })
    .unwrap();

    let started = block_on(service.start(progress_start("delivery:1"))).unwrap();
    assert_eq!(started.state, ActivityState::Active);
    block_on(service.update(update(&started.id, 1, "Arriving"))).unwrap();
    let ended = block_on(service.end(end(&started.id, DismissalPolicy::Default))).unwrap();
    assert_eq!(ended.state, ActivityState::Ended);

    let commands = commands.lock().unwrap();
    for command in ["capabilities", "read", "write", "observe", "post", "cancel"] {
        assert!(commands.iter().any(|sent| sent == command), "{command}");
    }
    assert_eq!(driver.lock().records.len(), 1);
}

#[test]
fn req_a_006_capabilities_are_neutral_and_android_detail_stays_on_the_engine() {
    // REQ-A-006
    let harness = connect();
    let capabilities = block_on(harness.service.capabilities()).unwrap();
    assert_eq!(capabilities.backend, BACKEND_NAME);
    assert_eq!(
        capabilities,
        Capabilities {
            layouts: FeatureSupport::Unsupported,
            images: FeatureSupport::Unsupported,
            native_presentation: FeatureSupport::Unsupported,
            final_content: FeatureSupport::Unsupported,
            delayed_dismissal: FeatureSupport::Unsupported,
            ..Capabilities::all(BACKEND_NAME, FeatureSupport::Supported)
        }
    );

    harness.driver.lock().capabilities.presentation.events = false;
    assert!(
        !block_on(harness.service.capabilities())
            .unwrap()
            .actions
            .is_supported()
    );

    // Promotion and the permissions are neutral; the engine stays reachable for the rest.
    assert_eq!(
        block_on(harness.service.permissions()).unwrap(),
        Permissions {
            notifications: PermissionState::Granted,
            promotion: PermissionState::Denied,
            ..Permissions::all(PermissionState::NotApplicable)
        }
    );
    harness.driver.lock().capabilities.promotion_enabled = true;
    assert!(
        harness
            .backend
            .engine()
            .capabilities()
            .unwrap()
            .can_promote()
    );
    let promoting = block_on(harness.service.capabilities()).unwrap();
    assert!(promoting.native_presentation.is_supported());
    assert_eq!(
        block_on(harness.service.permissions()).unwrap().promotion,
        PermissionState::Granted
    );
    // The engine cannot prompt and stores no image: the host does both.
    assert_eq!(
        block_on(
            harness
                .service
                .request_permission(PermissionKind::Notifications)
        )
        .unwrap_err()
        .code,
        ErrorCode::Unsupported
    );
    assert_eq!(
        block_on(harness.service.write_image("cover", vec![1]))
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    let promoted = block_on(harness.service.start(progress_start("delivery:1"))).unwrap();
    assert_eq!(promoted.content["promotionState"], "promoted");
    assert_eq!(promoted.content["degradedFeatures"], json!([]));

    // The backend stays usable as a trait object and from any thread.
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<LiveUpdateBackend<MemoryDriver>>();
    let (sender, _events) = mpsc::channel();
    let dynamic: Arc<dyn LiveActivityBackend> = Arc::new(LiveUpdateBackend::new(
        BackendContext::new(ChannelSink(sender)),
        MemoryDriver::default(),
    ));
    assert_eq!(
        block_on(dynamic.capabilities()).unwrap().backend,
        BACKEND_NAME
    );
}
