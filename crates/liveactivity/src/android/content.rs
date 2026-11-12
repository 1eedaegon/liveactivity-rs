//! Conversions between the neutral contract and liveupdate's JSON contract.

use super::BACKEND_NAME;
use crate::{
    ActivityRecord, ActivityState, CONTRACT_VERSION, Capabilities, Content, DismissalPolicy,
    EndRequest, Error, ErrorCode, Fallback, FeatureSupport, PermissionState, Permissions, Result,
    StartRequest, Template, TemplateKind, Timer, UpdateRequest,
};
use serde_json::{Map, Value};

/// Members of a liveupdate start request that the neutral request carries itself.
const START_MEMBERS: [&str; 2] = ["activityKey", "schemaVersion"];
/// How much of a template activity a secure lock screen reveals.
const TEMPLATE_VISIBILITY: liveupdate::Visibility = liveupdate::Visibility::Private;
/// Members of a liveupdate record that the neutral record carries itself.
const RECORD_MEMBERS: [&str; 3] = ["id", "activityKey", "state"];

fn invalid(message: &'static str) -> Error {
    Error::new(ErrorCode::InvalidRequest, message)
}

fn object(content: Value) -> Result<Map<String, Value>> {
    match content {
        Value::Object(members) => Ok(members),
        _ => Err(invalid("content must be a JSON object")),
    }
}

pub(crate) fn start_request(request: StartRequest) -> Result<liveupdate::StartRequest> {
    let schema_version = u8::try_from(request.schema_version)
        .map_err(|_| invalid("schemaVersion is out of range"))?;
    let value = match request.content {
        Content::Native { value } => value,
        Content::Template(template) => {
            return Ok(liveupdate::StartRequest {
                activity_key: request.activity_key,
                schema_version,
                template: match template.kind()? {
                    TemplateKind::Progress => liveupdate::Template::Progress,
                    TemplateKind::Timer => liveupdate::Template::Timer,
                },
                fallback: match request.fallback {
                    Fallback::None => liveupdate::Fallback::None,
                    Fallback::Notification => liveupdate::Fallback::Notification,
                },
                visibility: TEMPLATE_VISIBILITY,
                snapshot: template_snapshot(request.revision, template),
            });
        }
    };
    let mut members = object(value)?;
    if START_MEMBERS.iter().any(|name| members.contains_key(*name)) {
        return Err(invalid(
            "content must not repeat activityKey or schemaVersion",
        ));
    }
    members.insert("activityKey".into(), request.activity_key.into());
    members.insert("schemaVersion".into(), schema_version.into());
    // The parser's message may quote notification content, so it is not passed on.
    serde_json::from_value(Value::Object(members))
        .map_err(|_| invalid("content does not match the liveupdate start contract"))
}

pub(crate) fn snapshot(request: UpdateRequest) -> Result<liveupdate::Snapshot> {
    let value = match request.content {
        Content::Native { value } => value,
        Content::Template(template) => return Ok(template_snapshot(request.revision, template)),
    };
    let mut members = object(value)?;
    if members
        .get("revision")
        .is_some_and(|revision| revision.as_u64() != Some(request.revision))
    {
        return Err(invalid(
            "content revision differs from the request revision",
        ));
    }
    members.insert("revision".into(), request.revision.into());
    serde_json::from_value(Value::Object(members))
        .map_err(|_| invalid("content does not match the liveupdate snapshot contract"))
}

/// A portable template as the engine's snapshot of its progress or timer template.
fn template_snapshot(revision: u64, template: Template) -> liveupdate::Snapshot {
    liveupdate::Snapshot {
        revision,
        title: template.title,
        status: template.status,
        progress: template.progress,
        timer: template.timer.map(|timer| liveupdate::Timer {
            started_at: timer.started_at,
            ends_at: timer.ends_at,
        }),
        stale_at: template.stale_at,
        presentation: None,
    }
}

/// The portable template that a snapshot of the engine's progress or timer template is.
fn template(snapshot: &liveupdate::Snapshot) -> Option<Template> {
    if snapshot.presentation.is_some() {
        return None;
    }
    let template = Template {
        title: snapshot.title.clone(),
        status: snapshot.status.clone(),
        progress: snapshot.progress,
        timer: snapshot.timer.as_ref().map(|timer| Timer {
            started_at: timer.started_at,
            ends_at: timer.ends_at,
        }),
        stale_at: snapshot.stale_at,
    };
    template.kind().is_ok().then_some(template)
}

pub(crate) fn end_policy(request: &EndRequest) -> Result<liveupdate::EndPolicy> {
    if request.content.is_some() {
        return Err(Error::new(
            ErrorCode::Unsupported,
            "Android live updates end without final content",
        ));
    }
    match request.dismissal {
        DismissalPolicy::Immediate => Ok(liveupdate::EndPolicy::Immediate),
        DismissalPolicy::Default => Ok(liveupdate::EndPolicy::Default),
        DismissalPolicy::After { at } => Ok(liveupdate::EndPolicy::After { at }),
    }
}

