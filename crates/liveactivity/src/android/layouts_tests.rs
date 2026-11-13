use super::*;
use serde_json::{Value as Json, json};
use std::sync::{Arc, Mutex as StdMutex};

/// liveupdate's Kotlin side in memory: the receipts, the posted notifications and the event queue.
#[derive(Default)]
struct Android {
    /// Posts fail while set: liveupdate's start then answers an error after it wrote its receipt.
    refuse_posts: bool,
    receipts: Vec<liveupdate::DriverRecord>,
    posted: BTreeMap<String, (liveupdate::Snapshot, bool)>,
    events: Vec<liveupdate::ActivityEvent>,
    starts: usize,
}

#[derive(Clone, Default)]
struct FakeDriver(Arc<StdMutex<Android>>);

impl liveupdate::Driver for FakeDriver {
    fn capabilities(&self) -> liveupdate::Result<liveupdate::Capabilities> {
        Ok(liveupdate::Capabilities {
            supported: true,
            notifications_enabled: true,
            promotion_enabled: true,
            presentation: liveupdate::PresentationCapabilities {
                standard: true,
                big_text: true,
                timer: true,
                progress_style: true,
                metric_style: true,
                actions: true,
                images: true,
                events: true,
                samsung_now_bar: true,
                // The extensions of liveupdate 0.2 are not offered: the rules for a device without them.
                ..liveupdate::PresentationCapabilities::default()
            },
        })
    }
    fn read(&self) -> liveupdate::Result<Vec<liveupdate::DriverRecord>> {
        Ok(self.0.lock().unwrap().receipts.clone())
    }
    fn write(&self, records: &[liveupdate::DriverRecord]) -> liveupdate::Result<()> {
        self.0.lock().unwrap().receipts = records.to_vec();
        Ok(())
    }
    fn observe(&self) -> liveupdate::Result<Vec<liveupdate::Observation>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .posted
            .iter()
            .map(
                |(id, (snapshot, promotion_requested))| liveupdate::Observation {
                    id: id.clone(),
                    snapshot: snapshot.clone(),
                    state: liveupdate::ActivityState::Active,
                    promotion_requested: *promotion_requested,
                    promotion_state: liveupdate::PromotionState::NotPromoted,
                },
            )
            .collect())
    }
    fn post(&self, record: &liveupdate::DriverRecord) -> liveupdate::Result<()> {
        let mut android = self.0.lock().unwrap();
        if android.refuse_posts {
            return Err(liveupdate::Error::new(
                liveupdate::ErrorCode::NativeError,
                "refused",
            ));
        }
        if !android.posted.contains_key(&record.id) {
            android.starts += 1;
        }
        android.posted.insert(
            record.id.clone(),
            (record.snapshot.clone(), record.promotion_requested),
        );
        Ok(())
    }
    fn cancel(&self, id: &str) -> liveupdate::Result<()> {
        self.0.lock().unwrap().posted.remove(id);
        Ok(())
    }
    fn drain_events(&self) -> liveupdate::Result<liveupdate::EventBatch> {
        let events = std::mem::take(&mut self.0.lock().unwrap().events);
        Ok(liveupdate::EventBatch { events, dropped: 0 })
    }
}

#[derive(Clone, Default)]
struct MemoryStore(Arc<StdMutex<Entries>>, Arc<StdMutex<bool>>);

impl MemoryStore {
    fn corrupt(&self, corrupt: bool) {
        *self.1.lock().unwrap() = corrupt;
    }
}

impl Store for MemoryStore {
    fn load(&self) -> Result<Entries> {
        if *self.1.lock().unwrap() {
            return Err(Error::native());
        }
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, entries: &Entries) -> Result<()> {
        *self.0.lock().unwrap() = entries.clone();
        Ok(())
    }
}

struct Fixture {
    android: FakeDriver,
    store: MemoryStore,
    updates: liveupdate::LiveUpdate<FakeDriver>,
    layouts: Layouts<FakeDriver, MemoryStore>,
}

fn fixture() -> Fixture {
    let android = FakeDriver::default();
    let store = MemoryStore::default();
    let settings = Settings {
        app_name: "Mochi".into(),
        samsung_now_bar: false,
        files: None,
        image_folder: "liveactivity/images".into(),
    };
    Fixture {
        updates: liveupdate::LiveUpdate::new(android.clone()),
        layouts: Layouts::new(settings, store.clone()),
        android,
        store,
    }
}

