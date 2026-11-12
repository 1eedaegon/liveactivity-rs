//! Conversions between the neutral contract and activitykit's request contract v2.

use super::{BACKEND_NAME, template};
use crate::{
    ActivityRecord, ActivityState, CONTRACT_VERSION, Content, DismissalPolicy, EndRequest, Error,
    ErrorCode, Result, StartRequest, Template, UpdateRequest,
};
use activitykit::{
    Activity, End, Layout, Start, State, Update, Values,
    contract::{
        Backend, LayoutActivityRecord, LayoutSnapshot, LayoutStartInput, PromotionState, Visibility,
    },
};
use serde_json::{Map, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The schema version of activitykit's layout requests.
const SCHEMA_VERSION: u8 = 2;
/// Members of a layout start request that the neutral request carries itself.
const START_MEMBERS: [&str; 2] = ["activityKey", "schemaVersion"];
/// Members of a layout record that the neutral record carries itself.
const RECORD_MEMBERS: [&str; 3] = ["id", "activityKey", "state"];
/// 9999-12-31T23:59:59.999Z in epoch milliseconds: a cap every platform's `SystemTime` holds.
const LATEST_MILLIS: u64 = 253_402_300_799_999;

fn invalid(message: &'static str) -> Error {
    Error::new(ErrorCode::InvalidRequest, message)
}

fn object(content: Value) -> Result<Map<String, Value>> {
    match content {
        Value::Object(members) => Ok(members),
        _ => Err(invalid("content must be a JSON object")),
    }
}

fn time(millis: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(millis.min(LATEST_MILLIS))
}

fn millis(at: SystemTime) -> Option<u64> {
    u64::try_from(at.duration_since(UNIX_EPOCH).ok()?.as_millis()).ok()
}

/// The values of `template`, with its stale date.
fn template_values(template: &Template) -> Result<(Values, Option<SystemTime>)> {
    let values = template::values(template.kind()?, template);
    Ok((values, template.stale_at.map(time)))
}

pub(crate) fn start(request: StartRequest) -> Result<Start> {
    let value = match request.content {
        Content::Native { value } => value,
        Content::Template(template) => {
            let (values, stale_at) = template_values(&template)?;
            let start = Start::new(template::layout(template.kind()?), values)
                .key(request.activity_key)
                .revision(request.revision);
            return Ok(match stale_at {
                Some(at) => start.stale_at(at),
                None => start,
            });
        }
    };
    if request.schema_version != u32::from(SCHEMA_VERSION) {
        return Err(invalid("schemaVersion must be 2 for a layout activity"));
    }
    let mut members = object(value)?;
    if START_MEMBERS.iter().any(|name| members.contains_key(*name)) {
        return Err(invalid(
            "content must not repeat activityKey or schemaVersion",
        ));
    }
    members.insert("activityKey".into(), request.activity_key.into());
    members.insert("schemaVersion".into(), SCHEMA_VERSION.into());
    // The parser's message may quote the activity's values, so it is not passed on.
    let input: LayoutStartInput = serde_json::from_value(Value::Object(members))
        .map_err(|_| invalid("content does not match the activitykit start contract"))?;
    let layout = Layout {
        lock_screen: input.layout.lock_screen,
        island: input.layout.island,
        background: input.layout.background,
        keyline: input.layout.keyline,
        styles: input.styles.unwrap_or_default(),
    };
    let start = Start::new(layout, input.snapshot.values)
        .key(input.activity_key)
        .revision(input.snapshot.revision);
    let start = match input.snapshot.stale_at {
        Some(at) => start.stale_at(time(at)),
        None => start,
    };
    Ok(match input.snapshot.timestamp {
        Some(at) => start.timestamp(time(at)),
        None => start,
    })
}

/// Refuses a template of another kind than the one that made `values`: its fields would change
/// the keys that the layout of the start binds.
pub(crate) fn keeps_template(values: &Values, content: Option<&Content>) -> Result<()> {
    let Some(template) = content.and_then(Content::template) else {
        return Ok(());
    };
    let started = template::read(values, None).and_then(|started| started.kind().ok());
    if started == Some(template.kind()?) {
        Ok(())
    } else {
        Err(invalid("an update keeps the template of the start"))
    }
}

/// The snapshot that `content` and the request's `revision` make.
fn snapshot(revision: u64, content: Content) -> Result<LayoutSnapshot> {
    let value = match content {
        Content::Native { value } => value,
        Content::Template(template) => {
            return Ok(LayoutSnapshot {
                revision,
                values: template::values(template.kind()?, &template),
                stale_at: template.stale_at,
                timestamp: None,
            });
        }
    };
    let mut members = object(value)?;
    if members
        .get("revision")
        .is_some_and(|repeated| repeated.as_u64() != Some(revision))
    {
        return Err(invalid(
            "content revision differs from the request revision",
        ));
    }
    members.insert("revision".into(), revision.into());
    serde_json::from_value(Value::Object(members))
        .map_err(|_| invalid("content does not match the activitykit snapshot contract"))
}

pub(crate) fn update(request: UpdateRequest) -> Result<Update> {
    let snapshot = snapshot(request.revision, request.content)?;
    let update = Update::new(snapshot.values).revision(snapshot.revision);
    let update = match snapshot.stale_at {
        Some(at) => update.stale_at(time(at)),
        None => update,
    };
    Ok(match snapshot.timestamp {
        Some(at) => update.timestamp(time(at)),
        None => update,
    })
}

/// An end without final content keeps the last accepted values at the handle's next revision,
/// as activitykit ends it; the request's revision belongs to final content.
pub(crate) fn end(request: EndRequest) -> Result<End> {
    let end = match request.dismissal {
        DismissalPolicy::Immediate => End::immediately(),
        DismissalPolicy::Default => End::default(),
        DismissalPolicy::After { at } => End::after(time(at)),
    };
    let Some(content) = request.content else {
        return Ok(end);
    };
    let snapshot = snapshot(request.revision, content)?;
    let end = end.values(snapshot.values).revision(snapshot.revision);
    let end = match snapshot.stale_at {
        Some(at) => end.stale_at(time(at)),
        None => end,
    };
    Ok(match snapshot.timestamp {
        Some(at) => end.timestamp(time(at)),
        None => end,
    })
}

/// What a handle knows of its activity, as plain data.
pub(crate) struct Seen {
    pub(crate) id: String,
    pub(crate) key: String,
    pub(crate) state: State,
    pub(crate) revision: Option<u64>,
    pub(crate) values: Values,
    pub(crate) stale_at: Option<u64>,
    pub(crate) timestamp: Option<u64>,
    pub(crate) relevance: f64,
    pub(crate) degraded_features: Vec<String>,
}

pub(crate) fn seen(activity: &Activity) -> Seen {
    Seen {
        id: activity.id().to_owned(),
        key: activity.key().to_owned(),
        state: activity.state(),
        revision: activity.revision(),
        values: activity.values(),
        stale_at: activity.stale_at().and_then(millis),
        timestamp: activity.timestamp().and_then(millis),
        relevance: activity.relevance(),
        degraded_features: activity.degraded_features(),
    }
}

/// The neutral record. A handle that only an event named has no revision yet and reports 0.
pub(crate) fn record(seen: Seen) -> Result<ActivityRecord> {
    let revision = seen.revision.unwrap_or_default();
    let state = activity_state(seen.state);
    let template = template::read(&seen.values, seen.stale_at);
    let wire = LayoutActivityRecord {
        id: seen.id,
        activity_key: seen.key,
        schema_version: SCHEMA_VERSION,
        snapshot: LayoutSnapshot {
            revision,
            values: seen.values,
            stale_at: seen.stale_at,
            timestamp: seen.timestamp,
        },
        backend: Backend::ActivityKit,
        state: seen.state,
        visibility: Visibility::Unknown,
        degraded_features: seen.degraded_features,
        promotion_requested: false,
        promotion_state: PromotionState::NotApplicable,
        relevance: Some(seen.relevance),
    };
    let Ok(Value::Object(mut members)) = serde_json::to_value(&wire) else {
        return Err(Error::new(
            ErrorCode::BackendFailure,
            "the activitykit record is not a JSON object",
        ));
    };
    for name in RECORD_MEMBERS {
        members.remove(name);
    }
    Ok(ActivityRecord {
        contract_version: CONTRACT_VERSION,
        id: wire.id,
        activity_key: wire.activity_key,
        revision,
        state,
        template,
        content: Value::Object(members),
        backend: BACKEND_NAME.into(),
    })
}

pub(crate) fn error(value: activitykit::Error) -> Error {
    let code = match value.code() {
        "UNSUPPORTED_PLATFORM" => ErrorCode::UnsupportedPlatform,
        "INVALID_INPUT" => ErrorCode::InvalidRequest,
        "PAYLOAD_TOO_LARGE" => ErrorCode::PayloadTooLarge,
        "NOT_FOUND" => ErrorCode::NotFound,
        "CONFLICT" => ErrorCode::Conflict,
        "STALE_REVISION" => ErrorCode::StaleRevision,
        "PERMISSION_DENIED" => ErrorCode::PermissionDenied,
        "NOT_FOREGROUND" => ErrorCode::NotForeground,
        "CONFIGURATION_REQUIRED" => ErrorCode::ConfigurationRequired,
        "UNSUPPORTED_FEATURE" => ErrorCode::Unsupported,
        "SYSTEM_LIMIT" => ErrorCode::SystemLimit,
        // `NATIVE_ERROR`, and any code a later activitykit adds.
        _ => ErrorCode::BackendFailure,
    };
    // Display adds the detail of some variants: the feature and iOS version, the limit's scope.
    let text = value.to_string();
    let message = text
        .strip_prefix(value.code())
        .and_then(|rest| rest.strip_prefix(": "))
        .unwrap_or_else(|| value.message());
    Error::new(code, message)
}

pub(crate) fn not_found() -> Error {
    Error::new(ErrorCode::NotFound, "The active activity was not found.")
}

fn activity_state(value: State) -> ActivityState {
    match value {
        State::Pending => ActivityState::Pending,
        State::Active => ActivityState::Active,
        State::Stale => ActivityState::Stale,
        State::Ended => ActivityState::Ended,
        State::Dismissed => ActivityState::Dismissed,
        // `Unknown`, and any state a later activitykit adds.
        _ => ActivityState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use activitykit::LimitScope;
    use serde_json::json;

    fn start_content() -> Value {
        json!({
            "fallback": "none",
            "layout": {"lockScreen": {"type": "text", "value": "Recording"}},
            "snapshot": {"revision": 0, "values": {"level": 0.2}, "staleAt": 1_800_000_000_000_u64}
        })
    }

    fn code<T>(result: Result<T>) -> ErrorCode {
        result.map(|_| ()).unwrap_err().code
    }

    #[test]
    fn req_k_001_start_content_is_the_layout_contract() {
        // REQ-K-001
        let request = |content| StartRequest::new("rec", 2, content);
        let start = start(request(start_content())).unwrap();
        let shown = format!("{start:?}");
        assert!(
            shown.contains("\"rec\"") && shown.contains("Recording"),
            "{shown}"
        );

        let mut repeated_key = start_content();
        repeated_key["activityKey"] = json!("other");
        let mut unknown_member = start_content();
        unknown_member["template"] = json!("progress");
        let mut private = start_content();
        private["snapshot"]["revision"] = json!("private-value");
        for content in [json!("layout"), repeated_key, unknown_member, private] {
            let error = super::start(request(content)).map(|_| ()).unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidRequest);
            assert!(!error.message.contains("private-value"));
        }
        assert_eq!(
            code(super::start(StartRequest::new("rec", 1, start_content()))),
            ErrorCode::InvalidRequest
        );
    }

    #[test]
    fn req_k_001_update_and_end_content_is_a_snapshot_at_the_request_revision() {
        // REQ-K-001
        let values = json!({"values": {"level": 0.8}, "timestamp": 1_800_000_000_000_u64});
        let shown = format!(
            "{:?}",
            update(UpdateRequest::new("id", 3, values.clone())).unwrap()
        );
        assert!(shown.contains("Some(3)"), "{shown}");

        let repeated = json!({"revision": 3, "values": {}});
        assert!(update(UpdateRequest::new("id", 3, repeated.clone())).is_ok());
        assert_eq!(
            code(update(UpdateRequest::new("id", 4, repeated))),
            ErrorCode::InvalidRequest
        );
        assert_eq!(
            code(update(UpdateRequest::new("id", 4, json!({"level": 0.8})))),
            ErrorCode::InvalidRequest
        );

        let end_with = |content, dismissal| end(EndRequest::new("id", 5, content, dismissal));
        assert_eq!(
            end_with(None, DismissalPolicy::Immediate).unwrap(),
            End::immediately()
        );
        assert_eq!(
            end_with(None, DismissalPolicy::Default).unwrap(),
            End::default()
        );
        assert_eq!(
            end_with(
                None,
                DismissalPolicy::After {
                    at: 1_800_000_000_500
                }
            )
            .unwrap(),
            End::after(UNIX_EPOCH + Duration::from_millis(1_800_000_000_500))
        );
        assert_eq!(
            end_with(Some(values), DismissalPolicy::Immediate).unwrap(),
            End::immediately()
                .values([("level", 0.8)])
                .revision(5)
                .timestamp(time(1_800_000_000_000))
        );
        assert_eq!(
            code(end_with(Some(json!(1)), DismissalPolicy::Default)),
            ErrorCode::InvalidRequest
        );
    }

    #[test]
    fn req_k_002_every_activitykit_error_keeps_its_category() {
        // REQ-K-002
        use activitykit::Error as Kit;
        let text = || "m".to_owned();
        for (kit, code) in [
            (Kit::Unsupported, ErrorCode::UnsupportedPlatform),
            (Kit::InvalidInput(text()), ErrorCode::InvalidRequest),
            (Kit::PayloadTooLarge(text()), ErrorCode::PayloadTooLarge),
            (Kit::NotFound(text()), ErrorCode::NotFound),
            (Kit::Conflict(text()), ErrorCode::Conflict),
            (Kit::StaleRevision(text()), ErrorCode::StaleRevision),
            (Kit::PermissionDenied(text()), ErrorCode::PermissionDenied),
            (Kit::NotForeground(text()), ErrorCode::NotForeground),
            (
                Kit::ConfigurationRequired(text()),
                ErrorCode::ConfigurationRequired,
            ),
            (
                Kit::UnsupportedFeature {
                    feature: "button".into(),
                    minimum_ios: (17, 0),
                },
                ErrorCode::Unsupported,
            ),
            (
                Kit::SystemLimit {
                    scope: LimitScope::App,
                },
                ErrorCode::SystemLimit,
            ),
            (Kit::Native(text()), ErrorCode::BackendFailure),
        ] {
            assert_eq!(error(kit).code, code);
        }
        assert_eq!(error(Kit::NotFound(text())).message, "m");
        let feature = Kit::UnsupportedFeature {
            feature: "transient".into(),
            minimum_ios: (18, 0),
        };
        assert_eq!(error(feature).message, "transient needs iOS 18.0");
        assert_eq!(not_found().code, ErrorCode::NotFound);
    }

    #[test]
    fn req_k_003_a_record_carries_the_layout_record_as_content() {
        // REQ-K-003
        let seen = |state, revision| Seen {
            id: "la-ios-layout-1".into(),
            key: "rec".into(),
            state,
            revision,
            values: Values::from([("level", 0.2)]),
            stale_at: None,
            timestamp: Some(1_800_000_000_000),
            relevance: 0.5,
            degraded_features: vec!["layout.custom.wave".into()],
        };
        let record = record(seen(State::Active, Some(4))).unwrap();
        assert_eq!(
            serde_json::to_value(&record).unwrap(),
            json!({
                "contractVersion": 1,
                "id": "la-ios-layout-1",
                "activityKey": "rec",
                "revision": 4,
                "state": "active",
                "backend": "ios-activitykit",
                "content": {
                    "schemaVersion": 2,
                    "snapshot": {
                        "revision": 4,
                        "values": {"level": 0.2},
                        "timestamp": 1_800_000_000_000_u64
                    },
                    "backend": "activityKit",
                    "visibility": "unknown",
                    "degradedFeatures": ["layout.custom.wave"],
                    "promotionRequested": false,
                    "promotionState": "notApplicable",
                    "relevance": 0.5
                }
            })
        );

        for (state, neutral) in [
            (State::Pending, ActivityState::Pending),
            (State::Active, ActivityState::Active),
            (State::Stale, ActivityState::Stale),
            (State::Ended, ActivityState::Ended),
            (State::Dismissed, ActivityState::Dismissed),
            (State::Unknown, ActivityState::Unknown),
        ] {
            let record = super::record(seen(state, None)).unwrap();
            assert_eq!((record.state, record.revision), (neutral, 0));
        }
    }

    #[test]
    fn req_k_005_portable_templates_become_layout_activities_of_one_kind() {
        // REQ-K-005
        let progress = Template::progress("Delivery", "Preparing", 0.25).stale_at(9_000);
        let request = StartRequest::template("delivery", progress.clone()).revision(4);
        let shown = format!("{:?}", start(request).unwrap());
        assert!(shown.contains("\"delivery\"") && shown.contains("liveActivityTemplate"));
        assert!(shown.contains("revision: 4"), "{shown}");

        let update = update(UpdateRequest::new("id", 5, progress.clone())).unwrap();
        assert!(format!("{update:?}").contains("Preparing"));
        let final_content = EndRequest::new("id", 6, None, DismissalPolicy::Immediate)
            .with_content(progress.clone());
        assert_eq!(
            end(final_content).unwrap(),
            End::immediately()
                .values(template::values(crate::TemplateKind::Progress, &progress))
                .revision(6)
                .stale_at(time(9_000))
        );

        // An update keeps the kind that the start bound its layout to.
        let started = template::values(crate::TemplateKind::Progress, &progress);
        let timer = Content::from(Template::timer("Tea", "Brewing", 1_000, 61_000));
        assert!(keeps_template(&started, Some(&Content::from(progress.clone()))).is_ok());
        assert_eq!(
            code(keeps_template(&started, Some(&timer))),
            ErrorCode::InvalidRequest
        );
        // Native content and a plain end are activitykit's own to judge.
        assert!(keeps_template(&started, Some(&Content::from(json!({})))).is_ok());
        assert!(keeps_template(&started, None).is_ok());
        // A layout activity of the app's own takes no template.
        let own = Values::from([("level", 0.2)]);
        assert_eq!(
            code(keeps_template(&own, Some(&timer))),
            ErrorCode::InvalidRequest
        );

        let record = record(Seen {
            id: "la-1".into(),
            key: "delivery".into(),
            state: State::Active,
            revision: Some(5),
            values: started,
            stale_at: Some(9_000),
            timestamp: None,
            relevance: 0.0,
            degraded_features: Vec::new(),
        })
        .unwrap();
        assert_eq!(record.template, Some(progress));
    }

    #[test]
    fn times_cross_as_epoch_milliseconds() {
        assert_eq!(millis(time(1_800_000_000_000)), Some(1_800_000_000_000));
        assert_eq!(millis(time(u64::MAX)), Some(LATEST_MILLIS));
        assert_eq!(millis(UNIX_EPOCH - Duration::from_secs(1)), None);
    }
}
