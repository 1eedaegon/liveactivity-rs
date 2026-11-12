//! Every feature of the crate as one function, in the order of the README, whose examples are these.
//!
//! The functions run on any backend. `main` runs them on a backend without a platform, which has the progress and
//! timer activities and their taps; what needs a platform answers its error code, which `main` prints.
//!
//! ```sh
//! cargo run -p liveactivity --example features --features testkit
//! ```

use liveactivity::{
    Alert, AndroidContent, AndroidPresentation, Bound, EndInput, Event, FeatureSupport, Layout,
    LayoutEndInput, LayoutPush, LayoutSnapshot, LayoutStartInput, LayoutUpdateInput, LiveActivity,
    LiveActivityBackend, PermissionKind, PermissionState, PresentationStyle, ProgressSegment,
    ProgressStyle, Result, StartInput, UpdateInput,
    activitykit::{
        layout::{button, progress, text, vstack},
        values,
    },
};

/// Connect. One builder: the application names where events go, and whatever is not named is the default.
fn connect(backend: impl LiveActivityBackend) -> Result<LiveActivity> {
    LiveActivity::builder()
        .backend(backend) // leave this out for the platform's own backend
        .event_sink(|event: Event| match event {
            Event::Action(tap) => println!("tapped {} on {}", tap.action_id, tap.activity_id),
            Event::State(change) => println!("{} is {:?}", change.activity_id, change.state),
            Event::PushToken(token) => println!("send {} to the server", token.token),
            _ => {}
        })
        .build()
}

/// Progress and timer activities: a title, a status, and a progress from 0 to 1 or a countdown.
async fn progress_and_timer(live: &LiveActivity) -> Result<()> {
    let order = live
        .start(StartInput::progress(
            "order:42",
            "Order 42",
            "Preparing",
            0.1,
        ))
        .await?;
    // An update sends the whole content again, under a higher revision.
    live.update(UpdateInput::progress(
        &order.id,
        1,
        "Order 42",
        "On the way",
        0.6,
    ))
    .await?;
    live.end(EndInput::new(&order.id)).await?;

    // A countdown between two times, in epoch milliseconds.
    let (started_at, ends_at) = (1_800_000_000_000, 1_800_000_180_000);
    let tea = live
        .start(StartInput::timer(
            "tea:1", "Tea", "Steeping", started_at, ends_at,
        ))
        .await?;
    live.end(EndInput::immediate(&tea.id)).await?;
    Ok(())
}

/// A layout of the application's own: a tree of nodes, fixed at start, and values that updates replace.
async fn layout(live: &LiveActivity) -> Result<String> {
    let layout = Layout {
        lock_screen: vstack([
            text(Bound::key("title")),
            progress("done"),
            // A button: its tap reaches the event sink as `Event::Action`.
            button("track", text("Track")),
        ]),
        island: None,
        background: None,
        keyline: None,
    };
    let values = values! { "title" => "Delivery 7", "done" => 0.4 }.into_inner();
    let start = LayoutStartInput::new("delivery:7", layout, LayoutSnapshot::new(0, values));
    let record = live.start_layout(start).await?;

    // Every value again, under a higher revision.
    let values = values! { "title" => "Delivery 7", "done" => 0.9 }.into_inner();
    let update = LayoutUpdateInput::new(&record.id, LayoutSnapshot::new(1, values));
    live.update_layout(update).await?;
    Ok(record.id)
}

/// An alert with an update, and a relevance. iOS lights up and shows the alert; Android updates silently and
/// names `alert` in the record's `degraded_features`.
async fn alert_and_relevance(live: &LiveActivity, id: &str) -> Result<()> {
    let values = values! { "title" => "Delivery 7", "done" => 1.0 }.into_inner();
    let mut snapshot = LayoutSnapshot::new(2, values);
    // Which of the application's activities the Dynamic Island shows first.
    snapshot.relevance = Some(90.0);
    let mut update = LayoutUpdateInput::new(id, snapshot);
    update.alert = Some(Alert::new("Delivery arrived", "Your parcel is at the door"));
    let record = live.update_layout(update).await?;
    if record.degraded_features.iter().any(|left| left == "alert") {
        println!("this platform updated without the alert");
    }
    Ok(())
}

