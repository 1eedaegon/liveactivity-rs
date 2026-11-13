//! The contract as a caller and a wire see it.
#![cfg(feature = "testkit")]

use liveactivity::{
    ActionEvent, EndInput, Error, LiveActivity, StartInput, UpdateInput, block_on,
    testkit::MemoryBackend, validate_request,
};
use serde_json::json;

#[test]
fn req_c_001_the_contract_has_stable_json() {
    // REQ-C-001
    assert_eq!(
        serde_json::to_value(StartInput::progress(
            "order:42",
            "Order 42",
            "Preparing",
            0.1
        ))
        .unwrap(),
        json!({
            "activityKey": "order:42",
            "schemaVersion": 1,
            "template": "progress",
            "fallback": "notification",
            "snapshot": {"revision": 0, "title": "Order 42", "status": "Preparing", "progress": 0.1}
        })
    );
    assert_eq!(
        serde_json::to_value(UpdateInput::timer("id", 3, "Tea", "Brewing", 1_000, 61_000)).unwrap(),
        json!({
            "id": "id",
            "snapshot": {
                "revision": 3,
                "title": "Tea",
                "status": "Brewing",
                "timer": {"startedAt": 1_000, "endsAt": 61_000}
            }
        })
    );
    assert_eq!(
        serde_json::to_value(EndInput::immediate("id")).unwrap(),
        json!({"id": "id", "policy": {"kind": "immediate"}})
    );
    assert_eq!(
        serde_json::to_value(Error::new("STALE_REVISION", "Activity revision is stale.")).unwrap(),
        json!({"code": "STALE_REVISION", "message": "Activity revision is stale."})
    );
    let tap = ActionEvent {
        action_id: "track".into(),
        activity_id: "id".into(),
        activity_key: None,
    };
    assert_eq!(
        serde_json::to_value(tap).unwrap(),
        json!({"actionId": "track", "activityId": "id"})
    );

    let live = LiveActivity::builder()
        .backend(MemoryBackend::new())
        .build()
        .unwrap();
    let record = block_on(
        live.start(StartInput::timer("tea:1", "Tea", "Brewing", 1_000, 61_000).without_fallback()),
    );
    assert_eq!(record.unwrap_err().code, "UNSUPPORTED_FEATURE");
    let record =
        block_on(live.start(StartInput::timer("tea:1", "Tea", "Brewing", 1_000, 61_000))).unwrap();
    let wire = serde_json::to_value(&record).unwrap();
    assert_eq!(wire["activityKey"], "tea:1");
    assert_eq!(wire["state"], "active");
    assert_eq!(wire["promotionState"], "notApplicable");
    assert_eq!(
        liveactivity::ERROR_CODES.len(),
        12,
        "a new code is a change of the contract"
    );
}

#[test]
fn req_c_002_invalid_requests_reach_no_backend() {
    // REQ-C-002
    let live = LiveActivity::builder()
        .backend(MemoryBackend::new())
        .build()
        .unwrap();
    let invalid = |input: StartInput| block_on(live.start(input)).unwrap_err().code;

    for input in [
        StartInput::progress("order 42", "Order 42", "Preparing", 0.1),
        StartInput::progress("order:42", " ", "Preparing", 0.1),
        StartInput::progress("order:42", "Order 42", "Preparing", 1.5),
        StartInput::timer("order:42", "Order 42", "Preparing", 2, 2),
        StartInput::progress("order:42", "Order 42", "Preparing", 0.1).stale(u64::MAX),
    ] {
        assert_eq!(invalid(input), "INVALID_INPUT");
    }
    assert!(block_on(live.list()).unwrap().is_empty());
    assert_eq!(
        block_on(live.update(UpdateInput::progress("", 1, "Order 42", "On the way", 0.6)))
            .unwrap_err()
            .code,
        "INVALID_INPUT"
    );
    assert_eq!(
        block_on(live.end(EndInput::new(" "))).unwrap_err().code,
        "INVALID_INPUT"
    );

    // The same check, for a request that arrives as JSON.
    assert!(validate_request("start", json!({"activityKey": "order:42"})).is_err());
    let start = serde_json::to_value(StartInput::progress(
        "order:42",
        "Order 42",
        "Preparing",
        0.1,
    ));
    let start = start.unwrap();
    assert!(validate_request("start", start.clone()).is_ok());
    let parsed: StartInput = liveactivity::parse_request("start", start).unwrap();
    assert_eq!(parsed.activity_key, "order:42");
    let layout = json!({"id": "id", "snapshot": {"revision": 1, "values": {}, "extra": true}});
    let refused =
        liveactivity::parse_request::<liveactivity::LayoutUpdateInput>("update_layout", layout);
    assert_eq!(refused.unwrap_err().code, "INVALID_INPUT");
}

