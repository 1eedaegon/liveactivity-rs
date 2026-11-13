use super::*;
use serde_json::json;

fn start() -> serde_json::Value {
    json!({"activityKey":"demo:1","schemaVersion":2,"fallback":"notification",
        "layout":{"lockScreen":{"type":"text","bind":"status","style":"s"},"keyline":{"bind":"tone"}},
        "styles":{"s":{"color":{"bind":"tone"},"font":"headline"}},
        "snapshot":{"revision":0,"values":{"status":"On time","tone":"green"},"staleAt":5,"timestamp":7},
        "android":{"visibility":"private"}})
}
#[test]
fn a_request_reads_into_the_contracts_types_and_writes_back_the_same_json() {
    let input: LayoutStartInput = parse("start_layout", start()).unwrap();
    assert_eq!(serde_json::to_value(&input).unwrap(), start());
    assert_eq!(input.snapshot.values["tone"], Value::Text("green".into()));
    assert_eq!(input.snapshot.timestamp, Some(7));
    // activitykit is shown the request without what only Android reads.
    let mut shown = start();
    shown.as_object_mut().unwrap().remove("android");
    assert_eq!(serde_json::to_value(input.wire()).unwrap(), shown);
    let end =
        json!({"id":"a","policy":{"kind":"after","at":3},"snapshot":{"revision":1,"values":{}}});
    let typed: LayoutEndInput = parse("end_layout", end.clone()).unwrap();
    assert_eq!(serde_json::to_value(&typed).unwrap(), end);
    assert_eq!(serde_json::to_value(typed.wire()).unwrap(), end);
    let plain: LayoutEndInput =
        parse("end_layout", json!({"id":"a","policy":{"kind":"default"}})).unwrap();
    assert_eq!(plain.snapshot, None);
    assert_eq!(
        serde_json::to_value(plain.wire()).unwrap(),
        json!({"id":"a","policy":{"kind":"default"}})
    );
    let immediate = LayoutEndInput {
        id: "a".into(),
        policy: EndPolicy::Immediate {},
        snapshot: None,
    };
    assert_eq!(
        serde_json::to_value(immediate.wire()).unwrap()["policy"]["kind"],
        "immediate"
    );
}
#[test]
fn parse_is_activitykits_strict_check_and_the_types_validate_the_same() {
    let mut nulled = start();
    nulled["snapshot"]["timestamp"] = json!(null);
    assert_eq!(
        parse::<LayoutStartInput>("start_layout", nulled),
        Err(Error::invalid())
    );
    let mut unknown = start();
    unknown["extra"] = json!(1);
    assert_eq!(
        parse::<LayoutStartInput>("start_layout", unknown),
        Err(Error::invalid())
    );
    // Valid for activitykit, but not this type.
    assert_eq!(
        parse::<LayoutStartInput>("end_layout", json!({"id":"a","policy":{"kind":"default"}})),
        Err(Error::invalid())
    );
    let input: LayoutStartInput = serde_json::from_value(start()).unwrap();
    assert_eq!(input.validate(), Ok(()));
    assert_eq!(input.snapshot.validate(), Ok(()));
    let mut late = input.snapshot.clone();
    late.timestamp = Some(8_640_000_000_000_001);
    assert_eq!(late.validate(), Err(Error::invalid()));
    let update = LayoutUpdateInput::new("a", input.snapshot.clone());
    assert_eq!(
        (update.validate(), input.validate_update(&update)),
        (Ok(()), Ok(()))
    );
    let mut crimson = update.clone();
    crimson
        .snapshot
        .values
        .insert("tone".into(), Value::Text("crimson".into()));
    assert_eq!(input.validate_update(&crimson), Err(Error::invalid()));
    let end = LayoutEndInput {
        id: "a".into(),
        policy: EndPolicy::Default {},
        snapshot: Some(crimson.snapshot),
    };
    assert_eq!(
        (end.validate(), input.validate_end(&end)),
        (Ok(()), Err(Error::invalid()))
    );
    let bare = LayoutEndInput {
        snapshot: None,
        ..end
    };
    assert_eq!(input.validate_end(&bare), Ok(()));
}
fn code<T>(result: Result<T>) -> Option<String> {
    result.err().map(|error| error.code)
}

