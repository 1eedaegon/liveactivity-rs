use super::{
    fake::{self, FakeKit},
    *,
};
use crate::{
    block_on,
    layout::{LayoutPush, Value},
};
use activitykit::{Error as Kit, LimitScope};
use serde_json::{Value as Json, json};

fn apple() -> (Apple<FakeKit>, FakeKit) {
    let kit = FakeKit::default();
    (Apple::new(kit.clone()), kit)
}

fn start_json(key: &str, tone: &str) -> Json {
    json!({"activityKey":key,"schemaVersion":2,"fallback":"notification",
        "layout":{"lockScreen":{"type":"text","bind":"status","style":"s"}},
        "styles":{"s":{"color":{"bind":"tone"}}},
        "snapshot":{"revision":5,"values":{"status":"On time","tone":tone},"staleAt":1000,"timestamp":1_790_000_000_000u64},
        "android":{"visibility":"public"}})
}

fn start(key: &str, tone: &str) -> LayoutStartInput {
    serde_json::from_value(start_json(key, tone)).unwrap()
}

fn snapshot(revision: u64, tone: &str) -> LayoutSnapshot {
    serde_json::from_value(json!({"revision":revision,"values":{"status":"Late","tone":tone}}))
        .unwrap()
}

fn update(id: &str, revision: u64, tone: &str) -> LayoutUpdateInput {
    LayoutUpdateInput::new(id, snapshot(revision, tone))
}

fn end(id: &str, policy: EndPolicy, snapshot: Option<LayoutSnapshot>) -> LayoutEndInput {
    LayoutEndInput {
        id: id.into(),
        policy,
        snapshot,
    }
}

fn progress(key: &str, revision: u64, progress: f64) -> StartInput {
    serde_json::from_value(
        json!({"activityKey":key,"schemaVersion":1,"template":"progress","fallback":"none",
        "snapshot":{"revision":revision,"title":"Job","status":"Running","progress":progress}}),
    )
    .unwrap()
}

fn template_update(id: &str, revision: u64, progress: f64) -> UpdateInput {
    serde_json::from_value(json!({"id":id,"snapshot":{"revision":revision,"title":"Job","status":"Running","progress":progress}}))
        .unwrap()
}

