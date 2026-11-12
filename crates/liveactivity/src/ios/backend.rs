use super::{BACKEND_NAME, content, events};
use crate::{
    ActivityRecord, BackendContext, BackendFuture, Capabilities, EndRequest, Error, ErrorCode,
    LiveActivityBackend, PermissionKind, PermissionState, Permissions, Result, StartRequest,
    UpdateRequest,
};
use std::{
    collections::VecDeque,
    sync::{Mutex, PoisonError},
};

/// The size ActivityKit allows the content of an activity.
const MAX_CONTENT_BYTES: u64 = 4096;
/// How many activities the backend keeps a handle of.
const HANDLES_KEPT: usize = 64;
use activitykit::{Activity, ActivityKit};

/// `LiveActivityBackend` over the activitykit crate.
///
/// The Swift layer completes every call from the main thread, so a returned future must not be
/// blocked on there. On a device with Live Activities, one thread of the backend reads
/// activitykit's events and hands each to the framework sink: a start or a state change as
/// `stateChanged`, a tap as `action`. A tap holds its intent, and an app the system launched
/// for it, until the sink has returned.
pub struct ActivityKitBackend {
    kit: ActivityKit,
    /// The handles of the last [`HANDLES_KEPT`] activities a command touched, oldest first.
    /// activitykit finds an id through a handle alive in the process, else through the system's
    /// list, which an activity that ended at once soon leaves: a kept handle lets a repeated end
    /// reach activitykit's record of it, and a late update its refusal.
    handles: Mutex<VecDeque<Activity>>,
}

impl ActivityKitBackend {
    /// Creates the backend from the context supplied by `LiveActivity::connect`, on
    /// activitykit's default settings.
    ///
    /// # Errors
    ///
    /// Fails when the thread that forwards events cannot start.
    pub fn new(context: BackendContext) -> Result<Self> {
        Self::with_kit(context, ActivityKit::new())
    }

    /// Creates the backend on a kit that the adapter built, for example with other receipts.
    ///
    /// # Errors
    ///
    /// Fails when the thread that forwards events cannot start.
    pub fn with_kit(context: BackendContext, kit: ActivityKit) -> Result<Self> {
        // Elsewhere activitykit reports no event, so no thread waits for one.
        if kit.authorization().supported {
            events::forward(&kit, context)?;
        }
        Ok(Self {
            kit,
            handles: Mutex::default(),
        })
    }

    /// The wrapped kit, for iOS detail outside the neutral contract such as images and the
    /// authorization. A framework adapter keeps the backend in an `Arc` to reach it.
    pub fn kit(&self) -> &ActivityKit {
        &self.kit
    }
}

impl ActivityKitBackend {
    /// The neutral record of `activity`, whose handle the backend keeps from now on.
    fn answer(&self, activity: Activity) -> Result<ActivityRecord> {
        let record = content::record(content::seen(&activity))?;
        let mut handles = self.handles.lock().unwrap_or_else(PoisonError::into_inner);
        handles.retain(|kept| kept.id() != activity.id());
        if handles.len() == HANDLES_KEPT {
            handles.pop_front();
        }
        handles.push_back(activity);
        Ok(record)
    }
}

impl ActivityKitBackend {
    fn read_permissions(&self, requested: bool) -> Permissions {
        let authorization = self.kit.authorization();
        let live_activities = match (authorization.supported, authorization.enabled, requested) {
            (false, _, _) => PermissionState::NotApplicable,
            (true, true, _) => PermissionState::Granted,
            (true, false, true) => PermissionState::SettingsRequired,
            (true, false, false) => PermissionState::Denied,
        };
        Permissions {
            live_activities,
            ..Permissions::all(PermissionState::NotApplicable)
        }
    }
}

