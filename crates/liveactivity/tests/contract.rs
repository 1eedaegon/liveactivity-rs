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
    let tap = ActionEvent::new("track", "id");
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
fn req_n_002_clippy_denies_unwrap_and_expect_outside_tests() {
    // REQ-N-002
    let denied = "deny(clippy::unwrap_used, clippy::expect_used)";
    // The library: everywhere but in its tests.
    let library = include_str!("../src/lib.rs");
    assert!(library.contains(&format!("#![cfg_attr(not(test), {denied})]")));
    // An example is an application's code, and has no tests.
    for (name, example) in [
        ("usage", include_str!("../examples/usage.rs")),
        ("adapter", include_str!("../examples/adapter.rs")),
        ("features", include_str!("../examples/features.rs")),
    ] {
        assert!(example.contains(&format!("#![{denied}]")), "{name}");
    }
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

/// The code that a host, which is neither platform, answers a layout start with: a right request is refused by
/// the backend, a wrong one before it.
fn answer_to_a_start(members: serde_json::Value) -> String {
    let live = LiveActivity::builder().build().unwrap();
    let mut request = json!({"activityKey":"ride:7","schemaVersion":2,"fallback":"notification",
        "layout":{"lockScreen":{"type":"text","bind":"eta"}},
        "snapshot":{"revision":0,"values":{"eta":"4 min","muted":false,"count":3}}});
    request
        .as_object_mut()
        .unwrap()
        .extend(members.as_object().unwrap().clone());
    match liveactivity::parse_request::<liveactivity::LayoutStartInput>("start_layout", request) {
        Ok(input) => block_on(live.start_layout(input)).unwrap_err().code,
        Err(error) => error.code,
    }
}

#[test]
fn req_i_017_a_request_that_this_version_cannot_read_in_full_is_refused() {
    // REQ-I-017
    // activitykit's widget draws a later activity without what it does not know; a request of the application
    // stays strict.
    let layout = |lock_screen: serde_json::Value| json!({"layout":{"lockScreen":lock_screen}});
    let text = json!({"type":"text","bind":"eta"});
    assert_eq!(
        answer_to_a_start(layout(text.clone())),
        "UNSUPPORTED_PLATFORM"
    );
    for later in [
        // A node of a type that a later version adds.
        layout(json!({"type":"chart","bind":"eta"})),
        layout(json!({"type":"vstack","children":[text.clone(),{"type":"chart"}]})),
        // A member of a node, of the layout, of a style and of the snapshot that a later version adds.
        layout(json!({"type":"text","bind":"eta","later":1})),
        json!({"layout":{"lockScreen":text.clone(),"later":1}}),
        json!({"styles":{"card":{"later":1}}}),
        json!({"snapshot":{"revision":0,"values":{"eta":"4 min"},"later":1}}),
        // A value of a kind that a later version adds.
        json!({"snapshot":{"revision":0,"values":{"eta":{"kind":"later"}}}}),
    ] {
        assert_eq!(answer_to_a_start(later.clone()), "INVALID_INPUT", "{later}");
    }
}

#[test]
fn req_i_018_a_node_says_whether_it_is_drawn_in_standby() {
    // REQ-I-018
    use liveactivity::activitykit::layout::text;
    assert_eq!(
        serde_json::to_value(text("Gate 4").when_fullscreen()).unwrap(),
        json!({"type":"text","value":"Gate 4","ifFullscreen":true})
    );
    assert_eq!(
        serde_json::to_value(text("Gate 4").unless_fullscreen()).unwrap(),
        json!({"type":"text","value":"Gate 4","ifFullscreen":false})
    );
    // A Boolean on any node is taken on every platform; anything else is not.
    let layout = |member: serde_json::Value| {
        json!({"layout":{"lockScreen":{"type":"vstack","children":[
            {"type":"text","bind":"eta","ifFullscreen":member}]}}})
    };
    for right in [json!(true), json!(false)] {
        assert_eq!(
            answer_to_a_start(layout(right.clone())),
            "UNSUPPORTED_PLATFORM",
            "{right}"
        );
    }
    for wrong in [json!(null), json!("yes"), json!(1), json!({"bind":"eta"})] {
        assert_eq!(
            answer_to_a_start(layout(wrong.clone())),
            "INVALID_INPUT",
            "{wrong}"
        );
    }
}

#[test]
fn req_i_004_a_wrong_toggle_reaches_no_backend() {
    // REQ-I-004
    let toggle = |members: serde_json::Value| {
        let mut node = json!({"type":"toggle","actionId":"mute","bind":"muted",
            "child":{"type":"text","value":"Mute"}});
        node.as_object_mut()
            .unwrap()
            .extend(members.as_object().unwrap().clone());
        node
    };
    let on_the_lock_screen = |node: serde_json::Value| json!({"layout":{"lockScreen":node}});
    let controls = |count: usize| {
        let buttons: Vec<_> = (0..count)
            .map(|place| toggle(json!({"actionId": format!("mute:{place}")})))
            .collect();
        on_the_lock_screen(json!({"type":"hstack","children":buttons}))
    };
    for right in [on_the_lock_screen(toggle(json!({}))), controls(4)] {
        assert_eq!(
            answer_to_a_start(right.clone()),
            "UNSUPPORTED_PLATFORM",
            "{right}"
        );
    }
    for wrong in [
        // A value that is no flag, a value the snapshot does not have, and an id that is none.
        on_the_lock_screen(toggle(json!({"bind":"count"}))),
        on_the_lock_screen(toggle(json!({"bind":"missing"}))),
        on_the_lock_screen(toggle(json!({"actionId":"no id"}))),
        // Inside a button, a button inside it, and more than a region takes.
        on_the_lock_screen(json!({"type":"button","actionId":"go","child":toggle(json!({}))})),
        on_the_lock_screen(toggle(
            json!({"child":{"type":"button","actionId":"go","child":{"type":"text","value":"Go"}}}),
        )),
        controls(5),
        // Where no button may stand: the compact island.
        json!({"layout":{"lockScreen":{"type":"text","bind":"eta"},
            "island":{"compactLeading":toggle(json!({}))}}}),
    ] {
        assert_eq!(answer_to_a_start(wrong.clone()), "INVALID_INPUT", "{wrong}");
    }

    // The state a toggle was asked for is a member of the event; a button's tap is written as before.
    let tap = |on| ActionEvent {
        on,
        ..ActionEvent::new("mute", "id")
    };
    assert_eq!(
        serde_json::to_value(tap(Some(true))).unwrap(),
        json!({"actionId": "mute", "activityId": "id", "on": true})
    );
    assert_eq!(
        serde_json::to_value(tap(None)).unwrap(),
        json!({"actionId": "mute", "activityId": "id"})
    );
}

#[test]
fn req_i_006_a_wrong_schedule_reaches_no_backend() {
    // REQ-I-006
    let alert = json!({"title":"Your ride","body":"It starts now"});
    for right in [
        json!({"scheduled":{"at":1_900_000_000_000_u64,"alert":alert}}),
        json!({"ios":{"transient":true}}),
        json!({"ios":{"transient":false},"scheduled":{"at":0,"alert":alert}}),
    ] {
        assert_eq!(
            answer_to_a_start(right.clone()),
            "UNSUPPORTED_PLATFORM",
            "{right}"
        );
    }
    for wrong in [
        // A schedule needs its alert, and a time of the contract.
        json!({"scheduled":{"at":1_900_000_000_000_u64}}),
        json!({"scheduled":{"at":8_640_000_000_000_001_u64,"alert":alert}}),
        json!({"scheduled":{"at":-1,"alert":alert}}),
        json!({"scheduled":{"at":1_900_000_000_000_u64,"alert":{"title":"","body":"It starts now"}}}),
        json!({"scheduled":{"at":1_900_000_000_000_u64,"alert":alert,"repeat":true}}),
        json!({"scheduled":null}),
        // The iOS options are read strictly, as Android's are.
        json!({"ios":{}}),
        json!({"ios":{"transient":"yes"}}),
        json!({"ios":{"transient":true,"style":"transient"}}),
        json!({"ios":null}),
    ] {
        assert_eq!(answer_to_a_start(wrong.clone()), "INVALID_INPUT", "{wrong}");
    }
}

#[test]
fn req_i_007_a_wrong_sound_reaches_no_backend() {
    // REQ-I-007
    let scheduled = |sound: serde_json::Value| {
        json!({"scheduled":{"at":1_900_000_000_000_u64,
            "alert":{"title":"Your ride","body":"It starts now","sound":sound}}})
    };
    let update = |sound: serde_json::Value| {
        let request = json!({"id":"a","snapshot":{"revision":1,"values":{"eta":"2 min"}},
            "alert":{"title":"Gate changed","body":"Now gate 12","sound":sound}});
        validate_request("update_layout", request).map_err(|error| error.code)
    };
    for right in ["chime.aiff", "a", "Alarm_2-b.caf"] {
        assert_eq!(
            answer_to_a_start(scheduled(json!(right))),
            "UNSUPPORTED_PLATFORM",
            "{right}"
        );
        assert_eq!(update(json!(right)), Ok(()), "{right}");
    }
    let long = "a".repeat(65);
    for wrong in [
        json!(""),
        json!("../chime.aiff"),
        json!("sounds/chime.aiff"),
        json!(".chime"),
        json!("chime aiff"),
        json!(long),
        json!(7),
        serde_json::Value::Null,
    ] {
        assert_eq!(
            answer_to_a_start(scheduled(wrong.clone())),
            "INVALID_INPUT",
            "{wrong}"
        );
        assert_eq!(
            update(wrong.clone()),
            Err("INVALID_INPUT".to_owned()),
            "{wrong}"
        );
    }
}

#[test]
fn a_running_metric_of_a_content_is_written_as_androids_side_reads_it() {
    // liveupdate's Android side reads `startedAt` and `endsAt`. liveupdate up to 0.3.0 wrote the Rust names, so a
    // content with a stopwatch or a countdown could not be posted.
    use liveactivity::{
        AndroidContent, AndroidPresentation, LayoutSnapshot, LayoutUpdateInput, Metric,
        MetricStyle, MetricValue, PresentationStyle,
    };
    let metric = |label: &str, value: MetricValue| Metric {
        label: label.into(),
        value,
        semantic_style: None,
    };
    let style = PresentationStyle::Metric(MetricStyle {
        metrics: vec![
            metric("Lap", MetricValue::Stopwatch { started_at: 1_000 }),
            metric("Left", MetricValue::Countdown { ends_at: 9_000 }),
        ],
        critical_metric: None,
    });
    let mut snapshot = LayoutSnapshot::new(1, Default::default());
    snapshot.android = Some(AndroidContent {
        title: "Run".into(),
        status: "Lap 3".into(),
        presentation: AndroidPresentation::new(style),
    });
    let written = serde_json::to_value(LayoutUpdateInput::new("a", snapshot)).unwrap();
    let text = written.to_string();
    assert!(
        text.contains(r#""startedAt":1000"#) && text.contains(r#""endsAt":9000"#),
        "{text}"
    );
    // The request is read back as it is written, and in no other spelling.
    assert_eq!(validate_request("update_layout", written.clone()), Ok(()));
    let renamed: serde_json::Value =
        serde_json::from_str(&text.replace("startedAt", "started_at")).unwrap();
    assert_eq!(
        validate_request("update_layout", renamed).unwrap_err().code,
        "INVALID_INPUT"
    );
}

#[test]
fn req_a_024_a_wrong_ongoing_activity_member_reaches_no_backend() {
    // REQ-A-024
    let option = |value: serde_json::Value| json!({"android":{"visibility":"private","wearOngoingActivity":value}});
    let content = |value: serde_json::Value| {
        json!({"snapshot":{"revision":0,"values":{"eta":"4 min","muted":false,"count":3},
            "android":{"title":"Ride","status":"On the way",
                "presentation":{"style":{"kind":"standard"},"wearOngoingActivity":value}}}})
    };
    for right in [
        option(json!(false)),
        option(json!(true)),
        content(json!(false)),
        content(json!(true)),
    ] {
        assert_eq!(
            answer_to_a_start(right.clone()),
            "UNSUPPORTED_PLATFORM",
            "{right}"
        );
    }
    // The member where it does not belong: beside the presentation, in its style and in one of its actions. A
    // presentation may write it as `true`, its default, and nothing else may.
    let misplaced = |content: serde_json::Value| json!({"snapshot":{"revision":0,"values":{"eta":"4 min","muted":false,"count":3},"android":content}});
    let standard = json!({"style":{"kind":"standard"}});
    for wrong in [
        option(json!("no")),
        option(json!(0)),
        option(serde_json::Value::Null),
        content(json!("no")),
        content(json!(0)),
        content(serde_json::Value::Null),
        misplaced(
            json!({"title":"Ride","status":"On the way","presentation":standard,
            "wearOngoingActivity":true}),
        ),
        misplaced(json!({"title":"Ride","status":"On the way",
            "presentation":{"style":{"kind":"standard","wearOngoingActivity":true}}})),
        misplaced(json!({"title":"Ride","status":"On the way",
            "presentation":{"style":{"kind":"bigText","value":{"text":"Long","wearOngoingActivity":true}}}})),
        misplaced(json!({"title":"Ride","status":"On the way",
            "presentation":{"style":{"kind":"standard"},
                "actions":[{"id":"call","title":"Call","wearOngoingActivity":true}]}})),
        json!({"wearOngoingActivity":true}),
    ] {
        assert_eq!(answer_to_a_start(wrong.clone()), "INVALID_INPUT", "{wrong}");
    }
}

#[test]
fn req_e_009_a_tap_is_written_with_what_is_known_of_it() {
    // REQ-E-009
    // Nothing known of the tap: the member is left out, and the event reads as it was written before.
    let plain = ActionEvent::new("pet", "id");
    assert_eq!(
        serde_json::to_value(&plain).unwrap(),
        json!({"actionId": "pet", "activityId": "id"})
    );
    let late = ActionEvent {
        activity_key: Some("mochi:buttons".into()),
        tap: liveactivity::Tap {
            id: Some("tap-7".into()),
            at: Some(1_800_000_000_123),
            revision: Some(3),
            late: true,
        },
        ..ActionEvent::new("pet", "id")
    };
    let written = json!({"actionId": "pet", "activityId": "id", "activityKey": "mochi:buttons",
        "tap": {"id": "tap-7", "at": 1_800_000_000_123_u64, "revision": 3, "late": true}});
    assert_eq!(serde_json::to_value(&late).unwrap(), written);
    assert_eq!(
        serde_json::from_value::<ActionEvent>(written).unwrap(),
        late
    );
    // Each member is left out when it is not known; a tap that is not late says nothing of it.
    let stored = ActionEvent {
        tap: liveactivity::Tap {
            id: Some("42".into()),
            at: Some(1_000),
            ..liveactivity::Tap::default()
        },
        ..ActionEvent::new("pet", "id")
    };
    assert_eq!(
        serde_json::to_value(&stored).unwrap()["tap"],
        json!({"id": "42", "at": 1_000})
    );
}

#[test]
fn req_a_029_a_target_is_checked_on_every_platform() {
    // REQ-A-029
    let start = |android: serde_json::Value| {
        json!({"activityKey": "rec:1", "schemaVersion": 2, "fallback": "notification",
            "layout": {"lockScreen": {"type": "vstack", "children": [
                {"type": "text", "value": "Recorder"},
                {"type": "button", "actionId": "record", "child": {"type": "text", "value": "Record"}},
                {"type": "link", "url": "myapp://open", "child": {"type": "text", "value": "Open"}},
            ]}},
            "snapshot": {"revision": 0, "values": {}}, "android": android})
    };
    let target =
        json!({"kind": "foregroundService", "component": "dev.example.app.RecorderService"});
    let verdict = |android: serde_json::Value| {
        validate_request("start_layout", start(android)).map_err(|error| error.code)
    };
    assert_eq!(
        verdict(json!({"visibility": "public", "targets": {"record": target}})),
        Ok(())
    );
    for wrong in [
        // A key that names no button or toggle of the lock screen tree.
        json!({"visibility": "public", "targets": {"missing": target}}),
        json!({"visibility": "public", "targets": {"link:1": target}}),
        // A target liveupdate does not take.
        json!({"visibility": "public", "targets": {"record": {"kind": "foregroundService", "component": "Recorder"}}}),
        json!({"visibility": "public", "targets": {"record": {"kind": "activity", "component": "dev.example.A"}}}),
        json!({"visibility": "public", "targets": {"record": {"kind": "service", "component": "dev.example.S",
            "action": "has space"}}}),
        json!({"visibility": "public", "targets": {"record": {"kind": "service", "component": "dev.example.S",
            "extra": 1}}}),
        json!({"visibility": "public", "targets": null}),
        json!({"visibility": "public", "targets": ["record"]}),
    ] {
        assert_eq!(
            verdict(wrong.clone()),
            Err("INVALID_INPUT".to_owned()),
            "{wrong}"
        );
    }
    // The options built in Rust write the same JSON.
    let options = liveactivity::AndroidOptions::new(liveactivity::AndroidVisibility::Public)
        .with_target(
            "record",
            liveactivity::ActionTarget::new(
                liveactivity::TargetKind::ForegroundService,
                "dev.example.app.RecorderService",
            ),
        );
    assert_eq!(
        serde_json::to_value(options).unwrap(),
        json!({"visibility": "public", "targets": {"record": target}})
    );
}