fn code<T: std::fmt::Debug>(result: Result<T>) -> String {
    result.unwrap_err().code
}
#[test]
fn a_layout_start_goes_to_activitykit_and_answers_a_v2_record() {
    let (apple, kit) = apple();
    let record = block_on(apple.start_layout(start("demo:1", "green"))).unwrap();
    assert_eq!(
        serde_json::to_value(&record).unwrap(),
        json!({"id":"la-ios-layout-1","activityKey":"demo:1","schemaVersion":2,
            "snapshot":{"revision":5,"values":{"status":"On time","tone":"green"},"staleAt":1000,"timestamp":1_790_000_000_000u64},
            "backend":"activityKit","state":"active","visibility":"unknown","degradedFeatures":[],
            "promotionRequested":false,"promotionState":"notApplicable"})
    );
    let book = kit.book();
    let [sent] = book.starts.as_slice() else {
        panic!("one start")
    };
    assert_eq!(
        (
            sent.key.as_str(),
            sent.revision,
            sent.stale_at,
            sent.timestamp
        ),
        ("demo:1", 5, Some(1000), Some(1_790_000_000_000))
    );
    assert_eq!(sent.values.get("tone"), Some(&Value::Text("green".into())));
    let layout = serde_json::to_value(&sent.layout.lock_screen).unwrap();
    assert_eq!(layout, json!({"type":"text","bind":"status","style":"s"}));
    assert_eq!(sent.layout.styles.len(), 1);
    drop(book);
    for (state, wire) in [
        (State::Pending, "pending"),
        (State::Stale, "stale"),
        (State::Ended, "ended"),
        (State::Dismissed, "dismissed"),
        (State::Unknown, "unknown"),
    ] {
        kit.set_state("la-ios-layout-1", state);
        let listed = block_on(apple.list_layout()).unwrap();
        assert_eq!(serde_json::to_value(listed[0].state).unwrap(), wire);
    }
    // What activitykit reports the system could not show reaches the record.
    kit.degrade("la-ios-layout-1", &["alertSound", "relevance"]);
    let listed = block_on(apple.list_layout()).unwrap();
    assert_eq!(listed[0].degraded_features, ["alertSound", "relevance"]);
}
#[test]
fn the_same_start_returns_the_running_activity_also_after_a_relaunch() {
    let (apple, kit) = apple();
    let first = block_on(apple.start_layout(start("demo:1", "green"))).unwrap();
    // The same start made later carries another timestamp, which activitykit does not compare.
    let mut later = start("demo:1", "green");
    later.snapshot.timestamp = Some(1_790_000_000_500);
    assert_eq!(
        block_on(apple.start_layout(later.clone())).unwrap().id,
        first.id
    );
    let relaunched = Apple::new(kit.relaunched());
    assert_eq!(
        block_on(relaunched.start_layout(later)).unwrap().id,
        first.id
    );
    assert_eq!(
        code(block_on(apple.start_layout(start("demo:1", "red")))),
        "CONFLICT"
    );
    block_on(apple.end_layout(end(&first.id, EndPolicy::Immediate {}, None))).unwrap();
    let fresh = block_on(apple.start_layout(start("demo:1", "green"))).unwrap();
    assert_ne!(fresh.id, first.id);
    assert_eq!(kit.book().starts.len(), 2);
}
#[test]
fn an_update_goes_out_at_the_requests_revision_under_activitykits_rules() {
    let (apple, kit) = apple();
    let id = block_on(apple.start_layout(start("demo:1", "green")))
        .unwrap()
        .id;
    let mut timed = update(&id, 7, "red");
    timed.snapshot.timestamp = Some(1_790_000_000_900);
    timed.snapshot.stale_at = Some(2000);
    let record = block_on(apple.update_layout(timed.clone())).unwrap();
    assert_eq!(
        (record.snapshot.revision, record.snapshot.timestamp),
        (7, Some(1_790_000_000_900))
    );
    assert_eq!(record.snapshot.values["tone"], Value::Text("red".into()));
    assert_eq!(record.snapshot.stale_at, Some(2000));
    assert_eq!(
        kit.book().updates[0],
        UpdateSpec {
            values: timed.snapshot.values.clone().into(),
            revision: 7,
            stale_at: Some(2000),
            timestamp: Some(1_790_000_000_900),
            relevance: None,
            alert: None,
        }
    );
    assert_eq!(
        block_on(apple.update_layout(timed))
            .unwrap()
            .snapshot
            .revision,
        7
    );
    assert_eq!(
        code(block_on(apple.update_layout(update(&id, 7, "green")))),
        "CONFLICT"
    );
    assert_eq!(
        code(block_on(apple.update_layout(update(&id, 6, "green")))),
        "STALE_REVISION"
    );
    let next = block_on(apple.update_layout(update(&id, 8, "green"))).unwrap();
    assert_eq!(
        (next.snapshot.revision, next.snapshot.timestamp),
        (8, Some(2_000))
    );
    assert_eq!(kit.book().updates[2].timestamp, None);
}
#[test]
fn an_unknown_or_ended_activity_is_not_found_and_a_relaunch_finds_it_by_its_id() {
    let (apple, kit) = apple();
    assert_eq!(
        code(block_on(apple.update_layout(update(
            "la-ios-layout-9",
            1,
            "red"
        )))),
        "NOT_FOUND"
    );
    assert_eq!(
        code(block_on(apple.end_layout(end(
            "missing",
            EndPolicy::Default {},
            None
        )))),
        "NOT_FOUND"
    );
    let id = block_on(apple.start_layout(start("demo:1", "green")))
        .unwrap()
        .id;
    block_on(apple.update_layout(update(&id, 6, "red"))).unwrap();
    let later = Apple::new(kit.relaunched());
    let listed = block_on(later.list_layout()).unwrap();
    assert_eq!((listed.len(), listed[0].snapshot.revision), (1, 6));
    assert_eq!(
        block_on(later.update_layout(update(&id, 7, "green")))
            .unwrap()
            .snapshot
            .revision,
        7
    );
    kit.set_state(&id, State::Ended);
    assert_eq!(
        code(block_on(apple.update_layout(update(&id, 8, "red")))),
        "NOT_FOUND"
    );
    assert!(kit.calls().contains(&format!("activity {id}")));
}
#[test]
fn an_end_sends_its_policy_and_its_final_content_at_its_revision_and_ending_again_answers() {
    let (apple, kit) = apple();
    let id = block_on(apple.start_layout(start("demo:1", "green")))
        .unwrap()
        .id;
    let mut last = snapshot(9, "gray");
    last.timestamp = Some(5);
    let record = block_on(apple.end_layout(end(
        &id,
        EndPolicy::After {
            at: 8_640_000_000_000_000,
        },
        Some(last),
    )))
    .unwrap();
    assert_eq!(
        (record.state, record.snapshot.revision),
        (ActivityState::Ended, 9)
    );
    assert_eq!(record.snapshot.values["tone"], Value::Text("gray".into()));
    let again = block_on(apple.end_layout(end(&id, EndPolicy::Immediate {}, None))).unwrap();
    assert_eq!(again.state, ActivityState::Ended);
    let book = kit.book();
    assert_eq!(
        book.ends[0].policy,
        EndPolicy::After {
            at: 8_640_000_000_000_000
        }
    );
    assert_eq!(
        (book.ends[0].timestamp, book.ends[0].revision),
        (Some(5), Some(9))
    );
    assert_eq!(
        book.ends[1],
        EndSpec {
            policy: EndPolicy::Immediate {},
            values: None,
            revision: None,
            stale_at: None,
            timestamp: None,
            relevance: None,
        }
    );
    drop(book);
    let id = block_on(apple.start_layout(start("demo:2", "green")))
        .unwrap()
        .id;
    block_on(apple.update_layout(update(&id, 6, "red"))).unwrap();
    let stale = Some(snapshot(5, "gray"));
    assert_eq!(
        code(block_on(apple.end_layout(end(
            &id,
            EndPolicy::Default {},
            stale
        )))),
        "STALE_REVISION"
    );
    let other = Some(snapshot(6, "gray"));
    assert_eq!(
        code(block_on(apple.end_layout(end(
            &id,
            EndPolicy::Default {},
            other
        )))),
        "CONFLICT"
    );
    let kept = block_on(apple.end_layout(end(&id, EndPolicy::Default {}, None))).unwrap();
    assert_eq!(
        (kept.snapshot.revision, kept.snapshot.values["tone"].clone()),
        (7, Value::Text("red".into()))
    );
}
#[test]
fn an_activity_that_ended_at_once_answers_a_second_end_after_the_system_drops_it() {
    let (apple, kit) = apple();
    let ended = |id: &str| end(id, EndPolicy::Immediate {}, None);
    let id = block_on(apple.start_layout(start("demo:1", "green")))
        .unwrap()
        .id;
    block_on(apple.end_layout(ended(&id))).unwrap();
    assert!(!kit.listed(&id));
    let again = block_on(apple.end_layout(ended(&id))).unwrap();
    assert_eq!(
        (again.id.as_str(), again.state, again.snapshot.revision),
        (id.as_str(), ActivityState::Ended, 6)
    );
    assert_eq!(
        code(block_on(apple.update_layout(update(&id, 5, "red")))),
        "NOT_FOUND"
    );
    // A later launch keeps no handle of it, and activitykit 0.1 finds an id only through one or the system's list.
    let relaunched = Apple::new(kit.relaunched());
    assert_eq!(
        code(block_on(relaunched.end_layout(ended(&id)))),
        "NOT_FOUND"
    );
}
#[test]
fn the_last_64_activities_that_ended_answer_again_each_counted_once() {
    let (apple, kit) = apple();
    let ended = |id: &str| end(id, EndPolicy::Immediate {}, None);
    let started = |key: &str| {
        block_on(apple.start_layout(start(key, "green")))
            .unwrap()
            .id
    };
    let again = |id: &str| match block_on(apple.end_layout(ended(id))) {
        Ok(_) => "ok".to_owned(),
        Err(error) => error.code,
    };
    let (first, second) = (started("demo:first"), started("demo:second"));
    block_on(apple.end_layout(ended(&first))).unwrap();
    block_on(apple.end_layout(ended(&second))).unwrap();
    // Ending the first again keeps its place: it counts once, from its first end.
    for _ in 0..3 {
        assert_eq!(again(&first), "ok");
    }
    for n in 0..62 {
        let other = started(&format!("demo:n{n}"));
        block_on(apple.end_layout(ended(&other))).unwrap();
    }
    // 64 activities ended: both answer.
    assert_eq!((again(&first), again(&second)), ("ok".into(), "ok".into()));
    // One more lets the oldest go, which the system no longer lists.
    let last = started("demo:last");
    block_on(apple.end_layout(ended(&last))).unwrap();
    assert!(!kit.listed(&first));
    assert_eq!(
        (again(&first), again(&second)),
        ("NOT_FOUND".into(), "ok".into())
    );
}
#[test]
fn a_broken_request_never_reaches_activitykit_and_activitykit_errors_keep_their_codes() {
    let (apple, kit) = apple();
    let mut broken = start("demo:1", "green");
    broken.layout.lock_screen.style = Some("missing".into());
    assert_eq!(block_on(apple.start_layout(broken)), Err(Error::invalid()));
    let mut heavy = start("demo:1", "green");
    heavy.snapshot.values = (0..4)
        .map(|key| (format!("k{key}"), Value::Text("x".repeat(1000))))
        .collect();
    heavy.layout.lock_screen = serde_json::from_value(json!({"type":"text","value":"a"})).unwrap();
    heavy.styles = None;
    assert_eq!(
        block_on(apple.start_layout(heavy)),
        Err(Error::payload_too_large())
    );
    let mut late = update("la-ios-layout-1", 1, "red");
    late.snapshot.timestamp = Some(8_640_000_000_000_001);
    assert_eq!(block_on(apple.update_layout(late)), Err(Error::invalid()));
    assert!(kit.calls().is_empty());
    for error in [
        Kit::NotForeground("ActivityKit refused the start: visibility".into()),
        Kit::PermissionDenied("Live Activities are turned off".into()),
        Kit::SystemLimit {
            scope: LimitScope::Device,
        },
        Kit::Unsupported,
        Kit::ConfigurationRequired(
            "Embed a widget extension built from the layout template.".into(),
        ),
    ] {
        kit.fail_next(error.clone());
        assert_eq!(
            block_on(apple.start_layout(start("demo:3", "green"))),
            Err(Error::from(error))
        );
    }
    kit.fail_next(Kit::Native("ActivityKit failed".into()));
    assert_eq!(
        code(block_on(apple.update_layout(update(
            "la-ios-layout-1",
            1,
            "red"
        )))),
        "NATIVE_ERROR"
    );
}
#[test]
fn capabilities_and_permissions_follow_activitykits_authorization() {
    let (apple, kit) = apple();
    // Every feature, as a caller reads it: each follows the authorization, and `actions` also the buttons.
    let features = |backend: &str, all: &str, actions: &str| {
        json!({"platform":"ios","backend":backend,"start":all,"update":all,"end":all,"progress":all,"timer":all,
            "actions":actions,"stateObservation":all,"silentUpdate":all,"nativeLiveActivity":all,
            "maxPayloadBytes":4096,"layout":all,"push":all})
    };
    let reported = |apple: &Apple<FakeKit>| serde_json::to_value(apple.capabilities()).unwrap();
    assert_eq!(
        reported(&apple),
        features("activityKit", "supported", "supported")
    );
    assert_eq!(
        apple.permissions(false).live_activities,
        PermissionState::Granted
    );
    kit.book().authorization = Some(Authorization {
        supported: true,
        enabled: false,
        buttons: false,
        frequent_pushes: false,
    });
    assert_eq!(
        reported(&apple),
        features("activityKit", "supported", "unsupported")
    );
    assert_eq!(
        apple.permissions(false).live_activities,
        PermissionState::Denied
    );
    let asked = apple
        .request_permissions(PermissionKind::LiveActivities)
        .unwrap();
    assert_eq!(
        (asked.live_activities, asked.notifications, asked.promotion),
        (
            PermissionState::SettingsRequired,
            PermissionState::NotApplicable,
            PermissionState::NotApplicable
        )
    );
    assert_eq!(
        code(apple.request_permissions(PermissionKind::Notifications)),
        "UNSUPPORTED_FEATURE"
    );
    assert_eq!(
        code(apple.request_permissions(PermissionKind::Promotion)),
        "UNSUPPORTED_FEATURE"
    );
    kit.book().authorization = Some(Authorization {
        supported: false,
        enabled: false,
        buttons: false,
        frequent_pushes: false,
    });
    assert_eq!(
        reported(&apple),
        features("unsupported", "unsupported", "unsupported")
    );
    assert_eq!(
        apple.permissions(true).live_activities,
        PermissionState::NotApplicable
    );
    kit.fail_next(Kit::Unsupported);
    assert_eq!(
        code(block_on(apple.start_layout(start("demo:1", "green")))),
        "UNSUPPORTED_PLATFORM"
    );
}
#[test]
fn an_image_is_checked_and_then_written_by_activitykit() {
    let (apple, kit) = apple();
    let png = crate::image::tests::png(2, 2, 0);
    assert_eq!(apple.write_image("badge.png", &png), Ok(()));
    assert_eq!(
        code(apple.write_image("../badge.png", &png)),
        "INVALID_INPUT"
    );
    assert_eq!(
        code(apple.write_image("big.png", &crate::image::tests::png(1225, 2, 0))),
        "PAYLOAD_TOO_LARGE"
    );
    kit.fail_next(Kit::ConfigurationRequired(
        "The widget extension names no App Group under ActivityKitAppGroup.".into(),
    ));
    assert_eq!(
        code(apple.write_image("badge.png", &png)),
        "CONFIGURATION_REQUIRED"
    );
    assert_eq!(kit.calls(), ["image badge.png", "image badge.png"]);
}
#[test]
fn a_template_activity_starts_updates_ends_and_lists_as_contract_v1() {
    let (apple, kit) = apple();
    let record = block_on(apple.start_template(progress("job:1", 0, 0.3))).unwrap();
    assert_eq!(
        serde_json::to_value(&record).unwrap(),
        json!({"id":"la-ios-layout-1","activityKey":"job:1","schemaVersion":1,"template":"progress",
            "snapshot":{"revision":0,"title":"Job","status":"Running","progress":0.3},
            "backend":"activityKit","state":"active","visibility":"unknown","degradedFeatures":[],
            "promotionRequested":false,"promotionState":"notApplicable"})
    );
    assert_eq!(
        block_on(apple.start_template(progress("job:1", 0, 0.3))).unwrap(),
        record
    );
    let id = record.id;
    let updated = block_on(apple.update_template(template_update(&id, 3, 0.5))).unwrap();
    assert_eq!(
        (updated.snapshot.revision, updated.snapshot.progress),
        (3, Some(0.5))
    );
    assert_eq!(
        block_on(apple.update_template(template_update(&id, 3, 0.5))).unwrap(),
        updated
    );
    assert_eq!(
        code(block_on(
            apple.update_template(template_update(&id, 3, 0.9))
        )),
        "CONFLICT"
    );
    assert_eq!(
        code(block_on(
            apple.update_template(template_update(&id, 0, 0.1))
        )),
        "STALE_REVISION"
    );
    let timer = serde_json::from_value(json!({"id":id,"snapshot":{"revision":4,"title":"Job","status":"Running","timer":{"startedAt":1,"endsAt":2}}})).unwrap();
    assert_eq!(
        block_on(apple.update_template(timer)),
        Err(Error::invalid())
    );
    assert_eq!(
        block_on(apple.list_templates()).unwrap(),
        std::slice::from_ref(&updated)
    );
    let ending =
        serde_json::from_value::<EndInput>(json!({"id":id,"policy":{"kind":"immediate"}})).unwrap();
    let ended = block_on(apple.end_template(ending.clone())).unwrap();
    assert_eq!(
        (
            ended.state,
            ended.snapshot.progress,
            ended.snapshot.revision
        ),
        (ActivityState::Ended, Some(0.5), 4)
    );
    assert_eq!(
        block_on(apple.end_template(ending)).unwrap().state,
        ActivityState::Ended
    );
    assert_eq!(
        code(block_on(
            apple.update_template(template_update(&id, 5, 0.5))
        )),
        "NOT_FOUND"
    );
    assert_eq!(
        kit.book().starts[0].values.get(template::MARKER),
        Some(&Value::Text("progress".into()))
    );
}
#[test]
fn the_two_contract_versions_do_not_see_each_others_activities() {
    let (apple, kit) = apple();
    let layout = block_on(apple.start_layout(start("demo:1", "green"))).unwrap();
    let template = block_on(apple.start_template(progress("job:1", 0, 0.3))).unwrap();
    assert_eq!(
        block_on(apple.list_layout()).unwrap(),
        std::slice::from_ref(&layout)
    );
    assert_eq!(
        block_on(apple.list_templates()).unwrap(),
        std::slice::from_ref(&template)
    );
    assert_eq!(
        code(block_on(
            apple.update_template(template_update(&layout.id, 6, 0.5))
        )),
        "NOT_FOUND"
    );
    assert_eq!(
        code(block_on(apple.update_layout(update(
            &template.id,
            1,
            "red"
        )))),
        "NOT_FOUND"
    );
    let ending =
        serde_json::from_value(json!({"id":layout.id,"policy":{"kind":"immediate"}})).unwrap();
    assert_eq!(code(block_on(apple.end_template(ending))), "NOT_FOUND");
    assert_eq!(
        code(block_on(apple.end_layout(end(
            &template.id,
            EndPolicy::Immediate {},
            None
        )))),
        "NOT_FOUND"
    );
    // One key, one activity: a layout cannot start under a running template's key, nor a template under a layout's.
    assert_eq!(
        code(block_on(apple.start_layout(start("job:1", "green")))),
        "CONFLICT"
    );
    assert_eq!(
        code(block_on(apple.start_template(progress("demo:1", 0, 0.3)))),
        "CONFLICT"
    );
    kit.degrade(&template.id, &["dynamicIsland"]);
    assert_eq!(
        block_on(apple.list_templates()).unwrap()[0].degraded_features,
        ["dynamicIsland"]
    );
}
#[test]
fn a_start_with_a_push_reaches_the_kit_as_it_is() {
    let (apple, kit) = apple();
    let mut channel = start("demo:1", "green");
    channel.push = Some(LayoutPush::Channel("dGVzdA==".into()));
    let first = block_on(apple.start_layout(channel)).unwrap();
    let mut token = start("demo:2", "green");
    token.push = Some(LayoutPush::Token);
    block_on(apple.start_layout(token)).unwrap();
    block_on(apple.start_layout(start("demo:3", "green"))).unwrap();
    let pushes: Vec<_> = kit
        .book()
        .starts
        .iter()
        .map(|start| start.push.clone())
        .collect();
    assert_eq!(
        pushes,
        [
            Some(LayoutPush::Channel("dGVzdA==".into())),
            Some(LayoutPush::Token),
            None
        ]
    );
    // The push is no content: the same start again, asking for a token instead, is the running activity.
    let mut again = start("demo:1", "green");
    again.push = Some(LayoutPush::Token);
    assert_eq!(block_on(apple.start_layout(again)).unwrap().id, first.id);
    // A token is not in the record: ActivityKit makes it later and reports it as an event.
    assert_eq!(kit.book().starts.len(), 3);
}
#[test]
fn permissions_and_capabilities_report_pushes() {
    let (apple, kit) = apple();
    assert_eq!(apple.capabilities().push, FeatureSupport::Supported);
    assert_eq!(
        apple.permissions(false).frequent_pushes,
        PermissionState::Granted
    );
    kit.book().authorization = Some(Authorization {
        supported: true,
        enabled: true,
        buttons: true,
        frequent_pushes: false,
    });
    // A switch in Settings, which the app offers with NSSupportsLiveActivitiesFrequentUpdates: a request changes
    // nothing about it.
    assert_eq!(
        apple.permissions(false).frequent_pushes,
        PermissionState::Denied
    );
    assert_eq!(
        apple.permissions(true).frequent_pushes,
        PermissionState::Denied
    );
    kit.book().authorization = Some(Authorization {
        supported: false,
        enabled: false,
        buttons: false,
        frequent_pushes: false,
    });
    assert_eq!(apple.capabilities().push, FeatureSupport::Unsupported);
    assert_eq!(
        apple.permissions(false).frequent_pushes,
        PermissionState::NotApplicable
    );
}
#[test]
fn a_tap_names_the_action_the_activity_and_its_key() {
    let kit = FakeKit::default();
    assert_eq!(
        action_event(
            &kit.handle("la-ios-layout-1", "mochi:buttons"),
            "pet".into(),
            None
        ),
        ActionEvent {
            action_id: "pet".into(),
            activity_id: "la-ios-layout-1".into(),
            activity_key: Some("mochi:buttons".into()),
            on: None,
        }
    );
    // activitykit names no key once ActivityKit no longer lists the activity.
    assert_eq!(
        action_event(&kit.handle("gone", ""), "pet".into(), None).activity_key,
        None
    );
}

