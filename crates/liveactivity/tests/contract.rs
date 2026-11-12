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

    let live = LiveActivity::connect(
        |_: ActionEvent| {},
        |context| Ok(MemoryBackend::new(context)),
    )
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
    let live = LiveActivity::connect(
        |_: ActionEvent| {},
        |context| Ok(MemoryBackend::new(context)),
    )
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
