use crate::{
    ActivityRecord, ActivityState, EndInput, EndPolicy, FeatureSupport, LiveActivity, Result,
    StartInput, UpdateInput,
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
    /// The platform, as the backend's capabilities name it.
    pub platform: String,
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
            "the {} backend is not exchangeable: {:#?}",
            self.platform,
            self.failures()
        );
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
        self.checks.push(Check { name, failure });
        result.ok()
    }

    /// Records that `result` failed with `code`.
    fn refuses<T: std::fmt::Debug>(&mut self, name: &'static str, result: Result<T>, code: &str) {
        let failure = match &result {
            Err(error) if error.code == code => None,
            other => Some(format!("expected {code}, got {other:?}")),
        };
        self.checks.push(Check { name, failure });
    }
}

/// Keys of earlier runs in this process stay out of the way of a backend that keeps state.
static RUN: AtomicU64 = AtomicU64::new(0);

/// A platform may not have confirmed the activity yet, so a running one is any state but an end.
fn running(record: &ActivityRecord) -> bool {
    !matches!(
        record.state,
        ActivityState::Ended | ActivityState::Dismissed
    )
}

/// Runs the template lifecycle on `live` and reports each rule a backend keeps to be exchangeable with another: one
/// caller, written once, then behaves the same on each. The run ends the activities it starts.
pub async fn conformance(live: &LiveActivity) -> Report {
    let run = RUN.fetch_add(1, Ordering::Relaxed);
    let key = |name: &str| format!("conformance:{run}:{name}");
    let mut report = Report {
        platform: String::new(),
        checks: Vec::new(),
    };

    let capabilities = report.holds(
        "capabilities offer the template lifecycle",
        live.capabilities().await,
        |capabilities| {
            [
                capabilities.start,
                capabilities.update,
                capabilities.end,
                capabilities.progress,
                capabilities.timer,
            ]
            .iter()
            .all(|feature| *feature == FeatureSupport::Supported)
        },
    );
    let Some(capabilities) = capabilities else {
        return report;
    };
    report.platform = capabilities.platform.clone();
    report.holds("permissions answer", live.permissions().await, |_| true);

    let start = StartInput::progress(&key("delivery"), "Delivery", "Preparing", 0.25);
    let started = report.holds(
        "start returns a running record with its key and content",
        live.start(start.clone()).await,
        |record| {
            running(record)
                && !record.id.is_empty()
                && record.activity_key == start.activity_key
                && record.snapshot == start.snapshot
        },
    );
    let Some(started) = started else {
        return report;
    };
    let id = started.id.as_str();
    let update = |revision, status, progress| {
        UpdateInput::progress(id, revision, "Delivery", status, progress)
    };

    report.holds(
        "a repeated start keeps its handle",
        live.start(start.clone()).await,
        |record| record.id == id,
    );
    report.refuses(
        "a start with the key of a running activity and other content is a conflict",
        live.start(StartInput::progress(
            &key("delivery"),
            "Delivery",
            "Other",
            0.5,
        ))
        .await,
        "CONFLICT",
    );
    report.refuses(
        "a request outside the contract is invalid",
        live.start(StartInput::progress(
            &key("invalid"),
            "Delivery",
            "Preparing",
            1.5,
        ))
        .await,
        "INVALID_INPUT",
    );
    report.holds(
        "a newer revision replaces the content",
        live.update(update(2, "Arriving", 0.5)).await,
        |record| record.snapshot.revision == 2 && record.snapshot.status == "Arriving",
    );
    report.holds(
        "the same revision with the same content is idempotent",
        live.update(update(2, "Arriving", 0.5)).await,
        |record| record.snapshot.revision == 2,
    );
    report.refuses(
        "an older revision is stale",
        live.update(update(1, "Late", 0.4)).await,
        "STALE_REVISION",
    );
    report.refuses(
        "the same revision with other content is a conflict",
        live.update(update(2, "Other", 0.6)).await,
        "CONFLICT",
    );
    report.holds(
        "a repeated start after an update still keeps its handle",
        live.start(start.clone()).await,
        |record| record.id == id && record.snapshot.revision == 2,
    );
    report.refuses(
        "an update of an unknown handle is not found",
        live.update(UpdateInput::progress(
            "conformance-unknown",
            1,
            "Delivery",
            "Gone",
            0.5,
        ))
        .await,
        "NOT_FOUND",
    );
    report.holds(
        "list reports the activity at its accepted revision",
        live.list().await,
        |records| {
            records
                .iter()
                .any(|record| record.id == id && record.snapshot.revision == 2)
        },
    );

    let timer = report.holds(
        "a timer starts beside a progress activity",
        live.start(StartInput::timer(
            &key("tea"),
            "Tea",
            "Brewing",
            1_800_000_000_000,
            1_800_000_600_000,
        ))
        .await,
        |record| running(record) && record.id != id && record.snapshot.timer.is_some(),
    );
    if let Some(timer) = &timer {
        report.refuses(
            "an update keeps the template of the start",
            live.update(UpdateInput::progress(&timer.id, 1, "Tea", "Brewing", 0.5))
                .await,
            "INVALID_INPUT",
        );
    }

    report.holds(
        "an end ends the activity with its last content",
        live.end(EndInput::immediate(id)).await,
        |record| record.state == ActivityState::Ended && record.snapshot.status == "Arriving",
    );
    report.holds(
        "a repeated end is idempotent",
        live.end(EndInput::immediate(id)).await,
        |record| record.state == ActivityState::Ended,
    );
    report.refuses(
        "a late update cannot revive an ended activity",
        live.update(update(4, "Revived", 0.9)).await,
        "NOT_FOUND",
    );
    let restarted = report.holds(
        "a start after the end is a new activity with a new handle",
        live.start(start).await,
        |record| running(record) && record.id != id,
    );

    // A start that refuses the ordinary presentation needs the native one.
    let strict =
        StartInput::progress(&key("strict"), "Delivery", "Preparing", 0.25).without_fallback();
    let strict = if capabilities.native_live_activity == FeatureSupport::Supported {
        report.holds(
            "a start without fallback runs where the native presentation is available",
            live.start(strict).await,
            running,
        )
    } else {
        report.refuses(
            "a start without fallback is unsupported without the native presentation",
            live.start(strict).await,
            "UNSUPPORTED_FEATURE",
        );
        None
    };
    report.holds(
        "queued events synchronize without a failure",
        live.sync_events().await,
        |()| true,
    );

    // The run leaves nothing on screen.
    for record in restarted.iter().chain(&timer).chain(&strict) {
        let end = EndInput {
            id: record.id.clone(),
            policy: EndPolicy::Immediate {},
        };
        report.holds(
            "the activities of the run end",
            live.end(end).await,
            |record| record.state == ActivityState::Ended,
        );
    }
    report
}
