//! The progress and timer templates of contract v1 on activitykit, which has no templates: each becomes a layout
//! activity with a fixed tree, and the snapshot's fields as values. A value
//! under [`MARKER`] names the template, so that the v1 commands find their activities and the layout commands leave
//! them alone.

use crate::{
    layout::{Island, IslandExpanded, Layout, LayoutSnapshot, LayoutStartInput, Value},
    models::{Snapshot, StartInput, Template, Timer},
};
use activitykit::{
    Style,
    layout::{
        Color, Font, Frame, HorizontalAlignment, IndicatorKind, Node, progress, symbol, text,
        timer, vstack,
    },
};
use std::collections::BTreeMap;

/// The value key that marks a template activity; its value is `progress` or `timer`.
pub(crate) const MARKER: &str = crate::layout::TEMPLATE_MARKER;
const TITLE: &str = "title";
const STATUS: &str = "status";
const PROGRESS: &str = "progress";
/// The progress as text, `42%`, for the compact island.
const PERCENT: &str = "percent";
const STARTED_AT: &str = "startedAt";
const ENDS_AT: &str = "endsAt";

fn name(template: Template) -> &'static str {
    match template {
        Template::Progress => "progress",
        Template::Timer => "timer",
    }
}

/// The template a snapshot's fields belong to; the contract gives it exactly one of progress and timer.
fn template_of(snapshot: &Snapshot) -> Template {
    if snapshot.progress.is_some() {
        Template::Progress
    } else {
        Template::Timer
    }
}

/// The meter of the template: a progress bar or a countdown.
fn meter(template: Template) -> Node {
    match template {
        Template::Progress => progress(PROGRESS),
        Template::Timer => timer(STARTED_AT, ENDS_AT).style("digits"),
    }
}

/// Title, status and meter, for the Lock Screen and in the expanded island.
fn card(template: Template, status: &str, spacing: f64) -> Node {
    vstack([
        text(activitykit::Bound::key(TITLE)).style("title"),
        text(activitykit::Bound::key(STATUS)).style(status),
        meter(template),
    ])
    .alignment(HorizontalAlignment::Leading)
    .spacing(spacing)
}

fn styles() -> BTreeMap<String, Style> {
    BTreeMap::from([
        ("card".into(), Style::new().padding(16.0)),
        (
            "title".into(),
            Style::new().font(Font::Headline).line_limit(1),
        ),
        (
            "status".into(),
            Style::new().font(Font::Subheadline).line_limit(2),
        ),
        (
            "caption".into(),
            Style::new().font(Font::Caption).line_limit(2),
        ),
        ("digits".into(), Style::new().monospaced_digit()),
        (
            "compact".into(),
            Style::new()
                .monospaced_digit()
                .frame(Frame::new().max_width(60.0)),
        ),
    ])
}

/// The fixed layout of `template`.
pub(crate) fn layout(template: Template) -> (Layout, BTreeMap<String, Style>) {
    let (leading, trailing, minimal) = match template {
        Template::Progress => (
            symbol("arrow.down.circle"),
            text(activitykit::Bound::key(PERCENT)).style("digits"),
            progress(PROGRESS).kind(IndicatorKind::Circular),
        ),
        Template::Timer => (
            symbol("timer"),
            timer(STARTED_AT, ENDS_AT).style("compact"),
            symbol("timer"),
        ),
    };
    let layout = Layout {
        lock_screen: card(template, "status", 8.0).style("card"),
        island: Some(Island {
            expanded: Some(IslandExpanded {
                center: Some(card(template, "caption", 6.0)),
                ..IslandExpanded::default()
            }),
            compact_leading: Some(leading),
            compact_trailing: Some(trailing),
            minimal: Some(minimal),
            margins: None,
        }),
        // The system's secondary background, in light and dark.
        background: Some(
            Color::Adaptive {
                light: "#F2F2F7".into(),
                dark: "#1C1C1E".into(),
            }
            .into(),
        ),
        keyline: None,
        url: None,
    };
    (layout, styles())
}

/// The values of a template snapshot.
pub(crate) fn values(snapshot: &Snapshot) -> BTreeMap<String, Value> {
    let template = template_of(snapshot);
    let mut values = BTreeMap::from([
        (MARKER.to_owned(), Value::Text(name(template).into())),
        (TITLE.to_owned(), Value::Text(snapshot.title.clone())),
        (STATUS.to_owned(), Value::Text(snapshot.status.clone())),
    ]);
    if let Some(progress) = snapshot.progress {
        values.insert(PROGRESS.into(), Value::Number(progress));
        values.insert(
            PERCENT.into(),
            Value::Text(format!("{}%", (progress * 100.0).round())),
        );
    }
    if let Some(timer) = &snapshot.timer {
        values.insert(STARTED_AT.into(), Value::Number(timer.started_at as f64));
        values.insert(ENDS_AT.into(), Value::Number(timer.ends_at as f64));
    }
    values
}

/// A template snapshot as the snapshot of its layout activity.
pub(crate) fn layout_snapshot(snapshot: &Snapshot) -> (Template, LayoutSnapshot) {
    let layout = LayoutSnapshot {
        revision: snapshot.revision,
        values: values(snapshot),
        stale_at: snapshot.stale_at,
        timestamp: None,
        relevance: None,
        android: None,
    };
    (template_of(snapshot), layout)
}

/// The layout start of a template start.
pub(crate) fn start(input: &StartInput) -> LayoutStartInput {
    let (layout, styles) = layout(input.template);
    LayoutStartInput {
        activity_key: input.activity_key.clone(),
        schema_version: 2,
        fallback: input.fallback,
        layout,
        styles: Some(styles),
        snapshot: layout_snapshot(&input.snapshot).1,
        android: None,
        ios: None,
        scheduled: None,
        push: None,
    }
}

