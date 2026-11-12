use super::{fake::FakeKit, *};
use crate::layout::Value;
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
    LayoutUpdateInput {
        id: id.into(),
        snapshot: snapshot(revision, tone),
    }
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
            timestamp: None
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
    // Every feature, as JavaScript reads it: each follows the authorization, and `actions` also the buttons.
    let features = |backend: &str, all: &str, actions: &str| {
        json!({"platform":"ios","backend":backend,"start":all,"update":all,"end":all,"progress":all,"timer":all,
            "actions":actions,"stateObservation":all,"silentUpdate":all,"nativeLiveActivity":all,
            "maxPayloadBytes":4096,"layout":all})
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
fn a_tap_names_the_action_the_activity_and_its_key() {
    let kit = FakeKit::default();
    assert_eq!(
        action_event(
            &kit.handle("la-ios-layout-1", "mochi:buttons"),
            "pet".into()
        ),
        ActionEvent {
            action_id: "pet".into(),
            activity_id: "la-ios-layout-1".into(),
            activity_key: Some("mochi:buttons".into())
        }
    );
    // activitykit names no key once ActivityKit no longer lists the activity.
    assert_eq!(
        action_event(&kit.handle("gone", ""), "pet".into()).activity_key,
        None
    );
}

#[test]
fn block_on_drives_a_future_that_another_thread_completes() {
    let (sender, receiver) = activitykit::async_channel::bounded(1);
    let answer = thread::spawn(move || {
        thread::sleep(std::time::Duration::from_millis(20));
        sender.send_blocking(42).unwrap();
    });
    assert_eq!(block_on(async { receiver.recv().await.unwrap() }), 42);
    answer.join().unwrap();
}