#[test]
fn req_c_005_a_request_is_read_strictly_in_this_crates_types() {
    // REQ-C-005
    // What activitykit's contract does not carry is taken, and reads back as the same JSON.
    let mut rich = start();
    rich["snapshot"]["relevance"] = json!(80);
    rich["snapshot"]["android"] = json!({"title":"On time","status":"Gate 4","presentation":{
        "style":{"kind":"progress","value":{"progress":40,"styledByProgress":true,
            "segments":[{"length":60},{"length":40}],"points":[{"position":60}]}},
        "shortCriticalText":"4 min"}});
    let input: LayoutStartInput = parse("start_layout", rich.clone()).unwrap();
    assert_eq!(input.snapshot.relevance, Some(80.0));
    let content = input.snapshot.android.as_ref().unwrap();
    assert_eq!(
        content.presentation.short_critical_text.as_deref(),
        Some("4 min")
    );
    assert!(same(&serde_json::to_value(&input).unwrap(), &rich));
    // The presentation is read as liveupdate reads it: a default may be written out.
    let mut spelled = rich.clone();
    spelled["snapshot"]["android"]["presentation"]["style"]["value"]["indeterminate"] =
        json!(false);
    spelled["snapshot"]["android"]["presentation"]["actions"] = json!([]);
    let same_input: LayoutStartInput = parse("start_layout", spelled).unwrap();
    assert_eq!(same_input, input);
    let update = json!({"id":"a","snapshot":{"revision":1,"values":{"status":"Late","tone":"red"}},
        "alert":{"title":"Gate changed","body":"Now gate 12"}});
    let typed: LayoutUpdateInput = parse("update_layout", update.clone()).unwrap();
    assert_eq!(typed.alert.as_ref().unwrap().title, "Gate changed");
    assert_eq!(serde_json::to_value(&typed).unwrap(), update);

    // Strictly: no unknown member at any level, no `null`, nothing that serde would correct.
    let refused = |edit: fn(&mut serde_json::Value)| {
        let mut request = start();
        edit(&mut request);
        code(parse::<LayoutStartInput>("start_layout", request))
    };
    let invalid = Some("INVALID_INPUT".to_owned());
    assert_eq!(refused(|request| request["ios"] = json!({})), invalid);
    assert_eq!(
        refused(|request| request["snapshot"]["extra"] = json!(1)),
        invalid
    );
    assert_eq!(
        refused(|request| request["android"]["presentation"] = json!({})),
        invalid
    );
    assert_eq!(
        refused(|request| {
            request["snapshot"]["android"] = json!({"title":"On time","status":"Gate 4",
                "presentation":{"style":{"kind":"standard"},"sound":"loud"}});
        }),
        invalid
    );
    assert_eq!(
        refused(|request| request["snapshot"]["relevance"] = json!(null)),
        invalid
    );
    assert_eq!(
        refused(|request| request["snapshot"]["android"] = json!(null)),
        invalid
    );
    assert_eq!(
        refused(|request| request["layout"]["lockScreen"]["surprise"] = json!(true)),
        invalid
    );
    // The presentation is liveupdate's, and as strict: a default may be written out, but a `null` is no
    // default and an array is no object.
    assert_eq!(
        refused(|request| {
            request["snapshot"]["android"] = json!({"title":"On time","status":"Gate 4",
                "presentation":{"style":{"kind":"standard"},"largeIcon":null}});
        }),
        invalid
    );
    assert_eq!(
        refused(|request| {
            request["snapshot"]["android"] = json!({"title":"On time","status":"Gate 4",
                "presentation":[{"kind":"standard"},"4 min",null,[],null]});
        }),
        invalid
    );
    assert_eq!(
        refused(|request| {
            request["snapshot"]["android"] = json!({"title":"On time","status":"Gate 4",
                "presentation":{"style":{"kind":"standard"},"actions":[],
                    "shortCriticalText":"4 min"}});
        }),
        None
    );
    // A struct that serde also reads from an array does not encode back to the array.
    assert_eq!(
        refused(|request| request["android"] = json!(["private"])),
        invalid
    );
    // JSON nested deeper than a request can be is refused unread.
    let deep = (0..40).fold(json!(1), |inner, _| json!([inner]));
    assert_eq!(
        refused(|request| request["snapshot"]["values"]["status"] = json!("x")),
        None
    );
    let mut nested = start();
    nested["snapshot"]["values"]["status"] = deep;
    assert_eq!(
        code(parse::<LayoutStartInput>("start_layout", nested)),
        invalid
    );

    // A request is read as the request of its command: a start is no update, whatever the caller's type.
    assert_eq!(
        code(parse::<LayoutStartInput>("update_layout", start())),
        invalid
    );
    assert_eq!(code(parse::<LayoutStartInput>("start", start())), invalid);
}