impl LiveActivityBackend for ActivityKitBackend {
    fn capabilities(&self) -> BackendFuture<'_, Capabilities> {
        Box::pin(async move {
            let authorization = self.kit.authorization();
            Ok(Capabilities {
                actions: (authorization.supported && authorization.buttons).into(),
                max_content_bytes: Some(MAX_CONTENT_BYTES),
                ..Capabilities::all(BACKEND_NAME, authorization.supported.into())
            })
        })
    }

    fn permissions(&self) -> BackendFuture<'_, Permissions> {
        Box::pin(async move { Ok(self.read_permissions(false)) })
    }

    /// Live Activities are a switch in Settings: there is no prompt, so a request of a
    /// disabled one reports `settingsRequired`.
    fn request_permission(&self, kind: PermissionKind) -> BackendFuture<'_, Permissions> {
        Box::pin(async move {
            match kind {
                PermissionKind::LiveActivities => Ok(self.read_permissions(true)),
                _ => Err(Error::new(
                    ErrorCode::Unsupported,
                    "This backend only exposes Live Activity settings.",
                )),
            }
        })
    }

    fn write_image(&self, name: String, png: Vec<u8>) -> BackendFuture<'_, ()> {
        Box::pin(async move { self.kit.write_image(&name, &png).map_err(content::error) })
    }

    fn start(&self, request: StartRequest) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move {
            let start = content::start(request)?;
            let activity = self.kit.start_with(start).await.map_err(content::error)?;
            self.answer(activity)
        })
    }

    fn update(&self, request: UpdateRequest) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move {
            let id = request.id.clone();
            let content = request.content.clone();
            let update = content::update(request)?;
            let activity = self
                .kit
                .activity(&id)
                .await
                .map_err(content::error)?
                .ok_or_else(content::not_found)?;
            content::keeps_template(&activity.values(), Some(&content))?;
            activity.update_with(update).await.map_err(content::error)?;
            self.answer(activity)
        })
    }

    fn end(&self, request: EndRequest) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move {
            let id = request.id.clone();
            let content = request.content.clone();
            let end = content::end(request)?;
            let activity = self
                .kit
                .activity(&id)
                .await
                .map_err(content::error)?
                .ok_or_else(content::not_found)?;
            content::keeps_template(&activity.values(), content.as_ref())?;
            activity.end(end).await.map_err(content::error)?;
            self.answer(activity)
        })
    }

    fn list(&self) -> BackendFuture<'_, Vec<ActivityRecord>> {
        Box::pin(async move {
            self.kit
                .activities()
                .await
                .map_err(content::error)?
                .iter()
                .map(|activity| content::record(content::seen(activity)))
                .collect()
        })
    }
}

#[cfg(all(test, not(target_os = "ios")))]
mod tests {
    use super::*;
    use crate::FeatureSupport;
    use crate::{DismissalPolicy, ErrorCode, EventSink, LiveActivity, LiveActivityEvent};
    use futures_executor::block_on;
    use serde_json::json;

    struct NoEvents;

    impl EventSink for NoEvents {
        fn emit(&self, _event: LiveActivityEvent) -> Result<()> {
            unreachable!("no events off iOS")
        }
    }

    #[test]
    fn req_k_004_off_ios_the_backend_answers_as_activitykit_does() {
        // REQ-K-004
        let service = LiveActivity::connect(NoEvents, ActivityKitBackend::new).unwrap();

        let capabilities = block_on(service.capabilities()).unwrap();
        assert_eq!(capabilities.backend, BACKEND_NAME);
        assert_eq!(
            capabilities,
            Capabilities {
                max_content_bytes: Some(MAX_CONTENT_BYTES),
                ..Capabilities::all(BACKEND_NAME, FeatureSupport::Unsupported)
            }
        );
        assert_eq!(
            block_on(service.permissions()).unwrap(),
            Permissions::all(PermissionState::NotApplicable)
        );
        assert_eq!(
            block_on(service.request_permission(PermissionKind::LiveActivities)).unwrap(),
            Permissions::all(PermissionState::NotApplicable)
        );
        assert_eq!(
            block_on(service.request_permission(PermissionKind::Notifications))
                .unwrap_err()
                .code,
            ErrorCode::Unsupported
        );
        // A portable template reaches the kit as a layout activity, and an image its folder.
        let template = crate::Template::progress("Delivery", "Preparing", 0.25);
        assert_eq!(
            block_on(service.start(StartRequest::template("delivery", template)))
                .unwrap_err()
                .code,
            ErrorCode::UnsupportedPlatform
        );
        assert_eq!(
            block_on(service.write_image("cover", vec![1]))
                .unwrap_err()
                .code,
            ErrorCode::InvalidRequest
        );

        let code = |result: Result<ActivityRecord>| result.unwrap_err().code;
        let layout = json!({"lockScreen": {"type": "text", "value": "Recording"}});
        let valid = json!({"fallback": "none", "layout": layout, "snapshot": {"revision": 0, "values": {}}});
        // A valid request reaches the kit, which has no platform; an invalid one stops before it.
        assert_eq!(
            code(block_on(service.start(StartRequest::new("rec", 2, valid)))),
            ErrorCode::UnsupportedPlatform
        );
        assert_eq!(
            code(block_on(service.start(StartRequest::new(
                "rec",
                2,
                json!({})
            )))),
            ErrorCode::InvalidRequest
        );
        assert_eq!(
            code(block_on(service.update(UpdateRequest::new(
                "id",
                1,
                json!({"values": {}})
            )))),
            ErrorCode::UnsupportedPlatform
        );
        assert_eq!(
            code(block_on(service.end(EndRequest::new(
                "id",
                2,
                None,
                DismissalPolicy::Default
            )))),
            ErrorCode::UnsupportedPlatform
        );
        assert_eq!(
            block_on(service.list()).unwrap_err().code,
            ErrorCode::UnsupportedPlatform
        );

        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ActivityKitBackend>();
    }
}
