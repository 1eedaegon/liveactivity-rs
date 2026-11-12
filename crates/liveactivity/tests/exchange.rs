//! The default backends and a substitute, behind one caller: the backend is exchanged at the
//! single point of construction and nothing else of the caller changes.
#![cfg(all(feature = "liveupdate", feature = "testkit"))]

mod common;

use common::{ChannelSink, MemoryDriver, StringBridge};
use liveactivity::{
    ActivityState, DismissalPolicy, EndRequest, ErrorCode, EventSink, LiveActivity,
    LiveActivityBackend, LiveActivityEvent, LiveActivityEventPayload, PermissionState,
    StartRequest, Template, UpdateRequest,
    android::LiveUpdateBackend,
    testkit::{MemoryBackend, block_on, conformance},
};
use std::sync::{
    Arc, Mutex,
    mpsc::{self, Receiver},
};

/// A connected service and the events its sink received.
type Connected = (LiveActivity, Receiver<LiveActivityEvent>);

/// The one line of an application that names a backend.
type Connect = fn() -> Connected;

fn connect<B: LiveActivityBackend>(
    factory: impl FnOnce(liveactivity::BackendContext) -> B,
) -> Connected {
    let (sender, events) = mpsc::channel();
    let service = LiveActivity::connect(ChannelSink(sender), |context| Ok(factory(context)));
    (service.unwrap(), events)
}

/// The default Android backend, on an in-memory driver in place of the device.
fn android_default() -> Connected {
    connect(|context| LiveUpdateBackend::new(context, MemoryDriver::default()))
}

/// The same engine over a framework's string bridge, as a framework plugin connects it.
fn android_over_a_framework_bridge() -> Connected {
    connect(|context| {
        let bridge = StringBridge {
            driver: MemoryDriver::default(),
            commands: Arc::new(Mutex::new(Vec::new())),
        };
        LiveUpdateBackend::over_bridge(context, bridge)
    })
}

/// A backend that is neither default engine.
fn substitute() -> Connected {
    connect(MemoryBackend::new)
}

const BACKENDS: [(&str, Connect); 3] = [
    ("android-liveupdate", android_default),
    ("android-liveupdate", android_over_a_framework_bridge),
    ("memory", substitute),
];

/// An application's use of the interface, written once. It names no backend and reads only
/// the neutral members of what it gets.
fn deliver(service: &LiveActivity) -> Vec<(u64, ActivityState, Option<Template>)> {
    let seen =
        |record: liveactivity::ActivityRecord| (record.revision, record.state, record.template);
    let started = block_on(service.start(StartRequest::template(
        "order:42",
        Template::progress("Order 42", "Preparing", 0.1),
    )))
    .unwrap();
    let updated = block_on(service.update(UpdateRequest::new(
        &started.id,
        1,
        Template::progress("Order 42", "On the way", 0.6),
    )))
    .unwrap();
    let listed = block_on(service.list()).unwrap();
    let ended = block_on(service.end(EndRequest::new(
        &started.id,
        2,
        None,
        DismissalPolicy::Immediate,
    )))
    .unwrap();
    assert_eq!(listed.len(), 1);
    vec![seen(started), seen(updated), seen(ended)]
}

#[test]
fn req_x_001_every_backend_keeps_the_rules_of_the_interface() {
    // REQ-X-001
    for (name, connect) in BACKENDS {
        let (service, _events) = connect();
        let report = block_on(conformance(&service));
        assert_eq!(report.backend, name);
        assert!(report.checks.len() >= 18, "{}", report.checks.len());
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
            (
                0,
                ActivityState::Active,
                Some(Template::progress("Order 42", "Preparing", 0.1))
            ),
            (
                1,
                ActivityState::Active,
                Some(Template::progress("Order 42", "On the way", 0.6))
            ),
            (
                1,
                ActivityState::Ended,
                Some(Template::progress("Order 42", "On the way", 0.6))
            ),
        ]
    );
    for exchanged in &answers[1..] {
        assert_eq!(exchanged, &answers[0]);
    }
}

#[test]
fn req_x_003_what_backends_differ_in_is_reported_and_never_guessed() {
    // REQ-X-003
    let (android, _events) = android_default();
    let (memory, _events) = substitute();

    // Images: one backend stores them, the other says it does not.
    let android_capabilities = block_on(android.capabilities()).unwrap();
    assert!(!android_capabilities.images.is_supported());
    assert_eq!(
        block_on(android.write_image("cover", vec![1]))
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    assert!(
        block_on(memory.capabilities())
            .unwrap()
            .images
            .is_supported()
    );
    block_on(memory.write_image("cover", vec![1])).unwrap();
    assert_eq!(
        block_on(memory.write_image(" ", vec![1])).unwrap_err().code,
        ErrorCode::InvalidRequest
    );

    // Permissions: each backend answers the same question in the same terms.
    assert_eq!(
        block_on(android.permissions()).unwrap().notifications,
        PermissionState::Granted
    );
    assert_eq!(
        block_on(memory.permissions()).unwrap().live_activities,
        PermissionState::Granted
    );

    // Native content belongs to one backend: another refuses it instead of drawing something.
    let liveupdate_content = common::progress_content(0.25);
    block_on(android.start(StartRequest::new("native:1", 1, liveupdate_content.clone()))).unwrap();
    assert_eq!(
        block_on(memory.start(StartRequest::new("native:1", 1, liveupdate_content)))
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
}

#[test]
fn req_x_004_events_of_an_exchanged_backend_reach_the_same_sink() {
    // REQ-X-004
    struct Shared(Arc<MemoryBackend>);
    let (sender, events) = mpsc::channel();
    let mut kept = None;
    let service = LiveActivity::connect(ChannelSink(sender), |context| {
        let backend = Arc::new(MemoryBackend::new(context));
        kept = Some(Shared(backend.clone()));
        Ok(backend)
    })
    .unwrap();
    let Shared(backend) = kept.unwrap();

    let started = block_on(service.start(StartRequest::template(
        "order:42",
        Template::progress("Order 42", "Preparing", 0.1),
    )))
    .unwrap();
    backend.tap(&started.id, "track").unwrap();
    block_on(service.end(EndRequest::new(
        &started.id,
        1,
        None,
        DismissalPolicy::Default,
    )))
    .unwrap();

    assert_eq!(
        events.recv().unwrap(),
        LiveActivityEvent::keyed_action(&started.id, "order:42", "track")
    );
    let LiveActivityEventPayload::StateChanged { activity } = events.recv().unwrap().payload else {
        panic!("an end is a state change");
    };
    assert_eq!(activity.state, ActivityState::Ended);
    assert_eq!(
        backend.tap("unknown", "track").unwrap_err().code,
        ErrorCode::NotFound
    );

    fn assert_sink<S: EventSink>() {}
    assert_sink::<ChannelSink>();
}