#[test]
fn req_c_003_whatever_the_builder_is_not_told_is_the_default() {
    // REQ-C-003
    // Neither side named: the backend of the platform, which a host has none of, and taps dropped.
    let unnamed = LiveActivity::builder().build().unwrap();
    let shortest = liveactivity::connect_native(|_: liveactivity::Event| {}).unwrap();
    for live in [unnamed, shortest] {
        let start = StartInput::progress("order:42", "Order 42", "Preparing", 0.1);
        assert_eq!(
            block_on(live.start(start)).unwrap_err().code,
            "UNSUPPORTED_PLATFORM"
        );
        // The two queries answer: nothing is supported here, under the host's own name.
        let capabilities = block_on(live.capabilities()).unwrap();
        assert_eq!(capabilities.platform, std::env::consts::OS);
        assert_eq!(capabilities.max_payload_bytes, 0);
        assert_eq!(
            block_on(live.permissions()).unwrap().notifications,
            liveactivity::PermissionState::NotApplicable
        );
        assert_eq!(
            block_on(live.push_tokens()).unwrap(),
            liveactivity::PushTokens::default()
        );
        block_on(live.sync_events()).unwrap();
        // Everything else is refused, after the check of the request.
        let refused = [
            block_on(live.update(UpdateInput::progress("id", 1, "Order 42", "Late", 0.5))).err(),
            block_on(live.end(EndInput::new("id"))).err(),
            block_on(live.list()).err(),
            block_on(live.list_layout()).err(),
            block_on(live.request_permissions(liveactivity::PermissionKind::Notifications)).err(),
        ];
        for error in refused {
            assert_eq!(error.unwrap().code, "UNSUPPORTED_PLATFORM");
        }
    }

    // Only the backend named: a tap has nowhere to go and is dropped without a failure.
    let backend = std::sync::Arc::new(MemoryBackend::new());
    let live = LiveActivity::builder()
        .backend(backend.clone())
        .build()
        .unwrap();
    let order = block_on(live.start(StartInput::progress(
        "order:42",
        "Order 42",
        "Preparing",
        0.1,
    )))
    .unwrap();
    backend.tap(&order.id, "track").unwrap();
    // A backend that is connected to nothing yet keeps working, too.
    MemoryBackend::new().tap("unknown", "track").unwrap_err();
}

#[test]
fn req_n_001_the_crate_declares_only_its_engines_and_serde() {
    // REQ-N-001
    let manifest = include_str!("../Cargo.toml");
    let dependencies = manifest
        .split("[dependencies]")
        .nth(1)
        .and_then(|rest| rest.split("\n[").next())
        .expect("the manifest has dependencies");
    let declared: Vec<&str> = dependencies
        .lines()
        .filter_map(|line| line.split(['=', '.']).next())
        .map(str::trim)
        .filter(|name| !name.is_empty() && !name.starts_with('#'))
        .collect();

    assert_eq!(
        declared,
        [
            "activitykit",
            "liveupdate",
            "serde",
            "serde_json",
            "thiserror"
        ]
    );
}

