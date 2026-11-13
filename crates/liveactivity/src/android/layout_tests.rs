use super::*;
use serde_json::{Value as Json, json};

fn layout(lock_screen: Json) -> Layout {
    serde_json::from_value(json!({ "lockScreen": lock_screen })).unwrap()
}

fn snapshot(values: Json) -> LayoutSnapshot {
    serde_json::from_value(json!({ "revision": 3, "values": values })).unwrap()
}

fn no_image(_: &str) -> Option<String> {
    None
}

/// The projection with the app name Mochi and no Now Bar, checked as liveupdate checks a v2 snapshot.
fn projected(layout: &Layout, values: Json) -> Projection {
    projected_with(
        layout,
        values,
        &Environment {
            app_name: "Mochi",
            samsung_now_bar: false,
            image: &no_image,
            public: false,
            links: false,
            chronometer: false,
        },
    )
}

fn projected_with(layout: &Layout, values: Json, environment: &Environment<'_>) -> Projection {
    let projection = project(
        layout,
        false,
        &snapshot(values),
        template(layout),
        environment,
    );
    projection
        .snapshot
        .validate()
        .expect("liveupdate takes the projection");
    projection
}

fn presentation(projection: &Projection) -> &liveupdate::AndroidPresentation {
    projection.snapshot.presentation.as_ref().unwrap()
}

