//! A layout activity on Android: the projection of its lock screen tree and values onto a liveupdate schema-v2
//! request, which liveupdate's renderer draws with Android's notification styles. Only the lock screen is walked,
//! depth first in document order; a node shows when its `if` value is truthy and its `unless` value is not.
//!
//! A notification has no label per node: Android reads its title and text. So what a node says to a screen reader
//! is used where the notification would otherwise show nothing of the node: as the text of a node Android cannot
//! draw, and as the title of a button or a link without a text.

use crate::layout::{Bindable, Element, ImageSource, Layout, LayoutSnapshot, Node, Value};
use std::collections::BTreeMap;

/// The progress style's one segment: a layout progress is a fraction, liveupdate's an integer up to the segments'
/// sum.
const PROGRESS_SCALE: u32 = 1000;
const MAX_TITLE: usize = 120;
const MAX_STATUS: usize = 240;
const MAX_ACTION_TITLE: usize = 64;
const MAX_ACTIONS: usize = liveupdate::MAX_ACTIONS;

/// What the projection needs beside the layout and its values: the app around it, and how the activity started.
pub(crate) struct Environment<'a> {
    /// The title of a layout without a text.
    pub(crate) app_name: &'a str,
    /// The Samsung Now Bar option.
    pub(crate) samsung_now_bar: bool,
    /// The app-file path of a picture that `write_image` wrote, when it exists and liveupdate can draw it.
    pub(crate) image: &'a dyn Fn(&str) -> Option<String>,
    /// Whether a locked screen shows the notification's content.
    pub(crate) public: bool,
    /// Whether the device opens URLs from a notification and its actions.
    pub(crate) links: bool,
    /// Whether the device shows a clock in the header, beside a style.
    pub(crate) chronometer: bool,
}

/// One layout snapshot as liveupdate takes it, and what the projection left out.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Projection {
    pub(crate) snapshot: liveupdate::Snapshot,
    /// `layout.<type>` for the shown nodes left out and `layout.<member>` for what a shown node asks in vain, then
    /// the parts of the request Android does not draw.
    pub(crate) degraded: Vec<String>,
}

/// The template of a generation: the first progress gives Progress, else the first timer Timer, else two texts
/// BigText, else Standard. Conditions do not count, so that every update keeps it.
pub(crate) fn template(layout: &Layout) -> liveupdate::Template {
    let mut texts = 0;
    let mut timer = false;
    let mut progress = false;
    visit_all(
        &layout.lock_screen,
        false,
        &mut |node, in_button| match &node.element {
            Element::Progress { .. } => progress = true,
            Element::TimerText { .. } => timer = true,
            Element::Text { .. } if !in_button => texts += 1,
            // A node Android cannot draw that says in words what it is: `walk` takes those words as a text.
            element if !in_button && !drawn(element) && worded(node) => texts += 1,
            _ => {}
        },
    );
    if progress {
        liveupdate::Template::Progress
    } else if timer {
        liveupdate::Template::Timer
    } else if texts >= 2 {
        liveupdate::Template::BigText
    } else {
        liveupdate::Template::Standard
    }
}