#[test]
fn req_c_007_android_members_never_reach_the_ios_engine() {
    // REQ-C-007
    let (apple, kit) = apple();
    let first = block_on(apple.start_layout(start("demo:1", "green"))).unwrap();
    // The same start with other Android options and an Android content is the same activity.
    let mut json = start_json("demo:1", "green");
    json["android"]["visibility"] = json!("secret");
    json["snapshot"]["android"] = json!({"title":"On time","status":"Gate 4",
        "presentation":{"style":{"kind":"standard"}}});
    let other: LayoutStartInput = crate::parse_request("start_layout", json).unwrap();
    assert!(other.snapshot.android.is_some());
    assert_eq!(
        block_on(apple.start_layout(other.clone())).unwrap().id,
        first.id
    );
    assert_eq!(kit.book().starts.len(), 1);
    // activitykit is not asked about them, and the record does not repeat what iOS does not hold.
    assert_eq!(other.wire().android, None);
    let record = block_on(apple.list_layout()).unwrap().remove(0);
    assert_eq!(record.snapshot.android, None);
}

#[test]
fn req_i_001_a_relevance_reaches_the_kit_and_the_record_reports_it() {
    // REQ-I-001
    let (apple, kit) = apple();
    let mut first = start("demo:1", "green");
    first.snapshot.relevance = Some(80.0);
    let record = block_on(apple.start_layout(first)).unwrap();
    assert_eq!(kit.book().starts[0].relevance, Some(80.0));
    assert_eq!(record.snapshot.relevance, Some(80.0));
    // An update without a score keeps it; one with another replaces it.
    let kept = block_on(apple.update_layout(update(&record.id, 6, "red"))).unwrap();
    assert_eq!(kept.snapshot.relevance, Some(80.0));
    let mut raised = update(&record.id, 7, "green");
    raised.snapshot.relevance = Some(95.5);
    let raised = block_on(apple.update_layout(raised)).unwrap();
    assert_eq!(raised.snapshot.relevance, Some(95.5));
    let sent: Vec<_> = kit
        .book()
        .updates
        .iter()
        .map(|update| update.relevance)
        .collect();
    assert_eq!(sent, [None, Some(95.5)]);
    // The final content has one too.
    let mut last = snapshot(8, "red");
    last.relevance = Some(10.0);
    let ended =
        block_on(apple.end_layout(end(&record.id, EndPolicy::Default {}, Some(last)))).unwrap();
    assert_eq!(kit.book().ends[0].relevance, Some(10.0));
    assert_eq!(ended.snapshot.relevance, Some(10.0));
    // No score is the system's 0, which a record does not repeat.
    let plain = block_on(apple.start_layout(start("demo:2", "green"))).unwrap();
    assert_eq!(plain.snapshot.relevance, None);
    // A score that is no number never reaches the kit.
    let mut endless = start("demo:3", "green");
    endless.snapshot.relevance = Some(f64::INFINITY);
    assert_eq!(block_on(apple.start_layout(endless)), Err(Error::invalid()));
}