pub(crate) fn activity_record(record: liveupdate::ActivityRecord) -> Result<ActivityRecord> {
    let Ok(Value::Object(mut members)) = serde_json::to_value(&record) else {
        return Err(Error::new(
            ErrorCode::BackendFailure,
            "the liveupdate record is not a JSON object",
        ));
    };
    for name in RECORD_MEMBERS {
        members.remove(name);
    }
    Ok(ActivityRecord {
        contract_version: CONTRACT_VERSION,
        revision: record.snapshot.revision,
        state: activity_state(record.state),
        template: template(&record.snapshot),
        id: record.id,
        activity_key: record.activity_key,
        content: Value::Object(members),
        backend: BACKEND_NAME.into(),
    })
}

/// What the backend implements. Whether Android currently allows a notification is a
/// permission, which a start answers with `permissionDenied`.
pub(crate) fn capabilities(value: liveupdate::Capabilities) -> Capabilities {
    Capabilities {
        actions: (value.presentation.actions && value.presentation.events).into(),
        // The engine draws Android's system templates only and stores no image.
        layouts: FeatureSupport::Unsupported,
        images: FeatureSupport::Unsupported,
        native_presentation: value.can_promote().into(),
        // An end removes the notification at once.
        final_content: FeatureSupport::Unsupported,
        delayed_dismissal: FeatureSupport::Unsupported,
        ..Capabilities::all(BACKEND_NAME, FeatureSupport::Supported)
    }
}

/// The permissions as the engine observes them. It cannot tell a refusal from a prompt that
/// was never shown, so a disabled one is `denied`.
pub(crate) fn permissions(value: liveupdate::Capabilities) -> Permissions {
    let state = |enabled: bool| {
        if enabled {
            PermissionState::Granted
        } else {
            PermissionState::Denied
        }
    };
    Permissions {
        notifications: state(value.notifications_enabled),
        promotion: if value.supported {
            state(value.promotion_enabled)
        } else {
            PermissionState::NotApplicable
        },
        ..Permissions::all(PermissionState::NotApplicable)
    }
}

pub(crate) fn error(value: liveupdate::Error) -> Error {
    use liveupdate::ErrorCode as Engine;
    let code = match value.code {
        Engine::InvalidInput => ErrorCode::InvalidRequest,
        Engine::Conflict => ErrorCode::Conflict,
        Engine::NotFound => ErrorCode::NotFound,
        Engine::StaleRevision => ErrorCode::StaleRevision,
        Engine::PermissionDenied => ErrorCode::PermissionDenied,
        Engine::UnsupportedFeature => ErrorCode::Unsupported,
        Engine::SystemLimit => ErrorCode::SystemLimit,
        // `NativeError`, and any code a later liveupdate adds.
        _ => ErrorCode::BackendFailure,
    };
    Error::new(code, value.message)
}

fn activity_state(value: liveupdate::ActivityState) -> ActivityState {
    match value {
        liveupdate::ActivityState::Active => ActivityState::Active,
        liveupdate::ActivityState::Stale => ActivityState::Stale,
        liveupdate::ActivityState::Ended => ActivityState::Ended,
        liveupdate::ActivityState::Dismissed => ActivityState::Dismissed,
        // `Unknown`, and any state a later liveupdate adds.
        _ => ActivityState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_engine_code_keeps_its_category() {
        use liveupdate::ErrorCode as Engine;
        for (engine, code) in [
            (Engine::InvalidInput, ErrorCode::InvalidRequest),
            (Engine::Conflict, ErrorCode::Conflict),
            (Engine::NotFound, ErrorCode::NotFound),
            (Engine::StaleRevision, ErrorCode::StaleRevision),
            (Engine::PermissionDenied, ErrorCode::PermissionDenied),
            (Engine::UnsupportedFeature, ErrorCode::Unsupported),
            (Engine::SystemLimit, ErrorCode::SystemLimit),
            (Engine::NativeError, ErrorCode::BackendFailure),
        ] {
            let converted = error(liveupdate::Error::new(engine, "m"));
            assert_eq!(converted, Error::new(code, "m"), "{engine:?}");
        }
    }

    #[test]
    fn every_engine_state_has_a_neutral_state() {
        use liveupdate::ActivityState as Engine;
        for (engine, state) in [
            (Engine::Unknown, ActivityState::Unknown),
            (Engine::Active, ActivityState::Active),
            (Engine::Stale, ActivityState::Stale),
            (Engine::Ended, ActivityState::Ended),
            (Engine::Dismissed, ActivityState::Dismissed),
        ] {
            assert_eq!(activity_state(engine), state);
        }
    }

    #[test]
    fn a_dismissal_time_keeps_its_milliseconds() {
        let end = EndRequest::new("id", 1, None, DismissalPolicy::After { at: 42_001 });
        assert_eq!(
            end_policy(&end).unwrap(),
            liveupdate::EndPolicy::After { at: 42_001 }
        );
    }
}