#[test]
fn req_c_006_what_a_request_says_beside_the_layout_is_checked() {
    // REQ-C-006
    let invalid = Some("INVALID_INPUT".to_owned());
    let input: LayoutStartInput = serde_json::from_value(start()).unwrap();
    let with = |edit: fn(&mut LayoutSnapshot)| {
        let mut snapshot = input.snapshot.clone();
        edit(&mut snapshot);
        snapshot
    };
    let content = |title: &str, chip: &str| AndroidContent {
        title: title.into(),
        status: "Gate 4".into(),
        presentation: AndroidPresentation {
            short_critical_text: Some(chip.into()),
            ..AndroidPresentation::new(PresentationStyle::Standard)
        },
    };

    // A relevance is a number.
    assert_eq!(code(with(|s| s.relevance = Some(-3.5)).validate()), None);
    assert_eq!(
        code(with(|s| s.relevance = Some(f64::NAN)).validate()),
        invalid
    );
    assert_eq!(
        code(with(|s| s.relevance = Some(f64::NEG_INFINITY)).validate()),
        invalid
    );
    // An Android content is one liveupdate takes: here a title, and a chip text of at most 7 characters.
    let good = LayoutSnapshot {
        android: Some(content("On time", "4 min")),
        ..input.snapshot.clone()
    };
    assert_eq!(code(good.validate()), None);
    for wrong in [content(" ", "4 min"), content("On time", "four minutes")] {
        let snapshot = LayoutSnapshot {
            android: Some(wrong),
            ..input.snapshot.clone()
        };
        assert_eq!(code(snapshot.validate()), invalid);
        // In a start, an update and an end alike.
        let start = LayoutStartInput {
            snapshot: snapshot.clone(),
            ..input.clone()
        };
        assert_eq!(code(start.validate()), invalid);
        assert_eq!(
            code(LayoutUpdateInput::new("a", snapshot.clone()).validate()),
            invalid
        );
        let end = LayoutEndInput {
            id: "a".into(),
            policy: EndPolicy::Default {},
            snapshot: Some(snapshot),
        };
        assert_eq!(code(end.validate()), invalid);
    }
    // An alert has a title and a body within their lengths.
    let alert = |title: &str, body: &str| LayoutUpdateInput {
        alert: Some(Alert {
            title: title.into(),
            body: body.into(),
        }),
        ..LayoutUpdateInput::new("a", input.snapshot.clone())
    };
    assert_eq!(code(alert("Gate changed", "Now gate 12").validate()), None);
    assert_eq!(code(alert("", "Now gate 12").validate()), invalid);
    assert_eq!(code(alert("Gate changed", " ").validate()), invalid);
    assert_eq!(code(alert(&"a".repeat(121), "b").validate()), invalid);
    assert_eq!(code(alert("a", &"b".repeat(241)).validate()), invalid);
}

