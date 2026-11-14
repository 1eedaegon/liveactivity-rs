//! Requests that arrive as JSON, at the limits of the contract: the templates' requests of contract v1, a number's
//! way through the reader, the one error of a refusal, and what a layout request says beside its layout.

use liveactivity::{Error, LayoutStartInput, Validate, validate_request};
use serde_json::{Value, json};

const LARGEST_REVISION: u64 = 9_007_199_254_740_991;
const LARGEST_TIME: u64 = 8_640_000_000_000_000;

/// A start of the progress template that the contract takes.
fn progress() -> Value {
    json!({"activityKey":"job:1","schemaVersion":1,"template":"progress","fallback":"notification",
        "snapshot":{"revision":0,"title":"Job","status":"Running","progress":0.3}})
}

/// An update with a timer that the contract takes.
fn timer() -> Value {
    json!({"id":"opaque-handle-1","snapshot":{"revision":1,"title":"Timer","status":"Running",
        "timer":{"startedAt":0,"endsAt":1}}})
}

/// An end that the contract takes.
fn end() -> Value {
    json!({"id":"opaque-handle-1","policy":{"kind":"default"}})
}

/// `request` with the member at `pointer` set, whether it was there or not.
fn with(mut request: Value, pointer: &str, value: Value) -> Value {
    let (parent, member) = pointer.rsplit_once('/').unwrap();
    request.pointer_mut(parent).unwrap()[member] = value;
    request
}

/// `request` without the member at `pointer`, which was there.
fn without(mut request: Value, pointer: &str) -> Value {
    let (parent, member) = pointer.rsplit_once('/').unwrap();
    let parent = request
        .pointer_mut(parent)
        .unwrap()
        .as_object_mut()
        .unwrap();
    parent.remove(member).unwrap();
    request
}

fn refused() -> liveactivity::Result<()> {
    Err(Error::invalid())
}

#[test]
fn req_c_008_the_fixtures_of_the_templates_get_their_verdict() {
    // REQ-C-008, REQ-C-010
    let fixtures: Value = serde_json::from_str(include_str!("fixtures/requests.json")).unwrap();
    let fixtures = fixtures.as_array().unwrap();
    assert_eq!(fixtures.len(), 11);
    for fixture in fixtures {
        let request = &fixture["request"];
        let verdict = validate_request(
            request["command"].as_str().unwrap(),
            request["input"].clone(),
        );
        let expected = if fixture["valid"] == true {
            Ok(())
        } else {
            refused()
        };
        assert_eq!(verdict, expected, "{}", fixture["name"]);
    }
}

#[test]
fn req_c_008_a_template_request_is_taken_at_each_limit_and_refused_past_it() {
    // REQ-C-008
    type Request = fn() -> Value;
    let requests: [(&str, Request); 3] = [("start", progress), ("update", timer), ("end", end)];
    // Lengths count code points, whatever their bytes; a key is of one-byte characters.
    for (command, pointer, last, past) in [
        (
            0,
            "/activityKey",
            json!("a".repeat(128)),
            json!("a".repeat(129)),
        ),
        (
            0,
            "/snapshot/title",
            json!("😀".repeat(120)),
            json!("😀".repeat(121)),
        ),
        (
            0,
            "/snapshot/status",
            json!("한".repeat(240)),
            json!("한".repeat(241)),
        ),
        (0, "/snapshot/progress", json!(0), json!(-0.1)),
        (0, "/snapshot/progress", json!(1), json!(1.1)),
        (
            0,
            "/snapshot/revision",
            json!(LARGEST_REVISION),
            json!(LARGEST_REVISION + 1),
        ),
        (
            0,
            "/snapshot/staleAt",
            json!(LARGEST_TIME),
            json!(LARGEST_TIME + 1),
        ),
        (1, "/id", json!("한".repeat(256)), json!("한".repeat(257))),
        (
            1,
            "/snapshot/timer/endsAt",
            json!(LARGEST_TIME),
            json!(LARGEST_TIME + 1),
        ),
        (2, "/id", json!("한".repeat(256)), json!("한".repeat(257))),
        (
            2,
            "/policy",
            json!({"kind":"after","at":LARGEST_TIME}),
            json!({"kind":"after","at":LARGEST_TIME + 1}),
        ),
    ] {
        let (command, request) = requests[command];
        let taken = validate_request(command, with(request(), pointer, last));
        assert_eq!(taken, Ok(()), "{command} {pointer}");
        let beyond = validate_request(command, with(request(), pointer, past));
        assert_eq!(beyond, refused(), "{command} {pointer}");
    }
    // Every limit of a start at once.
    let mut input = progress();
    input["activityKey"] = json!("a".repeat(128));
    input["snapshot"] = json!({"revision":LARGEST_REVISION,"title":"😀".repeat(120),
        "status":"한".repeat(240),"progress":1,"staleAt":LARGEST_TIME});
    assert_eq!(validate_request("start", input), Ok(()));
}

