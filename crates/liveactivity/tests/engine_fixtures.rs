//! The request fixtures of the activitykit crate that this crate builds with, run against this crate's reader.
//! activitykit keeps them beside its own validator and its Swift decoder; here they show that reading a request in
//! this crate's types first gives the verdict activitykit gives, and where this crate's contract takes more. The
//! layouts' fixtures and, since activitykit 0.6.3, those of the templates. Beside them, what this crate needs of
//! that version's Swift sources and cannot run itself.

use liveactivity::{
    Error, LayoutEndInput, LayoutStartInput, LayoutUpdateInput, Validate, validate_request, weight,
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// The fixtures whose example of an unknown member is one this crate's contract defines: `relevance`, in a
/// snapshot. activitykit takes a relevance beside its request, this crate inside it.
const TAKEN_HERE: [&str; 1] = ["rejects an unknown member in the final content"];

/// The layout fixtures of the activitykit version in Cargo.lock.
fn fixtures() -> Value {
    fixtures_in("layout-requests.json")
}

/// The file `name` of the fixtures of the activitykit version in Cargo.lock, from that crate's own folder.
fn fixtures_in(name: &str) -> Value {
    let file = engine().join("tests/fixtures").join(name);
    serde_json::from_slice(&fs::read(file).expect("activitykit ships its fixtures")).unwrap()
}

/// The folder of the activitykit version in Cargo.lock.
fn engine() -> PathBuf {
    package("activitykit").0
}

/// The folder and the version of the crate `name` in Cargo.lock.
fn package(name: &str) -> (PathBuf, String) {
    let metadata = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--locked"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo answers its metadata");
    assert!(metadata.status.success(), "cargo metadata failed");
    let metadata: Value = serde_json::from_slice(&metadata.stdout).unwrap();
    let package = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == name)
        .unwrap_or_else(|| panic!("{name} is a dependency"))
        .clone();
    let manifest = package["manifest_path"].as_str().unwrap();
    (
        Path::new(manifest).parent().unwrap().to_owned(),
        package["version"].as_str().unwrap().to_owned(),
    )
}

#[test]
fn req_a_025_the_engine_says_what_an_earlier_android_shows() {
    // REQ-A-025
    // The contract says it by liveupdate's account: the rows of its specification that this crate hands on.
    let spec = fs::read_to_string(package("liveupdate").0.join("docs/spec.md"))
        .expect("liveupdate ships its specification");
    let row = |id: &str| {
        let start = format!("| {id} |");
        spec.lines()
            .find(|line| line.starts_with(&start))
            .unwrap_or_else(|| panic!("liveupdate's specification has {id}"))
            .to_owned()
    };
    let metrics = row("LU-V2-METRIC-4");
    assert!(metrics.contains("Below API 37") && metrics.contains("header clock"));
    let action = row("LU-V2-ACTION-3");
    assert!(action.contains("from API 31") && action.contains("drops the tap"));
}

#[test]
fn req_a_027_the_setup_names_the_aar_of_the_engine_in_the_lock() {
    // REQ-A-027
    let version = package("liveupdate").1;
    let setup = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/android-setup.md");
    let setup = fs::read_to_string(setup).expect("the setup document is in the repository");
    let aar = format!("liveupdate-android-{version}.aar");
    assert!(setup.contains(&aar), "the setup names {aar}");
    // And no AAR of another version.
    let named: Vec<&str> = setup
        .match_indices("liveupdate-android-")
        .map(|(at, _)| &setup[at..setup[at..].find(".aar").map_or(at, |end| at + end + 4)])
        .collect();
    assert!(named.iter().all(|name| *name == aar), "{named:?}");
}

#[test]
fn req_i_004_the_engines_intent_says_whether_a_tap_is_a_toggles() {
    // REQ-I-004
    // The system hands the application a value that an intent never set as false. With the state as one optional
    // value, as before activitykit 0.6.4, a button's tap reached the sink asking to switch off.
    let intent = engine().join("swift/Sources/ActivityKitIntent/ActionIntent.swift");
    let intent = fs::read_to_string(intent).expect("activitykit ships its intent");
    assert!(
        intent.contains("var isToggle: Bool"),
        "the intent says whether a tap is a toggle's"
    );
    assert!(
        !intent.contains("var isOn: Bool?"),
        "and carries no state that may be absent"
    );
    assert!(
        intent.contains("isOn: ActionPost.state(isToggle: isToggle, isOn: isOn)"),
        "and posts a state only as the two say"
    );
}

