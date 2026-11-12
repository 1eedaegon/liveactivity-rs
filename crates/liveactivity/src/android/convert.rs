use crate::{
    ActivityRecord, ActivityState, Backend, Capabilities, Error, Fallback, FeatureSupport,
    PermissionState, Permissions, PromotionState, Snapshot, StartInput, Template, Timer,
    Visibility,
};

pub(crate) fn start_request(input: StartInput) -> liveupdate::StartRequest {
    liveupdate::StartRequest {
        activity_key: input.activity_key,
        schema_version: input.schema_version,
        template: template(input.template),
        fallback: fallback(input.fallback),
        visibility: liveupdate::Visibility::Private,
        snapshot: live_snapshot(input.snapshot),
    }
}

pub(crate) fn activity_record(record: liveupdate::ActivityRecord) -> crate::Result<ActivityRecord> {
    let template = contract_template(record.template)?;
    let snapshot = contract_snapshot(record.snapshot)?;
    Ok(ActivityRecord {
        id: record.id,
        activity_key: record.activity_key,
        schema_version: record.schema_version,
        template,
        snapshot,
        backend: Backend::AndroidNotification,
        state: activity_state(record.state),
        visibility: Visibility::Unknown,
        degraded_features: record.degraded_features,
        promotion_requested: record.promotion_requested,
        promotion_state: promotion_state(record.promotion_state),
    })
}

pub(crate) fn capabilities(value: liveupdate::Capabilities) -> Capabilities {
    let supported = FeatureSupport::Supported;
    let support = |yes: bool| {
        if yes {
            supported
        } else {
            FeatureSupport::Unsupported
        }
    };
    Capabilities {
        platform: "android".into(),
        backend: Backend::AndroidNotification,
        start: supported,
        update: supported,
        end: supported,
        progress: supported,
        timer: supported,
        actions: support(value.presentation.actions && value.presentation.events),
        state_observation: supported,
        silent_update: supported,
        native_live_activity: support(value.can_promote()),
        max_payload_bytes: 4096,
        // The engine draws Android's system templates only.
        layout: FeatureSupport::Unsupported,
    }
}

/// The permissions as the engine observes them. It cannot tell a refusal from a prompt that was never shown, so a
/// disabled one is `denied`.
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
        live_activities: PermissionState::NotApplicable,
        promotion: if value.supported {
            state(value.promotion_enabled)
        } else {
            PermissionState::NotApplicable
        },
    }
}

/// A liveupdate error with its code, which is one of this crate's: liveupdate's codes are non-exhaustive.
pub(crate) fn error(value: liveupdate::Error) -> Error {
    let code = serde_json::to_value(value.code)
        .ok()
        .and_then(|value| value.as_str().map(contract_code))
        .unwrap_or_else(|| "NATIVE_ERROR".into());
    Error {
        code,
        message: value.message,
    }
}

/// `code` when the contract has it, else `NATIVE_ERROR`.
fn contract_code(code: &str) -> String {
    if crate::error::ERROR_CODES.contains(&code) {
        code.to_owned()
    } else {
        "NATIVE_ERROR".to_owned()
    }
}

pub(crate) fn live_snapshot(value: Snapshot) -> liveupdate::Snapshot {
    liveupdate::Snapshot {
        revision: value.revision,
        title: value.title,
        status: value.status,
        progress: value.progress,
        timer: value.timer.map(|timer| liveupdate::Timer {
            started_at: timer.started_at,
            ends_at: timer.ends_at,
        }),
        stale_at: value.stale_at,
        presentation: None,
    }
}

pub(crate) fn end_policy(value: crate::EndPolicy) -> liveupdate::EndPolicy {
    match value {
        crate::EndPolicy::Immediate {} => liveupdate::EndPolicy::Immediate,
        crate::EndPolicy::Default {} => liveupdate::EndPolicy::Default,
        crate::EndPolicy::After { at } => liveupdate::EndPolicy::After { at },
    }
}

fn contract_snapshot(value: liveupdate::Snapshot) -> crate::Result<Snapshot> {
    if value.presentation.is_some() {
        return Err(non_v1_record());
    }
    Ok(Snapshot {
        revision: value.revision,
        title: value.title,
        status: value.status,
        progress: value.progress,
        timer: value.timer.map(|timer| Timer {
            started_at: timer.started_at,
            ends_at: timer.ends_at,
        }),
        stale_at: value.stale_at,
    })
}

fn template(value: Template) -> liveupdate::Template {
    match value {
        Template::Progress => liveupdate::Template::Progress,
        Template::Timer => liveupdate::Template::Timer,
    }
}

fn contract_template(value: liveupdate::Template) -> crate::Result<Template> {
    match value {
        liveupdate::Template::Progress => Ok(Template::Progress),
        liveupdate::Template::Timer => Ok(Template::Timer),
        liveupdate::Template::Standard
        | liveupdate::Template::BigText
        | liveupdate::Template::Metric => Err(non_v1_record()),
    }
}

fn non_v1_record() -> Error {
    Error::new(
        "NATIVE_ERROR",
        "Android core returned a record outside the progress and timer contract",
    )
}

fn fallback(value: Fallback) -> liveupdate::Fallback {
    match value {
        Fallback::None => liveupdate::Fallback::None,
        Fallback::Notification => liveupdate::Fallback::Notification,
    }
}

fn activity_state(value: liveupdate::ActivityState) -> ActivityState {
    match value {
        liveupdate::ActivityState::Active => ActivityState::Active,
        liveupdate::ActivityState::Stale => ActivityState::Stale,
        liveupdate::ActivityState::Ended => ActivityState::Ended,
        liveupdate::ActivityState::Dismissed => ActivityState::Dismissed,
        liveupdate::ActivityState::Unknown => ActivityState::Unknown,
        _ => ActivityState::Unknown,
    }
}

