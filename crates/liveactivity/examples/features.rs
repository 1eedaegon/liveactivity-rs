//! Every feature of the crate as one function, in the order of the README, whose examples are these.
//!
//! The functions run on any backend. `main` runs them on a backend without a platform, which has the progress and
//! timer activities and their taps; what needs a platform answers its error code, which `main` prints.
//!
//! ```sh
//! cargo run -p liveactivity --example features --features testkit
//! ```

use liveactivity::{
    Accessibility, Alert, AndroidContent, AndroidPresentation, Bound, EndInput, EndPolicy, Event,
    FeatureSupport, Layout, LayoutEndInput, LayoutPush, LayoutSnapshot, LayoutStartInput,
    LayoutUpdateInput, LiveActivity, LiveActivityBackend, NotificationAction, PermissionKind,
    PermissionState, PresentationStyle, ProgressSegment, ProgressStyle, PublicVersion, Result,
    StartInput, TimerStyle, Transition, UpdateInput,
    activitykit::{
        layout::{button, link, progress, text, vstack},
        values,
    },
    liveupdate::Timer,
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
        url: None,
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

/// Links, accessibility, motion and privacy: members of the layout, which iOS draws. Android opens the
/// application from its notification and names what it left out.
async fn links_accessibility_and_motion(live: &LiveActivity) -> Result<()> {
    let layout = Layout {
        lock_screen: vstack([
            // What VoiceOver reads of a node that has no text of its own.
            progress("done").accessibility(
                Accessibility::new()
                    .label("Delivery progress")
                    .value(Bound::key("percent")),
            ),
            // Hidden on a locked device, when the person asked for that.
            text(Bound::key("address")).sensitive(),
            // Shown while `arrived` is true, and how it comes and goes.
            text("At the door")
                .when("arrived")
                .transition(Transition::MoveTop),
            // A tap on the label opens the application with this URL.
            link("myapp://delivery/9/map", text("Map")),
        ]),
        island: None,
        background: None,
        keyline: None,
        // A tap anywhere else on the activity.
        url: Some("myapp://delivery/9".into()),
    };
    let values = values! {
        "done" => 0.4, "percent" => "40 percent", "address" => "12 Hill Street", "arrived" => false,
    }
    .into_inner();
    let start = LayoutStartInput::new("delivery:9", layout, LayoutSnapshot::new(0, values));
    let record = live.start_layout(start).await?;
    // On Android: layout.transition, layout.link and layout.url.
    println!("left out: {:?}", record.degraded_features);
    Ok(())
}

/// An alert with an update, and a relevance. iOS lights up and shows the alert. Android alerts again with the
/// notification where the device offers that; elsewhere it updates silently and names `alert` in the record's
/// `degraded_features`.
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
            // The text of the status chip, at most 7 characters.
            short_critical_text: Some("8 min".into()),
            ..AndroidPresentation::new(PresentationStyle::Progress(ProgressStyle {
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
            }))
        },
    });
    live.update_layout(LayoutUpdateInput::new(id, snapshot))
        .await?;
    Ok(())
}

/// What else an Android content can say: a URL the notification opens, an action that opens one, a clock in the
/// header, what a secure lock screen shows, and the application's own look. Each is behind a capability.
async fn android_links_and_extras(live: &LiveActivity, id: &str, ends_at: u64) -> Result<()> {
    let capabilities = live.capabilities().await?;
    let offered = capabilities.android_presentation.unwrap_or_default();
    let values = values! { "title" => "Delivery 7", "done" => 0.8 }.into_inner();
    let mut snapshot = LayoutSnapshot::new(4, values);
    snapshot.android = Some(AndroidContent {
        title: "Delivery 7".into(),
        status: "At the door in 4 minutes".into(),
        presentation: AndroidPresentation {
            // A tap on the notification opens this URL in the application.
            open_url: offered.links.then(|| "myapp://delivery/7".into()),
            // An action that opens a URL sends no tap.
            actions: if offered.links {
                vec![NotificationAction {
                    open_url: Some("myapp://delivery/7/map".into()),
                    ..NotificationAction::new("map", "Map")
                }]
            } else {
                Vec::new()
            },
            // A countdown in the header, beside the style.
            chronometer: offered.chronometer.then(|| TimerStyle {
                timer: Timer {
                    started_at: ends_at - 240_000,
                    ends_at,
                },
                counts_down: true,
            }),
            // What a secure lock screen shows of an activity that started as private.
            public_version: offered.public_version.then(|| PublicVersion {
                title: "Delivery".into(),
                status: "On the way".into(),
            }),
            sub_text: offered.branding.then(|| "Mochi Express".into()),
            ..AndroidPresentation::new(PresentationStyle::Standard)
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

/// An end that keeps the last content on screen until a time, with a final snapshot. iOS keeps it for at most
/// four hours. Android keeps it where the device offers that, and answers `UNSUPPORTED_FEATURE` elsewhere.
async fn end_with_a_last_content(live: &LiveActivity, id: &str, until: u64) -> Result<()> {
    let values = values! { "title" => "Delivered", "done" => 1.0 }.into_inner();
    let mut end = LayoutEndInput::new(id);
    end.policy = EndPolicy::After { at: until };
    end.snapshot = Some(LayoutSnapshot::new(5, values));
    live.end_layout(end).await?;
    Ok(())
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
            "links, accessibility and motion: {}",
            code(links_accessibility_and_motion(&live).await)
        );
        println!(
            "alert and relevance: {}",
            code(alert_and_relevance(&live, "id").await)
        );
        println!(
            "android content: {}",
            code(android_content(&live, "id").await)
        );
        let in_four_minutes = 1_800_000_240_000;
        println!(
            "android links and extras: {}",
            code(android_links_and_extras(&live, "id", in_four_minutes).await)
        );
        println!("image: {}", code(image(&live, b"not a png").await));
        let start = LayoutStartInput::new(
            "delivery:8",
            Layout {
                lock_screen: text("Delivery 8"),
                island: None,
                background: None,
                keyline: None,
                url: None,
            },
            LayoutSnapshot::new(0, Default::default()),
        );
        println!("pushes: {}", code(pushes(&live, start).await));
        println!("end and sync: {}", code(end_and_sync(&live, "id").await));
        println!(
            "end with a last content: {}",
            code(end_with_a_last_content(&live, "id", in_four_minutes).await)
        );
        Ok(())
    })
}