#[test]
fn req_i_010_the_engines_widget_draws_a_color_pair_itself() {
    // REQ-I-010
    // A light and dark pair that was the system's tint of the activity came as its light color behind content of
    // the dark appearance. Since activitykit 0.6.5 the widget draws a pair behind the content, and the system
    // tints with every other color.
    let sources = engine().join("swift/Sources");
    let rule = sources.join("ActivityKitShared/LayoutCardBackground.swift");
    let rule = fs::read_to_string(rule).expect("activitykit ships the rule of a card's background");
    // Since activitykit 0.6.6 the rule reads the background once, as what the widget draws and as the system's tint.
    assert!(
        rule.contains("func cardColors(values:"),
        "the rule names what the widget draws and what the system tints"
    );
    assert!(
        rule.contains("if case .adaptive = color { return (color, nil) }"),
        "a pair is drawn"
    );
    let widget = sources.join("ActivityKitWidget/LayoutWidget.swift");
    let widget = fs::read_to_string(widget).expect("activitykit ships its widget");
    assert!(
        widget.contains("let drawn = colors.drawn"),
        "the widget draws it"
    );
    assert!(
        widget.contains(".activityBackgroundTint(colors.tint.map"),
        "and the system tints with every other color"
    );
}

#[test]
fn req_i_011_the_engines_buttons_and_toggles_begin_with_ios_17_2() {
    // REQ-I-011
    // Xcode writes the intent as one that begins with iOS 17.2: below it a tap ran nothing. Since activitykit 0.6.6
    // the intent, the widget's controls and the authorization's `buttons` begin there.
    let sources = engine().join("swift/Sources");
    let intent = fs::read_to_string(sources.join("ActivityKitIntent/ActionIntent.swift"))
        .expect("activitykit ships its intent");
    assert!(
        intent.contains("@available(iOS 17.2, *)"),
        "the intent begins with iOS 17.2"
    );
    assert!(
        !intent.contains("@available(iOS 17.0, *)"),
        "and not with iOS 17.0"
    );
    let widget = fs::read_to_string(sources.join("ActivityKitWidget/LayoutWidget.swift"))
        .expect("activitykit ships its widget");
    assert_eq!(
        widget
            .matches("guard #available(iOS 17.2, *) else { return nil }")
            .count(),
        2,
        "the widget draws a button and a toggle from iOS 17.2"
    );
    assert!(
        !widget.contains("#available(iOS 17.0, *)"),
        "and below it a label"
    );
    // Since activitykit 0.7.0 the authorization's floors are one table of the platform's support.
    let support = fs::read_to_string(sources.join("ActivityKitCore/PlatformSupport.swift"))
        .expect("activitykit ships the platform's support");
    assert!(
        support.contains("buttons = from(17, 2)"),
        "and says buttons from iOS 17.2, which the capability `actions` follows"
    );
}

#[test]
fn req_i_012_the_engines_widget_gives_a_layout_the_cards_width() {
    // REQ-I-012
    // iOS 17 did not find a control of content narrower than the card under a tap. Since activitykit 0.6.6 the
    // widget takes the card's whole width on the Lock Screen, and a narrower layout stands in its middle.
    let widget = engine().join("swift/Sources/ActivityKitWidget/LayoutWidget.swift");
    let widget = fs::read_to_string(widget).expect("activitykit ships its widget");
    assert!(
        widget.contains(".frame(maxWidth: drawn != nil || !isSmallFamily ? .infinity : nil"),
        "the content takes the card's width outside the small family"
    );
}