#[test]
fn req_c_008_a_template_request_of_a_wrong_shape_is_refused() {
    // REQ-C-008
    for (command, request) in [("start", progress()), ("update", timer()), ("end", end())] {
        assert_eq!(validate_request(command, request), Ok(()), "{command}");
    }
    // A key begins with a letter or a digit, and goes on with those and with `.`, `_`, `:` and `-`.
    for key in ["9a", "A.b_c:d-e"] {
        let start = with(progress(), "/activityKey", json!(key));
        assert_eq!(validate_request("start", start), Ok(()), "{key}");
    }
    for (pointer, value) in [
        ("/activityKey", json!("../secret")),
        ("/activityKey", json!(".hidden")),
        ("/activityKey", json!("a/b")),
        ("/activityKey", json!("key\n")),
        ("/activityKey", json!("é")),
        ("/activityKey", json!("a한")),
        ("/activityKey", json!("")),
        ("/schemaVersion", json!(2)),
        ("/fallback", json!(null)),
        // Blank is what ECMAScript's \S does not match: a byte order mark too.
        ("/snapshot/title", json!(" \n\t")),
        ("/snapshot/status", json!(" \n\t")),
        ("/snapshot/status", json!("\u{feff}")),
        ("/snapshot/progress", json!(null)),
        ("/snapshot/progress", json!("private-input")),
        ("/snapshot/revision", json!(-1)),
        ("/snapshot/revision", json!(0.5)),
        ("/snapshot/staleAt", json!(null)),
        ("/snapshot/staleAt", json!(-1)),
        ("/snapshot/staleAt", json!(0.5)),
        // No member that the contract does not name, and no `null` for one it names.
        ("/snapshot/token", json!("secret")),
        ("/unsafePath", json!("/tmp")),
        ("/snapshot", json!(null)),
    ] {
        let verdict = validate_request("start", with(progress(), pointer, value));
        assert_eq!(verdict, refused(), "{pointer}");
    }
    // A next-line character is no blank.
    let status = with(progress(), "/snapshot/status", json!("\u{0085}"));
    assert_eq!(validate_request("start", status), Ok(()));
    assert_eq!(validate_request("start", json!([])), refused());

    // In an update and an end alike.
    for (command, request) in [
        ("update", with(timer(), "/extra", json!(1))),
        ("update", with(timer(), "/snapshot/timer/extra", json!(1))),
        ("update", with(timer(), "/snapshot", json!(null))),
        ("update", with(timer(), "/id", json!(" "))),
        ("end", with(end(), "/extra", json!(1))),
        ("end", with(end(), "/policy/extra", json!(1))),
        ("end", with(end(), "/policy", json!(null))),
        ("end", with(end(), "/id", json!(" "))),
    ] {
        let verdict = validate_request(command, request.clone());
        assert_eq!(verdict, refused(), "{command} {request}");
    }
}

#[test]
fn req_c_008_a_snapshot_has_the_one_presentation_of_its_template() {
    // REQ-C-008
    let a_timer = json!({"startedAt":0,"endsAt":1});
    let timed = with(
        without(progress(), "/snapshot/progress"),
        "/snapshot/timer",
        a_timer.clone(),
    );
    let of_a_timer = with(timed.clone(), "/template", json!("timer"));
    assert_eq!(validate_request("start", of_a_timer), Ok(()));
    for (case, start) in [
        ("both", with(progress(), "/snapshot/timer", a_timer)),
        ("neither", without(progress(), "/snapshot/progress")),
        ("a timer for the progress template", timed),
        (
            "a progress for the timer template",
            with(progress(), "/template", json!("timer")),
        ),
    ] {
        assert_eq!(validate_request("start", start), refused(), "{case}");
    }
    // An update names no template, and takes either presentation, one of them.
    let both = with(timer(), "/snapshot/progress", json!(0.5));
    assert_eq!(validate_request("update", both), refused());
    let neither = without(timer(), "/snapshot/timer");
    assert_eq!(validate_request("update", neither), refused());
}

