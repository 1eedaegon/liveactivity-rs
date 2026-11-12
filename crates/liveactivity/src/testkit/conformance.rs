use crate::{
    ActivityRecord, ActivityState, DismissalPolicy, EndRequest, ErrorCode, Fallback, LiveActivity,
    Result, StartRequest, Template, UpdateRequest,
};
use std::sync::atomic::{AtomicU64, Ordering};

/// One rule of the interface and whether the backend kept it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// The rule.
    pub name: &'static str,
    /// What the backend did instead, when it broke the rule.
    pub failure: Option<String>,
}

/// The rules [`conformance`] checked on one backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The backend, as its capabilities name it.
    pub backend: String,
    /// Every rule, in the order checked.
    pub checks: Vec<Check>,
}

impl Report {
    /// Whether the backend kept every rule.
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|check| check.failure.is_none())
    }

    /// The rules the backend broke.
    pub fn failures(&self) -> Vec<&Check> {
        self.checks
            .iter()
            .filter(|check| check.failure.is_some())
            .collect()
    }

    /// Panics with the broken rules, for a test.
    ///
    /// # Panics
    ///
    /// When the backend broke a rule.
    pub fn assert_passed(&self) {
        assert!(
            self.passed(),
            "backend {} is not exchangeable: {:#?}",
            self.backend,
            self.failures()
        );
    }

    fn check(&mut self, name: &'static str, failure: Option<String>) {
        self.checks.push(Check { name, failure });
    }

    /// Records that `result` succeeded and `rule` holds of its value, and gives the value.
    fn holds<T: std::fmt::Debug>(
        &mut self,
        name: &'static str,
        result: Result<T>,
        rule: impl FnOnce(&T) -> bool,
    ) -> Option<T> {
        let failure = match &result {
            Ok(value) if rule(value) => None,
            other => Some(format!("{other:?}")),
        };
        self.check(name, failure);
        result.ok()
    }

    /// Records that `result` failed with `code`.
    fn refuses<T: std::fmt::Debug>(
        &mut self,
        name: &'static str,
        result: Result<T>,
        code: ErrorCode,
    ) {
        let failure = match &result {
            Err(error) if error.code == code => None,
            other => Some(format!("expected {code:?}, got {other:?}")),
        };
        self.check(name, failure);
    }
}