fn text(value: &str) -> Json {
    json!({ "type": "text", "value": value })
}
#[test]
fn the_template_comes_from_the_tree_and_stays_for_the_generation() {
    for (tree, expected) in [
        (
            json!({ "type": "vstack", "children": [text("a"), { "type": "progress", "bind": "p" }] }),
            liveupdate::Template::Progress,
        ),
        (
            json!({ "type": "vstack", "children": [{ "type": "timerText", "start": "s", "end": "e" }, { "type": "progress", "bind": "p" }] }),
            liveupdate::Template::Progress,
        ),
        (
            json!({ "type": "hstack", "children": [text("a"), { "type": "timerText", "start": "s", "end": "e" }] }),
            liveupdate::Template::Timer,
        ),
        (
            json!({ "type": "vstack", "children": [text("a"), { "type": "zstack", "children": [text("b")] }] }),
            liveupdate::Template::BigText,
        ),
        (text("a"), liveupdate::Template::Standard),
        // A button's text labels its action and is no text of the notification.
        (
            json!({ "type": "vstack", "children": [text("a"), { "type": "button", "actionId": "go", "child": text("Go") }] }),
            liveupdate::Template::Standard,
        ),
        // Conditions do not count.
        (
            json!({ "type": "vstack", "children": [text("a"), { "type": "progress", "bind": "p", "if": "late" }] }),
            liveupdate::Template::Progress,
        ),
        (
            json!({ "type": "vstack", "if": "never", "children": [text("a"), text("b")] }),
            liveupdate::Template::BigText,
        ),
    ] {
        assert_eq!(template(&layout(tree.clone())), expected, "{tree}");
    }
}
#[test]
fn a_hidden_progress_or_timer_keeps_the_template() {
    let tree = layout(json!({ "type": "vstack", "children": [
        text("Snack"),
        { "type": "progress", "bind": "done", "if": "late" },
    ] }));
    let hidden = projected(&tree, json!({ "late": false, "done": 0.65 }));
    let shown = projected(&tree, json!({ "late": true, "done": 0.65 }));
    for (projection, progress) in [(&hidden, 0), (&shown, 650)] {
        match &presentation(projection).style {
            liveupdate::PresentationStyle::Progress(style) => {
                assert_eq!(style.progress, progress);
                assert_eq!(style.segments.len(), 1);
                assert_eq!(style.segments[0].length, 1000);
            }
            style => panic!("{style:?}"),
        }
    }
    assert_eq!(presentation(&hidden).short_critical_text, None);

    let timer = layout(json!({ "type": "vstack", "children": [
        text("Nap"),
        { "type": "timerText", "start": "from", "end": "to", "countsDown": false, "unless": "awake" },
    ] }));
    let projection = projected(
        &timer,
        json!({ "awake": true, "from": 1_000.0, "to": 61_000.0 }),
    );
    assert_eq!(
        presentation(&projection).style,
        liveupdate::PresentationStyle::Timer(liveupdate::TimerStyle {
            timer: liveupdate::Timer {
                started_at: 1_000,
                ends_at: 61_000
            },
            counts_down: false,
        })
    );
    // An end that is not after the start ends a millisecond after it.
    let projection = projected(
        &timer,
        json!({ "awake": false, "from": 5_000.0, "to": 5_000.0 }),
    );
    assert!(matches!(&presentation(&projection).style,
        liveupdate::PresentationStyle::Timer(style) if style.timer == liveupdate::Timer { started_at: 5_000, ends_at: 5_001 }));
}
#[test]
fn shown_texts_give_the_title_and_the_status_in_document_order() {
    let tree = layout(json!({ "type": "vstack", "children": [
        { "type": "hstack", "children": [{ "type": "symbol", "name": "cat.fill" }, { "type": "text", "bind": "title" }] },
        { "type": "text", "value": "hidden", "if": "never" },
        { "type": "zstack", "children": [{ "type": "text", "bind": "status" }] },
    ] }));
    let projection = projected(
        &tree,
        json!({ "title": "Mochi's snack", "status": "On the way", "never": false }),
    );
    assert_eq!(projection.snapshot.title, "Mochi's snack");
    assert_eq!(projection.snapshot.status, "On the way");
    assert_eq!(
        presentation(&projection).style,
        liveupdate::PresentationStyle::BigText(liveupdate::BigTextStyle {
            text: "On the way".into(),
            summary: None
        })
    );
}
#[test]
fn texts_are_cut_to_the_limits_and_blank_ones_fall_back() {
    let long = "가".repeat(130);
    let projection = projected(
        &layout(json!({ "type": "vstack", "children": [text(&long), text(&"b".repeat(300))] })),
        json!({}),
    );
    assert_eq!(projection.snapshot.title.chars().count(), 120);
    assert!(projection.snapshot.title.ends_with('…'));
    assert_eq!(projection.snapshot.status.chars().count(), 240);

    // No text, or a blank one: the app's name. No second text: the chip, then the title.
    let projection = projected(
        &layout(
            json!({ "type": "vstack", "children": [text("  "), { "type": "progress", "bind": "p" }] }),
        ),
        json!({ "p": 0.5 }),
    );
    assert_eq!(projection.snapshot.title, "Mochi");
    assert_eq!(projection.snapshot.status, "50%");
    let projection = projected(&layout(json!({ "type": "spacer" })), json!({}));
    assert_eq!(
        (
            projection.snapshot.title.as_str(),
            projection.snapshot.status.as_str()
        ),
        ("Mochi", "Mochi")
    );
}
#[test]
fn bound_values_print_as_ecmascript_prints_them() {
    for (value, printed) in [
        (json!(3), "3"),
        (json!(2.5), "2.5"),
        (json!(-0.0), "0"),
        (json!(true), "true"),
        (json!(1e21), "1e+21"),
        (json!(1.5e-7), "1.5e-7"),
        (json!(123456789012.0), "123456789012"),
        (json!("Mochi"), "Mochi"),
    ] {
        let projection = projected(
            &layout(json!({ "type": "text", "bind": "v" })),
            json!({ "v": value }),
        );
        assert_eq!(projection.snapshot.title, printed, "{value}");
    }
}
#[test]
fn shown_buttons_with_a_text_become_up_to_three_actions() {
    let button =
        |id: &str, child: Json| json!({ "type": "button", "actionId": id, "child": child });
    let tree = layout(json!({ "type": "vstack", "children": [
        text("Mochi wants attention"),
        { "type": "hstack", "children": [
            button("pet", json!({ "type": "hstack", "children": [{ "type": "symbol", "name": "hand.wave" }, text("Pet Mochi")] })),
            button("feed", text("Feed")),
            button("quiet", json!({ "type": "symbol", "name": "moon" })),
            button("hidden", json!({ "type": "text", "value": "Hidden", "if": "never" })),
            button("play", text(&"p".repeat(70))),
            button("done", text("All done")),
        ] },
    ] }));
    let projection = projected(&tree, json!({ "never": false }));
    let actions = &presentation(&projection).actions;
    assert_eq!(
        actions
            .iter()
            .map(|action| action.id.as_str())
            .collect::<Vec<_>>(),
        ["pet", "feed", "play"]
    );
    assert_eq!(actions[0].title, "Pet Mochi");
    assert_eq!(actions[2].title.chars().count(), 64);
    assert!(
        actions
            .iter()
            .all(|action| action.icon.is_none() && !action.authentication_required)
    );
    // A button's text is not the status.
    assert_eq!(projection.snapshot.status, "Mochi wants attention");
    assert_eq!(projection.degraded, ["layout.symbol", "layout.button"]);
}
#[test]
fn the_first_shown_file_image_that_fits_is_the_large_icon() {
    let image = |name: &str| (name == "badge.png").then(|| format!("liveactivity/images/{name}"));
    let environment = Environment {
        app_name: "Mochi",
        samsung_now_bar: false,
        image: &image,
        public: false,
        links: false,
        chronometer: false,
    };
    let picture = |source: Json| json!({ "type": "image", "source": source });
    let tree = layout(json!({ "type": "vstack", "children": [
        text("Mochi"),
        { "type": "image", "source": { "file": "hidden.png" }, "if": "never" },
        picture(json!({ "file": "badge.png" })),
        picture(json!({ "file": "second.png" })),
    ] }));
    let projection = projected_with(&tree, json!({ "never": false }), &environment);
    assert_eq!(
        presentation(&projection).large_icon,
        Some(liveupdate::IconSource::AppFile(
            "liveactivity/images/badge.png".into()
        ))
    );
    assert_eq!(projection.degraded, ["layout.image"]);

    // A picture liveupdate cannot draw (missing, too large) gives no icon; an asset is never one.
    for source in [
        json!({ "file": "missing.png" }),
        json!({ "asset": "badge" }),
    ] {
        let tree = layout(
            json!({ "type": "vstack", "children": [text("Mochi"), picture(source.clone())] }),
        );
        let projection = projected_with(&tree, json!({}), &environment);
        assert_eq!(presentation(&projection).large_icon, None, "{source}");
        assert_eq!(projection.degraded, ["layout.image"], "{source}");
    }
}
#[test]
fn the_chip_is_the_progress_percentage_without_a_timer() {
    let progress = json!({ "type": "progress", "bind": "done" });
    let projection = projected(
        &layout(json!({ "type": "vstack", "children": [text("Snack"), progress.clone()] })),
        json!({ "done": 0.654 }),
    );
    assert_eq!(
        presentation(&projection).short_critical_text.as_deref(),
        Some("65%")
    );
    let projection = projected(
        &layout(json!({ "type": "vstack", "children": [text("Snack"), progress.clone()] })),
        json!({ "done": 4.0 }),
    );
    assert_eq!(
        presentation(&projection).short_critical_text.as_deref(),
        Some("100%")
    );

    // A shown timer: no chip, and the Progress template leaves the timer out.
    let tree = layout(
        json!({ "type": "vstack", "children": [text("Snack"), progress, { "type": "timerText", "start": "s", "end": "e" }] }),
    );
    let projection = projected(&tree, json!({ "done": 0.5, "s": 0.0, "e": 60_000.0 }));
    assert_eq!(presentation(&projection).short_critical_text, None);
    assert_eq!(projection.degraded, ["layout.timerText"]);
}
#[test]
fn the_now_bar_gets_the_title_and_the_status_when_configured() {
    let environment = Environment {
        app_name: "Mochi",
        samsung_now_bar: true,
        image: &no_image,
        public: false,
        links: false,
        chronometer: false,
    };
    let tree = layout(json!({ "type": "vstack", "children": [text("Snack"), text("On the way")] }));
    let projection = projected_with(&tree, json!({}), &environment);
    assert_eq!(
        presentation(&projection).samsung_now_bar,
        Some(liveupdate::SamsungNowBar {
            primary: Some("Snack".into()),
            secondary: Some("On the way".into())
        })
    );
    // Without a second text the chip is the secondary line, and without a chip there is none.
    let tree = layout(
        json!({ "type": "vstack", "children": [text("Snack"), { "type": "progress", "bind": "p" }] }),
    );
    let projection = projected_with(&tree, json!({ "p": 0.3 }), &environment);
    assert_eq!(
        presentation(&projection)
            .samsung_now_bar
            .as_ref()
            .unwrap()
            .secondary
            .as_deref(),
        Some("30%")
    );
    let projection = projected_with(&layout(text("Snack")), json!({}), &environment);
    assert_eq!(
        presentation(&projection)
            .samsung_now_bar
            .as_ref()
            .unwrap()
            .secondary,
        None
    );

    assert_eq!(
        presentation(&projected(&tree, json!({ "p": 0.3 }))).samsung_now_bar,
        None
    );
}
#[test]
fn degraded_features_follow_liveupdate_then_the_nodes_then_the_layout() {
    let request: Layout = serde_json::from_value(json!({
        "lockScreen": { "type": "vstack", "children": [
            { "type": "symbol", "name": "cat.fill" },
            { "type": "spacer" },
            text("a"), text("b"), text("c"), text("d"),
            { "type": "progress", "bind": "p" },
            { "type": "progress", "bind": "p" },
            { "type": "button", "actionId": "go", "child": { "type": "symbol", "name": "moon" } },
            { "type": "gauge", "bind": "p", "if": "never" },
        ] },
        "island": { "minimal": text("m") },
        "background": "black",
    }))
    .unwrap();
    let projection = project(
        &request,
        true,
        &snapshot(json!({ "p": 0.5, "never": false })),
        template(&request),
        &Environment {
            app_name: "Mochi",
            samsung_now_bar: false,
            image: &no_image,
            public: false,
            links: false,
            chronometer: false,
        },
    );
    projection.snapshot.validate().unwrap();
    assert_eq!(
        projection.degraded,
        [
            "layout.symbol",
            "layout.spacer",
            "layout.text",
            "layout.progress",
            "layout.button",
            "layout.island",
            "layout.styles",
            "layout.background",
        ]
    );
}
#[test]
fn an_action_id_shown_twice_becomes_one_action() {
    let button =
        |id: &str, label: &str| json!({ "type": "button", "actionId": id, "child": text(label) });
    let tree = layout(
        json!({ "type": "vstack", "children": [text("Snack"), button("stop", "Stop"), button("stop", "Stop again"), button("go", "Go")] }),
    );
    let projection = projected(&tree, json!({}));
    let actions = &presentation(&projection).actions;
    assert_eq!(
        actions
            .iter()
            .map(|action| (action.id.as_str(), action.title.as_str()))
            .collect::<Vec<_>>(),
        [("stop", "Stop"), ("go", "Go")]
    );
    assert_eq!(projection.degraded, ["layout.button"]);
}
#[test]
fn liveupdates_whitespace_counts_as_blank() {
    let feff = "\u{FEFF}\u{3000}";
    let tree = layout(
        json!({ "type": "vstack", "children": [text(feff), text(feff), { "type": "button", "actionId": "go", "child": text(feff) }] }),
    );
    let projection = projected(&tree, json!({}));
    assert_eq!(
        (
            projection.snapshot.title.as_str(),
            projection.snapshot.status.as_str()
        ),
        ("Mochi", "Mochi")
    );
    assert!(presentation(&projection).actions.is_empty());
    let nameless = Environment {
        app_name: " ",
        samsung_now_bar: false,
        image: &no_image,
        public: false,
        links: false,
        chronometer: false,
    };
    assert_eq!(
        projected_with(&layout(text(feff)), json!({}), &nameless)
            .snapshot
            .title,
        "Live activity"
    );
}

