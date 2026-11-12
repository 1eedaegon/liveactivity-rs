//! The default backend of the host, reached through the facade only.
#![cfg(all(
    feature = "activitykit",
    not(any(target_os = "android", target_os = "ios"))
))]

use futures_executor::block_on;
use liveactivity::{
    ErrorCode, EventSink, LiveActivity, LiveActivityBackend, LiveActivityEvent, Result,
    StartRequest, connect_native,
};
use serde_json::json;

struct NoEvents;

impl EventSink for NoEvents {
    fn emit(&self, _event: LiveActivityEvent) -> Result<()> {
        Ok(())
    }
}

#[test]
fn req_d_001_every_target_has_a_default_backend() {
    // REQ-D-001
    let service = connect_native(NoEvents).unwrap();

    let capabilities = block_on(service.capabilities()).unwrap();
    assert!(!capabilities.start.is_supported() && !capabilities.actions.is_supported());
    let layout = json!({"lockScreen": {"type": "text", "value": "Recording"}});
    let content =
        json!({"fallback": "none", "layout": layout, "snapshot": {"revision": 0, "values": {}}});
    let error = block_on(service.start(StartRequest::new("rec", 2, content))).unwrap_err();
    assert_eq!(error.code, ErrorCode::UnsupportedPlatform);
    block_on(service.sync_events()).unwrap();
}

#[test]
fn req_d_002_another_backend_replaces_the_default_through_the_same_interface() {
    // REQ-D-002
    let default = connect_native(NoEvents).unwrap();
    let other: LiveActivity = LiveActivity::connect(NoEvents, |context| {
        liveactivity::ios::ActivityKitBackend::with_kit(
            context,
            liveactivity::activitykit::ActivityKit::new(),
        )
    })
    .unwrap();

    fn assert_backend<T: LiveActivityBackend>() {}
    assert_backend::<liveactivity::ios::ActivityKitBackend>();
    assert_eq!(
        block_on(default.capabilities()).unwrap(),
        block_on(other.capabilities()).unwrap()
    );
}