#[test]
fn the_template_marker_is_no_value_key_of_a_layout_request() {
    let code = |result: Result<()>| result.map_err(|error| error.code);
    let invalid = Err("INVALID_INPUT".to_owned());
    let mut marked = start();
    marked["snapshot"]["values"]["liveActivityTemplate"] = json!("progress");
    // The commands' check, and the Rust API's.
    assert_eq!(
        code(crate::validate_request("start_layout", marked.clone())),
        invalid
    );
    let typed: LayoutStartInput = serde_json::from_value(marked).unwrap();
    assert_eq!(code(typed.validate()), invalid);
    let update =
        json!({"id":"a","snapshot":{"revision":1,"values":{"liveActivityTemplate":"timer"}}});
    assert_eq!(
        code(crate::validate_request("update_layout", update.clone())),
        invalid
    );
    let typed: LayoutUpdateInput = serde_json::from_value(update).unwrap();
    assert_eq!(code(typed.validate()), invalid);
    let end = json!({"id":"a","policy":{"kind":"immediate"},
        "snapshot":{"revision":1,"values":{"liveActivityTemplate":"timer"}}});
    assert_eq!(
        code(crate::validate_request("end_layout", end.clone())),
        invalid
    );
    let typed: LayoutEndInput = serde_json::from_value(end).unwrap();
    assert_eq!(code(typed.validate()), invalid);
    // Only that key: a longer one is a value like any other, and an end without content has none.
    let mut similar = start();
    similar["snapshot"]["values"]["liveActivityTemplates"] = json!("progress");
    assert_eq!(
        code(crate::validate_request("start_layout", similar)),
        Ok(())
    );
    let bare = json!({"id":"a","policy":{"kind":"immediate"}});
    assert_eq!(code(crate::validate_request("end_layout", bare)), Ok(()));
}

#[test]
fn req_a_007_a_content_takes_the_extensions_of_a_presentation() {
    // REQ-A-007
    let invalid = Some("INVALID_INPUT".to_owned());
    let content = |presentation: serde_json::Value| {
        let mut request = start();
        request["snapshot"]["android"] =
            json!({"title":"Driver on the way","status":"4 min","presentation":presentation});
        request
    };
    let read =
        |request: serde_json::Value| code(parse::<LayoutStartInput>("start_layout", request));
    let with = |member: &str, value: serde_json::Value| {
        let mut presentation = json!({"style":{"kind":"standard"}});
        presentation[member] = value;
        read(content(presentation))
    };

    // Everything liveupdate 0.2 added reads into the contract's types and writes back as it was sent.
    let all = json!({"style":{"kind":"standard"},
        "openUrl":"myapp://rides/7",
        "actions":[{"id":"map","title":"Map","openUrl":"myapp://rides/7/map"}],
        "chronometer":{"timer":{"startedAt":1,"endsAt":2},"countsDown":true},
        "alert":true,
        "publicVersion":{"title":"Ride","status":"On the way"},
        "smallIcon":{"kind":"resource","value":"ic_ride"},
        "accentColor":4278222848_u32,
        "subText":"Ride 7"});
    let input: LayoutStartInput = parse("start_layout", content(all.clone())).unwrap();
    assert_eq!(serde_json::to_value(&input).unwrap(), content(all));
    let sent = &input.snapshot.android.as_ref().unwrap().presentation;
    assert_eq!(sent.open_url.as_deref(), Some("myapp://rides/7"));
    assert_eq!(
        sent.public_version,
        Some(PublicVersion {
            title: "Ride".into(),
            status: "On the way".into(),
        })
    );
    // A flag at its default may be written, as everywhere in a presentation.
    assert_eq!(with("alert", json!(false)), None);

    // Each member keeps liveupdate's rule, on every platform.
    for (member, wrong) in [
        ("openUrl", json!("MyApp://rides/7")),
        ("openUrl", json!("intent://rides/7")),
        ("openUrl", json!("rides/7")),
        (
            "actions",
            json!([{"id":"map","title":"Map","openUrl":"file:///x"}]),
        ),
        (
            "chronometer",
            json!({"timer":{"startedAt":2,"endsAt":2},"countsDown":true}),
        ),
        ("alert", json!(1)),
        ("alert", json!(null)),
        ("publicVersion", json!({"title":" ","status":"On the way"})),
        (
            "publicVersion",
            json!({"title":"Ride","status":"On the way","icon":"x"}),
        ),
        (
            "smallIcon",
            json!({"kind":"appFile","value":"icons/ride.png"}),
        ),
        ("accentColor", json!(4_294_967_296_u64)),
        ("subText", json!("s".repeat(65))),
    ] {
        assert_eq!(with(member, wrong.clone()), invalid, "{member}: {wrong}");
    }
    // The clock in the header is beside a style; the timer style is that clock alone.
    let clock = json!({"timer":{"startedAt":1,"endsAt":2},"countsDown":true});
    assert_eq!(
        read(content(
            json!({"style":{"kind":"timer","value":clock},"chronometer":clock})
        )),
        invalid
    );

    // A public version is what a locked screen shows of a private activity, which a start is unless it says
    // otherwise.
    let public_version = json!({"title":"Ride","status":"On the way"});
    let started = |visibility: Option<&str>| {
        let mut request =
            content(json!({"style":{"kind":"standard"},"publicVersion":public_version}));
        match visibility {
            Some(visibility) => request["android"]["visibility"] = json!(visibility),
            None => {
                request.as_object_mut().unwrap().remove("android");
            }
        }
        read(request)
    };
    assert_eq!(started(Some("private")), None);
    assert_eq!(started(None), None);
    assert_eq!(started(Some("public")), invalid);
    assert_eq!(started(Some("secret")), invalid);
    // Without a public version the visibility is free.
    let mut plain = content(json!({"style":{"kind":"standard"}}));
    plain["android"]["visibility"] = json!("public");
    assert_eq!(read(plain), None);
}