/// The smallest PNG: one transparent pixel.
const PNG: [u8; 67] = [
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

/// Keys of earlier runs in this process stay out of the way of a backend that keeps state.
static RUN: AtomicU64 = AtomicU64::new(0);

/// A platform may not have confirmed the activity yet, so a running one is any state but an end.
fn active(record: &ActivityRecord) -> bool {
    !matches!(
        record.state,
        ActivityState::Ended | ActivityState::Dismissed
    )
}

/// Runs the portable lifecycle on `service` and reports each rule a backend keeps to be
/// exchangeable with another: one caller, written once, then behaves the same on each.
///
/// The activities are templates, which every backend with `templates` draws. The run ends the
/// activities it starts.
pub async fn conformance(service: &LiveActivity) -> Report {
    let run = RUN.fetch_add(1, Ordering::Relaxed);
    let key = |name: &str| format!("conformance:{run}:{name}");
    let mut report = Report {
        backend: String::new(),
        checks: Vec::new(),
    };

    let capabilities = report.holds(
        "capabilities name the backend and offer the template lifecycle",
        service.capabilities().await,
        |capabilities| {
            [
                capabilities.start,
                capabilities.update,
                capabilities.end,
                capabilities.list,
                capabilities.templates,
            ]
            .iter()
            .all(|feature| feature.is_supported())
        },
    );
    let Some(capabilities) = capabilities else {
        return report;
    };
    report.backend = capabilities.backend;

    let preparing = Template::progress("Delivery", "Preparing", 0.25);
    let arriving = Template::progress("Delivery", "Arriving", 0.5);
    let start = StartRequest::template(key("delivery"), preparing.clone());
    let started = report.holds(
        "start returns an active record with its key, revision and template",
        service.start(start.clone()).await,
        |record| {
            active(record)
                && !record.id.is_empty()
                && record.activity_key == start.activity_key
                && record.revision == 0
                && record.template.as_ref() == Some(&preparing)
        },
    );
    let Some(started) = started else {
        return report;
    };
    let id = started.id.as_str();

    report.holds(
        "a repeated start keeps its handle",
        service.start(start.clone()).await,
        |record| record.id == id,
    );
    report.refuses(
        "a start with the key of a running activity and other content is a conflict",
        service
            .start(StartRequest::template(key("delivery"), arriving.clone()))
            .await,
        ErrorCode::Conflict,
    );
    report.refuses(
        "a template with both a progress and a timer is invalid",
        service
            .start(StartRequest::template(
                key("invalid"),
                Template {
                    timer: Template::timer("t", "s", 1, 2).timer,
                    ..preparing.clone()
                },
            ))
            .await,
        ErrorCode::InvalidRequest,
    );

    report.holds(
        "a newer revision replaces the content",
        service
            .update(UpdateRequest::new(id, 2, arriving.clone()))
            .await,
        |record| record.revision == 2 && record.template.as_ref() == Some(&arriving),
    );
    report.holds(
        "the same revision with the same content is idempotent",
        service
            .update(UpdateRequest::new(id, 2, arriving.clone()))
            .await,
        |record| record.revision == 2,
    );
    report.refuses(
        "an older revision is stale",
        service
            .update(UpdateRequest::new(id, 1, preparing.clone()))
            .await,
        ErrorCode::StaleRevision,
    );
    report.refuses(
        "the same revision with other content is a conflict",
        service
            .update(UpdateRequest::new(id, 2, preparing.clone()))
            .await,
        ErrorCode::Conflict,
    );
    report.holds(
        "a repeated start after an update still keeps its handle",
        service.start(start.clone()).await,
        |record| record.id == id && record.revision == 2,
    );
    report.refuses(
        "an update of an unknown handle is not found",
        service
            .update(UpdateRequest::new(
                "conformance-unknown",
                1,
                arriving.clone(),
            ))
            .await,
        ErrorCode::NotFound,
    );
    report.holds(
        "list reports the activity at its accepted revision",
        service.list().await,
        |records| {
            records
                .iter()
                .any(|record| record.id == id && record.revision == 2 && active(record))
        },
    );

    let brewing = Template::timer("Tea", "Brewing", 1_800_000_000_000, 1_800_000_600_000);
    let timer = report.holds(
        "a timer template starts beside a progress template",
        service
            .start(StartRequest::template(key("tea"), brewing.clone()))
            .await,
        |record| active(record) && record.id != id && record.template.as_ref() == Some(&brewing),
    );
    if let Some(timer) = &timer {
        report.refuses(
            "an update keeps the template of the start",
            service
                .update(UpdateRequest::new(&timer.id, 1, arriving.clone()))
                .await,
            ErrorCode::InvalidRequest,
        );
    }

    let end = |id: &str| EndRequest::new(id, 3, None, DismissalPolicy::Immediate);
    // What an end can do differs between platforms: the capabilities say it before the call.
    let delayed = EndRequest::new(id, 3, None, DismissalPolicy::After { at: 1 });
    if !capabilities.delayed_dismissal.is_supported() {
        report.refuses(
            "a delayed dismissal the backend does not report is unsupported",
            service.end(delayed).await,
            ErrorCode::Unsupported,
        );
    }
    let done = Template::progress("Delivery", "Delivered", 1.0);
    let last = if capabilities.final_content.is_supported() {
        report.holds(
            "an end with final content ends the activity with it",
            service.end(end(id).with_content(done.clone())).await,
            |record| {
                record.state == ActivityState::Ended && record.template.as_ref() == Some(&done)
            },
        );
        done
    } else {
        report.refuses(
            "final content the backend does not report is unsupported",
            service.end(end(id).with_content(done)).await,
            ErrorCode::Unsupported,
        );
        report.holds(
            "an end ends the activity with its last content",
            service.end(end(id)).await,
            |record| {
                record.state == ActivityState::Ended
                    && record.revision >= 2
                    && record.template.as_ref() == Some(&arriving)
            },
        );
        arriving.clone()
    };
    report.holds(
        "a repeated end is idempotent",
        service.end(end(id)).await,
        |record| record.state == ActivityState::Ended && record.template.as_ref() == Some(&last),
    );
    report.refuses(
        "a late update cannot revive an ended activity",
        service
            .update(UpdateRequest::new(id, 4, preparing.clone()))
            .await,
        ErrorCode::NotFound,
    );
    let restarted = report.holds(
        "a start after the end is a new activity with a new handle",
        service.start(start).await,
        |record| active(record) && record.id != id,
    );

    // A start that refuses the ordinary presentation needs the native one.
    let strict = StartRequest::template(key("strict"), preparing.clone()).fallback(Fallback::None);
    let strict = if capabilities.native_presentation.is_supported() {
        report.holds(
            "a start without fallback runs where the native presentation is available",
            service.start(strict).await,
            active,
        )
    } else {
        report.refuses(
            "a start without fallback is unsupported without the native presentation",
            service.start(strict).await,
            ErrorCode::Unsupported,
        );
        None
    };
    let stores = service.write_image("conformance", PNG.to_vec()).await;
    if capabilities.images.is_supported() {
        // Storing may need setup of the host application, such as an App Group on iOS: a backend
        // then names that, and the image is not stored.
        let failure = match &stores {
            Ok(()) => None,
            Err(error) if error.code == ErrorCode::ConfigurationRequired => None,
            other => Some(format!("{other:?}")),
        };
        report.check(
            "an image is stored where images are reported, or the missing setup is named",
            failure,
        );
    } else {
        report.refuses(
            "an image the backend does not report is unsupported",
            stores,
            ErrorCode::Unsupported,
        );
    }

    report.holds(
        "permissions answer under the contract",
        service.permissions().await,
        |_| true,
    );
    report.holds(
        "queued events synchronize without a failure",
        service.sync_events().await,
        |()| true,
    );

    // The run leaves nothing on screen.
    for record in restarted.iter().chain(&timer).chain(&strict) {
        report.holds(
            "the activities of the run end",
            service.end(end(&record.id)).await,
            |record| record.state == ActivityState::Ended,
        );
    }
    report
}