#[test]
fn req_a_004_the_projection_names_what_android_leaves_out_of_the_ios_surface() {
    // REQ-A-004
    let tree: Layout = serde_json::from_value(json!({
        "url": "myapp://rides/7",
        "lockScreen": { "type": "vstack", "children": [
            { "type": "text", "value": "Ride", "accessibility": { "label": "Your ride" } },
            { "type": "text", "bind": "eta", "privacy": "sensitive", "transition": "moveTop" },
            { "type": "link", "url": "myapp://rides/7/map", "child": text("Map") },
            { "type": "text", "value": "Later", "if": "never", "privacy": "sensitive", "transition": "slide" },
        ] },
    }))
    .unwrap();
    let values = json!({ "eta": "4 min", "never": false });

    // A notification that a locked screen hides shows nothing of a sensitive node there: the mark is kept.
    let hidden = projected(&tree, values.clone());
    assert_eq!(
        hidden.degraded,
        ["layout.transition", "layout.link", "layout.url"]
    );
    // The label of a link is no text of the notification and no action; a label for a screen reader is no text.
    assert_eq!(
        (
            hidden.snapshot.title.as_str(),
            hidden.snapshot.status.as_str()
        ),
        ("Ride", "4 min")
    );
    assert!(presentation(&hidden).actions.is_empty());
    assert_eq!(template(&tree), liveupdate::Template::BigText);

    // One that a locked screen shows whole shows the sensitive node too.
    let public = Environment {
        app_name: "Mochi",
        samsung_now_bar: false,
        image: &no_image,
        public: true,
        links: false,
        chronometer: false,
    };
    assert_eq!(
        projected_with(&tree, values, &public).degraded,
        [
            "layout.transition",
            "layout.privacy",
            "layout.link",
            "layout.url"
        ]
    );

    // What labels an action is shown too: a private text inside a button, and how it comes and goes.
    let button = layout(json!({ "type": "button", "actionId": "call", "child":
        { "type": "text", "bind": "driver", "privacy": "sensitive", "transition": "opacity" } }));
    let named = projected_with(&button, json!({ "driver": "Mina" }), &public);
    assert_eq!(presentation(&named).actions[0].title, "Mina");
    assert_eq!(named.degraded, ["layout.transition", "layout.privacy"]);
    assert_eq!(
        projected(&button, json!({ "driver": "Mina" })).degraded,
        ["layout.transition"]
    );

    // A node that is not shown leaves nothing out, and neither does an accessibility member.
    let plain = layout(json!({ "type": "vstack", "children": [
        { "type": "text", "value": "Ride", "accessibility": { "hidden": true } },
        { "type": "link", "url": "myapp://x", "child": text("Map"), "if": "never" },
    ] }));
    assert!(
        projected_with(&plain, json!({ "never": false }), &public)
            .degraded
            .is_empty()
    );
}