#[test]
fn req_i_013_the_engines_build_names_swiftpms_own_build_system() {
    // REQ-I-013
    // From Xcode 27 `swift build` defaults to Swift Build, which does not build the bridge for iOS from these flags.
    let script =
        fs::read_to_string(engine().join("build.rs")).expect("activitykit ships its build script");
    assert!(
        script.contains(r#".args(["--build-system", "native"])"#),
        "the build names SwiftPM's own build system"
    );
}

#[test]
fn req_i_015_the_engines_widget_keeps_the_expanded_islands_corners_clear() {
    // REQ-I-015
    // The top outer corner of the expanded island cut the last letter of a short trailing value. Since activitykit
    // 0.7.0 the widget keeps the leading and trailing content 6 points from the top and from its outer side.
    let widget = engine().join("swift/Sources/ActivityKitWidget/LayoutWidget.swift");
    let widget = fs::read_to_string(widget).expect("activitykit ships its widget");
    assert!(
        widget.contains("private static let expandedCornerClearance: CGFloat = 6"),
        "the widget keeps 6 points clear"
    );
    for side in ["leading", "trailing"] {
        let region = if side == "leading" {
            "expandedLeading"
        } else {
            "expandedTrailing"
        };
        assert!(
            widget.contains(&format!(
                "Self.clear(LayoutRegionView(context: context, region: .{region}), of: .{side})"
            )),
            "of the {side} region"
        );
    }
}

#[test]
fn req_i_019_the_engine_reports_the_push_to_start_token_the_app_has() {
    // REQ-I-019
    // Since activitykit 0.9.1 the Swift layer reports the token the app has, then each change: before, it watched the
    // changes alone, and an app whose token did not change never learned it.
    let system = engine().join("swift/Sources/ActivityKitBridge/ActivityKitSystem.swift");
    let system = fs::read_to_string(system).expect("activitykit ships its system");
    // The members of `Activity`, not the event's case `.pushToStartToken(token:)`.
    let property = "Activity<LiveActivityLayoutAttributes>.pushToStartToken";
    let updates = system
        .find(&format!("{property}Updates"))
        .expect("the Swift layer watches the token's changes");
    let current = system
        .match_indices(property)
        .map(|(at, _)| at)
        .find(|at| !system[at + property.len()..].starts_with("Updates"))
        .expect("and reads the token the app has");
    assert!(
        updates < current,
        "it watches the changes before it reads the token, so that no change between the two is lost"
    );
    assert!(
        system.contains("token != reported"),
        "and reports no token twice"
    );
}

#[test]
fn req_i_016_the_engines_iphones_begin_with_ios_16_1() {
    // REQ-I-016
    // Since activitykit 0.8.0 an iPhone runs Live Activities from iOS 16.1, ActivityKit's first release, and the
    // documents name that floor.
    let support = engine().join("swift/Sources/ActivityKitCore/PlatformSupport.swift");
    let support = fs::read_to_string(support).expect("activitykit ships the platform's support");
    assert!(
        support.contains(
            "case .phone: return version.majorVersion > 16 || (version.majorVersion == 16 && version.minorVersion >= 1)"
        ),
        "an iPhone from iOS 16.1"
    );
    assert!(
        support.contains("case .pad: return version.majorVersion >= 17"),
        "an iPad from iPadOS 17"
    );
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |name: &str| fs::read_to_string(crate_root.join(name)).unwrap();
    let setup = read("docs/ios-setup.md");
    let readme = read("README.md");
    for said in [
        "- iOS 16.1 on an iPhone, iPadOS 17 on an iPad.",
        "a deployment target of iOS 16.1 or later",
        "The device is below iOS 16.1",
    ] {
        assert!(setup.contains(said), "the setup says {said:?}");
    }
    assert!(
        readme.contains("Activities need iOS 16.1"),
        "and so does the README"
    );
    for document in [&setup, &readme] {
        assert!(
            !document.contains("need iOS 16.2") && !document.contains("iOS 16.2 or later"),
            "no document names the floor of activitykit 0.7"
        );
    }
}

#[test]
fn req_a_034_the_setup_names_the_strings_of_the_aars_channels() {
    // REQ-A-034
    // Since liveupdate 0.9.0 the AAR names its two channels with string resources that an application overrides.
    let strings = package("liveupdate")
        .0
        .join("android/src/main/res/values/strings.xml");
    let strings =
        fs::read_to_string(strings).expect("liveupdate ships the strings of its channels");
    let names: Vec<&str> = strings
        .split("<string name=\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .collect();
    assert_eq!(
        names,
        [
            "liveupdate_channel_name",
            "liveupdate_channel_description",
            "liveupdate_alert_channel_name",
            "liveupdate_alert_channel_description",
            "liveupdate_watch_channel_name",
            "liveupdate_watch_channel_description",
        ],
        "the AAR names its channels by these strings"
    );
    let setup = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/android-setup.md");
    let setup = fs::read_to_string(setup).unwrap();
    for name in names {
        assert!(
            setup.contains(&format!("<string name=\"{name}\">")),
            "the setup shows how to override {name}"
        );
    }
}

#[test]
fn req_a_031_the_aar_has_what_the_setup_names() {
    // REQ-A-031
    let kotlin = package("liveupdate")
        .0
        .join("android/src/main/java/io/github/eedaegon/liveupdate");
    let read =
        |file: &str| fs::read_to_string(kotlin.join(file)).expect("liveupdate ships its Kotlin");
    let actions = read("LiveUpdateActions.kt");
    assert!(
        actions.contains("fun take(context: Context, intent: Intent?)"),
        "take"
    );
    let events = read("LiveUpdateEvents.kt");
    assert!(
        events.contains("fun addNativeListener(") && events.contains("fun removeNativeListener("),
        "listeners"
    );
    let foreground = read("LiveUpdateForeground.kt");
    assert!(foreground.contains("fun attachOr("), "attachOr");
    let setup = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/android-setup.md");
    let setup = fs::read_to_string(setup).expect("the setup document is in the repository");
    for named in [
        "LiveUpdateActions.take",
        "addNativeListener",
        "attachOr",
        "res/raw/keep.xml",
        "android_from_raw",
    ] {
        assert!(setup.contains(named), "the setup names {named}");
    }
}

#[test]
fn req_a_035_the_setup_names_the_aars_push_entry() {
    // REQ-A-035
    // liveupdate 0.10.0 hands a data message to its lifecycle from the host's messaging service, through a native
    // entry that its feature `push` exports, in the library that the manifest's meta-data names.
    let kotlin = package("liveupdate")
        .0
        .join("android/src/main/java/io/github/eedaegon/liveupdate/LiveUpdatePush.kt");
    let push = fs::read_to_string(kotlin).expect("liveupdate ships its push entry");
    let library = "io.github.eedaegon.liveupdate.LIBRARY";
    assert!(
        push.contains(&format!("const val LIBRARY = \"{library}\"")),
        "the meta-data"
    );
    assert!(
        push.contains("fun handle(context: Context, data: Map<String, String>): Boolean"),
        "handle"
    );
    let (engine, _) = package("liveupdate");
    let engine_manifest = fs::read_to_string(engine.join("Cargo.toml")).unwrap();
    assert!(
        engine_manifest.contains("push = [\"android\"]"),
        "liveupdate's feature `push`"
    );
    // This crate turns it on with its own feature, with the application's side.
    let manifest = include_str!("../Cargo.toml");
    assert!(
        manifest.contains("android-push = [\"app\", \"liveupdate/push\"]"),
        "the feature `android-push`"
    );
    let setup = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/android-setup.md");
    let setup = fs::read_to_string(setup).expect("the setup document is in the repository");
    for named in [
        library,
        "LiveUpdatePush.handle(this, message.data)",
        "android-push",
        "main process",
    ] {
        assert!(setup.contains(named), "the setup names {named}");
    }
}

fn cases<'a>(fixtures: &'a Value, group: &str) -> &'a Vec<Value> {
    fixtures[group].as_array().unwrap()
}