#[test]
fn req_i_002_an_alert_reaches_the_kit_with_its_update() {
    // REQ-I-002
    let (apple, kit) = apple();
    let id = block_on(apple.start_layout(start("demo:1", "green")))
        .unwrap()
        .id;
    let alert = crate::Alert::new("Gate changed", "Now gate 12");
    let mut loud = update(&id, 6, "red");
    loud.alert = Some(alert.clone());
    block_on(apple.update_layout(loud)).unwrap();
    block_on(apple.update_layout(update(&id, 7, "green"))).unwrap();
    let alerts: Vec<_> = kit
        .book()
        .updates
        .iter()
        .map(|update| update.alert.clone())
        .collect();
    assert_eq!(alerts, [Some(alert), None]);
    // An alert without a title is no alert, and the update does not go out.
    let mut blank = update(&id, 8, "red");
    blank.alert = Some(crate::Alert::new(" ", "Now gate 12"));
    assert_eq!(block_on(apple.update_layout(blank)), Err(Error::invalid()));
    assert_eq!(kit.book().updates.len(), 2);
}

#[test]
fn req_i_003_the_ios_surface_of_a_layout_reaches_the_kit() {
    // REQ-I-003
    use crate::layout::{
        Accessibility, Bindable, Bound, ContentTransition, Element, ExpandedRegion, Privacy,
        Transition,
    };
    let request = json!({"activityKey":"ride:7","schemaVersion":2,"fallback":"notification",
        "layout":{
            "url":"myapp://rides/7",
            "lockScreen":{"type":"vstack","children":[
                {"type":"text","bind":"eta","style":"count","privacy":"sensitive",
                    "accessibility":{"label":"Arrival","value":{"bind":"eta"}}},
                {"type":"text","value":"Arrived","if":"arrived","transition":"moveTop"},
                {"type":"link","url":{"bind":"map"},"child":{"type":"text","value":"Map"}}]},
            "island":{
                "margins":{"expanded":8},
                "expanded":{"leading":{"type":"text","bind":"eta"},
                    "priority":{"leading":100},"belowIfTooWide":["leading"]}}},
        "styles":{"count":{"contentTransition":"numericText"}},
        "snapshot":{"revision":0,"values":{"eta":"4 min","arrived":false,"map":"myapp://rides/7/map"}}});
    let (apple, kit) = apple();
    let start = crate::parse_request::<LayoutStartInput>("start_layout", request).unwrap();
    block_on(apple.start_layout(start.clone())).unwrap();

    let layout = kit.book().starts[0].layout.clone();
    assert_eq!(
        layout.url,
        Some(Bindable::Literal("myapp://rides/7".to_owned()))
    );
    let Element::Vstack { children, .. } = &layout.lock_screen.element else {
        panic!("the lock screen is the stack that was sent");
    };
    assert_eq!(children[0].privacy, Some(Privacy::Sensitive));
    assert_eq!(
        children[0].accessibility,
        Some(
            Accessibility::new()
                .label("Arrival")
                .value(Bound::key("eta"))
        )
    );
    assert_eq!(children[1].transition, Some(Transition::MoveTop));
    assert!(
        matches!(&children[2].element, Element::Link { url, .. } if *url == Bindable::Bound(Bound::key("map")))
    );
    assert_eq!(
        layout.styles["count"].content_transition,
        Some(ContentTransition::NumericText)
    );
    let island = layout.island.as_ref().unwrap();
    assert!(island.margins.is_some());
    let expanded = island.expanded.as_ref().unwrap();
    assert_eq!(expanded.priority.as_ref().unwrap().leading, Some(100.0));
    assert_eq!(
        expanded.below_if_too_wide,
        Some(vec![ExpandedRegion::Leading])
    );

    // A URL that a value gives is checked with every update, by the rule the start is checked by.
    let wrong: LayoutSnapshot = serde_json::from_value(
        json!({"revision":1,"values":{"eta":"3 min","arrived":false,"map":"not a url"}}),
    )
    .unwrap();
    assert_eq!(
        start.validate_update(&LayoutUpdateInput::new("id", wrong)),
        Err(Error::invalid())
    );
}

