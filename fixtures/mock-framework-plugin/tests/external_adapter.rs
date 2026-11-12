//! Conformance tests compiled outside liveactivity.

use futures_executor::block_on;
use liveactivity::{
    BackendContext, DismissalPolicy, EndRequest, Error, ErrorCode, EventSink, LiveActivity,
    LiveActivityBackend, LiveActivityEvent, LiveActivityEventPayload, StartRequest, UpdateRequest,
};
use mock_framework_plugin::{
    FrameworkEventSink, MockFrameworkPlugin, ObservedCall, connect_mock_plugin,
};
use serde_json::json;
use std::sync::{Arc, mpsc};

#[test]
fn req_f_001_external_plugin_implements_the_backend() {
    // REQ-F-001
    fn assert_backend<T: LiveActivityBackend>() {}
    fn assert_sink<T: EventSink>() {}

    assert_backend::<MockFrameworkPlugin>();
    assert_sink::<FrameworkEventSink>();
    connect_mock_plugin("external", None).expect("public connection succeeds");
}

#[test]
fn req_f_002_commands_reach_the_injected_backend() {
    // REQ-F-002
    let harness = connect_mock_plugin("external", None).unwrap();
    let start = StartRequest::new("delivery", 1, json!({"progress": 0.25}));
    let update = UpdateRequest::new("delivery-id", 1, json!({"progress": 0.5}));
    let end = EndRequest::new(
        "delivery-id",
        2,
        Some(json!({"progress": 1.0})),
        DismissalPolicy::Immediate,
    );

    block_on(harness.service.capabilities()).unwrap();
    block_on(harness.service.start(start.clone())).unwrap();
    block_on(harness.service.update(update.clone())).unwrap();
    block_on(harness.service.end(end.clone())).unwrap();
    assert!(block_on(harness.service.list()).unwrap().is_empty());

    assert_eq!(harness.calls.recv().unwrap(), ObservedCall::Capabilities);
    assert_eq!(harness.calls.recv().unwrap(), ObservedCall::Start(start));
    assert_eq!(harness.calls.recv().unwrap(), ObservedCall::Update(update));
    assert_eq!(harness.calls.recv().unwrap(), ObservedCall::End(end));
    assert_eq!(harness.calls.recv().unwrap(), ObservedCall::List);
    assert!(harness.calls.try_recv().is_err());
}

#[test]
fn req_f_010_a_backend_that_is_called_back_has_no_events_to_sync() {
    // REQ-F-010
    let harness = connect_mock_plugin("external", None).unwrap();

    block_on(harness.service.sync_events()).unwrap();

    assert!(harness.calls.try_recv().is_err());
    assert!(harness.events.try_recv().is_err());
}

#[test]
fn req_f_003_native_events_reach_the_framework_sink() {
    // REQ-F-003
    let harness = connect_mock_plugin("external", None).unwrap();
    let activity = block_on(harness.service.start(StartRequest::new(
        "timer",
        1,
        json!({"seconds": 10}),
    )))
    .unwrap();

    let event = harness.events.recv().unwrap();
    assert_eq!(event, LiveActivityEvent::state_changed(activity.clone()));
    assert!(matches!(
        event.payload,
        LiveActivityEventPayload::StateChanged { activity: observed }
            if observed == activity
    ));
    assert!(harness.events.try_recv().is_err());
}

#[test]
fn req_f_004_backends_are_substitutable() {
    // REQ-F-004
    let first_harness = connect_mock_plugin("tauri-android", None).unwrap();
    let second_harness = connect_mock_plugin("wry-android", None).unwrap();
    let first: &LiveActivity = &first_harness.service;
    let second: &LiveActivity = &second_harness.service;

    assert_eq!(
        block_on(first.capabilities()).unwrap().backend,
        "tauri-android"
    );
    assert_eq!(
        block_on(second.capabilities()).unwrap().backend,
        "wry-android"
    );
}

#[test]
fn req_f_005_backend_errors_keep_their_contract() {
    // REQ-F-005
    let expected = Error::new(ErrorCode::PermissionDenied, "notifications are disabled");
    let harness = connect_mock_plugin("denied", Some(expected.clone())).unwrap();
    let actual = block_on(
        harness
            .service
            .start(StartRequest::new("timer", 1, json!({}))),
    )
    .unwrap_err();

    assert_eq!(actual, expected);
}

