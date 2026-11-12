//! Serialized-shape regression tests for contract version 1.
#![cfg(feature = "testkit")]

use liveactivity::{
    ActivityState, Capabilities, Content, DismissalPolicy, EndRequest, Error, ErrorCode, Fallback,
    FeatureSupport, LiveActivityEvent, PermissionKind, PermissionState, Permissions, StartRequest,
    Template, TemplateKind, UpdateRequest,
};
use serde_json::json;

#[test]
fn req_f_007_public_models_have_stable_json() {
    // REQ-F-007
    let start = StartRequest::new("delivery", 2, json!({"progress": 0.25}));
    assert_eq!(
        serde_json::to_value(start).unwrap(),
        json!({
            "contractVersion": 1,
            "activityKey": "delivery",
            "schemaVersion": 2,
            "revision": 0,
            "fallback": "notification",
            "content": {"kind": "native", "value": {"progress": 0.25}}
        })
    );

    let update = UpdateRequest::new("activity-1", 7, json!({"progress": 0.5}));
    assert_eq!(
        serde_json::to_value(update).unwrap(),
        json!({
            "contractVersion": 1,
            "id": "activity-1",
            "revision": 7,
            "content": {"kind": "native", "value": {"progress": 0.5}}
        })
    );

    let end = EndRequest::new(
        "activity-1",
        8,
        None,
        DismissalPolicy::After {
            at: 1_800_000_000_000,
        },
    );
    assert_eq!(
        serde_json::to_value(end).unwrap(),
        json!({
            "contractVersion": 1,
            "id": "activity-1",
            "revision": 8,
            "content": null,
            "dismissal": {"type": "after", "at": 1_800_000_000_000_u64}
        })
    );

    assert_eq!(
        serde_json::to_value(LiveActivityEvent::action("activity-1", "open")).unwrap(),
        json!({
            "contractVersion": 1,
            "type": "action",
            "activityId": "activity-1",
            "action": "open"
        })
    );

    assert_eq!(
        serde_json::to_value(LiveActivityEvent::keyed_action(
            "activity-1",
            "delivery",
            "open"
        ))
        .unwrap(),
        json!({
            "contractVersion": 1,
            "type": "action",
            "activityId": "activity-1",
            "action": "open",
            "activityKey": "delivery"
        })
    );

    assert_eq!(
        serde_json::to_value(Error::new(ErrorCode::Unsupported, "not available")).unwrap(),
        json!({
            "contractVersion": 1,
            "code": "unsupported",
            "message": "not available"
        })
    );
}

#[test]
fn req_f_009_engine_error_codes_and_states_have_stable_names() {
    // REQ-F-009
    for (code, name) in [
        (ErrorCode::InvalidRequest, "invalidRequest"),
        (ErrorCode::Unsupported, "unsupported"),
        (ErrorCode::PermissionDenied, "permissionDenied"),
        (ErrorCode::BackendFailure, "backendFailure"),
        (ErrorCode::EventDelivery, "eventDelivery"),
        (ErrorCode::UnsupportedPlatform, "unsupportedPlatform"),
        (ErrorCode::NotForeground, "notForeground"),
        (ErrorCode::ConfigurationRequired, "configurationRequired"),
        (ErrorCode::PayloadTooLarge, "payloadTooLarge"),
        (ErrorCode::NotFound, "notFound"),
        (ErrorCode::StaleRevision, "staleRevision"),
        (ErrorCode::Conflict, "conflict"),
        (ErrorCode::SystemLimit, "systemLimit"),
    ] {
        assert_eq!(serde_json::to_value(code).unwrap(), json!(name));
        assert_eq!(
            serde_json::from_value::<ErrorCode>(json!(name)).unwrap(),
            code
        );
    }

    for (state, name) in [
        (ActivityState::Active, "active"),
        (ActivityState::Stale, "stale"),
        (ActivityState::Ended, "ended"),
        (ActivityState::Dismissed, "dismissed"),
        (ActivityState::Pending, "pending"),
        (ActivityState::Unknown, "unknown"),
    ] {
        assert_eq!(serde_json::to_value(state).unwrap(), json!(name));
        assert_eq!(
            serde_json::from_value::<ActivityState>(json!(name)).unwrap(),
            state
        );
    }
}