/// A start whose lock screen is `lock_screen`, with the values `status`, `muted` and `tone`.
fn start_showing(lock_screen: Json, members: Json) -> LayoutStartInput {
    let mut request = json!({"activityKey":"ride:7","schemaVersion":2,"fallback":"notification",
        "layout":{"lockScreen":lock_screen},
        "snapshot":{"revision":0,"values":{"status":"On the way","muted":false}}});
    request
        .as_object_mut()
        .unwrap()
        .extend(members.as_object().unwrap().clone());
    crate::parse_request("start_layout", request).unwrap()
}

#[test]
fn req_i_004_a_toggle_reaches_the_kit_and_its_tap_says_what_was_asked() {
    // REQ-I-004
    use crate::layout::Element;
    let toggle = json!({"type":"toggle","actionId":"mute","bind":"muted",
        "child":{"type":"text","value":"Mute"}});
    let (apple, kit) = apple();
    let record = block_on(apple.start_layout(start_showing(toggle, json!({})))).unwrap();
    let layout = kit.book().starts[0].layout.clone();
    let Element::Toggle {
        action_id,
        bind,
        child,
    } = &layout.lock_screen.element
    else {
        panic!("a toggle")
    };
    assert_eq!((action_id.as_str(), bind.as_str()), ("mute", "muted"));
    assert!(matches!(&child.element, Element::Text { value: Some(label), .. } if label == "Mute"));

    // A tap says which state the person asked for; a button's says none.
    let handle = kit.handle(&record.id, "ride:7");
    let tap = |on| ActionEvent {
        action_id: "mute".into(),
        activity_id: record.id.clone(),
        activity_key: Some("ride:7".into()),
        on,
    };
    assert_eq!(
        action_event(&handle, "mute".into(), Some(true)),
        tap(Some(true))
    );
    assert_eq!(
        action_event(&handle, "mute".into(), Some(false)),
        tap(Some(false))
    );
    assert_eq!(action_event(&handle, "mute".into(), None), tap(None));
}

