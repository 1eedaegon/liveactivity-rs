//! What code outside the crate builds on, so that it compiles against the next version: enums that may gain a
//! variant, and constructors that take what is required.

use liveactivity::{
    AndroidContent, AndroidOptions, AndroidPresentation, AndroidVisibility, BigTextStyle, EndInput,
    EndPolicy, ImageInput, Metric, MetricStyle, MetricValue, PresentationStyle, ProgressPoint,
    ProgressSegment, ProgressStyle, PublicVersion, SamsungNowBar, Snapshot, StartInput, Template,
    Timer, TimerStyle, UpdateInput, Validate,
};
use serde_json::json;
use std::{fs, path::Path};

/// The Rust files under `folder`, with their text.
fn sources(folder: &Path) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for entry in fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let text = fs::read_to_string(&path).unwrap();
            found.push((path.display().to_string(), text));
        }
    }
    found
}

#[test]
fn req_c_012_every_public_enum_may_gain_a_variant() {
    // REQ-C-012
    let mut enums = 0;
    for (file, text) in sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src")) {
        let lines: Vec<&str> = text.lines().collect();
        for (at, line) in lines.iter().enumerate() {
            if !line.starts_with("pub enum ") {
                continue;
            }
            enums += 1;
            // The attributes and the comment of the enum are the lines above it, up to an empty one.
            let above = lines[..at].iter().rev().take_while(|line| !line.is_empty());
            let marked = above
                .into_iter()
                .any(|line| line.trim() == "#[non_exhaustive]");
            assert!(marked, "{file}: `{line}` is not #[non_exhaustive]");
        }
    }
    assert!(enums >= 12, "the crate's enums were found: {enums}");

    // And outside the crate a match has its last arm, with every variant named before it.
    #[deny(unreachable_patterns)]
    fn named(template: Template, policy: &EndPolicy) -> (&'static str, &'static str) {
        let template = match template {
            Template::Progress => "progress",
            Template::Timer => "timer",
            _ => "later",
        };
        let policy = match policy {
            EndPolicy::Immediate {} => "immediate",
            EndPolicy::Default {} => "default",
            EndPolicy::After { .. } => "after",
            _ => "later",
        };
        (template, policy)
    }
    assert_eq!(
        named(Template::Timer, &EndPolicy::After { at: 1 }),
        ("timer", "after")
    );
}

#[test]
fn req_c_012_a_constructor_builds_what_the_literal_was() {
    // REQ-C-012
    assert_eq!(
        Timer::new(1, 2),
        Timer {
            started_at: 1,
            ends_at: 2
        }
    );
    let progress = Snapshot::progress(3, "Ride", "On the way", 0.5);
    assert_eq!(
        serde_json::to_value(&progress).unwrap(),
        json!({ "revision": 3, "title": "Ride", "status": "On the way", "progress": 0.5 })
    );
    let timer = Snapshot::timer(0, "Nap", "Asleep", 10, 20);
    assert_eq!(
        serde_json::to_value(&timer).unwrap(),
        json!({ "revision": 0, "title": "Nap", "status": "Asleep",
            "timer": { "startedAt": 10, "endsAt": 20 } })
    );

    // A start takes its template from its snapshot, as the two named constructors do.
    let start = StartInput::new("ride:7", Snapshot::progress(0, "Ride", "On the way", 0.5));
    assert_eq!(
        start,
        StartInput::progress("ride:7", "Ride", "On the way", 0.5)
    );
    assert_eq!(start.validate(), Ok(()));
    assert_eq!(
        StartInput::new("nap:1", timer.clone()).template,
        Template::Timer
    );
    // A snapshot that is of neither template is built, and refused where every request is checked.
    let neither = Snapshot {
        progress: None,
        ..Snapshot::progress(0, "Ride", "On the way", 0.5)
    };
    assert_eq!(
        StartInput::new("ride:7", neither)
            .validate()
            .unwrap_err()
            .code,
        "INVALID_INPUT"
    );
    assert_eq!(
        serde_json::to_value(UpdateInput::new("id", timer)).unwrap(),
        json!({ "id": "id", "snapshot": { "revision": 0, "title": "Nap", "status": "Asleep",
            "timer": { "startedAt": 10, "endsAt": 20 } } })
    );
    assert_eq!(
        serde_json::to_value(EndInput::after("id", 9)).unwrap(),
        json!({ "id": "id", "policy": { "kind": "after", "at": 9 } })
    );

    let content = AndroidContent::new(
        "Ride",
        "On the way",
        AndroidPresentation::new(PresentationStyle::Standard),
    );
    assert_eq!(
        serde_json::to_value(&content).unwrap(),
        json!({ "title": "Ride", "status": "On the way",
            "presentation": { "style": { "kind": "standard" } } })
    );
    assert_eq!(
        serde_json::to_value(ImageInput::new("cat.png", vec![1, 2])).unwrap(),
        json!({ "name": "cat.png", "png": [1, 2] })
    );
    // The options of a start: what is required, and one member more by its own method.
    assert_eq!(
        serde_json::to_value(
            AndroidOptions::new(AndroidVisibility::Private).with_wear_ongoing_activity(false)
        )
        .unwrap(),
        json!({ "visibility": "private", "wearOngoingActivity": false })
    );
    assert_eq!(
        serde_json::to_value(AndroidOptions::new(AndroidVisibility::Private)).unwrap(),
        json!({ "visibility": "private" })
    );
}