/// The liveupdate snapshot of `snapshot` under the generation's `template`.
pub(crate) fn project(
    layout: &Layout,
    styled: bool,
    snapshot: &LayoutSnapshot,
    template: liveupdate::Template,
    environment: &Environment<'_>,
) -> Projection {
    let values = &snapshot.values;
    let mut shown = Shown {
        public: environment.public,
        links: environment.links,
        ..Shown::default()
    };
    walk(&layout.lock_screen, values, &mut shown);

    let chip = match (shown.progress, shown.timer) {
        (Some(fraction), None) => Some(format!("{}%", (fraction * 100.0).round() as u32)),
        _ => None,
    };
    let body = shown.texts.get(1).filter(|text| !blank(text)).cloned();
    let title = shown.texts.first().filter(|text| !blank(text)).map_or_else(
        || cut(app_name(environment.app_name), MAX_TITLE),
        |text| cut(text, MAX_TITLE),
    );
    let status = body
        .clone()
        .or_else(|| chip.clone())
        .map_or_else(|| title.clone(), |text| cut(&text, MAX_STATUS));

    let style = match template {
        liveupdate::Template::Progress => {
            liveupdate::PresentationStyle::Progress(progress_style(shown.progress))
        }
        liveupdate::Template::Timer => liveupdate::PresentationStyle::Timer(timer_style(
            shown
                .timer
                .or_else(|| first_timer(&layout.lock_screen, values)),
        )),
        liveupdate::Template::BigText => {
            liveupdate::PresentationStyle::BigText(liveupdate::BigTextStyle {
                text: status.clone(),
                summary: None,
            })
        }
        _ => liveupdate::PresentationStyle::Standard,
    };
    // Beside a progress the timer is the clock in the header, where the device has one. Elsewhere a timer that the
    // progress style does not draw is left out; a second progress or timer is left out by `walk`.
    let beside = environment.chronometer && matches!(template, liveupdate::Template::Progress);
    let chronometer = shown
        .timer
        .filter(|_| beside)
        .map(|timer| timer_style(Some(timer)));
    let mut degraded = shown.degraded;
    if shown.timer.is_some()
        && chronometer.is_none()
        && !matches!(template, liveupdate::Template::Timer)
    {
        note(&mut degraded, "timerText");
    }
    // What a tap on the notification opens, where the device opens URLs and liveupdate takes this one.
    let open_url = layout
        .url
        .as_ref()
        .filter(|_| environment.links)
        .and_then(|url| address(url, values));

    let large_icon = shown
        .files
        .first()
        .and_then(|file| (environment.image)(file));
    if shown.files.len() > 1 || (!shown.files.is_empty() && large_icon.is_none()) {
        note(&mut degraded, "image");
    }
    let samsung_now_bar = environment
        .samsung_now_bar
        .then(|| liveupdate::SamsungNowBar {
            primary: Some(title.clone()),
            secondary: body
                .clone()
                .or_else(|| chip.clone())
                .map(|text| cut(&text, MAX_STATUS)),
        });
    for part in [
        (layout.island.is_some(), "layout.island"),
        (styled, "layout.styles"),
        (layout.background.is_some(), "layout.background"),
        (layout.keyline.is_some(), "layout.keyline"),
        // Without it the notification opens the application, not a URL of the layout.
        (layout.url.is_some() && open_url.is_none(), "layout.url"),
    ] {
        if part.0 {
            degraded.push(part.1.to_owned());
        }
    }
    Projection {
        snapshot: liveupdate::Snapshot {
            revision: snapshot.revision,
            title,
            status,
            progress: None,
            timer: None,
            stale_at: snapshot.stale_at,
            presentation: Some(liveupdate::AndroidPresentation {
                short_critical_text: chip,
                large_icon: large_icon.map(liveupdate::IconSource::AppFile),
                actions: shown.actions,
                samsung_now_bar,
                chronometer,
                open_url,
                ..liveupdate::AndroidPresentation::new(style)
            }),
        },
        degraded,
    }
}

/// The shown nodes of a lock screen, as the projection takes them.
#[derive(Default)]
struct Shown {
    texts: Vec<String>,
    actions: Vec<liveupdate::NotificationAction>,
    files: Vec<String>,
    /// The first shown progress, clamped to 0..1.
    progress: Option<f64>,
    /// The first shown timer: its start, its end and whether it counts down.
    timer: Option<(f64, f64, bool)>,
    degraded: Vec<String>,
    /// Whether a locked screen shows the notification's content, and so what a node marks as private.
    public: bool,
    /// Whether the device opens URLs, and so whether a link becomes an action.
    links: bool,
    /// How many links of the tree came so far, hidden ones counted: a link's id is its place among them.
    links_seen: usize,
}