fn code(verdict: liveactivity::Result<()>) -> Option<String> {
    verdict.err().map(|error| error.code)
}

#[test]
fn req_c_005_the_engines_fixtures_get_the_same_verdict_here() {
    // REQ-C-005
    let fixtures = fixtures();
    let (valid, invalid) = (cases(&fixtures, "valid"), cases(&fixtures, "invalid"));
    assert!(valid.len() >= 100 && invalid.len() >= 100);
    for case in valid {
        let command = case["command"].as_str().unwrap();
        let verdict = validate_request(command, case["input"].clone());
        assert_eq!(code(verdict), None, "{}", case["name"]);
    }
    let mut taken = Vec::new();
    for case in invalid {
        let (name, command) = (
            case["name"].as_str().unwrap(),
            case["command"].as_str().unwrap(),
        );
        let verdict = code(validate_request(command, case["input"].clone()));
        if TAKEN_HERE.contains(&name) {
            assert_eq!(verdict, None, "{name}");
            taken.push(name);
        } else {
            assert_eq!(verdict.as_deref(), case["code"].as_str(), "{name}");
        }
    }
    // Every exception names a fixture that exists: a change in activitykit's set shows here.
    assert_eq!(taken, TAKEN_HERE);
}

/// The weight that a fixture states for its request: the values of the snapshot, and with a start its layout and its
/// styles, which weigh as an empty object where the start has none. Nothing else of a request weighs.
fn stated_weight(input: &Value) -> usize {
    let part = |pointer| input.pointer(pointer).map(weight);
    let start = part("/layout").map(|layout| layout + part("/styles").unwrap_or(2));
    part("/snapshot/values").unwrap() + start.unwrap_or(0)
}