#[test]
fn req_i_005_a_node_that_follows_staleness_reaches_the_kit() {
    // REQ-I-005
    use crate::layout::Element;
    let lock_screen = json!({"type":"vstack","ifStale":false,"children":[
        {"type":"text","bind":"status","ifStale":false},
        {"type":"text","value":"No signal","ifStale":true,"unless":"muted"},
        {"type":"text","value":"Always"}]});
    let (apple, kit) = apple();
    block_on(apple.start_layout(start_showing(lock_screen, json!({})))).unwrap();
    let layout = kit.book().starts[0].layout.clone();
    let Element::Vstack { children, .. } = &layout.lock_screen.element else {
        panic!("a stack")
    };
    assert_eq!(layout.lock_screen.if_stale, Some(false));
    let conditions: Vec<_> = children.iter().map(|child| child.if_stale).collect();
    assert_eq!(conditions, [Some(false), Some(true), None]);
    // Beside another condition both are kept.
    assert_eq!(children[1].unless.as_deref(), Some("muted"));
}

#[test]
fn req_i_006_a_scheduled_and_a_transient_start_reach_the_kit() {
    // REQ-I-006
    let text = || json!({"type":"text","bind":"status"});
    let later = |key: &str, at: u64, transient: bool| {
        start_showing(
            text(),
            json!({"activityKey":key,
                "scheduled":{"at":at,"alert":{"title":"Your ride","body":"It starts now"}},
                "ios":{"transient":transient}}),
        )
    };
    let fresh = |id: &str| {
        LayoutUpdateInput::new(
            id,
            serde_json::from_value(json!({"revision":1,"values":{"status":"Soon","muted":false}}))
                .unwrap(),
        )
    };
    let (apple, kit) = apple();
    let ahead = fake::NOW + 600_000;
    let record = block_on(apple.start_layout(later("ride:7", ahead, true))).unwrap();
    {
        let book = kit.book();
        let sent = &book.starts[0];
        assert!(sent.transient);
        assert_eq!(
            sent.scheduled,
            Some(crate::ScheduledStart::new(
                ahead,
                crate::Alert::new("Your ride", "It starts now")
            ))
        );
    }
    // Until its time the activity waits: it is pending and takes no update.
    assert_eq!(record.state, ActivityState::Pending);
    assert_eq!(
        code(block_on(apple.update_layout(fresh(&record.id)))),
        "CONFLICT"
    );
    // When and how an activity begins is no part of what makes two starts the same activity.
    let again = block_on(apple.start_layout(later("ride:7", ahead + 900_000, false))).unwrap();
    assert_eq!(again.id, record.id);
    assert_eq!(kit.book().starts.len(), 1);
    // The system begins it: it is active, and takes updates.
    kit.set_state(&record.id, State::Active);
    let begun = block_on(apple.update_layout(fresh(&record.id))).unwrap();
    assert_eq!(begun.state, ActivityState::Active);

    // A pending activity can be ended, and a time that has passed starts an activity at once.
    let waiting = block_on(apple.start_layout(later("ride:8", ahead, false))).unwrap();
    let ended = block_on(apple.end_layout(end(&waiting.id, EndPolicy::Immediate {}, None)));
    assert_eq!(ended.unwrap().state, ActivityState::Ended);
    let passed = block_on(apple.start_layout(later("ride:9", fake::NOW - 1, false))).unwrap();
    assert_eq!(passed.state, ActivityState::Active);

    // A start without them asks for neither.
    block_on(apple.start_layout(start("demo:1", "green"))).unwrap();
    let book = kit.book();
    let plain = book.starts.last().unwrap();
    assert!(!plain.transient);
    assert_eq!(plain.scheduled, None);
}

