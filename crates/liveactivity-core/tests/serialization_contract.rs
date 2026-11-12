//! Serialized-shape regression tests for contract version 1.

use liveactivity_core::{
    DismissalPolicy, EndRequest, Error, ErrorCode, LiveActivityEvent, StartRequest, UpdateRequest,
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
            "content": {"progress": 0.25}
        })
    );

    let update = UpdateRequest::new("activity-1", 7, json!({"progress": 0.5}));
    assert_eq!(
        serde_json::to_value(update).unwrap(),
        json!({
            "contractVersion": 1,
            "id": "activity-1",
            "revision": 7,
            "content": {"progress": 0.5}
        })
    );

    let end = EndRequest::new(
        "activity-1",
        8,
        None,
        DismissalPolicy::After { at: 1_800_000_000 },
    );
    assert_eq!(
        serde_json::to_value(end).unwrap(),
        json!({
            "contractVersion": 1,
            "id": "activity-1",
            "revision": 8,
            "content": null,
            "dismissal": {"type": "after", "at": 1_800_000_000_u64}
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
        serde_json::to_value(Error::new(ErrorCode::Unsupported, "not available")).unwrap(),
        json!({
            "contractVersion": 1,
            "code": "unsupported",
            "message": "not available"
        })
    );
}