fn walk(node: &Node, values: &BTreeMap<String, Value>, shown: &mut Shown) {
    if !visible(node, values) {
        shown.links_seen += links_in(node);
        return;
    }
    // A notification changes without a transition. One that a locked screen hides shows nothing private there.
    if node.transition.is_some() {
        note(&mut shown.degraded, "transition");
    }
    if node.privacy.is_some() && shown.public {
        note(&mut shown.degraded, "privacy");
    }
    match &node.element {
        Element::Vstack { children, .. }
        | Element::Hstack { children, .. }
        | Element::Zstack { children, .. } => {
            children.iter().for_each(|child| walk(child, values, shown));
        }
        Element::Text { .. } => say(shown, text(node, values).unwrap_or_default()),
        Element::Progress { bind, .. } if shown.progress.is_none() => {
            shown.progress = Some(number(values.get(bind)).clamp(0.0, 1.0));
        }
        Element::TimerText {
            start,
            end,
            counts_down,
        } if shown.timer.is_none() => {
            shown.timer = Some((
                number(values.get(start)),
                number(values.get(end)),
                counts_down.unwrap_or(true),
            ));
        }
        Element::Image {
            source: ImageSource::File { file },
            ..
        } => shown.files.push(file.clone()),
        Element::Button { action_id, child } => {
            let (title, inside) = action_title(node, child, values, shown.public);
            let action =
                title.map(|title| liveupdate::NotificationAction::new(action_id.clone(), title));
            act(shown, action, &inside, "button");
        }
        Element::Link { url, child } => {
            shown.links_seen += 1;
            let id = format!("link:{}", shown.links_seen);
            let (title, inside) = action_title(node, child, values, shown.public);
            // Where the device opens no URL, or liveupdate does not take this one, the link is left out.
            let url = Some(url)
                .filter(|_| shown.links)
                .and_then(|url| address(url, values));
            let action = title
                .zip(url)
                .map(|(title, url)| liveupdate::NotificationAction {
                    open_url: Some(url),
                    ..liveupdate::NotificationAction::new(id, title)
                });
            act(shown, action, &inside, "link");
        }
        // A second progress or timer: the notification has one of each.
        element if drawn(element) => note(&mut shown.degraded, kind(element)),
        // What Android cannot draw. Decoration is left out without a word; what says in words what it is becomes a
        // text; the rest is named.
        element => match &node.accessibility {
            Some(accessibility) if accessibility.hidden == Some(true) => {}
            _ => match spoken(node, values) {
                Some(words) => say(shown, words),
                None => note(&mut shown.degraded, kind(element)),
            },
        },
    }
}

/// Takes `words` as a text of the notification: the first is its title, the second its status, a third is left out.
fn say(shown: &mut Shown, words: String) {
    match shown.texts.len() {
        0 | 1 => shown.texts.push(words),
        _ => note(&mut shown.degraded, "text"),
    }
}