#[test]
fn req_c_006_a_wrong_member_of_another_platform_reaches_no_backend() {
    // REQ-C-006
    // On a host, which is neither platform, a right request is refused by the backend and a wrong one before it.
    let live = LiveActivity::builder().build().unwrap();
    let start = |edit: fn(&mut liveactivity::LayoutStartInput)| {
        let mut input: liveactivity::LayoutStartInput = liveactivity::parse_request(
            "start_layout",
            json!({"activityKey":"ride:7","schemaVersion":2,"fallback":"notification",
                "layout":{"lockScreen":{"type":"text","bind":"eta"}},
                "snapshot":{"revision":0,"values":{"eta":"4 min"},"relevance":80,
                    "android":{"title":"Driver on the way","status":"4 min",
                        "presentation":{"style":{"kind":"standard"},"shortCriticalText":"4 min"}}}}),
        )
        .unwrap();
        edit(&mut input);
        block_on(live.start_layout(input)).unwrap_err().code
    };
    assert_eq!(start(|_| {}), "UNSUPPORTED_PLATFORM");
    assert_eq!(
        start(|input| input.snapshot.relevance = Some(f64::NAN)),
        "INVALID_INPUT"
    );
    assert_eq!(
        start(|input| {
            let content = input.snapshot.android.as_mut().unwrap();
            content.presentation.short_critical_text = Some("four minutes".into());
        }),
        "INVALID_INPUT"
    );
    let mut update = liveactivity::LayoutUpdateInput::new(
        "id",
        liveactivity::LayoutSnapshot::new(1, Default::default()),
    );
    update.alert = Some(liveactivity::Alert::new("", "Meet at the entrance"));
    assert_eq!(
        block_on(live.update_layout(update)).unwrap_err().code,
        "INVALID_INPUT"
    );
}

#[test]
fn req_i_003_a_wrong_member_of_the_ios_surface_reaches_no_backend() {
    // REQ-I-003
    // On a host, which is neither platform, a right request is refused by the backend and a wrong one before it.
    let live = LiveActivity::builder().build().unwrap();
    let start = |layout: serde_json::Value| {
        let request = json!({"activityKey":"ride:7","schemaVersion":2,"fallback":"notification",
            "layout":layout,"snapshot":{"revision":0,"values":{"eta":"4 min"}}});
        match liveactivity::parse_request::<liveactivity::LayoutStartInput>("start_layout", request)
        {
            Ok(input) => block_on(live.start_layout(input)).unwrap_err().code,
            Err(error) => error.code,
        }
    };
    let text = |members: serde_json::Value| {
        let mut node = json!({"type":"text","bind":"eta"});
        node.as_object_mut()
            .unwrap()
            .extend(members.as_object().unwrap().clone());
        json!({"lockScreen":node})
    };
    let link = |url: &str| json!({"lockScreen":{"type":"link","url":url,"child":{"type":"text","bind":"eta"}}});

    for right in [
        json!({"lockScreen":{"type":"text","bind":"eta"},"url":"myapp://rides/7"}),
        text(json!({"accessibility":{"label":"Arrival","value":{"bind":"eta"}}})),
        text(json!({"accessibility":{"hidden":true}})),
        text(json!({"transition":"pushLeading","if":"eta"})),
        text(json!({"privacy":"sensitive"})),
        link("https://example.com/rides/7"),
    ] {
        assert_eq!(start(right.clone()), "UNSUPPORTED_PLATFORM", "{right}");
    }
    for wrong in [
        json!({"lockScreen":{"type":"text","bind":"eta"},"url":"rides/7"}),
        text(json!({"accessibility":{}})),
        text(json!({"accessibility":{"hidden":true,"label":"Arrival"}})),
        text(json!({"accessibility":{"label":""}})),
        text(json!({"transition":"spin"})),
        text(json!({"privacy":"public"})),
        link("no scheme"),
        json!({"lockScreen":{"type":"button","actionId":"go",
            "child":{"type":"link","url":"myapp://x","child":{"type":"text","bind":"eta"}}}}),
    ] {
        assert_eq!(start(wrong.clone()), "INVALID_INPUT", "{wrong}");
    }
}
