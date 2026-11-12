use crate::{
    ActivityRecord, ActivityState, BackendContext, BackendFuture, CONTRACT_VERSION, Capabilities,
    Content, EndRequest, Error, ErrorCode, Fallback, FeatureSupport, LiveActivityBackend,
    LiveActivityEvent, PermissionKind, PermissionState, Permissions, Result, StartRequest,
    Template, UpdateRequest,
};
use serde_json::json;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// A reference backend that keeps its activities in memory and shows nothing.
///
/// It draws portable templates only, keeps the lifecycle rules that [`conformance`](super::conformance)
/// checks, and reports an end and a tap to the event sink.
pub struct MemoryBackend {
    context: BackendContext,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    activities: Vec<Activity>,
    images: Vec<String>,
    started: u64,
}

struct Activity {
    record: ActivityRecord,
    template: Template,
    /// The start as it was asked: a repeated start is compared with it, whatever came since.
    origin: (Template, u64, Fallback),
}

fn refusal(code: ErrorCode, message: &'static str) -> Error {
    Error::new(code, message)
}

fn not_found() -> Error {
    refusal(ErrorCode::NotFound, "Unknown activity.")
}

fn template(content: Content) -> Result<Template> {
    match content {
        Content::Template(template) => Ok(template),
        _ => Err(refusal(
            ErrorCode::Unsupported,
            "the memory backend draws templates only",
        )),
    }
}

fn terminal(record: &ActivityRecord) -> bool {
    matches!(
        record.state,
        ActivityState::Ended | ActivityState::Dismissed
    )
}

impl MemoryBackend {
    /// Stable identifier this backend reports in capabilities and records.
    pub const NAME: &'static str = "memory";

    /// Creates the backend from the context supplied by `LiveActivity::connect`.
    pub fn new(context: BackendContext) -> Self {
        Self {
            context,
            state: Mutex::default(),
        }
    }

    /// Reports a tap on `action` of the activity `id` to the event sink, as a platform would.
    ///
    /// # Errors
    ///
    /// Fails when no activity has `id` or the sink rejects the event.
    pub fn tap(&self, id: &str, action: &str) -> Result<()> {
        let key = self
            .lock()
            .activities
            .iter()
            .find(|activity| activity.record.id == id)
            .map(|activity| activity.record.activity_key.clone())
            .ok_or_else(not_found)?;
        self.context
            .emit(LiveActivityEvent::keyed_action(id, key, action))
    }

    /// The names of the images written so far, oldest first.
    pub fn images(&self) -> Vec<String> {
        self.lock().images.clone()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn started(&self, request: StartRequest) -> Result<ActivityRecord> {
        let template = template(request.content)?;
        // This backend has no native presentation to promote to.
        if request.fallback == Fallback::None {
            return Err(refusal(
                ErrorCode::Unsupported,
                "The native presentation is unavailable and fallback was not allowed.",
            ));
        }
        let origin = (template.clone(), request.revision, request.fallback);
        let mut state = self.lock();
        let running = state.activities.iter().find(|activity| {
            !terminal(&activity.record) && activity.record.activity_key == request.activity_key
        });
        if let Some(running) = running {
            return if running.origin == origin {
                Ok(running.record.clone())
            } else {
                Err(refusal(
                    ErrorCode::Conflict,
                    "An activity with this key already exists.",
                ))
            };
        }
        state.started += 1;
        let record = ActivityRecord {
            contract_version: CONTRACT_VERSION,
            id: format!("memory-{}", state.started),
            activity_key: request.activity_key,
            revision: request.revision,
            state: ActivityState::Active,
            content: json!({}),
            template: Some(template.clone()),
            backend: Self::NAME.into(),
        };
        state.activities.push(Activity {
            record: record.clone(),
            template,
            origin,
        });
        Ok(record)
    }

    fn updated(&self, request: UpdateRequest) -> Result<ActivityRecord> {
        let template = template(request.content)?;
        let mut state = self.lock();
        let activity = state
            .activities
            .iter_mut()
            .find(|activity| activity.record.id == request.id && !terminal(&activity.record))
            .ok_or_else(not_found)?;
        accept(activity, request.revision, template)?;
        Ok(activity.record.clone())
    }

    fn ended(&self, request: EndRequest) -> Result<ActivityRecord> {
        let record = {
            let mut state = self.lock();
            let activity = state
                .activities
                .iter_mut()
                .find(|activity| activity.record.id == request.id)
                .ok_or_else(not_found)?;
            if terminal(&activity.record) {
                return Ok(activity.record.clone());
            }
            if let Some(content) = request.content {
                accept(activity, request.revision, template(content)?)?;
            }
            activity.record.state = ActivityState::Ended;
            activity.record.clone()
        };
        // The sink owns its failures: the activity has ended either way.
        let _ = self
            .context
            .emit(LiveActivityEvent::state_changed(record.clone()));
        Ok(record)
    }
}

/// New content under the revision rules every backend keeps.
fn accept(activity: &mut Activity, revision: u64, template: Template) -> Result<()> {
    if template.kind()? != activity.template.kind()? {
        return Err(refusal(
            ErrorCode::InvalidRequest,
            "an update keeps the template of the start",
        ));
    }
    if revision < activity.record.revision {
        return Err(refusal(
            ErrorCode::StaleRevision,
            "Activity revision is stale.",
        ));
    }
    if revision == activity.record.revision && template != activity.template {
        return Err(refusal(
            ErrorCode::Conflict,
            "Revision already contains different content.",
        ));
    }
    activity.record.revision = revision;
    activity.record.template = Some(template.clone());
    activity.template = template;
    Ok(())
}

impl LiveActivityBackend for MemoryBackend {
    fn capabilities(&self) -> BackendFuture<'_, Capabilities> {
        Box::pin(async {
            Ok(Capabilities {
                layouts: FeatureSupport::Unsupported,
                native_presentation: FeatureSupport::Unsupported,
                ..Capabilities::all(Self::NAME, FeatureSupport::Supported)
            })
        })
    }

    fn start(&self, request: StartRequest) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move { self.started(request) })
    }

    fn update(&self, request: UpdateRequest) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move { self.updated(request) })
    }

    fn end(&self, request: EndRequest) -> BackendFuture<'_, ActivityRecord> {
        Box::pin(async move { self.ended(request) })
    }

    fn list(&self) -> BackendFuture<'_, Vec<ActivityRecord>> {
        Box::pin(async move {
            let state = self.lock();
            Ok(state
                .activities
                .iter()
                .map(|activity| activity.record.clone())
                .collect())
        })
    }

    fn permissions(&self) -> BackendFuture<'_, Permissions> {
        Box::pin(async { Ok(Permissions::all(PermissionState::Granted)) })
    }

    fn request_permission(&self, _kind: PermissionKind) -> BackendFuture<'_, Permissions> {
        self.permissions()
    }

    fn write_image(&self, name: String, _png: Vec<u8>) -> BackendFuture<'_, ()> {
        Box::pin(async move {
            self.lock().images.push(name);
            Ok(())
        })
    }
}