#[test]
fn req_c_008_a_timer_ends_after_its_start_and_an_end_says_when_only_with_after() {
    // REQ-C-008
    for (started_at, ends_at, valid) in [(0, 1, true), (1, 1, false), (2, 1, false)] {
        let times = json!({"startedAt":started_at,"endsAt":ends_at});
        let verdict = validate_request("update", with(timer(), "/snapshot/timer", times));
        assert_eq!(verdict.is_ok(), valid, "{started_at} to {ends_at}");
    }
    for (policy, valid) in [
        (json!({"kind":"default"}), true),
        (json!({"kind":"immediate"}), true),
        (json!({"kind":"after","at":0}), true),
        (json!({"kind":"after","at":-1}), false),
        (json!({"kind":"after","at":0.5}), false),
        (json!({"kind":"after"}), false),
        (json!({"kind":"immediate","at":0}), false),
        (json!({"kind":"default","at":0}), false),
        (json!({"kind":"later","at":0}), false),
        (json!({}), false),
    ] {
        let verdict = validate_request("end", with(end(), "/policy", policy.clone()));
        assert_eq!(verdict.is_ok(), valid, "{policy}");
    }
}

#[test]
fn req_c_009_a_number_keeps_its_value_when_read_from_json_text() {
    // REQ-C-009
    for text in [
        "977.9461044818163",
        "0.0018531278500143397",
        "96.00000000000001",
        "0.09999999999999999",
        "3875413157790899.5",
        "8640000000000000.6",
    ] {
        let read: Value = serde_json::from_str(text).unwrap();
        assert_eq!(read.as_f64(), text.parse().ok(), "{text}");
    }
    let start = |style: &str, values: &str| {
        let text = format!(
            r#"{{"activityKey":"job:1","schemaVersion":2,"fallback":"none","layout":{{"lockScreen":
            {{"type":"vstack","children":[{{"type":"text","value":"a","style":"s"}},
            {{"type":"timerText","start":"from","end":"to"}}]}}}},"styles":{{"s":{style}}},
            "snapshot":{{"revision":0,"values":{values}}}}}"#
        );
        validate_request("start_layout", serde_json::from_str(&text).unwrap())
    };
    let times = r#"{"from":1,"to":2}"#;
    assert_eq!(start(r#"{"size":12}"#, times), Ok(()));
    assert_eq!(
        start(r#"{"size":96,"minimumScaleFactor":0.1}"#, times),
        Ok(())
    );
    // Each number is just off what its member takes: past the end of a range, or beside a whole number. A read
    // that rounds would bring it back.
    for (style, values) in [
        (r#"{"size":96.00000000000001}"#, times),
        (r#"{"minimumScaleFactor":0.09999999999999999}"#, times),
        (r#"{"size":12}"#, r#"{"from":0.9999999999999999,"to":2}"#),
        (
            r#"{"size":12}"#,
            r#"{"from":3875413157790899.5,"to":3875413157790901}"#,
        ),
        (r#"{"size":12}"#, r#"{"from":1,"to":8640000000000000.6}"#),
    ] {
        assert_eq!(start(style, values), refused(), "{style} {values}");
    }
}

#[test]
fn req_c_010_a_refusal_repeats_nothing_of_the_request() {
    // REQ-C-010
    let secret = "private-input";
    for request in [
        with(progress(), "/snapshot/progress", json!(secret)),
        with(progress(), "/activityKey", json!(format!("../{secret}"))),
        with(progress(), "/snapshot/title", json!(secret.repeat(40))),
        with(progress(), &format!("/{secret}"), json!(1)),
    ] {
        let error = validate_request("start", request).unwrap_err();
        assert_eq!(error, Error::invalid());
        assert!(!format!("{error:?} {error}").contains(secret));
    }
    // A command that the contract does not have is refused the same way.
    assert_eq!(validate_request("endAll", json!({})), refused());
}

/// A start of a layout that the contract takes.
fn layout() -> Value {
    json!({"activityKey":"demo:1","schemaVersion":2,"fallback":"none",
        "layout":{"lockScreen":{"type":"text","value":"a"}},"snapshot":{"revision":0,"values":{}}})
}

/// The verdict on a start of a layout with one more member, on an update of one and on an end of one.
fn started(member: &str, value: Value) -> liveactivity::Result<()> {
    validate_request("start_layout", with(layout(), &format!("/{member}"), value))
}

fn updated(member: &str, value: Value) -> liveactivity::Result<()> {
    let update = json!({"id":"opaque-handle-1","snapshot":{"revision":1,"values":{}}});
    validate_request("update_layout", with(update, &format!("/{member}"), value))
}

fn ended(member: &str, value: Value) -> liveactivity::Result<()> {
    validate_request("end_layout", with(end(), &format!("/{member}"), value))
}

fn alert() -> Value {
    json!({"title":"Your ride","body":"It starts now"})
}

fn public_version() -> Value {
    json!({"title":"Delivery","status":"On the way"})
}

/// A start whose Android content has a public version, of an activity with `visibility`, or with none said.
fn with_a_public_content(visibility: Option<&str>) -> Value {
    let content = json!({"title":"On time","status":"Gate 4",
        "presentation":{"style":{"kind":"standard"},"publicVersion":public_version()}});
    let start = with(layout(), "/snapshot/android", content);
    match visibility {
        Some(visibility) => with(start, "/android", json!({"visibility":visibility})),
        None => start,
    }
}

#[test]
fn req_c_011_the_sound_of_an_alert_is_the_name_of_a_file() {
    // REQ-C-011
    assert_eq!(updated("alert", alert()), Ok(()));
    for sound in ["chime.aiff", "a", "0", "A_b-c.d", &"a".repeat(64)] {
        let alert = with(alert(), "/sound", json!(sound));
        assert_eq!(updated("alert", alert), Ok(()), "{sound}");
    }
    for sound in [
        json!(""),
        json!(".aiff"),
        json!("-a"),
        json!("a b"),
        json!("sounds/chime.aiff"),
        json!("a".repeat(65)),
        json!("é.aiff"),
        json!("chime.aiff\n"),
        json!(1),
        json!(null),
        json!(["a"]),
    ] {
        let alert = with(alert(), "/sound", sound.clone());
        assert_eq!(updated("alert", alert), refused(), "{sound}");
    }
}

#[test]
fn req_c_011_the_options_of_a_start_have_their_members_and_their_json_types() {
    // REQ-C-011
    // On iOS.
    for transient in [true, false] {
        assert_eq!(started("ios", json!({"transient":transient})), Ok(()));
    }
    for ios in [
        json!({}),
        json!({"transient":"yes"}),
        json!({"transient":1}),
        json!({"transient":null}),
        json!({"transient":true,"extra":1}),
        json!(null),
        json!([]),
        json!(true),
    ] {
        assert_eq!(started("ios", ios.clone()), refused(), "{ios}");
    }

    // A scheduled start: a time, up to the largest one, and the alert shown then.
    for at in [0, 1_900_000_000_000, LARGEST_TIME] {
        let scheduled = json!({"at":at,"alert":alert()});
        assert_eq!(started("scheduled", scheduled), Ok(()), "{at}");
    }
    let chime = with(alert(), "/sound", json!("chime.aiff"));
    assert_eq!(started("scheduled", json!({"at":1,"alert":chime})), Ok(()));
    for scheduled in [
        json!({}),
        json!({"at":1}),
        json!({"alert":alert()}),
        json!({"at":-1,"alert":alert()}),
        json!({"at":0.5,"alert":alert()}),
        json!({"at":LARGEST_TIME + 1,"alert":alert()}),
        json!({"at":"1","alert":alert()}),
        json!({"at":null,"alert":alert()}),
        json!({"at":1,"alert":null}),
        json!({"at":1,"alert":{"title":"a"}}),
        json!({"at":1,"alert":with(alert(), "/title", json!(" "))}),
        json!({"at":1,"alert":with(alert(), "/sound", json!("a b"))}),
        json!({"at":1,"alert":with(alert(), "/extra", json!(1))}),
        json!({"at":1,"alert":alert(),"extra":1}),
        json!(null),
        json!([]),
        json!(1),
    ] {
        assert_eq!(
            started("scheduled", scheduled.clone()),
            refused(),
            "{scheduled}"
        );
    }

    // On Android: a visibility, a placement, and a public version of a private activity only.
    for android in [
        json!({"visibility":"private"}),
        json!({"visibility":"private","publicVersion":public_version(),"placement":"foreground"}),
        json!({"visibility":"public","placement":"tagged"}),
        json!({"visibility":"secret","placement":"foreground"}),
        json!({"visibility":"private",
            "publicVersion":{"title":"😀".repeat(120),"status":"한".repeat(240)}}),
    ] {
        assert_eq!(started("android", android.clone()), Ok(()), "{android}");
    }
    for placement in [
        json!("Tagged"),
        json!("service"),
        json!(""),
        json!(null),
        json!(1),
        json!(["tagged"]),
    ] {
        let android = json!({"visibility":"private","placement":placement});
        assert_eq!(started("android", android), refused(), "{placement}");
    }
    for wrong in [
        json!({}),
        json!({"title":"a"}),
        json!({"status":"b"}),
        json!({"title":"","status":"b"}),
        json!({"title":"a","status":" "}),
        json!({"title":"a".repeat(121),"status":"b"}),
        json!({"title":"a","status":"b".repeat(241)}),
        with(public_version(), "/extra", json!(1)),
        json!({"title":1,"status":"b"}),
        json!(null),
        json!([]),
        json!("a"),
    ] {
        let android = json!({"visibility":"private","publicVersion":wrong});
        assert_eq!(started("android", android), refused(), "{wrong}");
    }
    for visibility in ["public", "secret"] {
        let android = json!({"visibility":visibility,"publicVersion":public_version()});
        assert_eq!(started("android", android), refused(), "{visibility}");
    }
    for android in [
        json!({"visibility":"hidden"}),
        json!({"placement":"tagged"}),
        json!({"visibility":"private","lights":true}),
    ] {
        assert_eq!(started("android", android.clone()), refused(), "{android}");
    }
}

#[test]
fn req_c_011_a_public_version_in_a_content_is_of_a_private_start() {
    // REQ-C-011
    // A start is private unless it says otherwise.
    for (visibility, verdict) in [
        (None, Ok(())),
        (Some("private"), Ok(())),
        (Some("public"), refused()),
        (Some("secret"), refused()),
    ] {
        let start = with_a_public_content(visibility);
        assert_eq!(
            validate_request("start_layout", start),
            verdict,
            "{visibility:?}"
        );
    }
    // An update does not say how its activity started: there only Android knows.
    let snapshot = with_a_public_content(None)["snapshot"].clone();
    assert_eq!(updated("snapshot", snapshot), Ok(()));
}

#[test]
fn req_c_011_each_member_belongs_to_its_command() {
    // REQ-C-011
    assert_eq!(validate_request("start_layout", layout()), Ok(()));
    assert_eq!(validate_request("end_layout", end()), Ok(()));
    let options = [
        ("ios", json!({"transient":true})),
        ("scheduled", json!({"at":1,"alert":alert()})),
        ("android", json!({"visibility":"private"})),
    ];
    for (member, value) in options {
        assert_eq!(started(member, value.clone()), Ok(()), "{member}");
        assert_eq!(updated(member, value.clone()), refused(), "{member}");
        assert_eq!(ended(member, value), refused(), "{member}");
    }
    assert_eq!(updated("alert", alert()), Ok(()));
    assert_eq!(started("alert", alert()), refused());
    assert_eq!(ended("alert", alert()), refused());
}

#[test]
fn req_c_011_a_typed_start_writes_its_members_as_it_read_them() {
    // REQ-C-011
    let full = with_a_public_content(Some("private"));
    let full = with(full, "/android/publicVersion", public_version());
    let full = with(full, "/android/placement", json!("foreground"));
    let full = with(full, "/ios", json!({"transient":true}));
    let chime = with(alert(), "/sound", json!("chime.aiff"));
    let full = with(full, "/scheduled", json!({"at":1,"alert":chime}));
    // A member that a request leaves out is not written, and one that says the default is.
    let plain = with(layout(), "/scheduled", json!({"at":1,"alert":alert()}));
    let spelled = with(layout(), "/ios", json!({"transient":false}));
    for start in [full, layout(), plain, spelled] {
        let typed: LayoutStartInput = serde_json::from_value(start.clone()).unwrap();
        assert_eq!(typed.validate(), Ok(()), "{start}");
        assert_eq!(serde_json::to_value(&typed).unwrap(), start);
    }
}
