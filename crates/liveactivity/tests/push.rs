//! Pushes from the application's server, without the feature `server`: the message, its targets and the request of
//! each platform (REQ-P).

use liveactivity::activitykit::layout::{text, vstack};
use liveactivity::push::{Application, Message, Platform, Priority, Target};
use liveactivity::{
    Alert, Bound, EndPolicy, Island, IslandExpanded, Layout, LayoutSnapshot, LayoutStartInput,
    Value,
};
use serde_json::{Value as Json, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, UNIX_EPOCH},
};

/// A token as the Swift layer reports it: 32 bytes, lowercase hex.
const TOKEN: &str = "0a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f9";
const BUNDLE: &str = "dev.example.app";
/// When the pushes of these tests are sent, in epoch milliseconds: a whole second.
const SENT_AT: u64 = 1_790_000_000_000;

fn values(pairs: Json) -> BTreeMap<String, Value> {
    serde_json::from_value(pairs).unwrap()
}

fn snapshot(revision: u64) -> LayoutSnapshot {
    LayoutSnapshot::new(revision, values(json!({ "eta": "2 min", "progress": 0.5 })))
}

fn application() -> Application {
    Application::new().bundle_id(BUNDLE)
}

fn layout() -> Layout {
    Layout::new(vstack([text(Bound::key("eta"))]))
        .island(Island::new().expanded(IslandExpanded::new().center(text(Bound::key("eta")))))
}

fn activity() -> Target {
    Target::ApnsActivity {
        token: TOKEN.into(),
    }
}