#[test]
fn req_c_012_the_reexported_vocabulary_is_built_on_its_constructors() {
    // REQ-C-012
    // liveupdate's constructors, through this crate: a member that liveupdate adds later is left at its default.
    let track = ProgressStyle {
        points: vec![ProgressPoint::new(60)],
        styled_by_progress: true,
        ..ProgressStyle::new(40, vec![ProgressSegment::new(60), ProgressSegment::new(40)])
    };
    let presentation = AndroidPresentation {
        public_version: Some(PublicVersion::new("Ride", "On the way")),
        samsung_now_bar: Some(SamsungNowBar::default()),
        chronometer: Some(TimerStyle::new(
            liveactivity::liveupdate::Timer::new(10, 20),
            true,
        )),
        ..AndroidPresentation::new(PresentationStyle::Progress(track))
    };
    // A presentation carries a watch's Ongoing Activity unless it says otherwise, and then says nothing of it.
    assert!(presentation.wear_ongoing_activity);
    let written = serde_json::to_value(&presentation).unwrap();
    assert!(written.get("wearOngoingActivity").is_none());
    assert_eq!(written["publicVersion"]["title"], "Ride");

    let metrics = MetricStyle::new(vec![Metric::new(
        "Steps",
        MetricValue::Integer {
            value: 42,
            unit: None,
        },
    )]);
    let styles = [
        PresentationStyle::Standard,
        PresentationStyle::BigText(BigTextStyle::new("A long text")),
        PresentationStyle::Metric(metrics),
    ];
    // Its enums are matched with a last arm.
    let kinds: Vec<&str> = styles
        .iter()
        .map(|style| match style {
            PresentationStyle::Standard => "standard",
            PresentationStyle::BigText(_) => "bigText",
            _ => "another",
        })
        .collect();
    assert_eq!(kinds, ["standard", "bigText", "another"]);
}

#[test]
fn req_a_028_a_progress_style_from_its_constructor_is_styled_by_its_progress() {
    // REQ-A-028
    let built = ProgressStyle::new(40, vec![ProgressSegment::new(100)]);
    assert!(built.styled_by_progress);
    assert_eq!(
        serde_json::to_value(&built).unwrap(),
        json!({ "progress": 40, "styledByProgress": true, "segments": [{ "length": 100 }] })
    );
    // The member is read as it was: a style that leaves it out is not styled by its progress.
    let read: ProgressStyle =
        serde_json::from_value(json!({ "progress": 40, "segments": [{ "length": 100 }] })).unwrap();
    assert!(!read.styled_by_progress);
    // And what the constructor built can still be turned off.
    let plain = ProgressStyle {
        styled_by_progress: false,
        ..built
    };
    assert!(
        serde_json::to_value(&plain)
            .unwrap()
            .get("styledByProgress")
            .is_none()
    );
}
