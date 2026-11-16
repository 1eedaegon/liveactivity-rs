//! The request fixtures of the activitykit crate that this crate builds with, run against this crate's reader.
//! activitykit keeps them beside its own validator and its Swift decoder; here they show that reading a request in
//! this crate's types first gives the verdict activitykit gives, and where this crate's contract takes more. The
//! layouts' fixtures and, since activitykit 0.6.3, those of the templates.

use liveactivity::{
    Error, LayoutEndInput, LayoutStartInput, LayoutUpdateInput, Validate, validate_request, weight,
};
use serde_json::Value;
use std::{fs, path::Path, process::Command};

/// The fixtures whose example of an unknown member is one this crate's contract defines: `relevance`, in a
/// snapshot. activitykit takes a relevance beside its request, this crate inside it.
const TAKEN_HERE: [&str; 1] = ["rejects an unknown member in the final content"];

/// The layout fixtures of the activitykit version in Cargo.lock.
fn fixtures() -> Value {
    fixtures_in("layout-requests.json")
}

/// The file `name` of the fixtures of the activitykit version in Cargo.lock, from that crate's own folder.
fn fixtures_in(name: &str) -> Value {
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
        .join("tests/fixtures")
        .join(name);
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