#[test]
fn req_c_005_the_valid_fixtures_read_into_the_typed_requests_and_weigh_the_same_written_back() {
    // REQ-C-005
    // By serde alone, as a caller reads a request that it does not hand to `parse_request`.
    let fixtures = fixtures();
    let mut weighed = 0;
    for case in cases(&fixtures, "valid") {
        let (name, input) = (&case["name"], case["input"].clone());
        let written = match case["command"].as_str().unwrap() {
            "start_layout" => {
                let typed: LayoutStartInput = serde_json::from_value(input).unwrap();
                assert_eq!(typed.validate(), Ok(()), "{name}");
                serde_json::to_value(&typed).unwrap()
            }
            "update_layout" => {
                let typed: LayoutUpdateInput = serde_json::from_value(input).unwrap();
                assert_eq!(typed.validate(), Ok(()), "{name}");
                serde_json::to_value(&typed).unwrap()
            }
            command => {
                assert_eq!(command, "end_layout", "{name}");
                let typed: LayoutEndInput = serde_json::from_value(input.clone()).unwrap();
                assert_eq!(typed.validate(), Ok(()), "{name}");
                let written = serde_json::to_value(&typed).unwrap();
                // An end is written as it was read, with final content and without.
                assert_eq!(written, input, "{name}");
                written
            }
        };
        // An end without final content has no weight of its own.
        if written.get("snapshot").is_some() {
            assert_eq!(case["weight"], stated_weight(&written), "{name}");
            weighed += 1;
        } else {
            assert!(case.get("weight").is_none(), "{name}");
        }
    }
    assert!(weighed >= 100);
}

/// The verdict on the update or the end of a case that names its start: the request's own check, and then the
/// check against that start.
fn after_its_start(case: &Value, request: &str) -> liveactivity::Result<()> {
    let start = case["start"].clone();
    assert_eq!(code(validate_request("start_layout", start.clone())), None);
    let start: LayoutStartInput = serde_json::from_value(start).unwrap();
    let following = case[request].clone();
    if request == "update" {
        validate_request("update_layout", following.clone()).and_then(|()| {
            let update: LayoutUpdateInput = serde_json::from_value(following).unwrap();
            start.validate_update(&update)
        })
    } else {
        validate_request("end_layout", following.clone()).and_then(|()| {
            let end: LayoutEndInput = serde_json::from_value(following).unwrap();
            start.validate_end(&end)
        })
    }
}

#[test]
fn req_c_005_updates_and_ends_are_checked_against_their_start_as_the_engine_does() {
    // REQ-C-005
    let fixtures = fixtures();
    let (updates, ends) = (cases(&fixtures, "updates"), cases(&fixtures, "ends"));
    assert!(updates.len() >= 20 && !ends.is_empty());
    for (group, request) in [(updates, "update"), (ends, "end")] {
        for case in group {
            let verdict = after_its_start(case, request);
            let expected = case.get("code").and_then(Value::as_str);
            assert_eq!(code(verdict).as_deref(), expected, "{}", case["name"]);
        }
    }
}

#[test]
fn req_c_010_every_refused_fixture_gets_one_of_the_two_errors() {
    // REQ-C-010
    let fixtures = fixtures();
    let alone = cases(&fixtures, "invalid").iter().map(|case| {
        let command = case["command"].as_str().unwrap();
        (case, validate_request(command, case["input"].clone()))
    });
    let after = [("updates", "update"), ("ends", "end")]
        .into_iter()
        .flat_map(|(group, request)| {
            let group = cases(&fixtures, group).iter();
            group.map(move |case| (case, after_its_start(case, request)))
        });
    let refusals: Vec<Error> = alone
        .chain(after)
        .filter_map(|(case, verdict)| {
            let error = verdict.err()?;
            // The whole error, with its message: it says nothing of the request it refuses.
            let known = [Error::invalid(), Error::payload_too_large()].contains(&error);
            assert!(known, "{}", case["name"]);
            Some(error)
        })
        .collect();
    assert!(refusals.len() >= 100);
    assert!(refusals.contains(&Error::invalid()));
    assert!(refusals.contains(&Error::payload_too_large()));
}

#[test]
fn req_c_008_the_engines_template_fixtures_get_the_same_verdict_here() {
    // REQ-C-008
    // The requests of contract v1 that activitykit ships since 0.6.3: every valid one is taken, and every other
    // is refused as invalid input, an unknown command among them.
    let fixtures = fixtures_in("v1-requests.json");
    let (valid, invalid) = (cases(&fixtures, "valid"), cases(&fixtures, "invalid"));
    assert!(valid.len() >= 10 && invalid.len() >= 30);
    for (cases, verdict) in [(valid, None), (invalid, Some("INVALID_INPUT"))] {
        for case in cases {
            let command = case["command"].as_str().unwrap();
            let given = code(validate_request(command, case["input"].clone()));
            assert_eq!(given.as_deref(), verdict, "{}", case["name"]);
        }
    }
}