fn header<'a>(headers: &'a [(&'static str, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header, _)| *header == name)
        .map(|(_, value)| value.as_str())
}

fn body(request: &liveactivity::push::Request) -> Json {
    serde_json::from_str(&request.body).unwrap()
}

/// The normal dependencies of the crate built with `features`, as `cargo tree` resolves them.
fn resolved(features: &[&str]) -> Vec<String> {
    let output = Command::new(env!("CARGO"))
        .args(["tree", "--locked", "-p", "liveactivity", "-e", "normal"])
        .args(["--prefix", "none", "--format", "{p}"])
        .args(features)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

#[test]
fn req_p_001_no_build_without_server_resolves_its_dependencies() {
    // REQ-P-001
    let sender = ["reqwest", "jsonwebtoken"];
    for features in [&[][..], &["--no-default-features"][..]] {
        let names = resolved(features);
        assert!(
            names.iter().any(|name| name == "activitykit"),
            "{features:?}"
        );
        for name in sender {
            assert!(
                !names.iter().any(|resolved| resolved == name),
                "{name} with {features:?}"
            );
        }
    }
    let names = resolved(&["--no-default-features", "--features", "server"]);
    for name in sender {
        assert!(names.iter().any(|resolved| resolved == name), "{name}");
    }
    // The server's own runtime is the one its HTTP client brings, not a dependency of this crate's.
    let manifest = include_str!("../Cargo.toml");
    let dependencies = manifest
        .split("[dependencies]")
        .nth(1)
        .and_then(|rest| rest.split("\n[").next())
        .unwrap();
    assert!(!dependencies.contains("tokio"));
}

#[test]
fn req_p_002_a_message_is_checked_as_its_call() {
    // REQ-P-002
    let app = application();
    // What the device would refuse is refused on the server: the value that marks a template, which only this
    // crate's contract knows.
    let marked = LayoutSnapshot::new(1, values(json!({ "liveActivityTemplate": true })));
    let error = Message::update("delivery:7", marked)
        .request(&activity(), &app)
        .unwrap_err();
    assert_eq!(error.code, "INVALID_INPUT");

    // activitykit's own: a start comes with an alert, since the system shows the started activity with it.
    let start = LayoutStartInput::new("delivery:7", layout(), snapshot(0));
    let push_to_start = Target::ApnsPushToStart {
        token: TOKEN.into(),
    };
    let error = Message::start("3f2a9c1e-delivery-7", start.clone())
        .request(&push_to_start, &app)
        .unwrap_err();
    assert_eq!(error.code, "INVALID_INPUT");
    assert!(
        Message::start("3f2a9c1e-delivery-7", start.clone())
            .alert(Alert::new("Delivery", "On its way"))
            .request(&push_to_start, &app)
            .is_ok()
    );

    // A start that the application schedules is the application's: a push starts now.
    let scheduled = LayoutStartInput {
        scheduled: Some(liveactivity::ScheduledStart::new(
            SENT_AT + 60_000,
            Alert::new("Delivery", "Soon"),
        )),
        ..start
    };
    let error = Message::start("3f2a9c1e-delivery-7", scheduled)
        .alert(Alert::new("Delivery", "On its way"))
        .request(&push_to_start, &app)
        .unwrap_err();
    assert_eq!(error.code, "INVALID_INPUT");

    // An end's policy and content belong to an end.
    let error = Message::update("delivery:7", snapshot(2))
        .policy(EndPolicy::Immediate {})
        .request(&activity(), &app)
        .unwrap_err();
    assert_eq!(error.code, "INVALID_INPUT");
    let error = Message::update("delivery:7", snapshot(2))
        .snapshot(snapshot(3))
        .request(&activity(), &app)
        .unwrap_err();
    assert_eq!(error.code, "INVALID_INPUT");

    // A key is a key of the contract.
    let error = Message::update("", snapshot(2))
        .request(&activity(), &app)
        .unwrap_err();
    assert_eq!(error.code, "INVALID_INPUT");

    // More than APNs takes is refused before it is sent.
    let long = "x".repeat(200);
    let many = (0..30)
        .map(|index| (format!("line{index}"), Json::from(long.clone())))
        .collect::<serde_json::Map<_, _>>();
    let error = Message::update(
        "delivery:7",
        LayoutSnapshot::new(2, values(Json::Object(many))),
    )
    .request(&activity(), &app)
    .unwrap_err();
    assert_eq!(error.code, "PAYLOAD_TOO_LARGE");
}

#[test]
fn req_p_003_a_target_is_the_json_an_application_sends() {
    // REQ-P-003
    let targets = [
        (
            activity(),
            json!({ "type": "apnsActivity", "token": TOKEN }),
        ),
        (
            Target::ApnsPushToStart {
                token: TOKEN.into(),
            },
            json!({ "type": "apnsPushToStart", "token": TOKEN }),
        ),
        (
            Target::ApnsChannel {
                channel: "dHJhY2tpbmc=".into(),
            },
            json!({ "type": "apnsChannel", "channel": "dHJhY2tpbmc=" }),
        ),
        (
            Target::Fcm {
                token: "dK3x:APA91b".into(),
            },
            json!({ "type": "fcm", "token": "dK3x:APA91b" }),
        ),
    ];
    for (target, wire) in targets {
        assert_eq!(serde_json::to_value(&target).unwrap(), wire);
        assert_eq!(serde_json::from_value::<Target>(wire).unwrap(), target);
    }
    assert_eq!(activity().platform(), Platform::Apns);
    assert_eq!(Target::Fcm { token: "t".into() }.platform(), Platform::Fcm);
    // A member the application should not send is refused, as the contract refuses one.
    assert!(
        serde_json::from_value::<Target>(
            json!({ "type": "apnsActivity", "token": TOKEN, "key": "x" })
        )
        .is_err()
    );
    assert!(serde_json::from_value::<Target>(json!({ "type": "apns", "token": TOKEN })).is_err());

    let app = application();
    // A token is hex of whole bytes; a channel is base64.
    for token in ["", "0a1", "zz", &"0".repeat(2050)] {
        let target = Target::ApnsActivity {
            token: token.into(),
        };
        let error = Message::update("delivery:7", snapshot(2))
            .request(&target, &app)
            .unwrap_err();
        assert_eq!(error.code, "INVALID_INPUT", "{token:?}");
    }
    let channel = Target::ApnsChannel {
        channel: "not base64!".into(),
    };
    let error = Message::update("delivery:7", snapshot(2))
        .request(&channel, &app)
        .unwrap_err();
    assert_eq!(error.code, "INVALID_INPUT");

    // A start goes to the push-to-start token alone, and an update or an end never goes there.
    let start = Message::start(
        "3f2a9c1e-delivery-7",
        LayoutStartInput::new("delivery:7", layout(), snapshot(0)),
    )
    .alert(Alert::new("Delivery", "On its way"));
    for target in [
        activity(),
        Target::ApnsChannel {
            channel: "dHJhY2tpbmc=".into(),
        },
    ] {
        assert_eq!(
            start.request(&target, &app).unwrap_err().code,
            "INVALID_INPUT"
        );
    }
    let push_to_start = Target::ApnsPushToStart {
        token: TOKEN.into(),
    };
    for message in [
        Message::update("delivery:7", snapshot(2)),
        Message::end("delivery:7"),
    ] {
        assert_eq!(
            message.request(&push_to_start, &app).unwrap_err().code,
            "INVALID_INPUT"
        );
    }
}

#[test]
fn req_p_004_the_apns_request() {
    // REQ-P-004
    let app = application();
    let update = Message::update("delivery:7", snapshot(3))
        .alert(Alert::new("Delivery", "2 minutes"))
        .sent_at(SENT_AT);
    let request = update.request(&activity(), &app).unwrap();
    assert_eq!(request.platform, Platform::Apns);
    assert_eq!(request.path, format!("/3/device/{TOKEN}"));
    assert_eq!(
        header(&request.headers, "apns-push-type"),
        Some("liveactivity")
    );
    assert_eq!(
        header(&request.headers, "apns-topic"),
        Some("dev.example.app.push-type.liveactivity")
    );
    assert_eq!(header(&request.headers, "apns-priority"), Some("10"));
    assert_eq!(header(&request.headers, "apns-channel-id"), None);
    // The body is what activitykit writes for the same push.
    let kit = liveactivity::activitykit::push::Payload::update(
        liveactivity::activitykit::Values::from(snapshot(3).values),
    )
    .revision(3)
    .alert(liveactivity::activitykit::Alert::new(
        "Delivery",
        "2 minutes",
    ))
    .sent_at(UNIX_EPOCH + Duration::from_millis(SENT_AT))
    .to_value()
    .unwrap();
    assert_eq!(body(&request), kit);
    assert_eq!(body(&request)["aps"]["event"], "update");
    assert_eq!(body(&request)["aps"]["content-state"]["revision"], 3);

    // A token in capitals is the same token.
    let upper = Target::ApnsActivity {
        token: TOKEN.to_uppercase(),
    };
    assert_eq!(
        update.request(&upper, &app).unwrap().path,
        format!("/3/device/{TOKEN}")
    );

    // A push that may wait.
    let low = Message::update("delivery:7", snapshot(4))
        .priority(Priority::Low)
        .request(&activity(), &app)
        .unwrap();
    assert_eq!(header(&low.headers, "apns-priority"), Some("5"));

    // An end: with the time it leaves the screen, at once, or at the system's time.
    let after = SENT_AT + 600_000;
    let end = Message::end("delivery:7")
        .snapshot(snapshot(5))
        .policy(EndPolicy::After { at: after })
        .sent_at(SENT_AT)
        .request(&activity(), &app)
        .unwrap();
    let aps = &body(&end)["aps"];
    assert_eq!(aps["event"], "end");
    assert_eq!(aps["content-state"]["revision"], 5);
    assert_eq!(aps["dismissal-date"], after / 1000);
    let now = Message::end("delivery:7")
        .policy(EndPolicy::Immediate {})
        .sent_at(SENT_AT)
        .request(&activity(), &app)
        .unwrap();
    assert_eq!(body(&now)["aps"]["dismissal-date"], SENT_AT / 1000);
    assert!(body(&now)["aps"].get("content-state").is_none());
    let default = Message::end("delivery:7")
        .request(&activity(), &app)
        .unwrap();
    assert!(body(&default)["aps"].get("dismissal-date").is_none());

    // A broadcast channel's request has its own path and no topic.
    let channel = Target::ApnsChannel {
        channel: "dHJhY2tpbmc=".into(),
    };
    let broadcast = Message::update("delivery:7", snapshot(6))
        .request(&channel, &app)
        .unwrap();
    assert_eq!(broadcast.path, "/4/broadcasts/apps/dev.example.app");
    assert_eq!(
        header(&broadcast.headers, "apns-channel-id"),
        Some("dHJhY2tpbmc=")
    );
    assert_eq!(header(&broadcast.headers, "apns-topic"), None);
    assert_eq!(
        header(&broadcast.headers, "apns-push-type"),
        Some("liveactivity")
    );

    // A start, to the push-to-start token: the layout and the key in the attributes, the alert required.
    let start = Message::start(
        "3f2a9c1e-delivery-7",
        LayoutStartInput::new("delivery:7", layout(), snapshot(0)),
    )
    .alert(Alert::new("Delivery", "On its way"))
    .request(
        &Target::ApnsPushToStart {
            token: TOKEN.into(),
        },
        &app,
    )
    .unwrap();
    assert_eq!(start.path, format!("/3/device/{TOKEN}"));
    let aps = &body(&start)["aps"];
    assert_eq!(aps["event"], "start");
    assert_eq!(aps["attributes-type"], "LiveActivityLayoutAttributes");
    assert_eq!(aps["attributes"]["activityKey"], "delivery:7");
    assert_eq!(aps["attributes"]["instanceID"], "3f2a9c1e-delivery-7");
    assert_eq!(aps["alert"]["title"], "Delivery");

    // The topic needs the application's bundle id.
    let error = update
        .request(&activity(), &Application::new())
        .unwrap_err();
    assert_eq!(error.code, "INVALID_INPUT");
}

const FCM_TOKEN: &str = "dK3x:APA91bHPRgkF3JUikC4ENAHEeMrd41Zxv3hVZjC9KtT8OvPVGJ-hQMRKRrZuJAEcl7B338qju59zJMjw2DELjzEvxwYv7hH5Ynpc1ODQ0aT4U4OFEeBHWsHO";

fn fcm() -> Target {
    Target::Fcm {
        token: FCM_TOKEN.into(),
    }
}

fn android() -> Application {
    Application::new().fcm_project("demo-project")
}

/// The data member that the device's layer takes, read back from the request.
fn layer(request: &liveactivity::push::Request) -> Json {
    let body = body(request);
    let member = body["message"]["data"]["liveupdate.layer"]
        .as_str()
        .unwrap();
    serde_json::from_str(member).unwrap()
}

#[test]
fn req_a_036_the_fcm_request() {
    // REQ-A-036, REQ-P-005
    let update =
        Message::update("delivery:7", snapshot(3)).alert(Alert::new("Delivery", "2 minutes"));
    let request = update.request(&fcm(), &android()).unwrap();
    assert_eq!(request.platform, Platform::Fcm);
    assert_eq!(request.path, "/v1/projects/demo-project/messages:send");
    assert_eq!(
        header(&request.headers, "content-type"),
        Some("application/json")
    );
    let message = &body(&request)["message"];
    assert_eq!(message["token"], FCM_TOKEN);
    assert_eq!(message["android"]["priority"], "high");
    assert_eq!(
        message["data"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["liveupdate.layer"]
    );
    assert_eq!(
        layer(&request),
        json!({ "v": 1, "command": "update", "activityKey": "delivery:7",
            "snapshot": { "revision": 3, "values": { "eta": "2 min", "progress": 0.5 } },
            "alert": { "title": "Delivery", "body": "2 minutes" } })
    );
    let low = Message::update("delivery:7", snapshot(4))
        .priority(Priority::Low)
        .request(&fcm(), &android())
        .unwrap();
    assert_eq!(body(&low)["message"]["android"]["priority"], "normal");

    // An end with its last content and its policy; a start with its whole request, which needs no alert here.
    let end = Message::end("delivery:7")
        .snapshot(snapshot(5))
        .policy(EndPolicy::After { at: SENT_AT })
        .request(&fcm(), &android())
        .unwrap();
    assert_eq!(
        layer(&end),
        json!({ "v": 1, "command": "end", "activityKey": "delivery:7",
            "snapshot": { "revision": 5, "values": { "eta": "2 min", "progress": 0.5 } },
            "policy": { "kind": "after", "at": SENT_AT } })
    );
    let input = LayoutStartInput::new("delivery:7", layout(), snapshot(0));
    let start = Message::start("ignored-on-android", input.clone())
        .request(&fcm(), &android())
        .unwrap();
    let wire = layer(&start);
    assert_eq!(wire["command"], "start");
    assert_eq!(wire["activityKey"], "delivery:7");
    assert_eq!(wire["start"], serde_json::to_value(&input).unwrap());

    // Checked as the call: a scheduled start, a marked value, a key that is none.
    let scheduled = LayoutStartInput {
        scheduled: Some(liveactivity::ScheduledStart::new(
            SENT_AT,
            Alert::new("Delivery", "Soon"),
        )),
        ..input
    };
    assert_eq!(
        Message::start("x", scheduled)
            .request(&fcm(), &android())
            .unwrap_err()
            .code,
        "INVALID_INPUT"
    );
    let marked = LayoutSnapshot::new(1, values(json!({ "liveActivityTemplate": true })));
    assert_eq!(
        Message::update("delivery:7", marked)
            .request(&fcm(), &android())
            .unwrap_err()
            .code,
        "INVALID_INPUT"
    );
    assert_eq!(
        Message::update("", snapshot(1))
            .request(&fcm(), &android())
            .unwrap_err()
            .code,
        "INVALID_INPUT"
    );
    // The project and the token.
    for application in [
        Application::new(),
        Application::new().fcm_project("Demo Project"),
    ] {
        assert_eq!(
            update.request(&fcm(), &application).unwrap_err().code,
            "INVALID_INPUT"
        );
    }
    for token in ["", "a b", "line\nbreak", &"t".repeat(4097)] {
        let target = Target::Fcm {
            token: token.into(),
        };
        assert_eq!(
            update.request(&target, &android()).unwrap_err().code,
            "INVALID_INPUT",
            "{token:?}"
        );
    }
    // More than a data message carries.
    let many = (0..30)
        .map(|index| (format!("line{index}"), Json::from("x".repeat(200))))
        .collect::<serde_json::Map<_, _>>();
    let large = Message::update(
        "delivery:7",
        LayoutSnapshot::new(2, values(Json::Object(many))),
    );
    assert_eq!(
        large.request(&fcm(), &android()).unwrap_err().code,
        "PAYLOAD_TOO_LARGE"
    );
    // One application names both platforms.
    let both = Application::new()
        .bundle_id(BUNDLE)
        .fcm_project("demo-project");
    assert!(update.request(&fcm(), &both).is_ok());
    assert!(update.request(&activity(), &both).is_ok());
}

/// The Rust files under `folder`.
fn sources(folder: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    found
}

#[test]
fn req_p_008_the_crate_reads_no_environment() {
    // REQ-P-008
    let push = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/push");
    let files = sources(&push);
    assert!(!files.is_empty());
    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        for read in ["env::var", "std::env", "env!(", "option_env!("] {
            assert!(!text.contains(read), "{}: {read}", file.display());
        }
        for handler in ["axum", "Router", "hyper::server"] {
            assert!(!text.contains(handler), "{}: {handler}", file.display());
        }
    }
}

#[test]
fn req_p_004_what_a_start_and_a_snapshot_carry() {
    // REQ-P-004
    let app = application();
    let push_to_start = Target::ApnsPushToStart {
        token: TOKEN.into(),
    };
    let started = |input: LayoutStartInput| {
        Message::start("3f2a9c1e-delivery-7", input)
            .alert(Alert::new("Delivery", "On its way").with_sound("chime.aiff"))
            .request(&push_to_start, &app)
    };
    // A start that asks for the activity's token, with the alert's sound.
    let asking = LayoutStartInput {
        push: Some(liveactivity::LayoutPush::Token),
        ..LayoutStartInput::new("delivery:7", layout(), snapshot(0))
    };
    let aps = body(&started(asking).unwrap())["aps"].clone();
    assert_eq!(aps["input-push-token"], 1);
    assert_eq!(aps["alert"]["sound"], "chime.aiff");
    // A channel is the application's to subscribe: a push-to-start cannot ask for one.
    let channel = LayoutStartInput {
        push: Some(liveactivity::LayoutPush::Channel("dHJhY2tpbmc=".into())),
        ..LayoutStartInput::new("delivery:7", layout(), snapshot(0))
    };
    assert_eq!(started(channel).unwrap_err().code, "UNSUPPORTED_FEATURE");
    // A transient activity is the application's.
    let transient = LayoutStartInput {
        ios: Some(liveactivity::IosOptions::new(true)),
        ..LayoutStartInput::new("delivery:7", layout(), snapshot(0))
    };
    assert_eq!(started(transient).unwrap_err().code, "INVALID_INPUT");

    // A snapshot's staleness, its time and its relevance go with it.
    let stale_at = SENT_AT + 300_000;
    let timed = LayoutSnapshot {
        stale_at: Some(stale_at),
        timestamp: Some(SENT_AT - 1_000),
        relevance: Some(0.75),
        ..snapshot(7)
    };
    let request = Message::update("delivery:7", timed)
        .request(&activity(), &app)
        .unwrap();
    let aps = body(&request)["aps"].clone();
    assert_eq!(aps["stale-date"], stale_at / 1000);
    assert_eq!(aps["relevance-score"], 0.75);
    assert_eq!(aps["content-state"]["staleAt"], stale_at);
    assert_eq!(aps["content-state"]["timestamp"], SENT_AT - 1_000);

    // An end that may wait.
    let end = Message::end("delivery:7")
        .priority(Priority::Low)
        .request(&activity(), &app)
        .unwrap();
    assert_eq!(header(&end.headers, "apns-priority"), Some("5"));

    // A bundle id goes into a path: nothing else than letters, digits, `.` and `-`.
    for bundle in ["", "dev/example", "dev.example?x=1", "dev example"] {
        let error = Message::update("delivery:7", snapshot(2))
            .request(&activity(), &Application::new().bundle_id(bundle))
            .unwrap_err();
        assert_eq!(error.code, "INVALID_INPUT", "{bundle:?}");
    }
    // Nor does a channel carry a line break into a header.
    let broken = Target::ApnsChannel {
        channel: "dGVz\r\ndA==".into(),
    };
    let error = Message::update("delivery:7", snapshot(2))
        .request(&broken, &app)
        .unwrap_err();
    assert_eq!(error.code, "INVALID_INPUT");
}