/// The template that marks `values`, if any.
pub(crate) fn of(values: &BTreeMap<String, Value>) -> Option<Template> {
    match values.get(MARKER)?.as_str()? {
        "progress" => Some(Template::Progress),
        "timer" => Some(Template::Timer),
        _ => None,
    }
}

/// The template snapshot that `values` hold, at `revision`.
pub(crate) fn snapshot(
    revision: u64,
    values: &BTreeMap<String, Value>,
    stale_at: Option<u64>,
) -> Option<(Template, Snapshot)> {
    let template = of(values)?;
    let text = |key: &str| values.get(key).and_then(Value::as_str).map(str::to_owned);
    let time = |key: &str| {
        values
            .get(key)
            .and_then(Value::as_f64)
            .filter(|at| at.fract() == 0.0 && *at >= 0.0)
            .map(|at| at as u64)
    };
    let (progress, timer) = match template {
        Template::Progress => (Some(values.get(PROGRESS)?.as_f64()?), None),
        Template::Timer => (
            None,
            Some(Timer {
                started_at: time(STARTED_AT)?,
                ends_at: time(ENDS_AT)?,
            }),
        ),
    };
    let snapshot = Snapshot {
        revision,
        title: text(TITLE)?,
        status: text(STATUS)?,
        progress,
        timer,
        stale_at,
    };
    Some((template, snapshot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, models::Fallback, validation::MAX_TIMESTAMP};
    use serde_json::json;

    fn input(template: Template, title: &str, status: &str) -> StartInput {
        let (progress, timer) = match template {
            Template::Progress => (Some(0.426), None),
            Template::Timer => (
                None,
                Some(Timer {
                    started_at: 1_000,
                    ends_at: MAX_TIMESTAMP,
                }),
            ),
        };
        StartInput {
            activity_key: "delivery:1".into(),
            schema_version: 1,
            template,
            fallback: Fallback::Notification,
            snapshot: Snapshot {
                revision: 3,
                title: title.into(),
                status: status.into(),
                progress,
                timer,
                stale_at: Some(9),
            },
        }
    }
    #[test]
    fn a_template_start_becomes_a_layout_start_with_the_fields_as_values() {
        let start = start(&input(Template::Progress, "Delivery", "On the way"));
        assert_eq!(
            (
                start.activity_key.as_str(),
                start.schema_version,
                start.fallback
            ),
            ("delivery:1", 2, Fallback::Notification)
        );
        assert_eq!(
            serde_json::to_value(&start.snapshot).unwrap(),
            json!({"revision":3,"staleAt":9,"values":{"liveActivityTemplate":"progress","title":"Delivery",
                "status":"On the way","progress":0.426,"percent":"43%"}})
        );
        assert_eq!(start.validate_contract(), Ok(()));
        let layout = serde_json::to_value(&start.layout).unwrap();
        assert_eq!(
            layout["lockScreen"]["children"][2],
            json!({"type":"progress","bind":"progress"})
        );
        assert_eq!(
            layout["island"]["compactTrailing"],
            json!({"type":"text","bind":"percent","style":"digits"})
        );
        assert_eq!(layout["island"]["minimal"]["kind"], "circular");
        assert_eq!(
            layout["background"],
            json!({"light":"#F2F2F7","dark":"#1C1C1E"})
        );
        let timer = start_timer();
        assert_eq!(timer.validate_contract(), Ok(()));
        let layout = serde_json::to_value(&timer.layout).unwrap();
        assert_eq!(
            layout["lockScreen"]["children"][2],
            json!({"type":"timerText","start":"startedAt","end":"endsAt","style":"digits"})
        );
        assert_eq!(
            timer.snapshot.values["endsAt"],
            Value::Number(MAX_TIMESTAMP as f64)
        );
        assert!(!timer.snapshot.values.contains_key("progress"));
    }

    fn start_timer() -> LayoutStartInput {
        start(&input(Template::Timer, "Nap", "Dozing"))
    }
    #[test]
    fn the_values_read_back_as_the_template_snapshot() {
        for template in [Template::Progress, Template::Timer] {
            let given = input(template, "Title", "Status").snapshot;
            let (read, snapshot) = snapshot(3, &values(&given), Some(9)).unwrap();
            assert_eq!((read, snapshot), (template, given.clone()));
            assert_eq!(layout_snapshot(&given).0, template);
            assert_eq!(of(&values(&given)), Some(template));
        }
        let mut broken = values(&input(Template::Timer, "a", "b").snapshot);
        broken.insert("endsAt".into(), Value::Number(1.5));
        assert_eq!(snapshot(1, &broken, None), None);
        broken.insert(MARKER.into(), Value::Text("chart".into()));
        assert_eq!((of(&broken), snapshot(1, &broken, None)), (None, None));
        assert_eq!(
            of(&BTreeMap::from([(MARKER.into(), Value::Number(1.0))])),
            None
        );
        assert_eq!(of(&BTreeMap::new()), None);
    }
    #[test]
    fn the_largest_template_snapshot_fits_the_weight_with_its_layout() {
        for template in [Template::Progress, Template::Timer] {
            let start = start(&input(template, &"😀".repeat(120), &"😀".repeat(240)));
            assert_eq!(start.validate_contract(), Ok(()), "{template:?}");
            let mut over = start.clone();
            over.snapshot
                .values
                .insert("status".into(), Value::Text("😀".repeat(1000)));
            assert_eq!(over.validate_contract(), Err(Error::payload_too_large()));
        }
    }
}