fn start(key: &str, values: Json) -> LayoutStartInput {
    serde_json::from_value(json!({
        "activityKey": key,
        "schemaVersion": 2,
        "fallback": "notification",
        "layout": { "lockScreen": { "type": "vstack", "children": [
            { "type": "text", "bind": "title" },
            { "type": "progress", "bind": "done" },
            { "type": "button", "actionId": "pet", "child": { "type": "text", "value": "Pet" } },
        ] } },
        "snapshot": { "revision": 0, "values": values },
    }))
    .unwrap()
}

fn snapshot(revision: u64, values: Json) -> LayoutSnapshot {
    serde_json::from_value(json!({ "revision": revision, "values": values })).unwrap()
}

fn values(title: &str, done: f64) -> Json {
    json!({ "title": title, "done": done })
}

fn update(id: &str, snapshot: LayoutSnapshot) -> LayoutUpdateInput {
    LayoutUpdateInput::new(id, snapshot)
}

fn code(result: Result<impl std::fmt::Debug>) -> String {
    result.unwrap_err().code
}
#[test]
fn a_layout_start_is_one_liveupdate_v2_start_and_its_record_has_the_values() {
    let f = fixture();
    let record = f
        .layouts
        .start(&f.updates, start("snack:1", values("Snack", 0.2)))
        .unwrap();
    assert_eq!(f.android.0.lock().unwrap().starts, 1);
    let receipt = f.android.0.lock().unwrap().receipts[0].clone();
    assert_eq!(receipt.origin.schema_version, 2);
    assert_eq!(receipt.origin.template, liveupdate::Template::Progress);
    assert_eq!(receipt.snapshot.title, "Snack");
    assert_eq!(
        (record.schema_version, record.backend),
        (2, Backend::AndroidNotification)
    );
    assert_eq!(record.snapshot, snapshot(0, values("Snack", 0.2)));
    assert_eq!(record.activity_key, "snack:1");
    assert_eq!(
        record.degraded_features.first().map(String::as_str),
        Some("nativeLiveActivity")
    );
    let listed = f.layouts.list(&f.updates).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(
        (listed[0].id.as_str(), &listed[0].snapshot),
        (record.id.as_str(), &record.snapshot)
    );
}
#[test]
fn a_record_the_store_does_not_know_is_left_out() {
    let f = fixture();
    let record = f
        .layouts
        .start(&f.updates, start("snack:1", values("Snack", 0.2)))
        .unwrap();
    f.store.save(&Entries::new()).unwrap();
    assert_eq!(f.layouts.list(&f.updates).unwrap(), []);
    assert_eq!(
        code(f.layouts.update(
            &f.updates,
            update(&record.id, snapshot(1, values("x", 0.3)))
        )),
        "NOT_FOUND"
    );
}
#[test]
fn a_snapshot_timestamp_is_kept_at_start_update_and_end() {
    let f = fixture();
    let mut input = start("snack:1", values("Snack", 0.2));
    input.snapshot.timestamp = Some(1_000);
    let record = f.layouts.start(&f.updates, input).unwrap();
    assert_eq!(record.snapshot.timestamp, Some(1_000));
    let mut next = snapshot(1, values("Snack", 0.4));
    next.timestamp = Some(2_000);
    assert_eq!(
        f.layouts
            .update(&f.updates, update(&record.id, next.clone()))
            .unwrap()
            .snapshot
            .timestamp,
        Some(2_000)
    );
    let mut last = snapshot(2, values("Snack", 1.0));
    last.timestamp = Some(3_000);
    let end = LayoutEndInput {
        id: record.id,
        policy: EndPolicy::Immediate {},
        snapshot: Some(last),
    };
    assert_eq!(
        f.layouts.end(&f.updates, end).unwrap().snapshot.timestamp,
        Some(3_000)
    );
}
#[test]
fn an_end_with_final_content_updates_then_ends() {
    let f = fixture();
    let record = f
        .layouts
        .start(&f.updates, start("snack:1", values("Snack", 0.2)))
        .unwrap();
    let last = snapshot(4, values("Delivered", 1.0));
    let end = LayoutEndInput {
        id: record.id.clone(),
        policy: EndPolicy::Default {},
        snapshot: Some(last.clone()),
    };
    let ended = f.layouts.end(&f.updates, end).unwrap();
    assert_eq!(ended.state, crate::ActivityState::Ended);
    assert_eq!(ended.snapshot, last);
    assert!(
        f.android.0.lock().unwrap().posted.is_empty(),
        "the notification is cancelled"
    );
    assert_eq!(
        f.layouts.list(&f.updates).unwrap()[0].snapshot,
        last,
        "the record keeps the final content"
    );

    // A stale final content is refused before anything ends.
    let f = fixture();
    let record = f
        .layouts
        .start(&f.updates, start("snack:2", values("Snack", 0.2)))
        .unwrap();
    f.layouts
        .update(
            &f.updates,
            update(&record.id, snapshot(3, values("Snack", 0.5))),
        )
        .unwrap();
    let end = LayoutEndInput {
        id: record.id,
        policy: EndPolicy::Immediate {},
        snapshot: Some(snapshot(2, values("Old", 0.4))),
    };
    assert_eq!(code(f.layouts.end(&f.updates, end)), "STALE_REVISION");
    assert_eq!(f.android.0.lock().unwrap().posted.len(), 1);
}
#[test]
fn an_end_after_a_time_is_unsupported() {
    let f = fixture();
    let record = f
        .layouts
        .start(&f.updates, start("snack:1", values("Snack", 0.2)))
        .unwrap();
    let end = |id: &str| LayoutEndInput {
        id: id.into(),
        policy: EndPolicy::After {
            at: 1_900_000_000_000,
        },
        snapshot: None,
    };
    assert_eq!(code(f.layouts.end(&f.updates, end("unknown"))), "NOT_FOUND");
    assert_eq!(
        code(f.layouts.end(&f.updates, end(&record.id))),
        "UNSUPPORTED_FEATURE"
    );
}
#[test]
fn a_keyed_start_returns_the_running_activity_and_another_start_conflicts() {
    let f = fixture();
    let first = f
        .layouts
        .start(&f.updates, start("snack:1", values("Snack", 0.2)))
        .unwrap();
    let again = f
        .layouts
        .start(&f.updates, start("snack:1", values("Snack", 0.2)))
        .unwrap();
    assert_eq!(again.id, first.id);
    assert_eq!(f.android.0.lock().unwrap().starts, 1);
    // The title is the same, so the projection is too; the values are not.
    let mut other = start("snack:1", values("Snack", 0.2));
    other
        .snapshot
        .values
        .insert("extra".into(), crate::Value::Bool(true));
    assert_eq!(code(f.layouts.start(&f.updates, other)), "CONFLICT");
    // Once the activity ended, the key starts a new one.
    f.layouts
        .end(
            &f.updates,
            LayoutEndInput {
                id: first.id.clone(),
                policy: EndPolicy::Immediate {},
                snapshot: None,
            },
        )
        .unwrap();
    let next = f
        .layouts
        .start(&f.updates, start("snack:1", values("Snack", 0.9)))
        .unwrap();
    assert_ne!(next.id, first.id);
}
#[test]
fn updates_keep_the_revision_rules_of_the_contract() {
    let f = fixture();
    let record = f
        .layouts
        .start(&f.updates, start("snack:1", values("Snack", 0.2)))
        .unwrap();
    let id = record.id.as_str();
    let updated = f
        .layouts
        .update(&f.updates, update(id, snapshot(2, values("Snack", 0.5))))
        .unwrap();
    assert_eq!(updated.snapshot.revision, 2);
    assert_eq!(
        code(
            f.layouts
                .update(&f.updates, update(id, snapshot(1, values("Snack", 0.6))))
        ),
        "STALE_REVISION"
    );
    assert_eq!(
        code(
            f.layouts
                .update(&f.updates, update(id, snapshot(2, values("Snack", 0.6))))
        ),
        "CONFLICT"
    );
    // The same revision with the same values answers the record without posting.
    assert_eq!(
        f.layouts
            .update(&f.updates, update(id, snapshot(2, values("Snack", 0.5))))
            .unwrap()
            .snapshot
            .revision,
        2
    );
    // An update keeps the keys and types of the start.
    assert_eq!(
        code(f.layouts.update(
            &f.updates,
            update(id, snapshot(3, json!({ "title": 3, "done": 0.5 })))
        )),
        "INVALID_INPUT"
    );
    assert_eq!(
        code(f.layouts.update(
            &f.updates,
            update("unknown", snapshot(3, values("Snack", 0.6)))
        )),
        "NOT_FOUND"
    );
    let posted = f.android.0.lock().unwrap().posted[id].0.clone();
    assert_eq!(posted.revision, 2);
}
#[test]
fn the_store_forgets_what_liveupdate_no_longer_keeps() {
    let f = fixture();
    let record = f
        .layouts
        .start(&f.updates, start("snack:1", values("Snack", 0.2)))
        .unwrap();
    // A start that never got an answer leaves a pending entry; liveupdate forgets the activity.
    let mut stored = f.store.load().unwrap();
    stored.insert(format!("{PENDING}snack:9"), stored[&record.id].clone());
    f.store.save(&stored).unwrap();
    f.android.0.lock().unwrap().receipts.clear();
    f.android.0.lock().unwrap().posted.clear();
    assert_eq!(f.layouts.list(&f.updates).unwrap(), []);
    assert!(f.store.load().unwrap().is_empty());
}
#[test]
fn list_and_list_layout_each_show_their_own_schema() {
    let f = fixture();
    let layout = f
        .layouts
        .start(&f.updates, start("snack:1", values("Snack", 0.2)))
        .unwrap();
    let template: crate::StartInput = serde_json::from_value(json!({
        "activityKey": "delivery:1", "schemaVersion": 1, "template": "progress", "fallback": "notification",
        "snapshot": { "revision": 0, "title": "Delivery", "status": "Preparing", "progress": 0.1 },
    }))
    .unwrap();
    let v1 = f
        .updates
        .start(android_liveupdate::start_request(template))
        .unwrap();
    let layouts = f.layouts.list(&f.updates).unwrap();
    assert_eq!(
        layouts
            .iter()
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>(),
        [layout.id.as_str()]
    );
    let records = android_liveupdate::v1_records(f.updates.list().unwrap()).unwrap();
    assert_eq!(
        records
            .iter()
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>(),
        [v1.id.as_str()]
    );
}
#[test]
fn a_stored_tap_comes_out_with_the_activity_key() {
    let f = fixture();
    let record = f
        .layouts
        .start(&f.updates, start("snack:1", values("Snack", 0.2)))
        .unwrap();
    let tap = |sequence: u64, activity: &str, action: &str| liveupdate::ActivityEvent {
        sequence,
        activity_id: activity.into(),
        occurred_at: 1_000,
        kind: liveupdate::EventKind::Action {
            action_id: action.into(),
        },
    };
    f.android.0.lock().unwrap().events = vec![
        tap(1, &record.id, "pet"),
        liveupdate::ActivityEvent {
            kind: liveupdate::EventKind::Dismissed,
            ..tap(2, &record.id, "pet")
        },
        tap(3, "gone", "pet"),
    ];
    // The stored dismissal is no event of the store's: the backend reports it with the other states.
    assert_eq!(
        f.layouts.events(&f.updates).unwrap(),
        [
            Event::Action(ActionEvent {
                action_id: "pet".into(),
                activity_id: record.id.clone(),
                activity_key: Some("snack:1".into()),
                on: None,
            }),
            Event::Action(ActionEvent {
                action_id: "pet".into(),
                activity_id: "gone".into(),
                activity_key: None,
                on: None,
            }),
        ]
    );
    assert_eq!(f.layouts.events(&f.updates).unwrap(), []);
}
#[test]
fn taps_of_undeclared_actions_are_dropped() {
    let f = fixture();
    let record = f
        .layouts
        .start(&f.updates, start("snack:1", values("Snack", 0.2)))
        .unwrap();
    f.android.0.lock().unwrap().events = vec![liveupdate::ActivityEvent {
        sequence: 1,
        activity_id: record.id,
        occurred_at: 1_000,
        kind: liveupdate::EventKind::Action {
            action_id: "feed".into(),
        },
    }];
    assert_eq!(f.layouts.events(&f.updates).unwrap(), []);
}