/// Adds `action`, the action of a button or a link, and names what stands beside its title. liveupdate takes three
/// actions and each id once: an action without a title, without room or with an id that is taken is left out and
/// named as `kind`.
fn act(
    shown: &mut Shown,
    action: Option<liveupdate::NotificationAction>,
    inside: &[&'static str],
    kind: &str,
) {
    let room = shown.actions.len() < MAX_ACTIONS;
    match action {
        Some(action) if room && shown.actions.iter().all(|other| other.id != action.id) => {
            shown.actions.push(action);
            inside
                .iter()
                .for_each(|kind| note(&mut shown.degraded, kind));
        }
        _ => note(&mut shown.degraded, kind),
    }
}

/// The title of the action of `owner`, a button or a link, cut to liveupdate's length, and what the action leaves
/// out of `child`. The title is the first shown text inside. Without one it is what the first picture inside says
/// in words, and then what the owner says itself. Left out is every other shown node, a transition, and on a
/// notification that a locked screen shows (`public`) a privacy mark; decoration is left out without a word.
fn action_title(
    owner: &Node,
    child: &Node,
    values: &BTreeMap<String, Value>,
    public: bool,
) -> (Option<String>, Vec<&'static str>) {
    let mut first_text: Option<Option<String>> = None;
    // The first picture with words, and its place among what is left out.
    let mut worded_picture: Option<(usize, String)> = None;
    let mut inside = Vec::new();
    visit_shown(child, values, &mut |node| {
        if node.transition.is_some() {
            inside.push("transition");
        }
        if node.privacy.is_some() && public {
            inside.push("privacy");
        }
        match &node.element {
            Element::Vstack { .. } | Element::Hstack { .. } | Element::Zstack { .. } => {}
            Element::Text { .. } if first_text.is_none() => {
                first_text = Some(text(node, values).filter(|text| !blank(text)));
            }
            _ if node
                .accessibility
                .as_ref()
                .is_some_and(|accessibility| accessibility.hidden == Some(true)) => {}
            element => {
                if worded_picture.is_none()
                    && let Some(words) = spoken(node, values)
                {
                    worded_picture = Some((inside.len(), words));
                }
                inside.push(kind(element));
            }
        }
    });
    let title = match (first_text.flatten(), worded_picture) {
        (Some(text), _) => Some(text),
        (None, Some((place, words))) => {
            inside.remove(place);
            Some(words)
        }
        (None, None) => spoken(owner, values),
    };
    (title.map(|title| cut(&title, MAX_ACTION_TITLE)), inside)
}

/// What a node says in words to a screen reader, as a notification can show it: its label, its value, or both.
/// A member bound to a value reads as a text shows that value, and says nothing when the value is empty.
fn spoken(node: &Node, values: &BTreeMap<String, Value>) -> Option<String> {
    let accessibility = node.accessibility.as_ref()?;
    let words: Vec<String> = [&accessibility.label, &accessibility.value]
        .into_iter()
        .flatten()
        .filter_map(|member| resolved(member, values))
        .filter(|words| !blank(words))
        .collect();
    (!words.is_empty()).then(|| words.join(", "))
}

/// Whether a node has a label or a value and is no decoration: what [`spoken`] reads, values aside.
fn worded(node: &Node) -> bool {
    node.accessibility.as_ref().is_some_and(|accessibility| {
        accessibility.hidden != Some(true)
            && (accessibility.label.is_some() || accessibility.value.is_some())
    })
}

/// Whether a notification shows something of the element itself: `walk` has an arm for it.
fn drawn(element: &Element) -> bool {
    matches!(
        element,
        Element::Vstack { .. }
            | Element::Hstack { .. }
            | Element::Zstack { .. }
            | Element::Text { .. }
            | Element::Progress { .. }
            | Element::TimerText { .. }
            | Element::Button { .. }
            | Element::Link { .. }
            | Element::Image {
                source: ImageSource::File { .. },
                ..
            }
    )
}

/// A text member as it reads now: the literal, or the value it is bound to as a text shows it.
fn resolved(member: &Bindable<String>, values: &BTreeMap<String, Value>) -> Option<String> {
    match member {
        Bindable::Literal(text) => Some(text.clone()),
        Bindable::Bound(bound) => values.get(&bound.bind).map(display),
    }
}

/// The URL of a layout or of a link as it reads now, when it is one liveupdate opens: a lowercase scheme that it
/// does not refuse, and printable ASCII.
fn address(url: &Bindable<String>, values: &BTreeMap<String, Value>) -> Option<String> {
    let url = resolved(url, values)?;
    let opened = liveupdate::AndroidPresentation {
        open_url: Some(url.clone()),
        ..liveupdate::AndroidPresentation::new(liveupdate::PresentationStyle::Standard)
    };
    opened.validate().is_ok().then_some(url)
}

/// How many links the tree under `node` has, `node` included.
fn links_in(node: &Node) -> usize {
    let mut links = 0;
    visit_all(node, false, &mut |node, _| {
        if matches!(node.element, Element::Link { .. }) {
            links += 1;
        }
    });
    links
}

/// Whether a layout could use what only some devices offer: a URL or a link, and a timer beside a progress. A
/// layout that could not costs no question to the device.
pub(crate) fn could_use(layout: &Layout) -> (bool, bool) {
    let (mut link, mut progress, mut timer) = (layout.url.is_some(), false, false);
    visit_all(
        &layout.lock_screen,
        false,
        &mut |node, _| match &node.element {
            Element::Link { .. } => link = true,
            Element::Progress { .. } => progress = true,
            Element::TimerText { .. } => timer = true,
            _ => {}
        },
    );
    (link, progress && timer)
}

/// The timer of a Timer template when none shows: the first in the tree, with its keys' current values.
fn first_timer(node: &Node, values: &BTreeMap<String, Value>) -> Option<(f64, f64, bool)> {
    let mut found = None;
    visit_all(node, false, &mut |node, _| {
        if let (
            None,
            Element::TimerText {
                start,
                end,
                counts_down,
            },
        ) = (&found, &node.element)
        {
            found = Some((
                number(values.get(start)),
                number(values.get(end)),
                counts_down.unwrap_or(true),
            ));
        }
    });
    found
}

fn progress_style(fraction: Option<f64>) -> liveupdate::ProgressStyle {
    let fraction = fraction.unwrap_or(0.0);
    liveupdate::ProgressStyle {
        progress: (fraction * f64::from(PROGRESS_SCALE)).round() as u32,
        indeterminate: false,
        styled_by_progress: false,
        segments: vec![liveupdate::ProgressSegment {
            length: PROGRESS_SCALE,
            id: None,
            color: None,
            semantic_style: None,
        }],
        points: Vec::new(),
        start_icon: None,
        end_icon: None,
        tracker_icon: None,
    }
}

/// A timer whose end is not after its start ends a millisecond after it, as liveupdate needs.
fn timer_style(timer: Option<(f64, f64, bool)>) -> liveupdate::TimerStyle {
    let (start, end, counts_down) = timer.unwrap_or((0.0, 0.0, true));
    let started_at = millis(start);
    let ends_at = millis(end)
        .max(started_at.saturating_add(1))
        .min(liveupdate::MAX_TIMESTAMP);
    liveupdate::TimerStyle {
        timer: liveupdate::Timer {
            started_at: started_at.min(ends_at - 1),
            ends_at,
        },
        counts_down,
    }
}

fn millis(value: f64) -> u64 {
    if value.is_finite() && value > 0.0 {
        (value.floor() as u64).min(liveupdate::MAX_TIMESTAMP)
    } else {
        0
    }
}

fn visible(node: &Node, values: &BTreeMap<String, Value>) -> bool {
    node.r#if.as_ref().is_none_or(|key| truthy(values.get(key)))
        && node
            .unless
            .as_ref()
            .is_none_or(|key| !truthy(values.get(key)))
}

/// `true`, a number other than 0, or text that is not empty.
fn truthy(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(value)) => *value != 0.0 && !value.is_nan(),
        Some(Value::Text(value)) => !value.is_empty(),
        None => false,
    }
}