#[test]
fn req_f_006_capabilities_are_backend_owned() {
    // REQ-F-006
    let harness = connect_mock_plugin("custom-provider", None).unwrap();
    let capabilities = block_on(harness.service.capabilities()).unwrap();

    assert_eq!(capabilities.backend, "custom-provider");
    assert!(capabilities.start.is_supported());
    assert!(capabilities.update.is_supported());
    assert!(capabilities.end.is_supported());
    assert!(capabilities.list.is_supported());
    assert!(capabilities.actions.is_supported());
}

#[test]
fn req_f_008_invalid_messages_stop_at_the_core_boundary() {
    // REQ-F-008
    let harness = connect_mock_plugin("guarded", None).unwrap();
    let mut wrong_version = StartRequest::new("timer", 1, json!({}));
    wrong_version.contract_version = 999;

    let error = block_on(harness.service.start(wrong_version)).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidRequest);
    assert!(harness.calls.try_recv().is_err());

    let empty_key = StartRequest::new("", 1, json!({}));
    let error = block_on(harness.service.start(empty_key)).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidRequest);
    assert!(harness.calls.try_recv().is_err());

    let (event_sender, events) = mpsc::channel();
    let context = BackendContext::new(FrameworkEventSink::new(event_sender));
    let mut event = LiveActivityEvent::action("activity-1", "open");
    event.contract_version = 999;
    let error = context.emit(event).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidRequest);
    assert!(events.try_recv().is_err());
}

#[test]
fn req_n_001_public_contract_is_framework_neutral() {
    // REQ-N-001
    let manifest = include_str!("../../../crates/liveactivity/Cargo.toml").to_ascii_lowercase();
    for framework in ["tauri", "wry", "dioxus", "slint"] {
        assert!(
            !manifest.contains(framework),
            "{framework} leaked into core"
        );
    }
}

#[test]
fn req_n_002_boundaries_are_send_and_sync() {
    // REQ-N-002
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<LiveActivity>();
    assert_send_sync::<BackendContext>();
    assert_send_sync::<MockFrameworkPlugin>();
    assert_send_sync::<FrameworkEventSink>();
}

#[test]
fn req_n_003_backend_is_dyn_compatible() {
    // REQ-N-003
    let (event_sender, _events) = mpsc::channel();
    let (call_sender, _calls) = mpsc::channel();
    let context = BackendContext::new(FrameworkEventSink::new(event_sender));
    let backend: Arc<dyn LiveActivityBackend> = Arc::new(MockFrameworkPlugin::new(
        context,
        call_sender,
        "dynamic",
        None,
    ));

    assert_eq!(block_on(backend.capabilities()).unwrap().backend, "dynamic");
}

#[test]
fn req_d_002_the_interface_alone_takes_a_backend_that_is_no_default_engine() {
    // REQ-D-002: this package depends on liveactivity without its default engines.
    use liveactivity::testkit::{MemoryBackend, conformance};

    let (event_sender, _events) = mpsc::channel();
    let service = LiveActivity::connect(FrameworkEventSink::new(event_sender), |context| {
        Ok(MemoryBackend::new(context))
    })
    .unwrap();

    let report = block_on(conformance(&service));
    assert_eq!(report.backend, MemoryBackend::NAME);
    report.assert_passed();
}

#[test]
fn req_f_012_a_backend_without_permissions_or_images_says_so() {
    // REQ-F-012
    use liveactivity::{PermissionKind, PermissionState, Permissions};

    let harness = connect_mock_plugin("external", None).unwrap();

    assert_eq!(
        block_on(harness.service.permissions()).unwrap(),
        Permissions::all(PermissionState::Unknown)
    );
    let refused = block_on(
        harness
            .service
            .request_permission(PermissionKind::Notifications),
    )
    .unwrap_err();
    assert_eq!(refused.code, ErrorCode::Unsupported);
    let refused = block_on(harness.service.write_image("cover", vec![1])).unwrap_err();
    assert_eq!(refused.code, ErrorCode::Unsupported);
}