#[test]
fn req_a_011_a_public_version_is_an_option_of_a_private_start() {
    // REQ-A-011
    let invalid = Some("INVALID_INPUT".to_owned());
    let with = |android: serde_json::Value| {
        let mut request = start();
        request["android"] = android;
        code(parse::<LayoutStartInput>("start_layout", request))
    };
    let version = json!({"title":"Ride","status":"On the way"});

    let private = json!({"visibility":"private","publicVersion":version});
    assert_eq!(with(private.clone()), None);
    // It reads into the contract's types and writes back as it was sent, and the iOS engine is not shown it.
    let mut request = start();
    request["android"] = private;
    let input: LayoutStartInput = parse("start_layout", request.clone()).unwrap();
    assert_eq!(serde_json::to_value(&input).unwrap(), request);
    let options = input.android.as_ref().unwrap();
    assert_eq!(
        options.public_version,
        Some(PublicVersion {
            title: "Ride".into(),
            status: "On the way".into(),
        })
    );
    assert!(
        serde_json::to_value(input.wire())
            .unwrap()
            .get("android")
            .is_none()
    );

    // A locked screen shows the content of a public activity and nothing of a secret one: neither has one.
    assert_eq!(
        with(json!({"visibility":"public","publicVersion":version})),
        invalid
    );
    assert_eq!(
        with(json!({"visibility":"secret","publicVersion":version})),
        invalid
    );
    // Its texts have liveupdate's bounds, and it takes no other member.
    for wrong in [
        json!({"title":" ","status":"On the way"}),
        json!({"title":"Ride","status":"s".repeat(241)}),
        json!({"title":"Ride"}),
        json!({"title":"Ride","status":"On the way","icon":"x"}),
        json!(null),
    ] {
        let options = json!({"visibility":"private","publicVersion":wrong});
        assert_eq!(with(options.clone()), invalid, "{options}");
    }
    // The placement is one of two words.
    assert_eq!(
        with(json!({"visibility":"private","placement":"foreground"})),
        None
    );
    assert_eq!(
        with(json!({"visibility":"private","placement":"tagged"})),
        None
    );
    assert_eq!(
        with(json!({"visibility":"private","placement":"floating"})),
        invalid
    );
    assert_eq!(
        with(json!({"visibility":"private","placement":null})),
        invalid
    );
}
