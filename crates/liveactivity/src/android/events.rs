use super::{LiveUpdateBackend, content};
use crate::{ActivityRecord, LiveActivityEvent, Result};
use liveupdate::{Driver, EventKind};
use std::sync::PoisonError;

/// What one [`LiveUpdateBackend::pump_events`] did with the events Android had queued.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EventReport {
    /// Events the framework sink accepted.
    pub delivered: u64,
    /// Events the framework sink rejected.
    pub rejected: u64,
    /// Dismissals sent to no one: the engine no longer lists the activity, or
    /// `reconciliation_failed`. A later `list` still reports the dismissed state.
    pub unresolved: u64,
    /// Whether reading the records of this drain's dismissals failed.
    pub reconciliation_failed: bool,
    /// Oldest events Android discarded before this drain.
    pub dropped: u64,
}

impl<D: Driver + 'static> LiveUpdateBackend<D> {
    /// Drains the action and dismissal events queued by Android and sends each to the framework
    /// sink, oldest first. Concurrent calls run one after the other, so the order holds across
    /// them.
    ///
    /// Android queues events instead of calling back, so the framework adapter calls this when
    /// its native side signals one and when the application resumes. A drain removes the events:
    /// one that the sink rejects is counted and not retried, and the rest are still delivered.
    /// An action becomes an `action` event. A dismissal becomes a `stateChanged` event with the
    /// record as the engine reconciles it, which is `dismissed` unless the activity had already
    /// ended or was never observed.
    ///
    /// # Errors
    ///
    /// Fails when the drain itself fails.
    pub fn pump_events(&self) -> Result<EventReport> {
        // Guards no data, only the order of the sink's events across drains.
        let _pump = self.pump.lock().unwrap_or_else(PoisonError::into_inner);
        let batch = self.engine.events().map_err(content::error)?;
        let dismissed = batch
            .events
            .iter()
            .any(|event| event.kind == EventKind::Dismissed);
        // A failed reconciliation leaves the dismissals unresolved and the actions deliverable.
        let reconciled = if dismissed {
            self.records()
        } else {
            Ok(Vec::new())
        };
        let mut report = EventReport {
            dropped: batch.dropped,
            reconciliation_failed: reconciled.is_err(),
            ..EventReport::default()
        };
        let records = reconciled.unwrap_or_default();
        for event in batch.events {
            let neutral = match event.kind {
                EventKind::Action { action_id } => {
                    Some(LiveActivityEvent::action(event.activity_id, action_id))
                }
                EventKind::Dismissed => records
                    .iter()
                    .find(|record| record.id == event.activity_id)
                    .cloned()
                    .map(LiveActivityEvent::state_changed),
            };
            match neutral.map(|event| self.context.emit(event)) {
                Some(Ok(())) => report.delivered += 1,
                Some(Err(_)) => report.rejected += 1,
                None => report.unresolved += 1,
            }
        }
        Ok(report)
    }

    pub(crate) fn records(&self) -> Result<Vec<ActivityRecord>> {
        self.engine
            .list()
            .map_err(content::error)?
            .into_iter()
            .map(content::activity_record)
            .collect()
    }
}
