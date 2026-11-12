//! The portable templates on activitykit, which has no templates: each becomes a layout activity
//! with a fixed tree and the template's fields as values. A value under [`MARKER`] names the
//! template, so that a record of such an activity reads back as a template. The tree and the
//! marker are those of tauri-plugin-live-activity 0.4, whose activities stay readable.

use crate::{Template, TemplateKind, Timer};
use activitykit::{
    Bound, Layout, Style, Value, Values,
    contract::{Island, IslandExpanded},
    layout::{
        Color, Font, Frame, HorizontalAlignment, IndicatorKind, Node, progress, symbol, text,
        timer, vstack,
    },
};

/// The value key that marks a template activity; its value is `progress` or `timer`.
pub(crate) const MARKER: &str = "liveActivityTemplate";
const TITLE: &str = "title";
const STATUS: &str = "status";
const PROGRESS: &str = "progress";
/// The progress as text, `42%`, for the compact island.
const PERCENT: &str = "percent";
const STARTED_AT: &str = "startedAt";
const ENDS_AT: &str = "endsAt";

fn name(kind: TemplateKind) -> &'static str {
    match kind {
        TemplateKind::Progress => "progress",
        TemplateKind::Timer => "timer",
    }
}

/// The meter of the template: a progress bar or a countdown.
fn meter(kind: TemplateKind) -> Node {
    match kind {
        TemplateKind::Progress => progress(PROGRESS),
        TemplateKind::Timer => timer(STARTED_AT, ENDS_AT).style("digits"),
    }
}

/// Title, status and meter, for the Lock Screen and the expanded island.
fn card(kind: TemplateKind, status: &str, spacing: f64) -> Node {
    vstack([
        text(Bound::key(TITLE)).style("title"),
        text(Bound::key(STATUS)).style(status),
        meter(kind),
    ])
    .alignment(HorizontalAlignment::Leading)
    .spacing(spacing)
}

/// The fixed layout of a template of `kind`.
pub(crate) fn layout(kind: TemplateKind) -> Layout {
    let (leading, trailing, minimal) = match kind {
        TemplateKind::Progress => (
            symbol("arrow.down.circle"),
            text(Bound::key(PERCENT)).style("digits"),
            progress(PROGRESS).kind(IndicatorKind::Circular),
        ),
        TemplateKind::Timer => (
            symbol("timer"),
            timer(STARTED_AT, ENDS_AT).style("compact"),
            symbol("timer"),
        ),
    };
    let island = Island {
        expanded: Some(IslandExpanded {
            center: Some(card(kind, "caption", 6.0)),
            ..IslandExpanded::default()
        }),
        compact_leading: Some(leading),
        compact_trailing: Some(trailing),
        minimal: Some(minimal),
    };
    let compact = Style::new()
        .monospaced_digit()
        .frame(Frame::new().max_width(60.0));
    Layout::new(card(kind, "status", 8.0).style("card"))
        .island(island)
        // The system's secondary background, in light and dark.
        .background(Color::Adaptive {
            light: "#F2F2F7".into(),
            dark: "#1C1C1E".into(),
        })
        .style("card", Style::new().padding(16.0))
        .style("title", Style::new().font(Font::Headline).line_limit(1))
        .style("status", Style::new().font(Font::Subheadline).line_limit(2))
        .style("caption", Style::new().font(Font::Caption).line_limit(2))
        .style("digits", Style::new().monospaced_digit())
        .style("compact", compact)
}

/// The values of a template of `kind`.
pub(crate) fn values(kind: TemplateKind, template: &Template) -> Values {
    let fixed = [
        (MARKER, Value::Text(name(kind).into())),
        (TITLE, Value::Text(template.title.clone())),
        (STATUS, Value::Text(template.status.clone())),
    ];
    let progress = template.progress.into_iter().flat_map(|progress| {
        [
            (PROGRESS, Value::Number(progress)),
            (
                PERCENT,
                Value::Text(format!("{}%", (progress * 100.0).round())),
            ),
        ]
    });
    let timer = template.timer.into_iter().flat_map(|timer| {
        [
            (STARTED_AT, Value::Number(timer.started_at as f64)),
            (ENDS_AT, Value::Number(timer.ends_at as f64)),
        ]
    });
    fixed.into_iter().chain(progress).chain(timer).collect()
}

/// The template that `values` hold, when a template made them.
pub(crate) fn read(values: &Values, stale_at: Option<u64>) -> Option<Template> {
    let text = |key: &str| values.get(key).and_then(Value::as_str).map(str::to_owned);
    let time = |key: &str| {
        values
            .get(key)
            .and_then(Value::as_f64)
            .filter(|at| at.fract() == 0.0 && *at >= 0.0)
            .map(|at| at as u64)
    };
    let (progress, timer) = match values.get(MARKER)?.as_str()? {
        "progress" => (Some(values.get(PROGRESS)?.as_f64()?), None),
        "timer" => (
            None,
            Some(Timer {
                started_at: time(STARTED_AT)?,
                ends_at: time(ENDS_AT)?,
            }),
        ),
        _ => return None,
    };
    Some(Template {
        title: text(TITLE)?,
        status: text(STATUS)?,
        progress,
        timer,
        stale_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_k_005_a_template_is_a_marked_layout_and_reads_back() {
        // REQ-K-005
        let progress = Template::progress("Delivery", "Preparing", 0.426).stale_at(9);
        let made = values(TemplateKind::Progress, &progress);
        assert_eq!(made.get(MARKER), Some(&Value::Text("progress".into())));
        assert_eq!(made.get(PERCENT), Some(&Value::Text("43%".into())));
        assert_eq!(read(&made, Some(9)), Some(progress));

        let timer = Template::timer("Tea", "Brewing", 1_000, 61_000);
        let made = values(TemplateKind::Timer, &timer);
        assert_eq!(made.get(ENDS_AT), Some(&Value::Number(61_000.0)));
        assert_eq!(read(&made, None), Some(timer));

        // A layout activity of the app's own is no template.
        assert_eq!(read(&Values::from([("level", 0.2)]), None), None);
        let foreign = Values::from([(MARKER, "metric")]);
        assert_eq!(read(&foreign, None), None);

        for kind in [TemplateKind::Progress, TemplateKind::Timer] {
            let layout = layout(kind);
            assert!(layout.island.is_some());
            assert_eq!(layout.styles.len(), 6);
        }
    }
}