#[test]
fn req_i_007_the_sound_of_an_alert_reaches_the_kit() {
    // REQ-I-007
    let (apple, kit) = apple();
    let id = block_on(apple.start_layout(start("demo:1", "green")))
        .unwrap()
        .id;
    let alert = crate::Alert::new("Gate changed", "Now gate 12").with_sound("chime.aiff");
    let mut loud = update(&id, 6, "red");
    loud.alert = Some(alert.clone());
    block_on(apple.update_layout(loud)).unwrap();
    assert_eq!(kit.book().updates[0].alert, Some(alert));
    // A sound that is no file name is no alert, and the update does not go out.
    let mut wrong = update(&id, 7, "red");
    wrong.alert =
        Some(crate::Alert::new("Gate changed", "Now gate 12").with_sound("../chime.aiff"));
    assert_eq!(block_on(apple.update_layout(wrong)), Err(Error::invalid()));
    assert_eq!(kit.book().updates.len(), 1);

    // The alert of a scheduled start keeps its sound too.
    let scheduled = start_showing(
        json!({"type":"text","bind":"status"}),
        json!({"scheduled":{"at":fake::NOW + 600_000,
            "alert":{"title":"Your ride","body":"It starts now","sound":"start.caf"}}}),
    );
    block_on(apple.start_layout(scheduled)).unwrap();
    let book = kit.book();
    let sound = book.starts[1]
        .scheduled
        .as_ref()
        .and_then(|scheduled| scheduled.alert.sound.as_deref());
    assert_eq!(sound, Some("start.caf"));
}