#[test]
fn a_file_store_keeps_the_entries_across_instances() {
    let folder = std::env::temp_dir().join(format!("live-activity-store-{}", std::process::id()));
    let store = FileStore {
        path: folder.join("layouts.json"),
    };
    assert_eq!(store.load().unwrap(), Entries::new());
    let mut entries = Entries::new();
    let input = start("snack:1", values("Snack", 0.2));
    entries.insert(
        "id".into(),
        Entry {
            snapshot: input.snapshot.clone(),
            origin: input,
            degraded: vec!["layout.island".into()],
            actions: vec!["call".into()],
        },
    );
    store.save(&entries).unwrap();
    assert_eq!(
        FileStore {
            path: folder.join("layouts.json")
        }
        .load()
        .unwrap(),
        entries
    );
    fs::write(folder.join("layouts.json"), b"not json").unwrap();
    assert_eq!(store.load().unwrap_err().code, "NATIVE_ERROR");
    fs::remove_dir_all(folder).unwrap();
}
#[test]
fn a_picture_liveupdate_can_draw_is_named_by_its_app_file_path() {
    let files = std::env::temp_dir().join(format!("live-activity-files-{}", std::process::id()));
    let images = files.join("liveactivity/images");
    fs::create_dir_all(&images).unwrap();
    let png = |width: u32, height: u32| {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend(width.to_be_bytes());
        bytes.extend(height.to_be_bytes());
        bytes
    };
    fs::write(images.join("badge.png"), png(256, 256)).unwrap();
    fs::write(images.join("huge.png"), png(2048, 16)).unwrap();
    fs::write(
        images.join("text.png"),
        b"not a picture at all, but long enough",
    )
    .unwrap();
    let settings = Settings {
        app_name: "Mochi".into(),
        samsung_now_bar: false,
        files: Some(files.clone()),
        image_folder: "liveactivity/images".into(),
    };
    let layouts: Layouts<FakeDriver, MemoryStore> = Layouts::new(settings, MemoryStore::default());
    assert_eq!(
        layouts.image("badge.png").as_deref(),
        Some("liveactivity/images/badge.png")
    );
    for name in ["huge.png", "text.png", "missing.png"] {
        assert_eq!(layouts.image(name), None, "{name}");
    }
    let without: Layouts<FakeDriver, MemoryStore> = Layouts::new(
        Settings {
            app_name: "Mochi".into(),
            samsung_now_bar: false,
            files: None,
            image_folder: "liveactivity/images".into(),
        },
        MemoryStore::default(),
    );
    assert_eq!(without.image("badge.png"), None);
    fs::remove_dir_all(files).unwrap();
}
#[test]
fn the_same_start_again_keeps_the_latest_values() {
    let f = fixture();
    let first = f
        .layouts
        .start(&f.updates, start("snack:1", values("Snack", 0.2)))
        .unwrap();
    f.layouts
        .update(
            &f.updates,
            update(&first.id, snapshot(2, values("Snack", 0.7))),
        )
        .unwrap();
    let again = f
        .layouts
        .start(&f.updates, start("snack:1", values("Snack", 0.2)))
        .unwrap();
    assert_eq!(again.snapshot, snapshot(2, values("Snack", 0.7)));
    assert_eq!(
        code(f.layouts.update(
            &f.updates,
            update(&first.id, snapshot(1, values("Snack", 0.3)))
        )),
        "STALE_REVISION"
    );
}
#[test]
fn an_ended_id_is_not_found_before_the_revision_rules() {
    let f = fixture();
    let record = f
        .layouts
        .start(&f.updates, start("snack:1", values("Snack", 0.2)))
        .unwrap();
    f.layouts
        .end(
            &f.updates,
            LayoutEndInput {
                id: record.id.clone(),
                policy: EndPolicy::Immediate {},
                snapshot: None,
            },
        )
        .unwrap();
    let same = snapshot(0, values("Snack", 0.2));
    assert_eq!(
        code(f.layouts.update(&f.updates, update(&record.id, same))),
        "NOT_FOUND"
    );
    assert_eq!(
        code(f.layouts.update(
            &f.updates,
            update(&record.id, snapshot(5, values("Snack", 0.9)))
        )),
        "NOT_FOUND"
    );
    // An end again answers the ended record, as on iOS: a final content, here one of a later revision, is not
    // applied to an activity that is over.
    let again = LayoutEndInput {
        id: record.id.clone(),
        policy: EndPolicy::Immediate {},
        snapshot: Some(snapshot(5, values("Snack", 0.9))),
    };
    let answered = f.layouts.end(&f.updates, again).unwrap();
    assert_eq!(
        (answered.state, answered.snapshot.revision),
        (crate::ActivityState::Ended, 0)
    );
    let plain = LayoutEndInput {
        id: record.id,
        policy: EndPolicy::Immediate {},
        snapshot: None,
    };
    assert_eq!(
        f.layouts.end(&f.updates, plain).unwrap().state,
        crate::ActivityState::Ended
    );
}
#[test]
fn a_start_whose_answer_was_lost_is_adopted_and_a_refused_one_is_dropped() {
    let f = fixture();
    f.android.0.lock().unwrap().refuse_posts = true;
    // liveupdate wrote its receipt and posts again on the next operation: the activity runs, the answer was lost.
    assert_eq!(
        code(
            f.layouts
                .start(&f.updates, start("snack:1", values("Snack", 0.2)))
        ),
        "NATIVE_ERROR"
    );
    assert!(
        f.store
            .load()
            .unwrap()
            .contains_key(&format!("{PENDING}snack:1"))
    );
    f.android.0.lock().unwrap().refuse_posts = false;
    let listed = f.layouts.list(&f.updates).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(
        (listed[0].activity_key.as_str(), &listed[0].snapshot),
        ("snack:1", &snapshot(0, values("Snack", 0.2)))
    );
    assert!(
        !f.store
            .load()
            .unwrap()
            .contains_key(&format!("{PENDING}snack:1"))
    );
    // An activity that ended is not adopted by a later pending start of its key.
    f.layouts
        .end(
            &f.updates,
            LayoutEndInput {
                id: listed[0].id.clone(),
                policy: EndPolicy::Immediate {},
                snapshot: None,
            },
        )
        .unwrap();
    let mut stored = f.store.load().unwrap();
    stored.insert(
        format!("{PENDING}snack:1"),
        stored.values().next().unwrap().clone(),
    );
    f.store.save(&stored).unwrap();
    assert_eq!(f.layouts.list(&f.updates).unwrap().len(), 1);
    assert!(
        !f.store
            .load()
            .unwrap()
            .contains_key(&format!("{PENDING}snack:1"))
    );
}
#[test]
fn a_store_that_cannot_be_read_loses_no_tap() {
    let f = fixture();
    let record = f
        .layouts
        .start(&f.updates, start("snack:1", values("Snack", 0.2)))
        .unwrap();
    f.android.0.lock().unwrap().events = vec![liveupdate::ActivityEvent {
        sequence: 1,
        activity_id: record.id.clone(),
        occurred_at: 1_000,
        kind: liveupdate::EventKind::Action {
            action_id: "pet".into(),
        },
    }];
    f.store.corrupt(true);
    assert_eq!(
        f.layouts.events(&f.updates).unwrap(),
        [Event::Action(ActionEvent {
            action_id: "pet".into(),
            activity_id: record.id,
            activity_key: None,
            on: None,
        })]
    );
}
#[test]
fn a_start_with_a_push_is_projected_without_it() {
    let f = fixture();
    let mut request = start("snack:1", values("Snack", 0.2));
    request.push = Some(crate::layout::LayoutPush::Token);
    let record = f.layouts.start(&f.updates, request).unwrap();
    assert_eq!(f.android.0.lock().unwrap().starts, 1);
    assert_eq!(record.snapshot, snapshot(0, values("Snack", 0.2)));
    // Android has no pushes: the same start asking for a channel instead is the same activity.
    let mut again = start("snack:1", values("Snack", 0.2));
    again.push = Some(crate::layout::LayoutPush::Channel("dGVzdA==".into()));
    let same = f.layouts.start(&f.updates, again).unwrap();
    assert_eq!(same.id, record.id);
    assert_eq!(f.android.0.lock().unwrap().starts, 1);
}