/// A device that offers what a projection can use beyond the styles.
fn offering() -> Environment<'static> {
    Environment {
        app_name: "Mochi",
        samsung_now_bar: false,
        image: &no_image,
        public: false,
        links: true,
        chronometer: true,
    }
}

#[test]
fn req_a_008_links_of_a_layout_become_what_the_notification_opens() {
    // REQ-A-008
    let tree: Layout = serde_json::from_value(json!({
        "url": { "bind": "target" },
        "lockScreen": { "type": "vstack", "children": [
            text("Ride"),
            { "type": "link", "url": "myapp://rides/7/map", "child": text("Map") },
            { "type": "button", "actionId": "call", "child": text("Call") },
            { "type": "link", "url": { "bind": "receipt" }, "child": { "type": "hstack", "children": [
                { "type": "symbol", "name": "doc" }, text("Receipt") ] } },
            { "type": "link", "url": "myapp://rides/7/help", "child": text("Help") },
            { "type": "link", "url": "myapp://rides/7/later", "child": text("Later"), "if": "never" },
        ] },
    }))
    .unwrap();
    let values = json!({
        "target": "myapp://rides/7", "receipt": "https://example.com/r/7", "never": false,
    });

    // The layout's URL is what a tap opens, and a link is an action that opens its URL, in its place among the
    // buttons. Its id counts the links of the tree, so that it is the same with every snapshot.
    let projection = projected_with(&tree, values.clone(), &offering());
    let shown = presentation(&projection);
    assert_eq!(shown.open_url.as_deref(), Some("myapp://rides/7"));
    let actions: Vec<_> = shown
        .actions
        .iter()
        .map(|action| {
            (
                action.id.as_str(),
                action.title.as_str(),
                action.open_url.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        actions,
        [
            ("link:1", "Map", Some("myapp://rides/7/map")),
            ("call", "Call", None),
            ("link:2", "Receipt", Some("https://example.com/r/7")),
        ]
    );
    // What stands beside a link's label is left out, and so is a link beyond the three actions.
    assert_eq!(projection.degraded, ["layout.symbol", "layout.link"]);
    // A link is no text of the notification.
    assert_eq!(projection.snapshot.title, "Ride");
    assert_eq!(template(&tree), liveupdate::Template::Standard);

    // A URL that liveupdate does not take is left out and named: a scheme with a capital, a scheme it refuses.
    let odd: Layout = serde_json::from_value(json!({
        "url": "MyApp://rides/7",
        "lockScreen": { "type": "link", "url": "file:///rides/7", "child": text("Map") },
    }))
    .unwrap();
    let projection = projected_with(&odd, json!({}), &offering());
    assert_eq!(presentation(&projection).open_url, None);
    assert!(presentation(&projection).actions.is_empty());
    assert_eq!(projection.degraded, ["layout.link", "layout.url"]);

    // A device without links shows the buttons alone, as before.
    let projection = projected(&tree, values);
    assert_eq!(presentation(&projection).open_url, None);
    assert_eq!(presentation(&projection).actions.len(), 1);
    assert_eq!(projection.degraded, ["layout.link", "layout.url"]);

    // A link keeps its number while one before it is hidden.
    let second = layout(json!({ "type": "vstack", "children": [
        { "type": "vstack", "if": "never", "children": [
            { "type": "link", "url": "myapp://first", "child": text("First") } ] },
        { "type": "link", "url": "myapp://second", "child": text("Second") },
    ] }));
    let projection = projected_with(&second, json!({ "never": false }), &offering());
    assert_eq!(presentation(&projection).actions[0].id, "link:2");
}

#[test]
fn req_a_009_a_timer_beside_a_progress_is_the_header_clock() {
    // REQ-A-009
    let tree = layout(json!({ "type": "vstack", "children": [
        text("Ride"),
        { "type": "progress", "bind": "done" },
        { "type": "timerText", "start": "from", "end": "until", "countsDown": false },
    ] }));
    let values = json!({ "done": 0.4, "from": 1000, "until": 5000 });

    let projection = projected_with(&tree, values.clone(), &offering());
    let shown = presentation(&projection);
    assert!(matches!(
        shown.style,
        liveupdate::PresentationStyle::Progress(_)
    ));
    assert_eq!(
        shown.chronometer,
        Some(liveupdate::TimerStyle {
            timer: liveupdate::Timer {
                started_at: 1000,
                ends_at: 5000,
            },
            counts_down: false,
        })
    );
    assert!(projection.degraded.is_empty());

    // Elsewhere the timer is left out and named, as before.
    let projection = projected(&tree, values);
    assert_eq!(presentation(&projection).chronometer, None);
    assert_eq!(projection.degraded, ["layout.timerText"]);

    // A layout whose style is the timer has its clock there, and no second one.
    let alone = layout(json!({ "type": "vstack", "children": [
        text("Tea"),
        { "type": "timerText", "start": "from", "end": "until" },
    ] }));
    let projection = projected_with(&alone, json!({ "from": 1000, "until": 5000 }), &offering());
    assert!(matches!(
        presentation(&projection).style,
        liveupdate::PresentationStyle::Timer(_)
    ));
    assert_eq!(presentation(&projection).chronometer, None);
}

#[test]
fn req_a_010_labels_are_the_text_of_what_android_cannot_draw() {
    // REQ-A-010
    let tree = layout(json!({ "type": "vstack", "children": [
        { "type": "symbol", "name": "car.fill",
          "accessibility": { "label": "Driver", "value": { "bind": "driver" } } },
        { "type": "gauge", "bind": "battery", "accessibility": { "value": "40 percent" } },
        { "type": "divider", "accessibility": { "hidden": true } },
        { "type": "button", "actionId": "call", "child": { "type": "hstack", "children": [
            { "type": "path", "d": "M0 0L1 1", "box": [1, 1] },
            { "type": "symbol", "name": "phone.fill", "accessibility": { "label": "Call" } },
            { "type": "gauge", "bind": "battery" } ] } },
        { "type": "button", "actionId": "stop", "accessibility": { "label": "Stop the ride" },
          "child": { "type": "hstack", "children": [
            { "type": "spacer" }, { "type": "divider", "accessibility": { "hidden": true } } ] } },
        { "type": "path", "d": "M0 0L1 1", "box": [1, 1] },
    ] }));
    let projection = projected(&tree, json!({ "driver": "Mina", "battery": 0.4 }));

    // What says in words what it is becomes a text, in its place among the texts.
    assert_eq!(projection.snapshot.title, "Driver, Mina");
    assert_eq!(projection.snapshot.status, "40 percent");
    assert_eq!(template(&tree), liveupdate::Template::BigText);
    // A button whose label is a picture is titled by the picture's label, or by its own.
    let titles: Vec<_> = presentation(&projection)
        .actions
        .iter()
        .map(|action| (action.id.as_str(), action.title.as_str()))
        .collect();
    assert_eq!(titles, [("call", "Call"), ("stop", "Stop the ride")]);
    // Decoration is left out without a word, in the tree and inside a button, and so is the picture that titles
    // a button. What has no words is named: what stands beside the picture that titles, the spacer beside a
    // button's own label, and the path.
    assert_eq!(
        projection.degraded,
        ["layout.path", "layout.gauge", "layout.spacer"]
    );

    // The notification has one progress: a second one is named, whatever it says in words.
    let twice = layout(json!({ "type": "vstack", "children": [
        { "type": "progress", "bind": "done" },
        { "type": "progress", "bind": "done", "accessibility": { "label": "Again" } },
        { "type": "progress", "bind": "done", "accessibility": { "hidden": true } },
    ] }));
    let projection = projected(&twice, json!({ "done": 0.4 }));
    assert_eq!(projection.snapshot.title, "Mochi");
    assert_eq!(projection.degraded, ["layout.progress"]);

    // A label bound to a value that is empty says nothing: the node is named, as before.
    let silent = layout(json!({ "type": "symbol", "name": "car.fill",
        "accessibility": { "label": { "bind": "driver" } } }));
    let projection = projected(&silent, json!({ "driver": "" }));
    assert_eq!(projection.snapshot.title, "Mochi");
    assert_eq!(projection.degraded, ["layout.symbol"]);
}

#[test]
fn req_a_013_a_toggle_is_titled_by_its_label_and_its_value() {
    // REQ-A-013
    let toggle = |id: &str, bind: &str, child: Json| json!({ "type": "toggle", "actionId": id, "bind": bind, "child": child });
    let titles = |projection: &Projection| -> Vec<(String, String)> {
        presentation(projection)
            .actions
            .iter()
            .map(|action| (action.id.clone(), action.title.clone()))
            .collect()
    };
    let pair = |id: &str, title: &str| (id.to_owned(), title.to_owned());

    // The label is the title, and the mark says that the value is on.
    let tree = layout(json!({ "type": "vstack", "children": [
        text("Ride"),
        toggle("mute", "muted", text("Mute")),
        toggle("share", "shared", json!({ "type": "text", "bind": "label" })),
    ] }));
    let off = projected(
        &tree,
        json!({ "muted": false, "shared": false, "label": "Share" }),
    );
    assert_eq!(titles(&off), [pair("mute", "Mute"), pair("share", "Share")]);
    assert_eq!(off.degraded, Vec::<String>::new());
    let on = projected(
        &tree,
        json!({ "muted": true, "shared": false, "label": "Share" }),
    );
    assert_eq!(
        titles(&on),
        [pair("mute", "✓ Mute"), pair("share", "Share")]
    );
    // Its label is no text of the notification, as a button's is none.
    assert_eq!(on.snapshot.title, "Ride");
    assert_eq!(template(&tree), liveupdate::Template::Standard);

    // A toggle takes a place among the three actions as a button does, and one without a place is named.
    let button = |id: &str| json!({ "type": "button", "actionId": id, "child": text(id) });
    let crowded = layout(json!({ "type": "hstack", "children": [
        button("one"), toggle("two", "muted", text("Two")), button("three"), toggle("four", "muted", text("Four")),
    ] }));
    let projection = projected(&crowded, json!({ "muted": true }));
    assert_eq!(
        titles(&projection),
        [
            pair("one", "one"),
            pair("two", "✓ Two"),
            pair("three", "three")
        ]
    );
    assert_eq!(projection.degraded, ["layout.toggle"]);

    // The title is found as a button's is: a picture that says what it is, or else the toggle's own label. The
    // mark counts toward liveupdate's length. Without a title the toggle is named.
    let pictured = layout(json!({ "type": "vstack", "children": [
        toggle("bell", "muted", json!({ "type": "symbol", "name": "bell.slash", "accessibility": { "label": "Silence" } })),
        json!({ "type": "toggle", "actionId": "pin", "bind": "muted", "accessibility": { "label": "Pin" },
            "child": { "type": "symbol", "name": "pin" } }),
        toggle("long", "muted", text(&"x".repeat(80))),
    ] }));
    let projection = projected(&pictured, json!({ "muted": true }));
    let long = format!("✓ {}…", "x".repeat(61));
    assert_eq!(
        titles(&projection),
        [
            pair("bell", "✓ Silence"),
            pair("pin", "✓ Pin"),
            pair("long", &long)
        ]
    );
    assert_eq!(long.chars().count(), 64);
    assert_eq!(projection.degraded, ["layout.symbol"]);
    let untitled = layout(toggle(
        "mute",
        "muted",
        json!({ "type": "symbol", "name": "bell" }),
    ));
    let projection = projected(&untitled, json!({ "muted": false }));
    assert!(presentation(&projection).actions.is_empty());
    assert_eq!(projection.degraded, ["layout.toggle"]);

    // A hidden toggle is no action.
    let hidden = layout(
        json!({ "type": "toggle", "actionId": "mute", "bind": "muted", "if": "shown",
        "child": text("Mute") }),
    );
    let projection = projected(&hidden, json!({ "muted": true, "shown": false }));
    assert!(presentation(&projection).actions.is_empty());

    // The state a tap asks for is that of the control whose action it is: the first shown one with the id that
    // has a title. An id is no toggle's when a button took it, when the toggle is hidden, or when nothing has it.
    let asked =
        |tree: &Layout, values: Json, id: &str| asked_for(tree, &snapshot(values).values, id);
    let shared = || json!({ "muted": false, "shared": true, "label": "Share" });
    assert_eq!(asked(&tree, shared(), "mute"), Some(true));
    assert_eq!(asked(&tree, shared(), "share"), Some(false));
    assert_eq!(asked(&tree, shared(), "none"), None);
    let twins = layout(json!({ "type": "vstack", "children": [
        { "type": "toggle", "actionId": "m", "bind": "muted", "if": "switch", "child": text("Mute") },
        { "type": "button", "actionId": "m", "child": text("Menu") },
        { "type": "button", "actionId": "n", "ifStale": true, "child": text("Never") },
        toggle("n", "muted", json!({ "type": "symbol", "name": "bell" })),
        toggle("n", "muted", text("Notify")),
    ] }));
    let with = |switch: bool| json!({ "muted": true, "switch": switch });
    assert_eq!(
        titles(&projected(&twins, with(false))),
        [pair("m", "Menu"), pair("n", "✓ Notify")]
    );
    assert_eq!(asked(&twins, with(false), "m"), None);
    assert_eq!(asked(&twins, with(true), "m"), Some(false));
    // Neither the button that only stale content shows nor the toggle without a title took `n`.
    assert_eq!(asked(&twins, with(false), "n"), Some(false));
    assert_eq!(
        asked(&hidden, json!({ "muted": true, "shown": false }), "mute"),
        None
    );
    // A toggle that comes after a button of its id is left out, and the id stays the button's.
    let taken = layout(json!({ "type": "hstack", "children": [
        { "type": "button", "actionId": "x", "child": text("Exit") },
        toggle("x", "muted", text("Extra")),
    ] }));
    let projection = projected(&taken, json!({ "muted": true }));
    assert_eq!(titles(&projection), [pair("x", "Exit")]);
    assert_eq!(projection.degraded, ["layout.toggle"]);
    assert_eq!(asked(&taken, json!({ "muted": true }), "x"), None);
}

#[test]
fn req_a_014_a_layout_is_drawn_as_while_fresh_and_its_staleness_is_named() {
    // REQ-A-014
    let tree = layout(json!({ "type": "vstack", "children": [
        { "type": "text", "bind": "status", "ifStale": false },
        { "type": "text", "value": "No signal", "ifStale": true },
        { "type": "button", "actionId": "retry", "ifStale": true, "child": text("Retry") },
        text("Gate 4"),
    ] }));
    let projection = projected(&tree, json!({ "status": "On the way" }));
    assert_eq!(projection.snapshot.title, "On the way");
    assert_eq!(projection.snapshot.status, "Gate 4");
    assert!(presentation(&projection).actions.is_empty());
    assert_eq!(projection.degraded, ["layout.ifStale"]);
    // The style of the generation counts the texts that can show: the stale one cannot.
    assert_eq!(template(&tree), liveupdate::Template::BigText);
    let only_when_stale = layout(json!({ "type": "vstack", "children": [
        text("Ride"),
        { "type": "progress", "bind": "p", "ifStale": true },
    ] }));
    assert_eq!(template(&only_when_stale), liveupdate::Template::Standard);

    // A condition inside a button's label is followed as well, and named once.
    let inside = layout(
        json!({ "type": "button", "actionId": "go", "child": { "type": "hstack", "children": [
        { "type": "text", "value": "Go", "ifStale": false },
        { "type": "text", "value": "Retry", "ifStale": true },
    ] } }),
    );
    let projection = projected(&inside, json!({}));
    assert_eq!(presentation(&projection).actions[0].title, "Go");
    assert_eq!(projection.degraded, ["layout.ifStale"]);

    // A layout without such nodes names nothing.
    let plain = layout(text("Ride"));
    assert_eq!(projected(&plain, json!({})).degraded, Vec::<String>::new());

    // A link that only stale content shows is none of the notification's: the links are counted without it,
    // whether the node around it is shown or hidden.
    let link = |url: &str, label: &str| json!({ "type": "link", "url": url, "child": text(label) });
    let mut stale = link("myapp://stale", "Retry");
    stale["ifStale"] = json!(true);
    let linked = layout(json!({ "type": "vstack", "children": [
        { "type": "hstack", "if": "shown", "children": [stale, link("myapp://first", "First")] },
        link("myapp://second", "Second"),
    ] }));
    let ids = |shown: bool| -> Vec<String> {
        let projection = projected_with(&linked, json!({ "shown": shown }), &offering());
        let actions = &presentation(&projection).actions;
        actions.iter().map(|action| action.id.clone()).collect()
    };
    assert_eq!(ids(true), ["link:1", "link:2"]);
    assert_eq!(ids(false), ["link:2"]);
}