fn number(value: Option<&Value>) -> f64 {
    match value {
        Some(Value::Number(value)) => *value,
        _ => 0.0,
    }
}

fn text(node: &Node, values: &BTreeMap<String, Value>) -> Option<String> {
    match &node.element {
        Element::Text {
            value: Some(value), ..
        } => Some(value.clone()),
        Element::Text {
            bind: Some(key), ..
        } => values.get(key).map(display),
        _ => None,
    }
}

/// A value as ECMAScript's `String` prints it, which the other platforms' renderers follow.
pub(crate) fn display(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => js_number(*value),
    }
}

fn js_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    let magnitude = value.abs();
    if (1e-6..1e21).contains(&magnitude) {
        return value.to_string();
    }
    // ECMAScript writes the exponent with a sign: 1e+21, 1.5e-7.
    let written = format!("{value:e}");
    match written.split_once('e') {
        Some((mantissa, exponent)) if !exponent.starts_with('-') => {
            format!("{mantissa}e+{exponent}")
        }
        _ => written,
    }
}

/// Empty or whitespace only, by liveupdate's rule, which refuses such a text.
fn blank(text: &str) -> bool {
    text.chars().all(|character| {
        matches!(
            character as u32,
            0x0009..=0x000D | 0x2000..=0x200A | 0x0020 | 0x00A0 | 0x1680 | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000 | 0xFEFF
        )
    })
}

/// `text` cut to `maximum` code points, ending with `…` when it was longer.
fn cut(text: &str, maximum: usize) -> String {
    if text.chars().count() <= maximum {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(maximum - 1).collect();
    cut.push('…');
    cut
}

/// The app's name, or a word when the app has none a notification can show.
fn app_name(name: &str) -> &str {
    if blank(name) { "Live activity" } else { name }
}

fn note(degraded: &mut Vec<String>, kind: &str) {
    let feature = format!("layout.{kind}");
    if !degraded.contains(&feature) {
        degraded.push(feature);
    }
}

/// The JSON `type` of an element.
fn kind(element: &Element) -> &'static str {
    match element {
        Element::Vstack { .. } => "vstack",
        Element::Hstack { .. } => "hstack",
        Element::Zstack { .. } => "zstack",
        Element::Spacer { .. } => "spacer",
        Element::Divider { .. } => "divider",
        Element::Text { .. } => "text",
        Element::Symbol { .. } => "symbol",
        Element::Image { .. } => "image",
        Element::Path { .. } => "path",
        Element::Progress { .. } => "progress",
        Element::Gauge { .. } => "gauge",
        Element::TimerText { .. } => "timerText",
        Element::Button { .. } => "button",
        Element::Link { .. } => "link",
        Element::Custom { .. } => "custom",
    }
}

/// Every node in document order, shown or not; `in_button` says whether a button holds it.
fn visit_all(node: &Node, in_button: bool, visit: &mut impl FnMut(&Node, bool)) {
    visit(node, in_button);
    match &node.element {
        Element::Vstack { children, .. }
        | Element::Hstack { children, .. }
        | Element::Zstack { children, .. } => {
            children
                .iter()
                .for_each(|child| visit_all(child, in_button, visit));
        }
        Element::Button { child, .. } => visit_all(child, true, visit),
        _ => {}
    }
}

/// The shown nodes under `node`, `node` included, in document order.
fn visit_shown(node: &Node, values: &BTreeMap<String, Value>, visit: &mut impl FnMut(&Node)) {
    if !visible(node, values) {
        return;
    }
    visit(node);
    match &node.element {
        Element::Vstack { children, .. }
        | Element::Hstack { children, .. }
        | Element::Zstack { children, .. } => {
            children
                .iter()
                .for_each(|child| visit_shown(child, values, visit));
        }
        Element::Button { child, .. } => visit_shown(child, values, visit),
        _ => {}
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