fn promotion_state(value: liveupdate::PromotionState) -> PromotionState {
    match value {
        liveupdate::PromotionState::Promoted => PromotionState::Promoted,
        liveupdate::PromotionState::NotPromoted => PromotionState::NotPromoted,
        liveupdate::PromotionState::NotApplicable => PromotionState::NotApplicable,
        liveupdate::PromotionState::Unknown => PromotionState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActivityState, Backend, Fallback, FeatureSupport, PromotionState, Snapshot, Template,
        Visibility,
    };

    fn snapshot(revision: u64) -> Snapshot {
        Snapshot {
            revision,
            title: "Delivery".into(),
            status: "Preparing".into(),
            progress: Some(0.25),
            timer: None,
            stale_at: None,
        }
    }

    #[test]
    fn a_start_becomes_a_private_rust_owned_request() {
        let request = start_request(StartInput {
            activity_key: "delivery:1".into(),
            schema_version: 1,
            template: Template::Progress,
            fallback: Fallback::Notification,
            snapshot: snapshot(0),
        });

        assert_eq!(request.visibility, liveupdate::Visibility::Private);
        assert_eq!(request.template, liveupdate::Template::Progress);
        assert_eq!(request.snapshot.progress, Some(0.25));
        assert!(request.snapshot.presentation.is_none());
    }

    #[test]
    fn the_android_boundary_adds_the_backend_to_liveupdate_records() {
        let record = activity_record(liveupdate::ActivityRecord {
            id: "handle".into(),
            activity_key: "delivery:1".into(),
            schema_version: 1,
            template: liveupdate::Template::Progress,
            snapshot: liveupdate::Snapshot {
                revision: 2,
                title: "Delivery".into(),
                status: "Arriving".into(),
                progress: Some(0.75),
                timer: None,
                stale_at: None,
                presentation: None,
            },
            state: liveupdate::ActivityState::Active,
            visibility: liveupdate::Visibility::Unknown,
            degraded_features: vec!["nativeLiveActivity".into()],
            promotion_requested: true,
            promotion_state: liveupdate::PromotionState::NotPromoted,
        })
        .unwrap();

        assert_eq!(record.backend, Backend::AndroidNotification);
        assert_eq!(record.state, ActivityState::Active);
        assert_eq!(record.visibility, Visibility::Unknown);
        assert_eq!(record.promotion_state, PromotionState::NotPromoted);
        assert_eq!(record.snapshot.revision, 2);
    }

    #[test]
    fn rich_core_templates_are_not_guessed_into_the_v1_contract() {
        for template in [
            liveupdate::Template::Standard,
            liveupdate::Template::BigText,
            liveupdate::Template::Metric,
        ] {
            let error = contract_template(template).unwrap_err();
            assert_eq!(error.code, "NATIVE_ERROR");
        }
    }

    #[test]
    fn a_core_snapshot_with_a_presentation_is_not_guessed_into_the_v1_contract() {
        let presentation =
            serde_json::from_value(serde_json::json!({ "style": { "kind": "standard" } })).unwrap();
        let record = activity_record(liveupdate::ActivityRecord {
            id: "handle".into(),
            activity_key: "delivery:1".into(),
            schema_version: 1,
            template: liveupdate::Template::Progress,
            snapshot: liveupdate::Snapshot {
                revision: 2,
                title: "Delivery".into(),
                status: "Arriving".into(),
                progress: Some(0.75),
                timer: None,
                stale_at: None,
                presentation: Some(presentation),
            },
            state: liveupdate::ActivityState::Active,
            visibility: liveupdate::Visibility::Unknown,
            degraded_features: Vec::new(),
            promotion_requested: false,
            promotion_state: liveupdate::PromotionState::NotApplicable,
        });
        assert_eq!(record.unwrap_err().code, "NATIVE_ERROR");
    }

    #[test]
    fn capabilities_and_errors_keep_stable_public_values() {
        let converted = capabilities(liveupdate::Capabilities {
            supported: true,
            notifications_enabled: true,
            promotion_enabled: false,
            presentation: liveupdate::PresentationCapabilities::default(),
        });
        assert_eq!(converted.backend, Backend::AndroidNotification);
        assert_eq!(converted.start, FeatureSupport::Supported);
        assert_eq!(converted.native_live_activity, FeatureSupport::Unsupported);

        let converted = error(liveupdate::Error::new(
            liveupdate::ErrorCode::StaleRevision,
            "Activity revision is stale.",
        ));
        assert_eq!(converted.code, "STALE_REVISION");
        assert_eq!(converted.message, "Activity revision is stale.");
        assert_eq!(
            end_policy(crate::EndPolicy::After { at: 42 }),
            liveupdate::EndPolicy::After { at: 42 },
        );
    }

    #[test]
    fn every_liveupdate_code_is_one_of_the_contracts() {
        use liveupdate::ErrorCode::*;
        for code in [
            InvalidInput,
            Conflict,
            NotFound,
            StaleRevision,
            PermissionDenied,
            UnsupportedFeature,
            SystemLimit,
            NativeError,
        ] {
            let converted = error(liveupdate::Error::new(code, "m"));
            assert!(
                crate::error::ERROR_CODES.contains(&converted.code.as_str()),
                "{code:?} → {}",
                converted.code
            );
            assert_eq!(
                serde_json::to_value(code).unwrap(),
                serde_json::json!(converted.code)
            );
        }
        // liveupdate's codes are non-exhaustive: a later one that the contract does not know is a native error.
        assert_eq!(contract_code("SOMETHING_NEW"), "NATIVE_ERROR");
    }
}