#[test]
fn req_f_011_portable_content_and_the_added_models_have_stable_json() {
    // REQ-F-011
    let start = StartRequest::template(
        "delivery",
        Template::progress("Delivery", "Preparing", 0.25).stale_at(9),
    )
    .revision(3)
    .fallback(Fallback::None);
    let wire = json!({
        "contractVersion": 1,
        "activityKey": "delivery",
        "schemaVersion": 1,
        "revision": 3,
        "fallback": "none",
        "content": {
            "kind": "template",
            "title": "Delivery",
            "status": "Preparing",
            "progress": 0.25,
            "staleAt": 9
        }
    });
    assert_eq!(serde_json::to_value(&start).unwrap(), wire);
    assert_eq!(serde_json::from_value::<StartRequest>(wire).unwrap(), start);
    // A request of before the portable members still reads.
    let older: StartRequest = serde_json::from_value(json!({
        "contractVersion": 1,
        "activityKey": "delivery",
        "schemaVersion": 2,
        "content": {"kind": "native", "value": {}}
    }))
    .unwrap();
    assert_eq!(
        (older.revision, older.fallback),
        (0, Fallback::Notification)
    );

    let timer = Template::timer("Tea", "Brewing", 1_000, 61_000);
    assert_eq!(
        serde_json::to_value(Content::from(timer.clone())).unwrap(),
        json!({
            "kind": "template",
            "title": "Tea",
            "status": "Brewing",
            "timer": {"startedAt": 1_000, "endsAt": 61_000}
        })
    );
    assert_eq!(timer.kind().unwrap(), TemplateKind::Timer);
    let end = EndRequest::new("activity-1", 8, None, DismissalPolicy::Default).with_content(timer);
    assert_eq!(
        serde_json::to_value(end).unwrap()["content"]["kind"],
        json!("template")
    );

    assert_eq!(
        serde_json::to_value(Capabilities {
            max_content_bytes: Some(4096),
            native_presentation: FeatureSupport::Unknown,
            ..Capabilities::all("memory", FeatureSupport::Supported)
        })
        .unwrap(),
        json!({
            "contractVersion": 1,
            "backend": "memory",
            "start": "supported",
            "update": "supported",
            "end": "supported",
            "list": "supported",
            "actions": "supported",
            "templates": "supported",
            "layouts": "supported",
            "images": "supported",
            "nativePresentation": "unknown",
            "finalContent": "supported",
            "delayedDismissal": "supported",
            "maxContentBytes": 4096
        })
    );
    assert!(!FeatureSupport::Unknown.is_supported());

    assert_eq!(
        serde_json::to_value(Permissions {
            notifications: PermissionState::Prompt,
            live_activities: PermissionState::SettingsRequired,
            ..Permissions::all(PermissionState::NotApplicable)
        })
        .unwrap(),
        json!({
            "contractVersion": 1,
            "notifications": "prompt",
            "liveActivities": "settingsRequired",
            "promotion": "notApplicable"
        })
    );
    assert_eq!(
        serde_json::to_value(PermissionKind::LiveActivities).unwrap(),
        json!("liveActivities")
    );
}

#[test]
fn req_f_013_the_core_keeps_the_limits_every_backend_shares() {
    // REQ-F-013
    use liveactivity::testkit::{MemoryBackend, block_on};
    use liveactivity::{EventSink, LiveActivity, LiveActivityEvent, UpdateRequest as Update};
    use serde_json::json;

    struct NoEvents;
    impl EventSink for NoEvents {
        fn emit(&self, _event: LiveActivityEvent) -> liveactivity::Result<()> {
            Ok(())
        }
    }
    let service =
        LiveActivity::connect(NoEvents, |context| Ok(MemoryBackend::new(context))).unwrap();
    let template = || Template::progress("Delivery", "Preparing", 0.25);
    let refused = |request: StartRequest| block_on(service.start(request)).unwrap_err().code;

    // A request that one default engine would refuse never reaches a backend that would take it.
    for key in ["my key", "", "-leading", &"k".repeat(129), "키"] {
        assert_eq!(
            refused(StartRequest::template(key, template())),
            ErrorCode::InvalidRequest,
            "{key}"
        );
    }
    let long = |title: usize, status: usize| Template {
        title: "t".repeat(title),
        status: "s".repeat(status),
        ..template()
    };
    for invalid in [
        long(121, 1),
        long(1, 241),
        Template::progress(" ", "Preparing", 0.25),
        Template::progress("Delivery", "Preparing", 1.5),
        Template::progress("Delivery", "Preparing", f64::NAN),
        Template::timer("Tea", "Brewing", 2, 2),
        Template::timer("Tea", "Brewing", 1, 253_402_300_800_000),
        template().stale_at(253_402_300_800_000),
        Template {
            progress: None,
            ..template()
        },
    ] {
        assert_eq!(
            refused(StartRequest::template("delivery", invalid)),
            ErrorCode::InvalidRequest
        );
    }
    assert_eq!(
        refused(StartRequest::template("delivery", template()).revision(1 << 53)),
        ErrorCode::InvalidRequest
    );
    // The members of portable content do nothing for native content, so they are refused there.
    let native = || StartRequest::new("delivery", 1, json!({}));
    assert_eq!(refused(native().revision(1)), ErrorCode::InvalidRequest);
    assert_eq!(
        refused(native().fallback(Fallback::None)),
        ErrorCode::InvalidRequest
    );

    let started = block_on(service.start(StartRequest::template("delivery", long(120, 240))));
    let started = started.unwrap();
    let late = block_on(service.update(Update::new(&started.id, 1 << 53, template())));
    assert_eq!(late.unwrap_err().code, ErrorCode::InvalidRequest);
}