/// An Android content: what the notification shows, in Android's own terms, instead of the projection of the
/// layout. iOS ignores it.
async fn android_content(live: &LiveActivity, id: &str) -> Result<()> {
    // Which notification styles the device draws: absent where there is no Android content.
    let styles = live.capabilities().await?.android_presentation;
    let segmented = styles.is_some_and(|styles| styles.progress_style);
    let values = values! { "title" => "Delivery 7", "done" => 0.6 }.into_inner();
    let mut snapshot = LayoutSnapshot::new(3, values);
    snapshot.android = Some(AndroidContent {
        title: "Delivery 7".into(),
        status: "2 stops away".into(),
        presentation: AndroidPresentation {
            style: PresentationStyle::Progress(ProgressStyle {
                progress: 60,
                indeterminate: false,
                styled_by_progress: true,
                segments: if segmented {
                    vec![segment(60, Some(0xFF00_7F00)), segment(40, None)]
                } else {
                    vec![segment(100, None)]
                },
                points: Vec::new(),
                start_icon: None,
                end_icon: None,
                tracker_icon: None,
            }),
            // The text of the status chip, at most 7 characters.
            short_critical_text: Some("8 min".into()),
            large_icon: None,
            actions: Vec::new(),
            samsung_now_bar: None,
        },
    });
    live.update_layout(LayoutUpdateInput::new(id, snapshot))
        .await?;
    Ok(())
}

fn segment(length: u32, color: Option<u32>) -> ProgressSegment {
    ProgressSegment {
        length,
        id: None,
        color,
        semantic_style: None,
    }
}

/// An image: write the PNG first, then show it with an image node whose source is `{ "file": name }`.
async fn image(live: &LiveActivity, png: &[u8]) -> Result<()> {
    live.write_image("badge.png", png).await
}

/// Pushes (iOS): a start asks for a token, the sink gets it as `Event::PushToken`, and `push_tokens` answers what
/// came before the sink listened. A server sends the payloads that `activitykit::push::Payload` builds.
async fn pushes(live: &LiveActivity, mut start: LayoutStartInput) -> Result<()> {
    start.push = Some(LayoutPush::Token);
    live.start_layout(start).await?;
    for held in live.push_tokens().await?.activities {
        println!("{} has the token {}", held.activity_id, held.token);
    }
    Ok(())
}

/// What the device can do and what the person allows.
async fn capabilities_and_permissions(live: &LiveActivity) -> Result<()> {
    let capabilities = live.capabilities().await?;
    if capabilities.layout != FeatureSupport::Supported {
        println!(
            "{}: progress and timer activities only",
            capabilities.platform
        );
    }
    let permissions = live.permissions().await?;
    if permissions.notifications == PermissionState::Prompt {
        // Android shows the system's dialog through the host; iOS has a switch in Settings and no dialog.
        live.request_permissions(PermissionKind::Notifications)
            .await?;
    }
    Ok(())
}

/// Ending a layout activity, and the events Android stored while the application did not run.
async fn end_and_sync(live: &LiveActivity, id: &str) -> Result<()> {
    live.end_layout(LayoutEndInput::new(id)).await?;
    // Call this when the application starts and resumes. On iOS it does nothing.
    live.sync_events().await
}

/// The error code of a call, or `ok`.
fn code<T>(result: Result<T>) -> String {
    result.map_or_else(|error| error.code, |_| "ok".to_owned())
}

fn main() -> Result<()> {
    let memory = std::sync::Arc::new(liveactivity::testkit::MemoryBackend::new());
    let live = connect(memory.clone())?;
    // Where no executor runs, `wait` blocks for the answer; on the main thread of an iOS app it answers an error
    // at once instead, because the answer comes from that thread.
    liveactivity::wait(async {
        println!(
            "progress and timer: {}",
            code(progress_and_timer(&live).await)
        );
        let order = live
            .start(StartInput::progress(
                "order:43",
                "Order 43",
                "Preparing",
                0.2,
            ))
            .await?;
        memory.tap(&order.id, "track")?;
        println!(
            "capabilities: {}",
            code(capabilities_and_permissions(&live).await)
        );

        // The rest needs a platform that draws layouts: here each answers its error code.
        println!("layout: {}", code(layout(&live).await));
        println!(
            "alert and relevance: {}",
            code(alert_and_relevance(&live, "id").await)
        );
        println!(
            "android content: {}",
            code(android_content(&live, "id").await)
        );
        println!("image: {}", code(image(&live, b"not a png").await));
        let start = LayoutStartInput::new(
            "delivery:8",
            Layout {
                lock_screen: text("Delivery 8"),
                island: None,
                background: None,
                keyline: None,
            },
            LayoutSnapshot::new(0, Default::default()),
        );
        println!("pushes: {}", code(pushes(&live, start).await));
        println!("end and sync: {}", code(end_and_sync(&live, "id").await));
        Ok(())
    })
}
