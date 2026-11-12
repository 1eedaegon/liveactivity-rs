//! The request fixtures of the activitykit crate that this crate builds with, run against this crate's reader.
//! activitykit keeps them beside its own validator and its Swift decoder; here they show that reading a request in
//! this crate's types first gives the verdict activitykit gives, and where this crate's contract takes more.

use liveactivity::{LayoutEndInput, LayoutStartInput, LayoutUpdateInput, validate_request};
use serde_json::Value;
use std::{fs, path::Path, process::Command};

/// The fixtures whose example of an unknown member is one this crate's contract defines: `relevance`, in a
/// snapshot. activitykit takes a relevance beside its request, this crate inside it.
const TAKEN_HERE: [&str; 1] = ["rejects an unknown member in the final content"];

/// The fixtures of the activitykit version in Cargo.lock, from that crate's own folder.
fn fixtures() -> Value {
    let metadata = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--locked"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo answers its metadata");
    assert!(metadata.status.success(), "cargo metadata failed");
    let metadata: Value = serde_json::from_slice(&metadata.stdout).unwrap();
    let manifest = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "activitykit")
        .expect("activitykit is a dependency")["manifest_path"]
        .as_str()
        .unwrap()
        .to_owned();
    let file = Path::new(&manifest)
        .parent()
        .unwrap()
        .join("tests/fixtures/layout-requests.json");
    serde_json::from_slice(&fs::read(file).expect("activitykit ships its fixtures")).unwrap()
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

#[test]
fn req_c_005_updates_and_ends_are_checked_against_their_start_as_the_engine_does() {
    // REQ-C-005
    let fixtures = fixtures();
    let (updates, ends) = (cases(&fixtures, "updates"), cases(&fixtures, "ends"));
    assert!(updates.len() >= 20 && !ends.is_empty());
    let started = |case: &Value| -> LayoutStartInput {
        let start = case["start"].clone();
        assert_eq!(code(validate_request("start_layout", start.clone())), None);
        serde_json::from_value(start).unwrap()
    };
    for case in updates {
        let start = started(case);
        let verdict = validate_request("update_layout", case["update"].clone()).and_then(|()| {
            let update: LayoutUpdateInput = serde_json::from_value(case["update"].clone()).unwrap();
            start.validate_update(&update)
        });
        let expected = case.get("code").and_then(Value::as_str);
        assert_eq!(code(verdict).as_deref(), expected, "{}", case["name"]);
    }
    for case in ends {
        let start = started(case);
        let verdict = validate_request("end_layout", case["end"].clone()).and_then(|()| {
            let end: LayoutEndInput = serde_json::from_value(case["end"].clone()).unwrap();
            start.validate_end(&end)
        });
        let expected = case.get("code").and_then(Value::as_str);
        assert_eq!(code(verdict).as_deref(), expected, "{}", case["name"]);
    }
}
